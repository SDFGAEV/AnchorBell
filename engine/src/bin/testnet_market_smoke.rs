use static_anchor_engine::execution::BinanceEnvironment;
use static_anchor_engine::market::{
    BinanceMarketConfig, BinanceMarketStream, BinanceSubscription, ReconnectPolicy,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let symbol = std::env::var("ANCHORBELL_TESTNET_SYMBOL").unwrap_or_else(|_| "BTCUSDT".into());
    let subscription = BinanceSubscription::new(symbol).expect("valid symbol");
    let config = BinanceMarketConfig {
        market_ws_base: BinanceEnvironment::Testnet
            .endpoints()
            .market_ws_base
            .into(),
        subscriptions: vec![subscription],
        price_scale: 2,
        quantity_scale: 8,
        max_frame_bytes: 1_048_576,
        connect_timeout_ms: 5_000,
        reconnect: ReconnectPolicy {
            max_attempts: Some(1),
            ..ReconnectPolicy::default()
        },
    };
    let mut stream = BinanceMarketStream::new(config);
    let mut count = 0_u32;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        stream.run_until_error(|event| {
            count += 1;
            println!("event={event:?}");
        }),
    )
    .await;
    println!("market_smoke_events={count} result={result:?}");
    if count == 0 {
        eprintln!("market smoke failed: no parsed events received");
        std::process::exit(2);
    }
}
