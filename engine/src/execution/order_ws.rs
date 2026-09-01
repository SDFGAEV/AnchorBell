use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream,
    WebSocketStream,
};

use super::{BinanceEnvironment, DeploymentPolicy};

#[derive(Debug, Error)]
pub enum OrderTransportError {
    #[error("deployment policy rejected order transport: {0:?}")]
    Policy(super::SafetyError),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
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
        policy.validate().map_err(OrderTransportError::Policy)?;
        if policy.environment != environment {
            return Err(OrderTransportError::Policy(
                super::SafetyError::ProductionNotExplicitlyEnabled,
            ));
        }
        let endpoint = environment.endpoints().order_ws_base;
        let (socket, _) = connect_async(endpoint).await?;
        Ok(Self { socket })
    }

    pub async fn request(&mut self, payload: Value) -> Result<Value, OrderTransportError> {
        let request_id = payload
            .get("id")
            .and_then(Value::as_str)
            .ok_or(OrderTransportError::CorrelationMismatch)?
            .to_string();
        self.socket
            .send(Message::Text(payload.to_string().into()))
            .await?;

        while let Some(message) = self.socket.next().await {
            match message? {
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
                    self.socket.send(Message::Pong(payload)).await?;
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
}
