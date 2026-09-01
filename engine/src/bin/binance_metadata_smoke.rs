use static_anchor_engine::execution::DeploymentConfig;
use static_anchor_engine::market::PublicMarketMetadataClient;
use static_anchor_engine::strategy::all_instruments;

#[tokio::main]
async fn main() {
    let deployment = match DeploymentConfig::from_process_environment() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("metadata smoke configuration rejected before network: {error:?}");
            std::process::exit(2);
        }
    };
    let client = match PublicMarketMetadataClient::new(
        deployment.environment.endpoints().rest_base,
        std::env::var("ANCHORBELL_HTTP_PROXY").ok().as_deref(),
    ) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("metadata client construction failed: {error}");
            std::process::exit(2);
        }
    };

    let infos = match client.exchange_info().await {
        Ok(infos) => infos,
        Err(error) => {
            eprintln!("exchangeInfo failed: {error}");
            std::process::exit(2);
        }
    };
    let mut failures = 0_u32;
    let mut checked = 0_u32;
    for instrument in all_instruments() {
        checked += 1;
        let Some(metadata) = infos
            .iter()
            .find(|metadata| metadata.symbol == instrument.symbol)
            .cloned()
        else {
            failures += 1;
            println!("symbol={} state=missing_exchange_info", instrument.symbol);
            continue;
        };

        if !metadata.is_trading_tradifi_perpetual() {
            failures += 1;
            println!(
                "symbol={} state=not_trading_tradifi_perpetual status={} contract_type={}",
                instrument.symbol, metadata.status, metadata.contract_type
            );
            continue;
        }

        match client.symbol_snapshot(instrument.symbol, metadata).await {
            Ok(snapshot) => println!(
                "symbol={} state=ok bid={} ask={} mark={} index={} funding={} next_funding={} two_sided_quote={}",
                instrument.symbol,
                snapshot.book_ticker.bid_price,
                snapshot.book_ticker.ask_price,
                snapshot.premium_index.mark_price,
                snapshot.premium_index.index_price,
                snapshot.premium_index.last_funding_rate,
                snapshot.premium_index.next_funding_time_ms,
                snapshot.book_ticker.has_two_sided_quote()
            ),
            Err(error) => {
                failures += 1;
                println!("symbol={} state=public_snapshot_error error={error}", instrument.symbol);
            }
        }
    }
    println!(
        "metadata_smoke_environment={} checked={} failures={}",
        deployment.environment, checked, failures
    );
    if failures != 0 {
        std::process::exit(2);
    }
}
