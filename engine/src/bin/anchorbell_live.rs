use std::{
    collections::BTreeMap,
    env, process,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use static_anchor_engine::{
    execution::{
        BinanceCredentials, BinanceEnvironment, BinanceMakerOrderRequest, BinanceRestClient,
        BinanceUserDataStream, DeploymentConfig, ExecutionSupervisor, GateDecision, Side,
        SupervisorConfig, SupervisorState, UserDataEvent, LIVE_SYMBOLS,
    },
    market::{
        binance::{BinanceMarketEvent, BookTicker, MarkPrice},
        BinanceC2cFxClient, BinanceC2cFxPoller, BinanceMarketConfig, BinanceMarketStream,
        BinanceSubscription, FxPollerConfig, FxUpdate, ReconnectPolicy,
    },
    paper::load_binance_index_anchor_set,
    strategy::{
        calendar_for, profile_for, AnchorCurrency, AnchorMakerStrategy, EquityRegion,
        VenueSessionState,
    },
};

const MAX_FRAME_BYTES: usize = 1_048_576;
const RECV_WINDOW_MS: u64 = 5_000;

#[derive(Debug)]
struct Args {
    environment: BinanceEnvironment,
    duration_secs: u64,
    proxy: Option<String>,
    price_scale: u32,
    quantity_scale: u32,
    max_position: i64,
    quantity: i64,
    entry_threshold_bps: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    funding_lead_ms: u64,
    max_subscriptions_per_shard: usize,
    send_orders: bool,
}

#[derive(Debug)]
enum Event {
    Market(BinanceMarketEvent),
    Fx(FxUpdate),
    User(UserDataEvent),
    Halt(String),
}

#[derive(Debug, Default)]
struct SymbolState {
    book: Option<BookTicker>,
    mark: Option<MarkPrice>,
    position_ticks: i64,
}

#[derive(Debug, Clone)]
struct WorkingOrder {
    client_order_id: String,
    side: Side,
    price_ticks: i64,
    quantity_ticks: i64,
}

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(value) => value,
        Err(error) => fail(&error),
    };
    match run(args).await {
        Ok(code) => process::exit(code),
        Err(error) => {
            eprintln!("live runner failed: {error}");
            process::exit(1);
        }
    }
}

async fn run(args: Args) -> Result<i32, String> {
    let deployment = DeploymentConfig::from_process_environment()
        .map_err(|error| format!("deployment config rejected: {error:?}"))?;
    if deployment.environment != args.environment {
        return Err(format!(
            "--environment {} does not match ANCHORBELL_BINANCE_ENV={}",
            args.environment, deployment.environment
        ));
    }
    let credentials = load_credentials(args.environment)?;
    let policy = deployment.policy(true);
    if args.send_orders && !policy.allow_live_orders {
        return Err("order submission is disabled by deployment policy".into());
    }
    let client = Arc::new(
        BinanceRestClient::new(args.environment, policy, args.proxy.as_deref())
            .map_err(|error| error.to_string())?,
    );
    let server_time = client.server_time_ms().await.map_err(|e| e.to_string())?;
    let symbols = LIVE_SYMBOLS
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    let anchors = load_binance_index_anchor_set(
        args.environment,
        &symbols,
        args.price_scale,
        args.proxy.as_deref(),
    )
    .await
    .map_err(|error| format!("cannot load Binance index anchors: {error}"))?
    .anchors;
    if anchors.len() != LIVE_SYMBOLS.len() {
        return Err("anchor set is not the exact nine-symbol universe".into());
    }

    let mut supervisor = ExecutionSupervisor::new(SupervisorConfig {
        max_market_age_ms: 5_000,
        max_fx_age_ms: 5_000,
        funding_lead_ms: args.funding_lead_ms,
        max_position: args.max_position,
        quantity_scale: args.quantity_scale,
    })
    .map_err(|reason| format!("supervisor config rejected: {reason:?}"))?;
    let mut state = BTreeMap::<String, SymbolState>::new();
    for symbol in &symbols {
        reconcile_symbol(
            &client,
            &credentials,
            symbol,
            args.quantity_scale,
            args.send_orders,
        )
        .await?;
        state.insert(symbol.clone(), SymbolState::default());
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(16_384);
    spawn_market(&args, tx.clone())?;
    spawn_fx(&args, tx.clone())?;
    let listen_key =
        spawn_user_data(&args, client.clone(), credentials.clone(), tx.clone()).await?;
    spawn_keepalive(client.clone(), credentials.clone(), listen_key, tx.clone());

    println!(
        "{}",
        serde_json::json!({
            "event":"live_canary_started",
            "environment":args.environment.as_str(),
            "server_time_ms":server_time,
            "symbols":symbols,
            "order_submission":args.send_orders,
            "paper_orders":false,
        })
    );

    let mut fx_at = BTreeMap::<String, u64>::new();
    let deadline = tokio::time::sleep(Duration::from_secs(args.duration_secs));
    tokio::pin!(deadline);
    let mut supervisor_ready = false;
    let mut working = BTreeMap::<String, WorkingOrder>::new();
    let strategy = AnchorMakerStrategy::new(args.entry_threshold_bps, 0);
    let mut order_sequence = 0_u64;

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            event = rx.recv() => {
                let Some(event) = event else {
                    supervisor.on_disconnect();
                    return Err("all event producers stopped; risk stopped".into());
                };
                match event {
                    Event::Market(value) => apply_market(&mut state, value),
                    Event::Fx(value) => {
                        fx_at.insert(value.currency, value.observed_at_ms);
                    }
                    Event::User(value) => {
                        apply_user(&mut state, &mut working, &value, args.quantity_scale)?;
                        supervisor.on_user_data(value)
                            .map_err(|reason| format!("user-data risk halt: {reason:?}"))?;
                    }
                    Event::Halt(reason) => {
                        supervisor.on_disconnect();
                        return Err(reason);
                    }
                }

                let now = now_ms();
                for symbol in &symbols {
                    let local = state.get(symbol).expect("initialized symbol");
                    let profile = profile_for(symbol)
                        .ok_or_else(|| format!("no profile for {symbol}"))?;
                    let market_at = local.mark.as_ref().map(|v| v.event_time_ms)
                        .into_iter()
                        .chain(local.book.as_ref().map(|v| v.event_time_ms))
                        .max().unwrap_or(0);
                    let currency = profile.anchor_currency.as_str().to_owned();
                    let anchor_ready = anchors.get(symbol)
                        .is_some_and(|anchor| anchor.valid_at(now, args.max_anchor_age_ms));
                    let next_funding = local.mark.as_ref()
                        .map(|v| v.next_funding_time_ms).unwrap_or(0);
                    supervisor.observe_symbol(
                        symbol,
                        market_at,
                        fx_at.get(&currency).copied().unwrap_or(0),
                        anchor_ready,
                        equity_is_closed(profile.region),
                        next_funding > now,
                        next_funding,
                        local.position_ticks,
                    ).map_err(|reason| format!("observation rejected: {reason:?}"))?;
                }
                if !supervisor_ready {
                    supervisor.reconciliation_clean()
                        .map_err(|reason| format!("initial reconciliation rejected: {reason:?}"))?;
                    supervisor_ready = true;
                    println!("{}", serde_json::json!({"event":"supervisor_healthy"}));
                }

                if supervisor.state() == SupervisorState::Healthy {
                    for symbol in &symbols {
                        let Some(intent) = make_intent(
                            symbol,
                            state.get(symbol).expect("initialized symbol"),
                            anchors.get(symbol).expect("validated anchor").close_price_ticks,
                            &strategy,
                            &args,
                        ) else { continue };
                        match supervisor.evaluate(symbol, intent, now) {
                            GateDecision::Allow => {
                                if args.send_orders && !working.contains_key(symbol) {
                                    order_sequence = order_sequence.saturating_add(1);
                                    let order = place_order(
                                        &client, &credentials, symbol, intent,
                                        args.price_scale, args.quantity_scale,
                                        now, order_sequence,
                                    ).await?;
                                    println!("{}", serde_json::json!({
                                        "event":"order_accepted",
                                        "symbol":symbol,
                                        "client_order_id":order.client_order_id,
                                        "side":side_name(order.side),
                                        "price_ticks":order.price_ticks,
                                        "quantity_ticks":order.quantity_ticks,
                                    }));
                                    working.insert(symbol.clone(), order);
                                }
                            }
                            GateDecision::Flatten(reason) => {
                                supervisor.begin_flatten()
                                    .map_err(|r| format!("flatten transition rejected: {r:?}"))?;
                                cancel_symbol_orders(&client, &credentials, symbol).await?;
                                println!("{}", serde_json::json!({
                                    "event":"flatten_required",
                                    "symbol":symbol,
                                    "reason":format!("{reason:?}"),
                                }));
                            }
                            GateDecision::Halt(reason)
                                if matches!(
                                    reason,
                                    static_anchor_engine::execution::GateReason::NotHealthy
                                        | static_anchor_engine::execution::GateReason::MarketStale
                                        | static_anchor_engine::execution::GateReason::FxStale
                                        | static_anchor_engine::execution::GateReason::AnchorUnavailable
                                ) => {
                                // Startup and transient feed gaps block new risk.
                                // Cancel a live quote before waiting for recovery.
                                if args.send_orders {
                                    if let Some(order) = working.remove(symbol) {
                                        cancel_order(
                                            &client, &credentials, symbol,
                                            &order.client_order_id,
                                        ).await?;
                                    }
                                }
                            }
                            GateDecision::Halt(reason) => {
                                return Err(format!("gate halt for {symbol}: {reason:?}"));
                            }
                            GateDecision::NoAction(_) => {}
                        }
                    }
                }
            }
        }
    }

    for symbol in &symbols {
        if let Some(order) = working.remove(symbol) {
            cancel_order(&client, &credentials, symbol, &order.client_order_id).await?;
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "event":"live_canary_stopped",
            "state":format!("{:?}", supervisor.state()),
            "symbols":symbols,
            "order_submission":args.send_orders,
            "flat_start_required":true,
        })
    );
    Ok(0)
}

async fn reconcile_symbol(
    client: &BinanceRestClient,
    credentials: &BinanceCredentials,
    symbol: &str,
    quantity_scale: u32,
    send_orders: bool,
) -> Result<(), String> {
    let timestamp = client.server_time_ms().await.map_err(|e| e.to_string())?;
    let open = client
        .current_open_orders(credentials, Some(symbol), timestamp, RECV_WINDOW_MS)
        .await
        .map_err(|e| e.to_string())?;
    if !open.is_empty() {
        if !send_orders {
            return Err(format!("read-only start found open orders for {symbol}"));
        }
        for order in open {
            if !order.client_order_id.starts_with("anchorbell-") {
                return Err(format!("untracked open order on {symbol}"));
            }
            cancel_order(client, credentials, symbol, &order.client_order_id).await?;
        }
    }
    let timestamp = client.server_time_ms().await.map_err(|e| e.to_string())?;
    let risks = client
        .position_risk(credentials, Some(symbol), timestamp, RECV_WINDOW_MS)
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = risks.iter().filter(|row| row.symbol == symbol);
    let row = rows
        .next()
        .ok_or_else(|| format!("positionRisk missing {symbol}"))?;
    let position = parse_ticks(&row.position_amount, quantity_scale)
        .ok_or_else(|| format!("invalid position precision for {symbol}"))?;
    if rows.next().is_some() {
        return Err(format!("multiple position legs for {symbol}"));
    }
    if position != 0 {
        return Err(format!("non-flat start on {symbol}: {position}"));
    }
    Ok(())
}

fn apply_market(state: &mut BTreeMap<String, SymbolState>, event: BinanceMarketEvent) {
    match event {
        BinanceMarketEvent::BookTicker(value) => {
            if let Some(local) = state.get_mut(&value.symbol) {
                local.book = Some(value);
            }
        }
        BinanceMarketEvent::MarkPrice(value) => {
            if let Some(local) = state.get_mut(&value.symbol) {
                local.mark = Some(value);
            }
        }
        BinanceMarketEvent::AggTrade(_) => {}
    }
}

fn apply_user(
    state: &mut BTreeMap<String, SymbolState>,
    working: &mut BTreeMap<String, WorkingOrder>,
    event: &UserDataEvent,
    quantity_scale: u32,
) -> Result<(), String> {
    match event {
        UserDataEvent::OrderUpdate(update) => {
            if matches!(
                update.status.as_str(),
                "FILLED" | "CANCELED" | "EXPIRED" | "REJECTED"
            ) {
                working.remove(&update.symbol);
            }
        }
        UserDataEvent::AccountUpdate(update) => {
            for position in &update.positions {
                let value = parse_ticks(&position.position_amount, quantity_scale)
                    .ok_or_else(|| format!("invalid account precision for {}", position.symbol))?;
                if let Some(local) = state.get_mut(&position.symbol) {
                    local.position_ticks = value;
                }
            }
        }
        UserDataEvent::ListenKeyExpired => {}
    }
    Ok(())
}

fn make_intent(
    symbol: &str,
    state: &SymbolState,
    anchor_ticks: i64,
    strategy: &AnchorMakerStrategy,
    args: &Args,
) -> Option<static_anchor_engine::execution::OrderIntent> {
    let book = state.book.as_ref()?;
    let mark = state.mark.as_ref()?;
    if book.bid_price.0 <= 0
        || book.ask_price.0 < book.bid_price.0
        || mark.index_price.0 <= 0
        || book.bid_quantity.0 <= 0
        || book.ask_quantity.0 <= 0
    {
        return None;
    }
    let gap = (i128::from(mark.mark_price.0) - i128::from(mark.index_price.0)).abs() * 10_000;
    if gap > i128::from(args.max_mark_index_gap_bps) * i128::from(mark.index_price.0) {
        return None;
    }
    strategy.generate_intent(
        stable_symbol_id(symbol),
        book.bid_price.0,
        book.ask_price.0,
        anchor_ticks,
        args.quantity,
    )
}

fn spawn_market(args: &Args, tx: tokio::sync::mpsc::Sender<Event>) -> Result<(), String> {
    let subscriptions = LIVE_SYMBOLS
        .iter()
        .map(|symbol| BinanceSubscription::new(*symbol).map_err(|e| format!("{e:?}")))
        .collect::<Result<Vec<_>, _>>()?;
    let endpoints = args.environment.endpoints();
    let base = BinanceMarketConfig {
        market_ws_base: endpoints.market_ws_base.into(),
        subscriptions,
        price_scale: args.price_scale,
        quantity_scale: args.quantity_scale,
        max_frame_bytes: MAX_FRAME_BYTES,
        connect_timeout_ms: 5_000,
        read_timeout_ms: 15_000,
        http_proxy: args.proxy.clone(),
        reconnect: ReconnectPolicy {
            max_attempts: Some(3),
            ..Default::default()
        },
    };
    for shard in base
        .into_shards(args.max_subscriptions_per_shard)
        .map_err(|e| format!("{e:?}"))?
    {
        let producer = tx.clone();
        tokio::spawn(async move {
            let mut stream = BinanceMarketStream::new(shard);
            let result = stream
                .run_until_error(|event| {
                    let _ = producer.try_send(Event::Market(event));
                })
                .await;
            let _ = producer
                .send(Event::Halt(format!("market stream ended: {result:?}")))
                .await;
        });
    }
    Ok(())
}

fn spawn_fx(args: &Args, tx: tokio::sync::mpsc::Sender<Event>) -> Result<(), String> {
    let client = BinanceC2cFxClient::new(args.proxy.as_deref()).map_err(|e| e.to_string())?;
    let currencies = vec![AnchorCurrency::Cny, AnchorCurrency::Hkd];
    let poller = BinanceC2cFxPoller::new(client, &currencies, FxPollerConfig::high_frequency())
        .map_err(|e| e.to_string())?;
    let (fx_tx, mut fx_rx) = tokio::sync::mpsc::channel::<FxUpdate>(128);
    let producer = tx.clone();
    tokio::spawn(async move {
        if let Err(error) = poller.run(fx_tx).await {
            let _ = producer
                .send(Event::Halt(format!("FX stream ended: {error}")))
                .await;
        }
    });
    tokio::spawn(async move {
        while let Some(update) = fx_rx.recv().await {
            if tx.send(Event::Fx(update)).await.is_err() {
                break;
            }
        }
    });
    Ok(())
}

async fn spawn_user_data(
    args: &Args,
    client: Arc<BinanceRestClient>,
    credentials: BinanceCredentials,
    tx: tokio::sync::mpsc::Sender<Event>,
) -> Result<String, String> {
    let listen_key = client
        .start_user_data_stream(&credentials)
        .await
        .map_err(|e| e.to_string())?;
    let stream =
        BinanceUserDataStream::new(args.environment, listen_key.clone(), args.proxy.clone())
            .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let result = stream
            .run(|event| {
                let _ = tx.try_send(Event::User(event));
            })
            .await;
        let _ = tx
            .send(Event::Halt(format!("user data stream ended: {result:?}")))
            .await;
    });
    Ok(listen_key)
}

fn spawn_keepalive(
    client: Arc<BinanceRestClient>,
    credentials: BinanceCredentials,
    listen_key: String,
    tx: tokio::sync::mpsc::Sender<Event>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            if let Err(error) = client
                .keepalive_user_data_stream(&credentials, &listen_key)
                .await
            {
                let _ = tx
                    .send(Event::Halt(format!("listenKey keepalive failed: {error}")))
                    .await;
                return;
            }
        }
    });
}

async fn place_order(
    client: &BinanceRestClient,
    credentials: &BinanceCredentials,
    symbol: &str,
    intent: static_anchor_engine::execution::OrderIntent,
    price_scale: u32,
    quantity_scale: u32,
    now: u64,
    sequence: u64,
) -> Result<WorkingOrder, String> {
    let client_order_id = format!("anchorbell-{}-{}", now, sequence);
    let _response = client
        .place_maker_order(
            credentials,
            BinanceMakerOrderRequest {
                symbol: symbol.to_owned(),
                side: intent.side,
                price: format_ticks(intent.price, price_scale),
                quantity: format_ticks(intent.quantity, quantity_scale),
                client_order_id: client_order_id.clone(),
                reduce_only: false,
            },
            client.server_time_ms().await.map_err(|e| e.to_string())?,
            RECV_WINDOW_MS,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(WorkingOrder {
        client_order_id,
        side: intent.side,
        price_ticks: intent.price,
        quantity_ticks: intent.quantity,
    })
}

async fn cancel_symbol_orders(
    client: &BinanceRestClient,
    credentials: &BinanceCredentials,
    symbol: &str,
) -> Result<(), String> {
    client
        .cancel_all_open_orders(
            credentials,
            symbol,
            client.server_time_ms().await.map_err(|e| e.to_string())?,
            RECV_WINDOW_MS,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn cancel_order(
    client: &BinanceRestClient,
    credentials: &BinanceCredentials,
    symbol: &str,
    client_order_id: &str,
) -> Result<(), String> {
    client
        .cancel_order(
            credentials,
            symbol,
            client_order_id,
            client.server_time_ms().await.map_err(|e| e.to_string())?,
            RECV_WINDOW_MS,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn equity_is_closed(region: EquityRegion) -> bool {
    let (_, minute) = local_clock();
    let weekday = local_clock().0;
    let state = calendar_for(region).detailed_state_at(weekday, minute, false, 30, true);
    matches!(
        state,
        VenueSessionState::Closed | VenueSessionState::MiddayBreak
    )
}

fn local_clock() -> (u8, u16) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = seconds / 86_400;
    let weekday = ((days + 4) % 7 + 1) as u8;
    let minute = ((seconds + 8 * 3_600) % 86_400 / 60) as u16;
    (weekday, minute)
}

fn load_credentials(environment: BinanceEnvironment) -> Result<BinanceCredentials, String> {
    BinanceCredentials::from_environment_for(environment)
        .map_err(|error| format!("credentials unavailable: {error:?}"))
}

fn stable_symbol_id(symbol: &str) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for byte in symbol.bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}

fn parse_ticks(value: &str, scale: u32) -> Option<i64> {
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |v| (true, v));
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > scale as usize
    {
        return None;
    }
    let multiplier = 10_i128.checked_pow(scale)?;
    let whole = whole.parse::<i128>().ok()?.checked_mul(multiplier)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i128>()
            .ok()?
            .checked_mul(10_i128.checked_pow(scale.saturating_sub(fraction.len() as u32))?)?
    };
    let signed = if negative {
        whole.checked_add(fraction)?.checked_neg()?
    } else {
        whole.checked_add(fraction)?
    };
    i64::try_from(signed).ok()
}

fn format_ticks(value: i64, scale: u32) -> String {
    let negative = value < 0;
    let value = value.unsigned_abs();
    let multiplier = 10_u64.pow(scale.min(18));
    let whole = value / multiplier;
    let fraction = value % multiplier;
    if scale == 0 {
        return format!("{}{}", if negative { "-" } else { "" }, whole);
    }
    format!(
        "{}{}.{:0width$}",
        if negative { "-" } else { "" },
        whole,
        fraction,
        width = scale as usize
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Buy => "BUY",
        Side::Sell => "SELL",
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        environment: BinanceEnvironment::Testnet,
        duration_secs: 60,
        proxy: None,
        price_scale: 8,
        quantity_scale: 8,
        max_position: 1,
        quantity: 1,
        entry_threshold_bps: 100,
        max_mark_index_gap_bps: 50,
        max_anchor_age_ms: 0,
        funding_lead_ms: 300_000,
        max_subscriptions_per_shard: 64,
        send_orders: false,
    };
    let mut values = env::args().skip(1);
    while let Some(flag) = values.next() {
        match flag.as_str() {
            "--help" | "-h" => {
                println!("anchorbell_live [--environment testnet|production] [--duration-secs N] [--proxy URL] [--send-orders]");
                process::exit(0);
            }
            "--environment" => {
                args.environment = values
                    .next()
                    .ok_or("missing --environment")?
                    .parse()
                    .map_err(|_| "invalid --environment".to_owned())?
            }
            "--duration-secs" => args.duration_secs = parse_next(&mut values, &flag)?,
            "--proxy" => args.proxy = Some(values.next().ok_or("missing --proxy")?),
            "--price-scale" => args.price_scale = parse_next(&mut values, &flag)?,
            "--quantity-scale" => args.quantity_scale = parse_next(&mut values, &flag)?,
            "--max-position" => args.max_position = parse_next(&mut values, &flag)?,
            "--quantity" => args.quantity = parse_next(&mut values, &flag)?,
            "--entry-threshold-bps" => args.entry_threshold_bps = parse_next(&mut values, &flag)?,
            "--max-mark-index-gap-bps" => {
                args.max_mark_index_gap_bps = parse_next(&mut values, &flag)?
            }
            "--max-anchor-age-ms" => args.max_anchor_age_ms = parse_next(&mut values, &flag)?,
            "--funding-lead-ms" => args.funding_lead_ms = parse_next(&mut values, &flag)?,
            "--max-subscriptions-per-shard" => {
                args.max_subscriptions_per_shard = parse_next(&mut values, &flag)?
            }
            "--send-orders" => args.send_orders = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if args.duration_secs == 0
        || args.price_scale > 18
        || args.quantity_scale > 18
        || args.max_position <= 0
        || args.quantity <= 0
        || args.entry_threshold_bps < 0
        || args.max_mark_index_gap_bps < 0
        || args.max_subscriptions_per_shard == 0
    {
        return Err("invalid numeric configuration".into());
    }
    Ok(args)
}

fn parse_next<T: std::str::FromStr>(
    values: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    values
        .next()
        .ok_or_else(|| format!("missing {flag}"))?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    process::exit(2);
}
