use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use thiserror::Error;
use tokio_tungstenite::tungstenite::Message;

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
                Ok(socket) => {
                    let mut socket = socket;
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
) -> Result<crate::network::ConnectedWebSocket, MarketStreamError> {
    crate::network::connect_websocket(url, timeout_ms, http_proxy)
        .await
        .map_err(map_network_error)
}

fn map_network_error(error: crate::network::NetworkError) -> MarketStreamError {
    match error {
        crate::network::NetworkError::InvalidUrl => MarketStreamError::Dns(Box::new(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid market URL"),
        )),
        crate::network::NetworkError::Dns(error) => MarketStreamError::Dns(error),
        crate::network::NetworkError::Tcp(error) => MarketStreamError::Tcp(error),
        crate::network::NetworkError::Proxy(error) => MarketStreamError::Proxy(error),
        crate::network::NetworkError::WebSocket(error) => MarketStreamError::WebSocket(error),
        crate::network::NetworkError::Timeout => MarketStreamError::ConnectTimeout,
    }
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
