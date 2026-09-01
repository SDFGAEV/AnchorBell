use std::{collections::BTreeMap, time::Duration};

use serde::Deserialize;
use thiserror::Error;

use super::{
    signing::{canonical_query, sign_query, SigningError},
    BinanceCredentials, BinanceEnvironment, DeploymentPolicy, SafetyError,
};

#[derive(Debug, Error)]
pub enum BinanceRestError {
    #[error("deployment policy rejected REST transport: {0:?}")]
    Policy(SafetyError),
    #[error("invalid HTTP proxy configuration")]
    InvalidProxy,
    #[error("REST client construction failed")]
    ClientBuild,
    #[error("REST request transport failed: {message}")]
    Transport { message: String },
    #[error("REST endpoint returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("REST response could not be decoded")]
    Decode,
    #[error("request signing failed: {0:?}")]
    Signing(SigningError),
    #[error("Binance returned application error {code}: {message}")]
    Exchange { code: i64, message: String },
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceOpenOrder {
    pub symbol: String,
    #[serde(rename = "clientOrderId")]
    pub client_order_id: String,
    pub status: String,
    #[serde(rename = "executedQty")]
    pub executed_quantity: String,
    #[serde(rename = "orderId")]
    pub order_id: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceTradFiContractResponse {
    pub code: i64,
    pub msg: String,
}

pub struct BinanceRestClient {
    environment: BinanceEnvironment,
    client: reqwest::Client,
}

impl BinanceRestClient {
    pub fn new(
        environment: BinanceEnvironment,
        policy: DeploymentPolicy,
        http_proxy: Option<&str>,
    ) -> Result<Self, BinanceRestError> {
        policy
            .validate_for(environment)
            .map_err(BinanceRestError::Policy)?;
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10));
        if let Some(proxy_url) = http_proxy {
            let proxy =
                reqwest::Proxy::http(proxy_url).map_err(|_| BinanceRestError::InvalidProxy)?;
            builder = builder.proxy(proxy);
        }
        let client = builder.build().map_err(|_| BinanceRestError::ClientBuild)?;
        Ok(Self {
            environment,
            client,
        })
    }

    pub async fn sign_tradfi_contract(
        &self,
        credentials: &BinanceCredentials,
        timestamp_ms: u64,
        recv_window_ms: u64,
    ) -> Result<BinanceTradFiContractResponse, BinanceRestError> {
        let body =
            signed_tradfi_contract_body(&credentials.api_secret, timestamp_ms, recv_window_ms)
                .map_err(BinanceRestError::Signing)?;
        let url = format!(
            "{}/fapi/v1/stock/contract",
            self.environment.endpoints().rest_base
        );
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("X-MBX-APIKEY", &credentials.api_key)
            .body(body)
            .send()
            .await
            .map_err(|error| BinanceRestError::Transport {
                message: error.to_string(),
            })?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|error| BinanceRestError::Transport {
                message: error.to_string(),
            })?;
        if status >= 400 {
            return Err(exchange_error(status, &body));
        }
        let result = serde_json::from_slice::<BinanceTradFiContractResponse>(&body)
            .map_err(|_| BinanceRestError::Decode)?;
        if result.code != 200 {
            return Err(BinanceRestError::Exchange {
                code: result.code,
                message: result.msg.clone(),
            });
        }
        Ok(result)
    }

    pub async fn current_open_orders(
        &self,
        credentials: &BinanceCredentials,
        symbol: Option<&str>,
        timestamp_ms: u64,
        recv_window_ms: u64,
    ) -> Result<Vec<BinanceOpenOrder>, BinanceRestError> {
        let query = signed_open_orders_query(
            symbol,
            &credentials.api_secret,
            timestamp_ms,
            recv_window_ms,
        )
        .map_err(BinanceRestError::Signing)?;
        let url = format!(
            "{}/fapi/v1/openOrders?{query}",
            self.environment.endpoints().rest_base
        );
        let response = self
            .client
            .get(url)
            .header("X-MBX-APIKEY", &credentials.api_key)
            .send()
            .await
            .map_err(|error| BinanceRestError::Transport {
                message: error.to_string(),
            })?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|error| BinanceRestError::Transport {
                message: error.to_string(),
            })?;
        if status >= 400 {
            return Err(exchange_error(status, &body));
        }
        serde_json::from_slice::<Vec<BinanceOpenOrder>>(&body).map_err(|_| BinanceRestError::Decode)
    }
}

fn signed_tradfi_contract_body(
    secret: &str,
    timestamp_ms: u64,
    recv_window_ms: u64,
) -> Result<String, SigningError> {
    signed_rest_query(BTreeMap::new(), secret, timestamp_ms, recv_window_ms)
}

fn signed_open_orders_query(
    symbol: Option<&str>,
    secret: &str,
    timestamp_ms: u64,
    recv_window_ms: u64,
) -> Result<String, SigningError> {
    let mut params = BTreeMap::new();
    if let Some(symbol) = symbol {
        params.insert("symbol".into(), symbol.to_owned());
    }
    signed_rest_query(params, secret, timestamp_ms, recv_window_ms)
}

fn exchange_error(status: u16, body: &[u8]) -> BinanceRestError {
    match serde_json::from_slice::<BinanceTradFiContractResponse>(body) {
        Ok(result) => BinanceRestError::Exchange {
            code: result.code,
            message: result.msg,
        },
        Err(_) => BinanceRestError::HttpStatus { status },
    }
}

fn signed_rest_query(
    mut params: BTreeMap<String, String>,
    secret: &str,
    timestamp_ms: u64,
    recv_window_ms: u64,
) -> Result<String, SigningError> {
    params.insert("timestamp".into(), timestamp_ms.to_string());
    params.insert("recvWindow".into(), recv_window_ms.to_string());
    let signature = sign_query(&params, secret)?;
    params.insert("signature".into(), signature);
    Ok(canonical_query(&params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_signed_tradfi_contract_body_without_order_fields() {
        let body = signed_tradfi_contract_body("secret", 100, 5000).unwrap();
        assert!(body.contains("recvWindow=5000"));
        assert!(body.contains("timestamp=100"));
        assert!(body.contains("signature="));
        assert!(!body.contains("apiKey="));
        assert!(!body.contains("symbol="));
        assert!(!body.contains("secret"));
    }

    #[test]
    fn decodes_successful_tradfi_contract_response() {
        let response: BinanceTradFiContractResponse = serde_json::from_value(serde_json::json!({
            "code": 200,
            "msg": "success"
        }))
        .unwrap();
        assert_eq!(response.code, 200);
        assert_eq!(response.msg, "success");
    }

    #[test]
    fn builds_symbol_scoped_signed_open_orders_query() {
        let query = signed_open_orders_query(Some("BTCUSDT"), "secret", 100, 5000).unwrap();
        assert!(query.contains("symbol=BTCUSDT"));
        assert!(!query.contains("apiKey="));
        assert!(query.contains("recvWindow=5000"));
        assert!(query.contains("signature="));
        assert!(!query.contains("secret"));
    }

    #[test]
    fn rejects_production_mismatch_before_client_creation() {
        let policy = DeploymentPolicy {
            environment: BinanceEnvironment::Testnet,
            allow_live_orders: false,
            allow_production: false,
            credentials_loaded: true,
        };
        assert!(matches!(
            BinanceRestClient::new(BinanceEnvironment::Production, policy, None),
            Err(BinanceRestError::Policy(SafetyError::EnvironmentMismatch))
        ));
    }
}
