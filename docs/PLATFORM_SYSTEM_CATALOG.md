# AnchorBell Industrial Platform System Catalog

Status: architecture baseline, 2026-09-05

This document is the operational architecture authority for an industrial quantitative service. AnchorBell is not organized as a simulation, a collection of runs, or a collection of strategy scripts. Simulation and statistical validation are engineering environments, but they never own live exchange authority.

## 1. Current system inventory

The Rust workspace is one deployable engine crate with explicit module boundaries:

| Plane | System | Current implementation | Owns | Authority |
| --- | --- | --- | --- | --- |
| Control | Topology and lifecycle | runtime, platform | dependency graph, lifecycle, health, restart policy | internal |
| Control | Operator console | anchorbell_dashboard | session configuration and read-only control | operator |
| Market data | Binance adapter | market/binance, connection, live, subscription | WebSocket/REST transport and normalized events | Binance |
| Market data | Capability/metadata | market/capability, market/metadata | filters, symbol capabilities, freshness | Binance |
| Market data | Reference/FX | market/fx, historical | external close/reference and CNY/HKD conversion | external reference |
| Market data | Anchor service | strategy/reference_model, anchor_policy, simulation anchor loader | authoritative close anchor, provenance, validity | derived from authoritative inputs |
| Decision | Strategy policy | strategy/anchor_maker, price_engine, quote_engine, signal_policy | quote and entry/exit intent | internal policy |
| Decision | Session/calendar | strategy/calendar, us_calendar, session | close/open windows and transitions | exchange/reference metadata |
| Decision | Inventory/capital | strategy/inventory, capital, universe, instrument_profile | allocation, eligibility, inventory skew | governed policy |
| Decision | Portfolio/risk | risk, strategy/risk_contracts, execution/risk, limits, safety | admission, limits, flatten, fail-closed decisions | safety kernel plus governed overlays |
| Decision | Funding controller | m8, execution/funding_risk | funding schedule/rate input and exposure overlay | Binance plus governed policy |
| Execution | Order gateway | execution/binance, rest, order_api, signing, binance_wire | typed signed requests and post-only transport | Binance |
| Execution | Lifecycle/reconciliation | execution/lifecycle, order_manager, order_ws, user_data, reconciliation | order/position/account truth | Binance |
| Execution | Recovery/checkpoint | execution/recovery, supervisor, session_checkpoint, deployment, environment | reconnect, restart, environment separation | internal plus Binance truth |
| Simulation | Simulation runtime | simulation, simulation_batch, anchorbell_simulation, anchorbell_simulation_batch | deterministic simulated execution | derived |
| Simulation | Replay/backtest | replay, backtest, backtest_realism, backtest_report, anchorbell_backtest | event-time replay and fill/latency/cost models | derived |
| Observability | Event recording | market/recorder, runtime/io | asynchronous event persistence | internal |
| Observability | Telemetry/audit | observability, event, core/events | structured events, metrics, audit chain | internal |
| Verification | Smoke/stress | *_smoke binaries, anchorbell_extreme_stress | contract, performance, recovery verification | derived evidence |
| Legacy boundary | Python prototype (archival) | src/core, src/data, src/exchange, src/execution, src/market, src/models, src/validation | historical reference only; excluded from the Rust build and live authority | none |

This is a mapping, not a second implementation. Each subcomponent has one owning system and one authority.

## 2. Plane dependencies

The fixed dependency direction is control -> market-data -> decision -> execution -> observability. Simulation consumes the same typed market/decision contracts in an isolated environment. It cannot use production credentials or mutate exchange state. Observability consumes copied events asynchronously and cannot block or modify a decision.

The control registry is a topology source, not a god object. Systems own their state and behavior; the registry records identity, dependencies, health, capabilities, and recovery intent.

## 3. Immutable core

- Binance wire schemas, signatures, timestamp/recv-window rules, exchange filters, and integer tick/accounting semantics.
- Environment separation and credential boundaries.
- Maker-only order construction and the no-taker invariant.
- Order identity, idempotency, reconciliation, and unknown-state fail-closed behavior.
- Anchor provenance, source identity, observation time, validity, and the rule that missing/invalid live authority stops new risk.
- Event ordering, sequence checks, bounded queues, and audit integrity.
- Effective flatten deadline, emergency exposure accounting, and halt/drain capability.

Automated adaptation may change policy parameters, but may never weaken these contracts.

## 4. Governed mutable layer

- strategy lineage M1 through M8 and future descendants;
- quote width, inventory skew, participation, and cancel/replace policy;
- funding-aware overlay thresholds;
- portfolio allocation, symbol selection, and capital budgets;
- queue, fill, fee, latency, and slippage models used only by simulation;
- validated calendar policy data and observability projections.

Every mutable release has a policy ID, parent policy ID, parameter/data digest, effective interval, approval state, and rollback target. M8 is a lineage label, not a separate architecture.

## 5. Automated discovery and diagnosis

At startup the runtime builds the typed system registry, validates dependencies, and rejects cycles. Each system emits a health snapshot containing lifecycle state, observation time, stale-data flag, invariant-failure count, queue depth, error rate, capability/readiness flags, and diagnostic reason codes.

The supervisor automatically detects missing, stale, contradictory, or out-of-order inputs; disables only the affected capability; prevents new risk when a required dependency is not tradable; restarts restartable adapters with bounded backoff; drains and reconciles before resuming; records transitions in audit/metrics; and escalates to halt when authoritative truth cannot be restored. The live control plane emits one structured transition event for discovery, readiness, staleness, degradation, and recovery, so runtime diagnosis is machine-readable and deduplicated.

No operator-maintained checklist is required for ordinary detection. Human approval remains required for production authorization, immutable-core changes, and policy promotion.

## 6. Automated improvement loop

observe -> diagnose -> propose -> isolated replay/simulation -> shadow -> gated promotion -> monitor -> rollback

Promotion gates include parity, worst-case drawdown and liquidation distance, maker-only compliance, stale-data behavior, funding settlement correctness, exchange-filter validity, and resource budgets. A proposal that improves returns while violating an invariant is rejected.

Simulation, backtest, replay, shadow, testnet, and production are separate typed environments. Evidence from one environment never implicitly authorizes the next.

## 7. Production vocabulary migration

Simulation is retained only where it describes an execution environment or historical artifact. It is not the platform identity.

| Legacy term | Operational term | Rule |
| --- | --- | --- |
| simulation batch | batch execution environment | isolated multi-policy execution |
| run version | run ID / policy lineage ID | reproducible and traceable |
| evidence record | reference-validation record | analytics output, never order authority |
| validation methods | analytics/validation | consumes events, cannot create orders |
| M1-M8 run | policy lineage | each child declares its parent |
| simulation result | simulation result | no implication of live performance |

No legacy module or vocabulary compatibility layer remains. Old entrypoints are intentionally removed; all integrations must target the current platform contracts and operational vocabulary.

## 8. Expansion assumptions

The platform is designed for multiple venues/products, thousands of symbols in market shards, multiple strategy lineages sharing one reference service, per-account risk budgets, tenant isolation, durable schema-evolving event storage, active/standby supervisors, process fencing, and automated capacity planning from queue/latency/error telemetry.

Scaling adds descriptors and adapters; it does not duplicate safety logic. A new venue implements the same market, capability, execution, lifecycle, and reconciliation contracts before policy eligibility.

## 9. Implementation status and gates

Implemented in this baseline:

- typed SystemRegistry with canonical catalog, dependency validation, immutable-core protection, and fail-closed health snapshots;
- runtime ownership of the registry, topology readiness, and health reporting;
- deduplicated health transition events from discovery through recovery in the live runner;
- explicit inventory and vocabulary boundary for all current systems.

Next gates:

1. Have each supervisor publish health snapshots to the registry.
2. Move anchor refresh scheduling behind one authoritative reference-data service.
3. Add machine-readable capability manifests and automatic contract checks.
4. Expose topology, health, and recovery events in dashboard and audit output.
5. Introduce policy lineage IDs while retaining explicit M1-M8 parentage.
6. Split analytics/validation writers from execution-facing runtime outputs.
7. Add CI checks that reject exchange I/O from decision modules and reject legacy validation vocabulary in production code.

Completion is measured by runtime discovery and recovery evidence, not by the number of renamed files.
