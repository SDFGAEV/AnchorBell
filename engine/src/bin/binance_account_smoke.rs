use std::time::{SystemTime, UNIX_EPOCH};

use anchorbell_engine::execution::{
    BinanceAccountStatusResponse, BinanceAccountStatusWire, BinanceCredentials,
    BinanceOrderWebSocket, DeploymentConfig,
};

#[tokio::main]
async fn main() {
    let config = match DeploymentConfig::from_process_environment() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("account smoke configuration rejected before credentials/network: {error:?}");
            std::process::exit(2);
        }
    };
    let credentials = match BinanceCredentials::from_environment_for(config.environment) {
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
    let policy = config.policy(true);
    let proxy = std::env::var("ANCHORBELL_HTTP_PROXY").ok();
    let mut socket = match BinanceOrderWebSocket::connect_with_proxy(
        config.environment,
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
    let response: BinanceAccountStatusResponse = match socket.request_typed(payload).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("account smoke request failed: {error}");
            std::process::exit(2);
        }
    };
    println!(
        "account_smoke_environment={} status={} can_trade={} positions={}",
        config.environment,
        response.status,
        response.result.can_trade,
        response.result.positions.len()
    );
}
