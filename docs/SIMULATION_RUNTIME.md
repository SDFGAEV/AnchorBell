# AnchorBell Simulation Runtime

This is the canonical operating guide for simulation, replay, and validation.
The runtime consumes the same typed market, decision, risk, and lifecycle
contracts as live execution but cannot submit exchange orders or read
production credentials.

## Operating boundaries

- Simulation is an execution environment, not an order authority.
- Replay is deterministic and rejects out-of-order input.
- Fill, queue, latency, fee, funding, and flatten assumptions are explicit.
- Every run emits a run ID, policy lineage, input digest, configuration digest,
  code revision, environment, and completion state.
- An open position at the end is an incomplete run, not a realized result.
- Analytics and validation consume copied evidence asynchronously.

## Canonical flow

market capture -> normalized event log -> deterministic replay ->
strategy/risk decision -> simulated lifecycle -> accounting ledger ->
metrics projection -> validation report

The live, simulation, and replay paths share contracts at the decision boundary.
They do not share credentials, exchange effects, or mutable state.
## Commands

Run from the repository root with the portable Rust toolchain:

~~~powershell
cargo run -p anchorbell-engine --bin anchorbell_backtest --locked -- --input runs\market.jsonl --anchors data\anchors.csv --queue-ahead 100 --trade-through 20 --market-to-decision-ms 2 --decision-to-exchange-ms 3 --maker-fee-ppm 200 --require-flat-at-end
~~~

For continuous public-market simulation, use the runner with live
authoritative index anchors and a bounded output directory. All automation and
downstream integrations use the simulation facade and the terminology in this
document.

## Evidence requirements

A result is operationally useful only when it includes event counts, fills,
partial fills, queue assumptions, latency, fees, funding, mark-to-market,
realized PnL, residual exposure, time-to-flat, stale-data transitions,
reconnections, and the full decision/gate reason distribution. A positive
short window is not a promotion signal.
## Promotion boundary

Simulation can produce validation evidence and a policy lineage candidate.
It cannot authorize Testnet or Production. Promotion requires independent
checks for maker-only behavior, stale-data handling, exchange filters,
funding settlement, flatten feasibility, resource budgets, deterministic
replay, and conservative tail outcomes.

The next implementation work is to split the current large simulation
implementation into execution model, allocation, ledger, metrics, and report
systems. Each system will register in the platform topology and publish
health/recovery events without moving any safety authority into analytics.
