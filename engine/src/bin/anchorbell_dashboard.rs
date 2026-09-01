use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use static_anchor_engine::{
    backtest::{ConservativeTopOfBook, FillDecision, FillModel, MakerQuote, TopOfBook},
    backtest_report::BacktestReport,
    execution::{
        BinanceAccountStatusResponse, BinanceAccountStatusWire, BinanceCredentials,
        BinanceEnvironment, BinanceOrderWebSocket, BinanceRestClient, DeploymentConfig,
        DeploymentConfigError, Side,
    },
    market::{BinanceMarketConfig, BinanceMarketStream, BinanceSubscription, ReconnectPolicy},
    strategy::{instrument_for, EquityRegion},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

const BIND_ADDRESS: &str = "127.0.0.1:8787";
const MAX_REQUEST_BYTES: usize = 1_048_576;

#[derive(Clone)]
struct DashboardState {
    session: Arc<Mutex<DashboardSession>>,
}

#[derive(Clone)]
struct DashboardSession {
    config: DeploymentConfig,
    credentials: Option<BinanceCredentials>,
    symbol: String,
    proxy: Option<String>,
}

impl Default for DashboardSession {
    fn default() -> Self {
        Self {
            config: DeploymentConfig::from_values(BinanceEnvironment::Testnet, false, false, None)
                .expect("default Testnet configuration must be valid"),
            credentials: None,
            symbol: "CXMTUSDT".to_owned(),
            proxy: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SessionRequest {
    environment: String,
    api_key: String,
    api_secret: String,
    allow_production: bool,
    allow_order_submission: bool,
    confirmation: String,
    symbol: String,
    proxy: String,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    environment: String,
    has_credentials: bool,
    allow_production: bool,
    allow_order_submission: bool,
    symbol: String,
    region: String,
    proxy_configured: bool,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(BIND_ADDRESS).await?;
    let state = DashboardState {
        session: Arc::new(Mutex::new(DashboardSession::default())),
    };
    println!("AnchorBell dashboard listening on http://{BIND_ADDRESS}");

    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, state).await {
                eprintln!("dashboard connection failed: {error}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: DashboardState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let request = read_request(&mut stream).await?;
    let (status, content_type, body) = route(request, state).await;
    let response = format!(
        "HTTP/1.1 {status} {}
Content-Type: {content_type}
Content-Length: {}
Connection: close
Cache-Control: no-store

",
        reason_phrase(status),
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn route(request: HttpRequest, state: DashboardState) -> (u16, &'static str, Vec<u8>) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => text_response(
            200,
            "text/html; charset=utf-8",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/index.html")),
        ),
        ("GET", "/styles.css") => text_response(
            200,
            "text/css; charset=utf-8",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/styles.css")),
        ),
        ("GET", "/app.js") => text_response(
            200,
            "application/javascript; charset=utf-8",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.js")),
        ),
        ("GET", "/api/status") => json_response(200, status_response(&state).await),
        ("POST", "/api/session") => update_session(request.body, &state).await,
        ("POST", "/api/session/clear") => {
            *state.session.lock().await = DashboardSession::default();
            json_response(200, json!({"ok": true, "message": "本地会话已清除"}))
        }
        ("POST", "/api/check/market") => market_check(&state).await,
        ("POST", "/api/check/account") => account_check(&state).await,
        ("POST", "/api/check/open-orders") => open_orders_check(&state).await,
        ("POST", "/api/backtest") => backtest_check(),
        _ => json_response(404, json!({"ok": false, "message": "未找到请求"})),
    }
}

async fn status_response(state: &DashboardState) -> Value {
    let session = state.session.lock().await;
    serde_json::to_value(StatusResponse {
        environment: session.config.environment.to_string(),
        has_credentials: session.credentials.is_some(),
        allow_production: session.config.allow_production,
        allow_order_submission: session.config.allow_live_orders,
        symbol: session.symbol.clone(),
        region: instrument_for(&session.symbol)
            .map(|instrument| match instrument.region {
                EquityRegion::AShare => "A股",
                EquityRegion::HongKong => "港股",
            })
            .unwrap_or("未知")
            .to_owned(),
        proxy_configured: session.proxy.is_some(),
    })
    .expect("status response is serializable")
}

async fn update_session(body: Vec<u8>, state: &DashboardState) -> (u16, &'static str, Vec<u8>) {
    let request: SessionRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return json_response(400, json!({"ok": false, "message": "配置格式无效"})),
    };
    let environment: BinanceEnvironment = match request.environment.parse() {
        Ok(environment) => environment,
        Err(_) => {
            return json_response(
                400,
                json!({"ok": false, "message": "环境必须是 testnet 或 production"}),
            )
        }
    };
    let confirmation =
        (!request.confirmation.trim().is_empty()).then_some(request.confirmation.as_str());
    let config = match DeploymentConfig::from_values(
        environment,
        request.allow_production,
        request.allow_order_submission,
        confirmation,
    ) {
        Ok(config) => config,
        Err(error) => return deployment_error(error),
    };
    let credentials = match (request.api_key.trim(), request.api_secret.trim()) {
        ("", "") => None,
        (api_key, api_secret) => {
            match BinanceCredentials::from_values(api_key.to_owned(), api_secret.to_owned()) {
                Ok(credentials) => Some(credentials),
                Err(_) => {
                    return json_response(400, json!({"ok": false, "message": "API 凭证不能为空"}))
                }
            }
        }
    };
    let symbol = request.symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return json_response(400, json!({"ok": false, "message": "交易标的不能为空"}));
    }
    if instrument_for(&symbol).is_none() {
        return json_response(
            400,
            json!({
                "ok": false,
                "message": "只允许 AnchorBell 确认的 15 个 A 股/港股标的"
            }),
        );
    }

    let proxy = match request.proxy.trim() {
        "" => None,
        value => Some(value.to_owned()),
    };
    *state.session.lock().await = DashboardSession {
        config,
        credentials,
        symbol,
        proxy,
    };
    json_response(
        200,
        json!({
            "ok": true,
            "message": format!("已切换到 {}，订单权限：{}", environment, if config.allow_live_orders { "开启" } else { "关闭" })
        }),
    )
}

fn deployment_error(error: DeploymentConfigError) -> (u16, &'static str, Vec<u8>) {
    let message = match error {
        DeploymentConfigError::InvalidEnvironment => "环境配置无效",
        DeploymentConfigError::ProductionNotExplicitlyEnabled => "Production 未显式授权",
        DeploymentConfigError::LiveOrdersNotExplicitlyEnabled => "Production 真实订单缺少确认",
    };
    json_response(400, json!({"ok": false, "message": message}))
}

async fn market_check(state: &DashboardState) -> (u16, &'static str, Vec<u8>) {
    let (environment, symbol, proxy) = {
        let session = state.session.lock().await;
        (
            session.config.environment,
            session.symbol.clone(),
            session.proxy.clone(),
        )
    };
    let subscription = match BinanceSubscription::new(symbol.clone()) {
        Ok(subscription) => subscription,
        Err(_) => return json_response(400, json!({"ok": false, "message": "交易标的格式无效"})),
    };
    let config = BinanceMarketConfig {
        market_ws_base: environment.endpoints().market_ws_base.into(),
        subscriptions: vec![subscription],
        price_scale: 8,
        quantity_scale: 8,
        max_frame_bytes: 1_048_576,
        connect_timeout_ms: 5_000,
        http_proxy: proxy,
        reconnect: ReconnectPolicy {
            max_attempts: Some(1),
            ..ReconnectPolicy::default()
        },
    };
    let mut stream = BinanceMarketStream::new(config);
    let mut count = 0_u32;
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        stream.run_until_error(|_| count += 1),
    )
    .await;
    json_response(
        200,
        json!({
            "ok": count > 0,
            "environment": environment.to_string(),
            "symbol": symbol,
            "events": count,
            "timeout": result.is_err(),
            "message": if count > 0 { "行情连接成功" } else { "未收到行情事件" }
        }),
    )
}

async fn account_check(state: &DashboardState) -> (u16, &'static str, Vec<u8>) {
    let (config, credentials, proxy) = session_snapshot(state).await;
    let credentials = match credentials {
        Some(credentials) => credentials,
        None => {
            return json_response(
                400,
                json!({"ok": false, "message": "请先在界面注入当前环境凭证"}),
            )
        }
    };
    let policy = config.policy(true);
    let mut socket = match BinanceOrderWebSocket::connect_with_proxy(
        config.environment,
        policy,
        proxy.as_deref(),
    )
    .await
    {
        Ok(socket) => socket,
        Err(error) => {
            return json_response(502, json!({"ok": false, "message": error.to_string()}))
        }
    };
    let timestamp_ms = now_ms();
    let wire = BinanceAccountStatusWire {
        request_id: format!("anchorbell-dashboard-account-{timestamp_ms}"),
        timestamp_ms,
        recv_window_ms: 5_000,
    };
    let payload = match wire.payload(&credentials.api_key, &credentials.api_secret) {
        Ok(payload) => payload,
        Err(_) => return json_response(500, json!({"ok": false, "message": "签名失败"})),
    };
    let response: BinanceAccountStatusResponse = match socket.request_typed(payload).await {
        Ok(response) => response,
        Err(error) => {
            return json_response(502, json!({"ok": false, "message": error.to_string()}))
        }
    };
    json_response(
        200,
        json!({
            "ok": true,
            "environment": config.environment.to_string(),
            "status": response.status,
            "can_trade": response.result.can_trade,
            "positions": response.result.positions.len(),
            "message": "账户只读查询成功"
        }),
    )
}

async fn open_orders_check(state: &DashboardState) -> (u16, &'static str, Vec<u8>) {
    let (config, credentials, symbol, proxy) = session_snapshot_with_symbol(state).await;
    let credentials = match credentials {
        Some(credentials) => credentials,
        None => {
            return json_response(
                400,
                json!({"ok": false, "message": "请先在界面注入当前环境凭证"}),
            )
        }
    };
    let client =
        match BinanceRestClient::new(config.environment, config.policy(true), proxy.as_deref()) {
            Ok(client) => client,
            Err(error) => {
                return json_response(400, json!({"ok": false, "message": error.to_string()}))
            }
        };
    match client
        .current_open_orders(&credentials, Some(&symbol), now_ms(), 5_000)
        .await
    {
        Ok(orders) => json_response(
            200,
            json!({
                "ok": true,
                "environment": config.environment.to_string(),
                "symbol": symbol,
                "count": orders.len(),
                "message": "当前挂单只读查询成功"
            }),
        ),
        Err(error) => json_response(502, json!({"ok": false, "message": error.to_string()})),
    }
}

fn backtest_check() -> (u16, &'static str, Vec<u8>) {
    let fixture = [
        (99_i64, 100_i64, 10_i64, 8_i64),
        (100, 101, 4, 6),
        (101, 102, 3, 2),
    ];
    let mut report = BacktestReport::default();
    for (bid, ask, bid_qty, ask_qty) in fixture {
        report.record_event();
        let decision = ConservativeTopOfBook.evaluate(
            MakerQuote {
                side: Side::Sell,
                price_ticks: bid,
                quantity: 5,
            },
            TopOfBook {
                bid_price_ticks: bid,
                ask_price_ticks: ask,
                bid_quantity: bid_qty,
                ask_quantity: ask_qty,
            },
        );
        if let FillDecision::Fill { quantity } = decision {
            report.record_fill(quantity, 1, 0);
            report.record_position(quantity);
        }
    }
    json_response(
        200,
        json!({
            "ok": true,
            "events": report.event_count,
            "fills": report.fill_count,
            "quantity": report.filled_quantity,
            "fees": report.fees_ticks,
            "net_pnl": report.net_pnl_ticks(),
            "peak_position": report.peak_absolute_position,
            "message": "内置确定性回测完成"
        }),
    )
}

async fn session_snapshot(
    state: &DashboardState,
) -> (DeploymentConfig, Option<BinanceCredentials>, Option<String>) {
    let session = state.session.lock().await;
    (
        session.config,
        session.credentials.clone(),
        session.proxy.clone(),
    )
}

async fn session_snapshot_with_symbol(
    state: &DashboardState,
) -> (
    DeploymentConfig,
    Option<BinanceCredentials>,
    String,
    Option<String>,
) {
    let session = state.session.lock().await;
    (
        session.config,
        session.credentials.clone(),
        session.symbol.clone(),
        session.proxy.clone(),
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_millis() as u64
}

async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, &'static str> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8_192];
    let header_end = loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|_| "request read failed")?;
        if count == 0 {
            return Err("request closed");
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("request too large");
        }
        if let Some(index) = find_bytes(&bytes, b"\r\n\r\n") {
            break index;
        }
    };
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "invalid headers")?;
    let mut lines = header.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_owned();
    let path = parts.next().ok_or("missing path")?.to_owned();
    let content_length = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.eq_ignore_ascii_case("content-length"))
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while bytes.len() < body_start + content_length {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|_| "request body read failed")?;
        if count == 0 {
            return Err("request body closed");
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("request too large");
        }
    }
    Ok(HttpRequest {
        method,
        path,
        body: bytes[body_start..body_start + content_length].to_vec(),
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn json_response(status: u16, value: Value) -> (u16, &'static str, Vec<u8>) {
    (
        status,
        "application/json; charset=utf-8",
        serde_json::to_vec(&value).expect("JSON response is serializable"),
    )
}

fn text_response(
    status: u16,
    content_type: &'static str,
    text: &str,
) -> (u16, &'static str, Vec<u8>) {
    (status, content_type, text.as_bytes().to_vec())
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Error",
    }
}
