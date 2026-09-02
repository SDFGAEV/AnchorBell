use std::{env, fs::File, io::BufReader, process};

use static_anchor_engine::historical::{
    summarize_agg_trades, summarize_klines, summarize_trades, PublicDataKind,
};

fn main() {
    let mut args = env::args().skip(1);
    let kind = match args.next().as_deref() {
        Some("klines") => PublicDataKind::Klines,
        Some("trades") => PublicDataKind::Trades,
        Some("aggTrades") => PublicDataKind::AggTrades,
        _ => {
            eprintln!("usage: binance_public_data_smoke <klines|trades|aggTrades> <csv-path>");
            process::exit(2);
        }
    };
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("missing CSV path");
            process::exit(2);
        }
    };
    let file = File::open(&path).unwrap_or_else(|error| {
        eprintln!("cannot open {path}: {error}");
        process::exit(1);
    });
    let reader = BufReader::new(file);
    let summary = match kind {
        PublicDataKind::Klines => summarize_klines(reader),
        PublicDataKind::Trades => summarize_trades(reader),
        PublicDataKind::AggTrades => summarize_agg_trades(reader),
    };
    match summary {
        Ok(summary) => println!(
            "kind={kind:?} rows={} first_ms={:?} last_ms={:?}",
            summary.row_count, summary.first_timestamp_ms, summary.last_timestamp_ms
        ),
        Err(error) => {
            eprintln!("parse failed: {error:?}");
            process::exit(1);
        }
    }
}
