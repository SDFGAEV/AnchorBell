use std::time::{SystemTime, UNIX_EPOCH};

use anchorbell_engine::execution::{
    BinanceAccountStatusWire, BinanceCredentials, BinanceEnvironment, BinanceOrderWebSocket,
    DeploymentPolicy,
};
use serde_json::Value;

#[tokio::main]
async fn main() {
    let credentials = match BinanceCredentials::from_environment() {
        Ok(credentials) => credentials,
        Err(error) => {
            eprintln!("account smoke stopped before network: {error:?}");
            std::process::exit(2);
        }
    };
    let timestamp_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        Err(error) => {
            eprintln!("account smoke clock failed: {error}");
            std::process::exit(2);
        }
    };
    let policy = DeploymentPolicy {
        environment: BinanceEnvironment::Testnet,
        allow_live_orders: false,
        allow_production: false,
        credentials_loaded: true,
    };
    let proxy = std::env::var("ANCHORBELL_HTTP_PROXY").ok();
    let mut socket = match BinanceOrderWebSocket::connect_with_proxy(
        BinanceEnvironment::Testnet,
        policy,
        proxy.as_deref(),
    )
    .await
    {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("account smoke connection failed: {error}");
            std::process::exit(2);
        }
    };
    let request = BinanceAccountStatusWire {
        request_id: format!("anchorbell-account-status-{timestamp_ms}"),
        timestamp_ms,
        recv_window_ms: 5_000,
    };
    let payload = match request.payload(&credentials.api_key, &credentials.api_secret) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("account smoke signing failed: {error:?}");
            std::process::exit(2);
        }
    };
    let response = match socket.request(payload).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("account smoke request failed: {error}");
            std::process::exit(2);
        }
    };
    let status = response.get("status").and_then(Value::as_u64).unwrap_or(0);
    let has_result = response.get("result").is_some();
    println!("account_smoke_status={status} result_present={has_result}");
    if status != 200 || !has_result {
        eprintln!("account smoke rejected: status={status}");
        std::process::exit(2);
    }
}
