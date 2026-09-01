use std::collections::BTreeMap;
use serde_json::{json, Value};
use super::signing::{signed_params, SigningError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpotDemoEndpoints {
    pub rest_base: &'static str,
    pub order_ws_base: &'static str,
}
impl SpotDemoEndpoints {
    pub const fn demo() -> Self {
        Self { rest_base: "https://demo-api.binance.com/api", order_ws_base: "wss://ws-api.testnet.binance.vision/ws-api/v3" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotOrderWire {
    pub request_id: String,
    pub symbol: String,
    pub side: &'static str,
    pub price: String,
    pub quantity: String,
    pub client_order_id: String,
    pub timestamp_ms: u64,
    pub recv_window_ms: u64,
}
impl SpotOrderWire {
    pub fn limit_maker_payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let mut params = BTreeMap::new();
        params.insert("apiKey".into(), api_key.into());
        params.insert("newClientOrderId".into(), self.client_order_id.clone());
        params.insert("price".into(), self.price.clone());
        params.insert("quantity".into(), self.quantity.clone());
        params.insert("side".into(), self.side.into());
        params.insert("symbol".into(), self.symbol.clone());
        params.insert("type".into(), "LIMIT_MAKER".into());
        let params = signed_params(params, api_key, secret, self.timestamp_ms, self.recv_window_ms)?;
        Ok(json!({"id": self.request_id, "method": "order.place", "params": params}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_endpoints_are_not_futures_endpoints() {
        let endpoints = SpotDemoEndpoints::demo();
        assert_eq!(endpoints.rest_base, "https://demo-api.binance.com/api");
        assert!(endpoints.order_ws_base.contains("ws-api.testnet.binance.vision"));
    }
    #[test]
    fn emits_spot_limit_maker_payload() {
        let order = SpotOrderWire { request_id: "spot-1".into(), symbol: "BTCUSDT".into(), side: "BUY", price: "70000.00".into(), quantity: "0.0001".into(), client_order_id: "anchorbell-spot-1".into(), timestamp_ms: 100, recv_window_ms: 5000 };
        let payload = order.limit_maker_payload("key", "secret").unwrap();
        assert_eq!(payload["method"], "order.place");
        assert_eq!(payload["params"]["type"], "LIMIT_MAKER");
        assert!(payload["params"]["signature"].as_str().is_some());
    }
}
