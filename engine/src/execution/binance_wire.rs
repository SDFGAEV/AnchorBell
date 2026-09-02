use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use super::signing::{signed_params, SigningError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceAccountStatusWire {
    pub request_id: String,
    pub timestamp_ms: u64,
    pub recv_window_ms: u64,
}

impl BinanceAccountStatusWire {
    pub fn payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let params = signed_params(
            BTreeMap::new(),
            api_key,
            secret,
            self.timestamp_ms,
            self.recv_window_ms,
        )?;
        Ok(json!({
            "id": self.request_id,
            "method": "account.status",
            "params": params,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceOrderStatusWire {
    pub request_id: String,
    pub symbol: String,
    pub client_order_id: String,
    pub timestamp_ms: u64,
    pub recv_window_ms: u64,
}

impl BinanceOrderStatusWire {
    pub fn payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let mut params = BTreeMap::new();
        params.insert("origClientOrderId".into(), self.client_order_id.clone());
        params.insert("symbol".into(), self.symbol.clone());
        let params = signed_params(
            params,
            api_key,
            secret,
            self.timestamp_ms,
            self.recv_window_ms,
        )?;
        Ok(json!({
            "id": self.request_id,
            "method": "order.status",
            "params": params,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinancePositionStatusWire {
    pub request_id: String,
    pub symbol: Option<String>,
    pub timestamp_ms: u64,
    pub recv_window_ms: u64,
}

impl BinancePositionStatusWire {
    pub fn payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let mut params = BTreeMap::new();
        if let Some(symbol) = &self.symbol {
            params.insert("symbol".into(), symbol.clone());
        }
        let params = signed_params(
            params,
            api_key,
            secret,
            self.timestamp_ms,
            self.recv_window_ms,
        )?;
        Ok(json!({
            "id": self.request_id,
            "method": "v2/account.position",
            "params": params,
        }))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinancePositionSnapshot {
    pub symbol: String,
    #[serde(rename = "positionAmt")]
    pub position_amount: String,
    #[serde(rename = "positionSide")]
    pub position_side: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceOrderStatusResult {
    pub symbol: String,
    #[serde(rename = "clientOrderId")]
    pub client_order_id: String,
    pub status: String,
    #[serde(rename = "executedQty")]
    pub executed_quantity: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceOrderStatusResponse {
    pub id: String,
    pub status: u16,
    pub result: BinanceOrderStatusResult,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceAccountStatusResult {
    #[serde(default)]
    pub can_trade: bool,
    #[serde(default)]
    pub positions: Vec<BinancePositionSnapshot>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceAccountStatusResponse {
    pub id: String,
    pub status: u16,
    pub result: BinanceAccountStatusResult,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinancePositionStatusResponse {
    pub id: String,
    pub status: u16,
    pub result: Vec<BinancePositionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceOrderWire {
    pub request_id: String,
    pub symbol: String,
    pub side: super::Side,
    pub price_ticks: i64,
    pub quantity_ticks: i64,
    pub price_scale: u32,
    pub quantity_scale: u32,
    pub client_order_id: String,
    pub reduce_only: bool,
    pub timestamp_ms: u64,
    pub recv_window_ms: u64,
}

impl BinanceOrderWire {
    pub fn order_place_payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let mut params = BTreeMap::new();
        params.insert("newClientOrderId".into(), self.client_order_id.clone());
        params.insert(
            "price".into(),
            format_ticks(self.price_ticks, self.price_scale),
        );
        params.insert(
            "quantity".into(),
            format_ticks(self.quantity_ticks, self.quantity_scale),
        );
        params.insert(
            "side".into(),
            match self.side {
                super::Side::Buy => "BUY",
                super::Side::Sell => "SELL",
            }
            .into(),
        );
        params.insert("symbol".into(), self.symbol.clone());
        params.insert("timeInForce".into(), "GTX".into());
        if self.reduce_only {
            params.insert("reduceOnly".into(), "true".into());
        }
        params.insert("type".into(), "LIMIT".into());
        let params = signed_params(
            params,
            api_key,
            secret,
            self.timestamp_ms,
            self.recv_window_ms,
        )?;
        Ok(json!({
            "id": self.request_id,
            "method": "order.place",
            "params": params,
        }))
    }

    pub fn cancel_payload(&self, api_key: &str, secret: &str) -> Result<Value, SigningError> {
        let mut params = BTreeMap::new();
        params.insert("origClientOrderId".into(), self.client_order_id.clone());
        params.insert("symbol".into(), self.symbol.clone());
        let params = signed_params(
            params,
            api_key,
            secret,
            self.timestamp_ms,
            self.recv_window_ms,
        )?;
        Ok(json!({
            "id": self.request_id,
            "method": "order.cancel",
            "params": params,
        }))
    }
}

pub fn format_ticks(value: i64, scale: u32) -> String {
    let divisor = 10_i64.checked_pow(scale).unwrap_or(i64::MAX);
    let negative = value.is_negative();
    let absolute = value.unsigned_abs();
    let whole = absolute / divisor as u64;
    let fraction = absolute % divisor as u64;
    if scale == 0 {
        return if negative {
            format!("-{whole}")
        } else {
            whole.to_string()
        };
    }
    let fraction_text = format!("{fraction:0width$}", width = scale as usize);
    let result = format!("{whole}.{fraction_text}");
    if negative {
        format!("-{result}")
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::Side;

    #[test]
    fn emits_signed_account_status_payload_without_order_fields() {
        let request = BinanceAccountStatusWire {
            request_id: "account-1".into(),
            timestamp_ms: 100,
            recv_window_ms: 5000,
        };
        let payload = request.payload("key", "secret").unwrap();
        assert_eq!(payload["method"], "account.status");
        assert_eq!(payload["id"], "account-1");
        assert!(payload["params"]["signature"].as_str().is_some());
        assert!(payload["params"].get("symbol").is_none());
    }

    #[test]
    fn emits_signed_order_status_by_client_order_id() {
        let request = BinanceOrderStatusWire {
            request_id: "status-1".into(),
            symbol: "BTCUSDT".into(),
            client_order_id: "anchorbell-1".into(),
            timestamp_ms: 100,
            recv_window_ms: 5000,
        };
        let payload = request.payload("key", "secret").unwrap();
        assert_eq!(payload["method"], "order.status");
        assert_eq!(payload["params"]["origClientOrderId"], "anchorbell-1");
        assert!(payload["params"].get("orderId").is_none());
        assert!(payload["params"]["signature"].as_str().is_some());
    }

    #[test]
    fn emits_signed_position_status_with_optional_symbol_scope() {
        let request = BinancePositionStatusWire {
            request_id: "position-1".into(),
            symbol: Some("BTCUSDT".into()),
            timestamp_ms: 100,
            recv_window_ms: 5000,
        };
        let payload = request.payload("key", "secret").unwrap();
        assert_eq!(payload["method"], "v2/account.position");
        assert_eq!(payload["params"]["symbol"], "BTCUSDT");
        assert!(payload["params"]["signature"].as_str().is_some());
    }

    #[test]
    fn decodes_reconciliation_relevant_exchange_responses() {
        let order: BinanceOrderStatusResponse = serde_json::from_value(serde_json::json!({
            "id": "status-1",
            "status": 200,
            "result": {
                "symbol": "BTCUSDT",
                "clientOrderId": "anchorbell-1",
                "status": "PARTIALLY_FILLED",
                "executedQty": "0.001"
            }
        }))
        .unwrap();
        assert_eq!(order.result.executed_quantity, "0.001");

        let positions: BinancePositionStatusResponse = serde_json::from_value(serde_json::json!({
            "id": "position-1",
            "status": 200,
            "result": [{
                "symbol": "BTCUSDT",
                "positionAmt": "0.001",
                "positionSide": "BOTH"
            }]
        }))
        .unwrap();
        assert_eq!(positions.result[0].position_side, "BOTH");
    }

    #[test]
    fn formats_integer_ticks_without_float_conversion() {
        assert_eq!(format_ticks(123400, 4), "12.3400");
        assert_eq!(format_ticks(250, 2), "2.50");
        assert_eq!(format_ticks(7, 0), "7");
        assert_eq!(format_ticks(-125, 2), "-1.25");
    }

    #[test]
    fn emits_maker_limit_order_payload() {
        let order = BinanceOrderWire {
            request_id: "req-1".into(),
            symbol: "ABCUSDT".into(),
            side: Side::Buy,
            price_ticks: 123400,
            quantity_ticks: 250,
            price_scale: 4,
            quantity_scale: 2,
            client_order_id: "anchorbell-1".into(),
            reduce_only: false,
            timestamp_ms: 100,
            recv_window_ms: 5000,
        };
        let payload = order.order_place_payload("key", "secret").unwrap();
        assert_eq!(payload["method"], "order.place");
        assert_eq!(payload["params"]["type"], "LIMIT");
        assert_eq!(payload["params"]["timeInForce"], "GTX");
        assert_eq!(payload["params"]["price"], "12.3400");
        assert_eq!(payload["params"]["quantity"], "2.50");
        assert!(payload["params"]["signature"].as_str().is_some());
    }

    #[test]
    fn emits_signed_cancel_payload_with_same_correlation_id() {
        let order = BinanceOrderWire {
            request_id: "req-2".into(),
            symbol: "ABCUSDT".into(),
            side: Side::Buy,
            price_ticks: 123400,
            quantity_ticks: 250,
            price_scale: 4,
            quantity_scale: 2,
            client_order_id: "anchorbell-1".into(),
            reduce_only: false,
            timestamp_ms: 100,
            recv_window_ms: 5000,
        };
        let payload = order.cancel_payload("key", "secret").unwrap();
        assert_eq!(payload["id"], "req-2");
        assert_eq!(payload["method"], "order.cancel");
        assert_eq!(payload["params"]["origClientOrderId"], "anchorbell-1");
        assert!(payload["params"]["signature"].as_str().is_some());
    }
}
