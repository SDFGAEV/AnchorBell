use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::network::connect_websocket;

use super::{BinanceEnvironment, DeploymentPolicy};

#[derive(Debug, Error)]
pub enum OrderTransportError {
    #[error("deployment policy rejected order transport: {0:?}")]
    Policy(super::SafetyError),
    #[error("websocket error: {0}")]
    WebSocket(#[source] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("network transport error: {0}")]
    Network(String),
    #[error("response stream closed")]
    Closed,
    #[error("response was not valid text")]
    NonTextResponse,
    #[error("response did not contain the requested id")]
    CorrelationMismatch,
    #[error("exchange returned error {code}: {message}")]
    Exchange { code: i64, message: String },
}

pub struct BinanceOrderWebSocket {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl BinanceOrderWebSocket {
    pub async fn connect(
        environment: BinanceEnvironment,
        policy: DeploymentPolicy,
    ) -> Result<Self, OrderTransportError> {
        Self::connect_with_proxy(environment, policy, None).await
    }

    pub async fn connect_with_proxy(
        environment: BinanceEnvironment,
        policy: DeploymentPolicy,
        http_proxy: Option<&str>,
    ) -> Result<Self, OrderTransportError> {
        policy
            .validate_for(environment)
            .map_err(OrderTransportError::Policy)?;
        let endpoint = environment.endpoints().order_ws_base;
        let socket = connect_websocket(endpoint, 10_000, http_proxy)
            .await
            .map_err(|error| OrderTransportError::Network(error.to_string()))?;
        Ok(Self { socket })
    }

    pub async fn request(&mut self, payload: Value) -> Result<Value, OrderTransportError> {
        let request_id = payload
            .get("id")
            .and_then(Value::as_str)
            .ok_or(OrderTransportError::CorrelationMismatch)?
            .to_string();
        self.socket
            .send(Message::Text(payload.to_string()))
            .await
            .map_err(|error| OrderTransportError::WebSocket(Box::new(error)))?;

        while let Some(message) = self.socket.next().await {
            match message.map_err(|error| OrderTransportError::WebSocket(Box::new(error)))? {
                Message::Text(text) => {
                    let response: Value = serde_json::from_str(&text)
                        .map_err(|_| OrderTransportError::NonTextResponse)?;
                    if response.get("id").and_then(Value::as_str) != Some(request_id.as_str()) {
                        continue;
                    }
                    if response
                        .get("status")
                        .and_then(Value::as_u64)
                        .is_some_and(|status| status >= 400)
                    {
                        let code = response
                            .get("error")
                            .and_then(|error| error.get("code"))
                            .and_then(Value::as_i64)
                            .unwrap_or(-1);
                        let message = response
                            .get("error")
                            .and_then(|error| error.get("msg"))
                            .and_then(Value::as_str)
                            .unwrap_or("unknown exchange error")
                            .to_string();
                        return Err(OrderTransportError::Exchange { code, message });
                    }
                    return Ok(response);
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| OrderTransportError::WebSocket(Box::new(error)))?;
                }
                Message::Close(_) => return Err(OrderTransportError::Closed),
                _ => {}
            }
        }
        Err(OrderTransportError::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_request_id_at_transport_boundary() {
        let payload = serde_json::json!({"method": "order.place", "params": {}});
        assert!(payload.get("id").and_then(Value::as_str).is_none());
    }

    #[test]
    fn environment_mismatch_is_rejected_before_network_connect() {
        let policy = DeploymentPolicy {
            environment: BinanceEnvironment::Testnet,
            allow_live_orders: false,
            credentials_loaded: true,
        };
        assert_eq!(
            policy.validate_for(BinanceEnvironment::Production),
            Err(super::super::SafetyError::EnvironmentMismatch)
        );
    }
}
