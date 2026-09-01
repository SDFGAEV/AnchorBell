use std::{
    net::SocketAddr,
    sync::OnceLock,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{
    client_async_tls,
    tungstenite::{http::Uri, Message},
};

use super::{
    binance::{parse_market_message, BinanceMarketEvent, ParseError},
    connection::{ConnectionSupervisor, ReconnectPolicy},
    BinanceSubscription,
};

#[derive(Debug, Error)]
pub enum MarketStreamError {
    #[error("no subscriptions configured")]
    NoSubscriptions,
    #[error("invalid subscription: {0:?}")]
    InvalidSubscription(super::SubscriptionError),
    #[error("websocket error: {0}")]
    WebSocket(#[source] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("websocket connection timed out")]
    ConnectTimeout,
    #[error("market DNS resolution failed: {0}")]
    Dns(#[source] Box<std::io::Error>),
    #[error("market TCP connection failed: {0}")]
    Tcp(#[source] Box<std::io::Error>),
    #[error("market proxy tunnel failed: {0}")]
    Proxy(String),
    #[error("market payload exceeds configured limit")]
    FrameTooLarge,
    #[error("market payload parse failed: {0:?}")]
    Parse(ParseError),
}

#[derive(Debug, Clone)]
pub struct BinanceMarketConfig {
    pub market_ws_base: String,
    pub subscriptions: Vec<BinanceSubscription>,
    pub price_scale: u32,
    pub quantity_scale: u32,
    pub max_frame_bytes: usize,
    pub connect_timeout_ms: u64,
    pub http_proxy: Option<String>,
    pub reconnect: ReconnectPolicy,
}

impl BinanceMarketConfig {
    pub fn combined_stream_url(&self) -> Result<String, MarketStreamError> {
        if self.subscriptions.is_empty() {
            return Err(MarketStreamError::NoSubscriptions);
        }
        let streams = self
            .subscriptions
            .iter()
            .map(BinanceSubscription::stream_names)
            .collect::<Result<Vec<_>, _>>()
            .map_err(MarketStreamError::InvalidSubscription)?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        Ok(format!(
            "{}/stream?streams={}",
            self.market_ws_base.trim_end_matches('/'),
            streams.join("/")
        ))
    }

    pub fn subscription_endpoint(&self) -> Result<String, MarketStreamError> {
        if self.subscriptions.is_empty() {
            return Err(MarketStreamError::NoSubscriptions);
        }
        Ok(format!(
            "{}/stream",
            self.market_ws_base.trim_end_matches('/')
        ))
    }

    pub fn subscription_request(&self) -> Result<serde_json::Value, MarketStreamError> {
        if self.subscriptions.is_empty() {
            return Err(MarketStreamError::NoSubscriptions);
        }
        let streams = self
            .subscriptions
            .iter()
            .map(BinanceSubscription::stream_names)
            .collect::<Result<Vec<_>, _>>()
            .map_err(MarketStreamError::InvalidSubscription)?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        Ok(json!({"method":"SUBSCRIBE","params":streams,"id":"anchorbell-market-1"}))
    }
}

pub struct BinanceMarketStream {
    config: BinanceMarketConfig,
    supervisor: ConnectionSupervisor,
}

impl BinanceMarketStream {
    pub fn new(config: BinanceMarketConfig) -> Self {
        let supervisor = ConnectionSupervisor::new(config.reconnect);
        Self { config, supervisor }
    }

    pub fn supervisor(&self) -> &ConnectionSupervisor {
        &self.supervisor
    }

    pub async fn run_until_error<F>(&mut self, mut on_event: F) -> Result<(), MarketStreamError>
    where
        F: FnMut(BinanceMarketEvent) + Send,
    {
        let url = self.config.combined_stream_url()?;
        loop {
            self.supervisor.on_connecting();
            let connection = connect_market_stream(
                &url,
                self.config.connect_timeout_ms,
                self.config.http_proxy.as_deref(),
            )
            .await;
            match connection {
                Ok((mut socket, _response)) => {
                    self.supervisor.on_connected();
                    eprintln!("market stream connected: {url}");
                    while let Some(message) = socket.next().await {
                        match message
                            .map_err(|error| MarketStreamError::WebSocket(Box::new(error)))?
                        {
                            Message::Text(text) => {
                                let payload = text.as_bytes();
                                if payload.len() > self.config.max_frame_bytes {
                                    return Err(MarketStreamError::FrameTooLarge);
                                }
                                if let Ok(control) =
                                    serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    if control.get("id").is_some()
                                        && control.get("result").is_some()
                                    {
                                        continue;
                                    }
                                }
                                let event = parse_market_message(
                                    payload,
                                    self.config.price_scale,
                                    self.config.quantity_scale,
                                )
                                .map_err(MarketStreamError::Parse)?;
                                on_event(event);
                            }
                            Message::Binary(payload) => {
                                if payload.len() > self.config.max_frame_bytes {
                                    return Err(MarketStreamError::FrameTooLarge);
                                }
                                let event = parse_market_message(
                                    &payload,
                                    self.config.price_scale,
                                    self.config.quantity_scale,
                                )
                                .map_err(MarketStreamError::Parse)?;
                                on_event(event);
                            }
                            Message::Ping(payload) => {
                                socket.send(Message::Pong(payload)).await.map_err(|error| {
                                    MarketStreamError::WebSocket(Box::new(error))
                                })?;
                            }
                            Message::Close(_) => break,
                            _ => {}
                        }
                    }
                    if let Some((_, delay)) = self.supervisor.on_disconnect() {
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Ok(());
                }
                Err(error) => {
                    if let Some((_, delay)) = self.supervisor.on_disconnect() {
                        tokio::time::sleep(delay).await;
                        if self.supervisor.state() == super::connection::ConnectionState::Halted {
                            return Err(error);
                        }
                    } else {
                        return Err(error);
                    }
                }
            }
        }
    }
}

async fn connect_market_stream(
    url: &str,
    timeout_ms: u64,
    http_proxy: Option<&str>,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    MarketStreamError,
> {
    if timeout_ms == 0 {
        return Err(MarketStreamError::ConnectTimeout);
    }
    static RUSTLS_PROVIDER: OnceLock<()> = OnceLock::new();
    RUSTLS_PROVIDER.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    let parsed: Uri = url.parse::<Uri>().map_err(|_| {
        MarketStreamError::Dns(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid market URL",
        )))
    })?;
    let host = parsed.host().ok_or_else(|| {
        MarketStreamError::Dns(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "market URL has no host",
        )))
    })?;
    let port = parsed
        .port_u16()
        .or_else(|| match parsed.scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or_else(|| {
            MarketStreamError::Dns(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "market URL has no port",
            )))
        })?;
    let proxy_endpoint = http_proxy.map(parse_proxy_endpoint).transpose()?;
    let (connect_host, connect_port) = proxy_endpoint
        .as_ref()
        .map(|(proxy_host, proxy_port)| (proxy_host.as_str(), *proxy_port))
        .unwrap_or((host, port));
    let mut addresses: Vec<SocketAddr> = tokio::net::lookup_host((connect_host, connect_port))
        .await
        .map_err(|error| MarketStreamError::Dns(Box::new(error)))?
        .collect();
    addresses.sort_by_key(|address| !address.is_ipv4());
    if addresses.is_empty() {
        return Err(MarketStreamError::Dns(Box::new(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "market host resolved to no addresses",
        ))));
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut last_tcp_error = None;
    let mut last_proxy_error = None;
    let mut last_websocket_error = None;
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let attempt_timeout = remaining.min(Duration::from_millis(1_500));
        match tokio::time::timeout(attempt_timeout, TcpStream::connect(address)).await {
            Ok(Ok(mut socket)) => {
                if proxy_endpoint.is_some() {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(
                        remaining,
                        establish_proxy_tunnel(&mut socket, host, port),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            last_proxy_error = Some(error);
                            continue;
                        }
                        Err(_) => break,
                    }
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, client_async_tls(url.to_owned(), socket))
                    .await
                {
                    Ok(Ok(connection)) => return Ok(connection),
                    Ok(Err(error)) => last_websocket_error = Some(error),
                    Err(_) => break,
                }
            }
            Ok(Err(error)) => last_tcp_error = Some(error),
            Err(_) => {}
        }
    }
    if let Some(error) = last_proxy_error {
        return Err(MarketStreamError::Proxy(error));
    }
    if let Some(error) = last_websocket_error {
        return Err(MarketStreamError::WebSocket(Box::new(error)));
    }
    if let Some(error) = last_tcp_error {
        return Err(MarketStreamError::Tcp(Box::new(error)));
    }
    Err(MarketStreamError::ConnectTimeout)
}

fn parse_proxy_endpoint(proxy_url: &str) -> Result<(String, u16), MarketStreamError> {
    let normalized = if proxy_url.contains("://") {
        proxy_url.to_owned()
    } else {
        format!("http://{proxy_url}")
    };
    let parsed: Uri = normalized
        .parse::<Uri>()
        .map_err(|_| MarketStreamError::Proxy("invalid HTTP proxy URL".to_owned()))?;
    if parsed.scheme_str().is_some_and(|scheme| scheme != "http") {
        return Err(MarketStreamError::Proxy(
            "only HTTP CONNECT proxies are supported".to_owned(),
        ));
    }
    let host = parsed
        .host()
        .ok_or_else(|| MarketStreamError::Proxy("HTTP proxy URL has no host".to_owned()))?;
    let port = parsed.port_u16().unwrap_or(80);
    Ok((host.to_owned(), port))
}

async fn establish_proxy_tunnel(
    socket: &mut TcpStream,
    target_host: &str,
    target_port: u16,
) -> Result<(), String> {
    let authority = format!("{target_host}:{target_port}");
    let request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n\r\n"
    );
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    let mut response = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        let read = socket
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("proxy closed before CONNECT response".to_owned());
        }
        response.extend_from_slice(&chunk[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > 8 * 1024 {
            return Err("proxy CONNECT response exceeds 8 KiB".to_owned());
        }
    }
    let header = String::from_utf8_lossy(&response);
    let status = header.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("proxy CONNECT rejected: {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_combined_market_stream_url() {
        let config = BinanceMarketConfig {
            market_ws_base: "wss://demo-fstream.binance.com/public/".into(),
            subscriptions: vec![BinanceSubscription::new("ABCUSDT").unwrap()],
            price_scale: 4,
            quantity_scale: 2,
            max_frame_bytes: 1_048_576,
            connect_timeout_ms: 5_000,
            http_proxy: None,
            reconnect: ReconnectPolicy::default(),
        };
        assert_eq!(
            config.combined_stream_url().unwrap(),
            "wss://demo-fstream.binance.com/public/stream?streams=abcusdt@bookTicker/abcusdt@markPrice@1s"
        );
    }

    #[test]
    fn builds_explicit_subscription_request() {
        let config = BinanceMarketConfig {
            market_ws_base: "wss://demo-fstream.binance.com/public".into(),
            subscriptions: vec![BinanceSubscription::new("ABCUSDT").unwrap()],
            price_scale: 4,
            quantity_scale: 2,
            max_frame_bytes: 1_048_576,
            connect_timeout_ms: 5_000,
            http_proxy: None,
            reconnect: ReconnectPolicy::default(),
        };
        let request = config.subscription_request().unwrap();
        assert_eq!(request["method"], "SUBSCRIBE");
        assert_eq!(request["params"][0], "abcusdt@bookTicker");
        assert_eq!(request["id"], "anchorbell-market-1");
        assert_eq!(
            config.subscription_endpoint().unwrap(),
            "wss://demo-fstream.binance.com/public/stream"
        );
    }

    #[test]
    fn parses_http_proxy_endpoint_without_enabling_it_by_default() {
        assert_eq!(
            parse_proxy_endpoint("127.0.0.1:7890").unwrap(),
            ("127.0.0.1".to_owned(), 7890)
        );
        assert!(matches!(
            parse_proxy_endpoint("https://127.0.0.1:7890"),
            Err(MarketStreamError::Proxy(_))
        ));
    }

    #[test]
    fn rejects_empty_subscription_set() {
        let config = BinanceMarketConfig {
            market_ws_base: "wss://demo-fstream.binance.com".into(),
            subscriptions: Vec::new(),
            price_scale: 4,
            quantity_scale: 2,
            max_frame_bytes: 1_048_576,
            connect_timeout_ms: 5_000,
            http_proxy: None,
            reconnect: ReconnectPolicy::default(),
        };
        assert!(matches!(
            config.combined_stream_url(),
            Err(MarketStreamError::NoSubscriptions)
        ));
    }
}
