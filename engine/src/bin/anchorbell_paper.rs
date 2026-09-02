use std::{env, fs, path::PathBuf, process, str::FromStr};

use static_anchor_engine::{
    execution::BinanceEnvironment,
    paper::{load_anchors, load_binance_index_anchor_set, run_live, PaperRunConfig},
};

#[derive(Debug)]
struct Args {
    anchors: Option<PathBuf>,
    index_anchors: bool,
    symbols: Option<Vec<String>>,
    environment: BinanceEnvironment,
    records: Option<PathBuf>,
    market_records: Option<PathBuf>,
    anchor_report: Option<PathBuf>,
    proxy: Option<String>,
    duration_secs: u64,
    price_scale: u32,
    quantity_scale: u32,
    max_subscriptions_per_shard: usize,
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
    entry_threshold_bps: i64,
    max_position: i64,
    requested_quantity: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    fee_ppm: i64,
}

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => fail(message),
    };
    let requested_symbols = args.symbols.clone();
    let mut index_anchor_conversions = None;
    let all_anchors = if args.index_anchors {
        let symbols = requested_symbols
            .as_deref()
            .filter(|symbols| !symbols.is_empty())
            .unwrap_or_else(|| {
                fail("--index-anchors requires an explicit --symbols list");
            });
        let anchor_set = load_binance_index_anchor_set(
            args.environment,
            symbols,
            args.price_scale,
            args.proxy.as_deref(),
        )
        .await
        .unwrap_or_else(|error| fail(format!("cannot load Binance index anchors: {error}")));
        index_anchor_conversions = Some(anchor_set.conversions);
        anchor_set.anchors
    } else {
        let path = args.anchors.as_deref().unwrap_or_else(|| {
            fail("missing --anchors; use --index-anchors for the live Binance index source");
        });
        load_anchors(path).unwrap_or_else(|error| {
            fail(format!("cannot load anchors: {error}"));
        })
    };
    let symbols = requested_symbols.unwrap_or_else(|| all_anchors.keys().cloned().collect());
    let mut anchors = std::collections::BTreeMap::new();
    for symbol in &symbols {
        let Some(anchor) = all_anchors.get(symbol) else {
            fail(format!("no anchor row for subscribed symbol {symbol}"));
        };
        anchors.insert(symbol.clone(), *anchor);
    }
    let anchor_report = anchors.clone();
    let snapshot_report = serde_json::json!({
        "environment": args.environment.as_str(),
        "anchor_source": if args.index_anchors {
            "binance_premium_index"
        } else {
            "csv"
        },
        "anchors": anchor_report,
        "index_anchor_conversions": index_anchor_conversions.clone(),
        "symbols": symbols,
        "price_scale": args.price_scale,
    });
    if let Some(path) = args.anchor_report.as_deref() {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).unwrap_or_else(|error| {
                fail(format!("cannot create anchor report directory: {error}"))
            });
        }
        let bytes = serde_json::to_vec_pretty(&snapshot_report)
            .expect("anchor snapshot report is serializable");
        fs::write(path, bytes)
            .unwrap_or_else(|error| fail(format!("cannot write anchor report: {error}")));
    }
    let result = run_live(
        PaperRunConfig {
            environment: args.environment,
            symbols: symbols.clone(),
            price_scale: args.price_scale,
            quantity_scale: args.quantity_scale,
            max_subscriptions_per_shard: args.max_subscriptions_per_shard,
            connect_timeout_ms: args.connect_timeout_ms,
            read_timeout_ms: args.read_timeout_ms,
            duration_secs: args.duration_secs,
            http_proxy: args.proxy,
            market_output_path: args.market_records,
        },
        anchors,
        args.entry_threshold_bps,
        args.max_position,
        args.requested_quantity,
        args.max_mark_index_gap_bps,
        args.max_anchor_age_ms,
        args.fee_ppm,
        args.records,
    )
    .await
    .unwrap_or_else(|error| fail(format!("paper run failed: {error}")));
    let report = serde_json::json!({
        "environment": args.environment.as_str(),
        "anchor_source": if args.index_anchors {
            "binance_premium_index"
        } else {
            "csv"
        },
        "anchors": anchor_report,
        "index_anchor_conversions": index_anchor_conversions,
        "symbols": symbols,
        "duration_secs": args.duration_secs,
        "summary": result.summary,
        "records_written": result.records_written,
        "records_dropped": result.records_dropped,
        "market_records_written": result.market_records_written,
        "market_records_dropped": result.market_records_dropped,
        "stopped_by_duration": result.stopped_by_duration,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report is serializable")
    );
}

fn parse_args() -> Result<Args, String> {
    let mut anchors = None;
    let mut index_anchors = false;
    let mut symbols = None;
    let mut environment = BinanceEnvironment::Testnet;
    let mut records = None;
    let mut market_records = None;
    let mut anchor_report = None;
    let mut proxy = None;
    let mut duration_secs = 60;
    let mut price_scale = 8;
    let mut quantity_scale = 8;
    let mut max_subscriptions_per_shard = 64;
    let mut connect_timeout_ms = 5_000;
    let mut read_timeout_ms = 15_000;
    let mut entry_threshold_bps = 100;
    let mut max_position = 1;
    let mut requested_quantity = 1;
    let mut max_mark_index_gap_bps = 50;
    let mut max_anchor_age_ms = 0;
    let mut fee_ppm = 0;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            "--anchors" => anchors = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--index-anchors" => index_anchors = true,
            "--symbols" => {
                let value = next(&mut args, &flag)?;
                let values = value
                    .split(',')
                    .map(|value| value.trim().to_ascii_uppercase())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    return Err("--symbols cannot be empty".into());
                }
                symbols = Some(values);
            }
            "--environment" => environment = parse(&mut args, &flag)?,
            "--records" => records = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--market-records" => market_records = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--anchor-report" => anchor_report = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--proxy" => proxy = Some(next(&mut args, &flag)?),
            "--duration-secs" => duration_secs = parse(&mut args, &flag)?,
            "--price-scale" => price_scale = parse(&mut args, &flag)?,
            "--quantity-scale" => quantity_scale = parse(&mut args, &flag)?,
            "--max-subscriptions-per-shard" => {
                max_subscriptions_per_shard = parse(&mut args, &flag)?
            }
            "--connect-timeout-ms" => connect_timeout_ms = parse(&mut args, &flag)?,
            "--read-timeout-ms" => read_timeout_ms = parse(&mut args, &flag)?,
            "--entry-threshold-bps" => entry_threshold_bps = parse(&mut args, &flag)?,
            "--max-position" => max_position = parse(&mut args, &flag)?,
            "--quantity" => requested_quantity = parse(&mut args, &flag)?,
            "--max-mark-index-gap-bps" => max_mark_index_gap_bps = parse(&mut args, &flag)?,
            "--max-anchor-age-ms" => max_anchor_age_ms = parse(&mut args, &flag)?,
            "--fee-ppm" => fee_ppm = parse(&mut args, &flag)?,
            unknown => return Err(format!("unknown option {unknown}; use --help")),
        }
    }
    Ok(Args {
        anchors,
        index_anchors,
        symbols,
        environment,
        records,
        market_records,
        anchor_report,
        proxy,
        duration_secs,
        price_scale,
        quantity_scale,
        max_subscriptions_per_shard,
        connect_timeout_ms,
        read_timeout_ms,
        entry_threshold_bps,
        max_position,
        requested_quantity,
        max_mark_index_gap_bps,
        max_anchor_age_ms,
        fee_ppm,
    })
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse<T>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String>
where
    T: FromStr,
    T::Err: std::fmt::Debug,
{
    next(args, flag)?
        .parse()
        .map_err(|error| format!("invalid {flag}: {error:?}"))
}

fn print_usage() {
    eprintln!(
        "usage: anchorbell_paper (--anchors ANCHORS.csv | --index-anchors --symbols SYMBOLS) [options]\n\
         options: --symbols BTCUSDT,ETHUSDT --environment testnet|production\n\
         --records PATH --market-records PATH --anchor-report PATH --proxy URL --duration-secs N\n\
         --price-scale N --quantity-scale N --max-position N --quantity N\n\
         --entry-threshold-bps N --max-mark-index-gap-bps N\n\
         --max-anchor-age-ms N --fee-ppm N"
    );
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    print_usage();
    process::exit(2);
}
