use std::{collections::BTreeMap, time::Duration};

use serde::Deserialize;
use thiserror::Error;

use super::{
    signing::{canonical_query, signed_params, SigningError},
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
    #[error("REST request transport failed")]
    Transport,
    #[error("REST endpoint returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("REST response could not be decoded")]
    Decode,
    #[error("request signing failed: {0:?}")]
    Signing(SigningError),
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

    pub async fn current_open_orders(
        &self,
        credentials: &BinanceCredentials,
        symbol: Option<&str>,
        timestamp_ms: u64,
        recv_window_ms: u64,
    ) -> Result<Vec<BinanceOpenOrder>, BinanceRestError> {
        let query = signed_open_orders_query(
            symbol,
            &credentials.api_key,
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
            .map_err(|_| BinanceRestError::Transport)?;
        if !response.status().is_success() {
            return Err(BinanceRestError::HttpStatus {
                status: response.status().as_u16(),
            });
        }
        response
            .json::<Vec<BinanceOpenOrder>>()
            .await
            .map_err(|_| BinanceRestError::Decode)
    }
}

fn signed_open_orders_query(
    symbol: Option<&str>,
    api_key: &str,
    secret: &str,
    timestamp_ms: u64,
    recv_window_ms: u64,
) -> Result<String, SigningError> {
    let mut params = BTreeMap::new();
    if let Some(symbol) = symbol {
        params.insert("symbol".into(), symbol.to_owned());
    }
    let params = signed_params(params, api_key, secret, timestamp_ms, recv_window_ms)?;
    Ok(canonical_query(&params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_symbol_scoped_signed_open_orders_query() {
        let query = signed_open_orders_query(Some("BTCUSDT"), "key", "secret", 100, 5000).unwrap();
        assert!(query.contains("symbol=BTCUSDT"));
        assert!(query.contains("apiKey=key"));
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
