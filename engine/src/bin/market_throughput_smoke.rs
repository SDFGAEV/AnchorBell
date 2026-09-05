use std::time::Instant;

use anchorbell_engine::market::binance::parse_market_message;

fn main() {
    let iterations = std::env::var("ANCHORBELL_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(100_000);
    let payload = br#"{"e":"markPriceUpdate","E":1000,"s":"ABCUSDT","p":"12.3456","i":"12.3000","T":2000,"r":"-0.00010000"}"#;

    let started = Instant::now();
    let mut parsed = 0_u64;
    for _ in 0..iterations {
        if parse_market_message(payload, 4, 2).is_ok() {
            parsed = parsed.saturating_add(1);
        }
    }

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f64();
    let events_per_second = if seconds > 0.0 {
        parsed as f64 / seconds
    } else {
        f64::INFINITY
    };
    println!(
        "iterations={} parsed={} elapsed_ms={} events_per_second={:.0}",
        iterations,
        parsed,
        elapsed.as_millis(),
        events_per_second
    );
    if parsed != iterations {
        std::process::exit(2);
    }
}
