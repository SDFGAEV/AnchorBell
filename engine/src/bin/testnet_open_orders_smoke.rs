use std::env;

use static_anchor_engine::execution::{
    BinanceCredentials, BinanceEnvironment, BinanceRestClient, DeploymentPolicy,
};

#[tokio::main]
async fn main() {
    let credentials = match BinanceCredentials::from_environment() {
        Ok(credentials) => credentials,
        Err(error) => {
            eprintln!("open orders smoke stopped before network: {error:?}");
            std::process::exit(2);
        }
    };
    let policy = DeploymentPolicy {
        environment: BinanceEnvironment::Testnet,
        allow_live_orders: false,
        credentials_loaded: true,
    };
    let proxy = env::var("ANCHORBELL_HTTP_PROXY").ok();
    let client = match BinanceRestClient::new(BinanceEnvironment::Testnet, policy, proxy.as_deref())
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("open orders smoke stopped before network: {error}");
            std::process::exit(2);
        }
    };
    let symbol = env::var("ANCHORBELL_SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_owned());
    match client
        .current_open_orders(&credentials, Some(&symbol), now_ms(), 5_000)
        .await
    {
        Ok(orders) => {
            println!("open_orders_symbol={symbol} count={}", orders.len());
        }
        Err(error) => {
            eprintln!("open orders smoke failed: {error}");
            std::process::exit(2);
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_millis() as u64
}
