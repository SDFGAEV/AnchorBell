use std::{env, fs::File, io::Read, path::PathBuf, process, str::FromStr};

use sha2::{Digest, Sha256};
use static_anchor_engine::{
    backtest::realism::{LatencyModel, QueueModel, RealisticFillModel},
    runtime::health_reporter::{timestamp_ms, RuntimeHealthReporter},
    simulation::{load_anchor_file, replay_jsonl_with_realism},
    strategy::universe::instrument_for,
};

#[derive(Debug)]
struct Args {
    input: PathBuf,
    anchors: PathBuf,
    records: Option<PathBuf>,
    price_scale: u32,
    quantity_scale: u32,
    entry_threshold_bps: i64,
    max_position: i64,
    requested_quantity: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    fee_ppm: i64,
    queue_ahead: i64,
    trade_through: i64,
    market_to_decision_ms: u64,
    decision_to_exchange_ms: u64,
    require_flat_at_end: bool,
}

#[tokio::main]
async fn main() {
    let mut health = RuntimeHealthReporter::new("target/backtest-runtime-audit.jsonl");
    health
        .start(
            &[
                "control.registry",
                "simulation.replay",
                "simulation.backtest",
                "observability.telemetry",
            ],
            timestamp_ms(),
        )
        .await
        .unwrap_or_else(|error| fail(format!("backtest health bootstrap failed: {error}")));
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => fail(message),
    };
    let anchors = load_anchor_file(&args.anchors).unwrap_or_else(|error| {
        fail(format!("cannot load anchors: {error}"));
    });
    if let Some(symbol) = anchors
        .keys()
        .find(|symbol| instrument_for(symbol).is_none())
    {
        fail(format!(
            "anchor symbol is outside the selected execution universe: {symbol}"
        ));
    }
    let input_sha256 = sha256_file(&args.input).unwrap_or_else(|error| {
        fail(format!("cannot hash input: {error}"));
    });
    let summary = match replay_jsonl_with_realism(
        &args.input,
        args.records.as_deref(),
        anchors.clone(),
        args.price_scale,
        args.quantity_scale,
        args.entry_threshold_bps,
        args.max_position,
        args.requested_quantity,
        args.max_mark_index_gap_bps,
        args.max_anchor_age_ms,
        args.fee_ppm,
        RealisticFillModel {
            queue: QueueModel {
                visible_ahead: args.queue_ahead,
                trade_through: args.trade_through,
            },
            latency: LatencyModel {
                market_to_decision_ms: args.market_to_decision_ms,
                decision_to_exchange_ms: args.decision_to_exchange_ms,
                cancel_to_exchange_ms: 0,
            },
        },
    ) {
        Ok(summary) => summary,
        Err(error) => {
            let reason = error.to_string();
            let _ = health
                .halted("simulation.backtest", timestamp_ms(), &reason)
                .await;
            fail(format!("backtest failed: {error}"));
        }
    };
    health
        .ready("simulation.backtest", timestamp_ms())
        .await
        .unwrap_or_else(|error| fail(format!("backtest health completion failed: {error}")));
    if args.require_flat_at_end && !summary.flat_at_end {
        fail(format!(
            "backtest ended with unmanaged exposure: position={}, working_orders={}",
            summary.current_absolute_position, summary.working_orders
        ));
    }
    let report = serde_json::json!({
        "input": args.input,
        "input_sha256": input_sha256,
        "anchors": anchors.len(),
        "price_scale": args.price_scale,
        "quantity_scale": args.quantity_scale,
        "entry_threshold_bps": args.entry_threshold_bps,
        "max_position": args.max_position,
        "requested_quantity": args.requested_quantity,
        "max_mark_index_gap_bps": args.max_mark_index_gap_bps,
        "max_anchor_age_ms": args.max_anchor_age_ms,
        "maker_fee_ppm": args.fee_ppm,
        "queue_ahead": args.queue_ahead,
        "trade_through": args.trade_through,
        "market_to_decision_ms": args.market_to_decision_ms,
        "decision_to_exchange_ms": args.decision_to_exchange_ms,
        "require_flat_at_end": args.require_flat_at_end,
        "summary": summary,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report is serializable")
    );
}

fn parse_args() -> Result<Args, String> {
    let mut input = None;
    let mut anchors = None;
    let mut records = None;
    let mut price_scale = 8;
    let mut quantity_scale = 8;
    // AdaptiveThreshold owns the live entry requirement; this is only the hard floor.
    let mut entry_threshold_bps = 0;
    let mut max_position = 1;
    let mut requested_quantity = 1;
    let mut max_mark_index_gap_bps = 50;
    let mut max_anchor_age_ms = 0;
    // Binance USDⓈ-M base maker fee: 0.02% = 200 ppm. Override explicitly when needed.
    let mut fee_ppm = 200;
    let mut queue_ahead = 0;
    let mut trade_through = 0;
    let mut market_to_decision_ms = 0;
    let mut decision_to_exchange_ms = 0;
    let mut require_flat_at_end = false;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            "--input" => input = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--anchors" => anchors = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--records" => records = Some(PathBuf::from(next(&mut args, &flag)?)),
            "--price-scale" => price_scale = parse(&mut args, &flag)?,
            "--quantity-scale" => quantity_scale = parse(&mut args, &flag)?,
            "--entry-threshold-bps" => entry_threshold_bps = parse(&mut args, &flag)?,
            "--max-position" => max_position = parse(&mut args, &flag)?,
            "--quantity" => requested_quantity = parse(&mut args, &flag)?,
            "--max-mark-index-gap-bps" => max_mark_index_gap_bps = parse(&mut args, &flag)?,
            "--max-anchor-age-ms" => max_anchor_age_ms = parse(&mut args, &flag)?,
            "--maker-fee-ppm" | "--fee-ppm" => fee_ppm = parse(&mut args, &flag)?,
            "--queue-ahead" => queue_ahead = parse(&mut args, &flag)?,
            "--trade-through" => trade_through = parse(&mut args, &flag)?,
            "--market-to-decision-ms" => market_to_decision_ms = parse(&mut args, &flag)?,
            "--decision-to-exchange-ms" => decision_to_exchange_ms = parse(&mut args, &flag)?,
            "--require-flat-at-end" => require_flat_at_end = true,
            unknown => return Err(format!("unknown option {unknown}; use --help")),
        }
    }
    Ok(Args {
        input: input.ok_or("missing --input")?,
        anchors: anchors.ok_or("missing --anchors")?,
        records,
        price_scale,
        quantity_scale,
        entry_threshold_bps,
        max_position,
        requested_quantity,
        max_mark_index_gap_bps,
        max_anchor_age_ms,
        fee_ppm,
        queue_ahead,
        trade_through,
        market_to_decision_ms,
        decision_to_exchange_ms,
        require_flat_at_end,
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

fn sha256_file(path: &PathBuf) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn print_usage() {
    eprintln!(
        "usage: anchorbell_backtest --input EVENTS.jsonl --anchors ANCHORS.csv [options]\n\
         options: --records PATH --price-scale N --quantity-scale N\n\
         --entry-threshold-bps N --max-position N --quantity N\n\
         --max-mark-index-gap-bps N --max-anchor-age-ms N --maker-fee-ppm N\n\
         --queue-ahead N --trade-through N --market-to-decision-ms N\n\
         --decision-to-exchange-ms N --require-flat-at-end"
    );
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    print_usage();
    process::exit(2);
}
