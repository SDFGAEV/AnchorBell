use std::{collections::BTreeMap, env, path::PathBuf, process, str::FromStr};

use static_anchor_engine::{
    execution::BinanceEnvironment,
    paper::{
        allocate_positions, load_anchors, load_binance_index_anchor_set, PaperStrategyVariant,
        PositionMode,
    },
    paper_lab::{run, PaperLabConfig, PaperLabSpec},
};

const DEFAULT_SYMBOLS: &str =
    "CXMTUSDT,UNITREEUSDT,CSOPSAMSUNG2LUSDT,CSOPSKHYNIX2LUSDT,GIGADEVUSDT,HK0625USDT,MINIMAXUSDT,ZHIPUUSDT,ZHONGJIUSDT";

#[derive(Debug)]
struct Args {
    environment: BinanceEnvironment,
    anchors: Option<PathBuf>,
    index_anchors: bool,
    symbols: Vec<String>,
    output_root: PathBuf,
    capital_usdt: i64,
    entry_threshold_bps: i64,
    threshold_scale_ppm: i64,
    max_mark_index_gap_bps: i64,
    fee_ppm: i64,
    queue_ahead: i64,
    trade_through: i64,
    market_to_decision_ms: u64,
    decision_to_exchange_ms: u64,
    cancel_to_exchange_ms: u64,
    duration_secs: u64,
}

fn main() {
    let args = parse_args().unwrap_or_else(|error| fail(error));
    if args.anchors.is_some() && args.index_anchors {
        fail("--anchors and --index-anchors are mutually exclusive");
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| fail(format!("cannot create runtime: {error}")));
    runtime.block_on(async move {
        let anchors = if let Some(path) = args.anchors.as_deref() {
            load_anchors(path)
                .unwrap_or_else(|error| fail(format!("cannot load paper-lab anchors: {error}")))
        } else {
            load_binance_index_anchor_set(args.environment, &args.symbols, 8, None)
                .await
                .unwrap_or_else(|error| {
                    fail(format!("cannot load paper-lab index anchors: {error}"))
                })
                .anchors
        };
        let modes = BTreeMap::<String, PositionMode>::new();
        let allocations = allocate_positions(&anchors, args.capital_usdt, &modes, 8)
            .unwrap_or_else(|error| fail(format!("cannot allocate paper-lab capital: {error}")));
        let specs = vec![
            ("F0_m0", PaperStrategyVariant::M0Fixed),
            ("F1_m1", PaperStrategyVariant::M1AdaptiveRisk),
            ("F2_m2", PaperStrategyVariant::M2Microstructure),
            ("F3_m3", PaperStrategyVariant::M3FillAware),
            ("F4_m4", PaperStrategyVariant::M4Statistical),
            ("R4_m4", PaperStrategyVariant::M4Statistical),
            ("R3_m3", PaperStrategyVariant::M3FillAware),
            ("R2_m2", PaperStrategyVariant::M2Microstructure),
            ("R1_m1", PaperStrategyVariant::M1AdaptiveRisk),
            ("R0_m0", PaperStrategyVariant::M0Fixed),
        ]
        .into_iter()
        .map(|(label, variant)| PaperLabSpec {
            label: label.to_owned(),
            variant,
        })
        .collect();
        let config = PaperLabConfig {
            environment: args.environment,
            symbols: args.symbols,
            anchors,
            entry_threshold_bps: args.entry_threshold_bps,
            threshold_scale_ppm: args.threshold_scale_ppm,
            max_position: 10_000_000,
            requested_quantity: 1_000_000,
            max_mark_index_gap_bps: args.max_mark_index_gap_bps,
            max_anchor_age_ms: 0,
            fee_ppm: args.fee_ppm,
            quantity_scale: 8,
            price_scale: 8,
            position_allocations: Some(allocations),
            output_root: args.output_root,
            specs,
            max_subscriptions_per_shard: 64,
            connect_timeout_ms: 5_000,
            read_timeout_ms: 15_000,
            metrics_refresh_ms: 1_000,
            index_anchor_refresh_ms: if args.index_anchors { 60_000 } else { 0 },
            fx_refresh_ms: 1_000,
            fx_max_age_ms: 5_000,
            queue_ahead: args.queue_ahead,
            trade_through: args.trade_through,
            market_to_decision_ms: args.market_to_decision_ms,
            decision_to_exchange_ms: args.decision_to_exchange_ms,
            cancel_to_exchange_ms: args.cancel_to_exchange_ms,
            duration_secs: args.duration_secs,
        };
        let result = run(config)
            .await
            .unwrap_or_else(|error| fail(format!("paper lab failed: {error}")));
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("lab result is serializable")
        );
    });
}
fn parse_args() -> Result<Args, String> {
    let mut environment = BinanceEnvironment::Production;
    let mut anchors = None;
    let mut index_anchors = true;
    let mut symbols = DEFAULT_SYMBOLS
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut output_root = PathBuf::from("target\\paper-lab-20260903");
    let mut capital_usdt = 1_500_i64.checked_mul(100_000_000).unwrap();
    let mut entry_threshold_bps = 5;
    let mut threshold_scale_ppm = 700_000;
    let mut max_mark_index_gap_bps = 50;
    let mut fee_ppm = 200;
    let mut queue_ahead = 0;
    let mut trade_through = 0;
    let mut market_to_decision_ms = 0;
    let mut decision_to_exchange_ms = 0;
    let mut cancel_to_exchange_ms = 0;
    let mut duration_secs = 0;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--anchors" => {
                anchors = Some(PathBuf::from(next(&mut args, &flag)?));
                index_anchors = false;
            }
            "--index-anchors" => index_anchors = true,
            "--environment" => {
                environment = next(&mut args, &flag)?
                    .parse()
                    .map_err(|_| "invalid --environment".to_owned())?;
            }
            "--symbols" => {
                symbols = next(&mut args, &flag)?
                    .split(',')
                    .map(|s| s.trim().to_ascii_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "--output-root" => output_root = PathBuf::from(next(&mut args, &flag)?),
            "--capital-usdt" => capital_usdt = parse_decimal(&next(&mut args, &flag)?, 8)?,
            "--entry-threshold-bps" => entry_threshold_bps = parse(&mut args, &flag)?,
            "--threshold-scale-ppm" => threshold_scale_ppm = parse(&mut args, &flag)?,
            "--max-mark-index-gap-bps" => max_mark_index_gap_bps = parse(&mut args, &flag)?,
            "--fee-ppm" => fee_ppm = parse(&mut args, &flag)?,
            "--queue-ahead" => queue_ahead = parse(&mut args, &flag)?,
            "--trade-through" => trade_through = parse(&mut args, &flag)?,
            "--market-to-decision-ms" => market_to_decision_ms = parse(&mut args, &flag)?,
            "--decision-to-exchange-ms" => decision_to_exchange_ms = parse(&mut args, &flag)?,
            "--cancel-to-exchange-ms" => cancel_to_exchange_ms = parse(&mut args, &flag)?,
            "--duration-secs" => duration_secs = parse(&mut args, &flag)?,
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if symbols.is_empty() {
        return Err("--symbols cannot be empty".to_owned());
    }
    Ok(Args {
        environment,
        anchors,
        index_anchors,
        symbols,
        output_root,
        capital_usdt,
        entry_threshold_bps,
        threshold_scale_ppm,
        max_mark_index_gap_bps,
        fee_ppm,
        queue_ahead,
        trade_through,
        market_to_decision_ms,
        decision_to_exchange_ms,
        cancel_to_exchange_ms,
        duration_secs,
    })
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse<T: FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String>
where
    T::Err: std::fmt::Debug,
{
    next(args, flag)?
        .parse()
        .map_err(|e| format!("invalid {flag}: {e:?}"))
}
fn parse_decimal(value: &str, scale: u32) -> Result<i64, String> {
    let value = value.trim();
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    let fraction_len = fraction.len() as u32;
    if parts.next().is_some()
        || whole.is_empty()
        || fraction.len() > scale as usize
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("expected a positive decimal".to_owned());
    }
    let unit = 10_i128.pow(scale);
    let whole = whole
        .parse::<i128>()
        .map_err(|_| "decimal overflows".to_owned())?;
    let fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i128>()
            .map_err(|_| "decimal overflows".to_owned())?
    };
    let scaled = whole
        .checked_mul(unit)
        .and_then(|v| v.checked_add(fraction_value * 10_i128.pow(scale - fraction_len)))
        .ok_or_else(|| "decimal overflows".to_owned())?;
    i64::try_from(scaled).map_err(|_| "decimal overflows".to_owned())
}

fn print_usage() {
    eprintln!("usage: anchorbell_paper_lab [--index-anchors|--anchors PATH] [--environment production] [--symbols S1,S2] [--output-root PATH] [--capital-usdt N] [--duration-secs N]");
    eprintln!(
        "defaults: shared feed + F0..F4 and R4..R0; queue/latency are explicit realism controls"
    );
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    print_usage();
    process::exit(2);
}
