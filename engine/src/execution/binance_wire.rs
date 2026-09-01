use std::collections::BTreeMap;

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
pub struct BinanceOrderWire {
    pub request_id: String,
    pub symbol: String,
    pub side: &'static str,
    pub price_ticks: i64,
    pub quantity_ticks: i64,
    pub price_scale: u32,
    pub quantity_scale: u32,
    pub client_order_id: String,
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
        params.insert("side".into(), self.side.into());
        params.insert("symbol".into(), self.symbol.clone());
        params.insert("timeInForce".into(), "GTX".into());
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
            side: "BUY",
            price_ticks: 123400,
            quantity_ticks: 250,
            price_scale: 4,
            quantity_scale: 2,
            client_order_id: "anchorbell-1".into(),
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
            side: "BUY",
            price_ticks: 123400,
            quantity_ticks: 250,
            price_scale: 4,
            quantity_scale: 2,
            client_order_id: "anchorbell-1".into(),
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
