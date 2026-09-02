use std::{env, fs::File, io::Read, path::PathBuf, process, str::FromStr};

use sha2::{Digest, Sha256};
use static_anchor_engine::{
    paper::{load_anchors, replay_jsonl},
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
    require_flat_at_end: bool,
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => fail(message),
    };
    let anchors = load_anchors(&args.anchors).unwrap_or_else(|error| {
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
    let summary = replay_jsonl(
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
    )
    .unwrap_or_else(|error| fail(format!("backtest failed: {error}")));
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
        "fee_ppm": args.fee_ppm,
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
    let mut entry_threshold_bps = 100;
    let mut max_position = 1;
    let mut requested_quantity = 1;
    let mut max_mark_index_gap_bps = 50;
    let mut max_anchor_age_ms = 0;
    let mut fee_ppm = 0;
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
            "--fee-ppm" => fee_ppm = parse(&mut args, &flag)?,
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
         --max-mark-index-gap-bps N --max-anchor-age-ms N --fee-ppm N\n\
         --require-flat-at-end"
    );
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    print_usage();
    process::exit(2);
}
