# AnchorBell performance verification

Run from the repository root on the target host with the portable Rust toolchain.

## Parser throughput

Set ANCHORBELL_BENCH_ITERATIONS=1000000 and run:

    cargo run -p static-anchor-engine --bin market_throughput_smoke --release --locked

Record iterations, parsed count, elapsed milliseconds, and events per second.

## Replay realism

Run the same JSONL twice with identical model parameters and compare the
canonical report and records:

    cargo run -p static-anchor-engine --bin anchorbell_backtest --locked --
      --input runs\\market.jsonl --anchors data\\anchors.csv
      --queue-ahead 100 --trade-through 20
      --market-to-decision-ms 2 --decision-to-exchange-ms 3
      --maker-fee-ppm 200 --require-flat-at-end

A run must declare the dataset digest, model parameters, fees, funding model,
and whether it finished flat. No result is accepted from a zero-latency,
zero-queue assumption alone.

## Reliability

- [ ] repeat parser smoke with a fixed iteration count
- [ ] run reconnect and read-silence tests
- [ ] inspect recorder dropped counters
- [ ] inspect health/readiness/liveness probes
- [ ] compare replay output hashes across repeated runs
