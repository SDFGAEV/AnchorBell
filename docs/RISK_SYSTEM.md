# AnchorBell Risk System

## Purpose

The risk plane is a typed, deterministic decision layer between strategy
signals and execution intents. It does not read sockets, mutate positions,
write files, or submit orders. It only interprets a decision snapshot and
returns a bounded risk decision.

The system separates four concerns:

1. **Hard safety**: data freshness, anchor validity, session state, contract
   metadata, reconciliation and emergency state.
2. **Strategy admission**: M1-M7 signal and evidence thresholds.
3. **Incremental overlays**: funding, tail, inventory and capital constraints.
4. **Execution validation**: maker-only, exchange filters and lifecycle state.

A failure in an overlay must not be confused with a failure in the hard-safety
plane. Every rejection has a stable reason code and remains observable.
## Immutable core

- The official external close remains the fixed anchor for a closure episode.
- Binance event, mark, index, funding settlement and order lifecycle semantics
  are preserved across simulation, replay, backtest and live adapters.
- Unknown, stale or contradictory state cannot create new risk.
- Normal opening and reduction intents remain maker-only and post-only.
- Risk code cannot call exchange clients or persistence.

## Mutable policy

The following are versioned validation policy inputs, not hidden constants:

- signal threshold and confidence floor;
- tail and inventory penalties;
- funding overlay tolerance;
- capital allocation and concentration limits;
- quote refresh and queue assumptions.

Changing a mutable policy creates a new method/run version and is
compared on the same event tape.
## Funding overlay contract

M8 is a strict child of M7. Funding is an incremental cash-flow overlay, not a
replacement for the inherited M7 signal.

- 'Collect' and 'Tolerate' permit the inherited strategy to decide.
- 'NoAction' permits the inherited strategy to decide.
- 'Avoid' with zero or favorable carry permits the inherited strategy to decide;
  the base signal still has to pass its own threshold.
- 'Avoid' with adverse carry may veto new risk.
- 'Exit' is reduce-only for an existing position.
- Missing, stale or special funding metadata remains fail-closed.

This rule prevents a weekend zero-funding window from becoming a blanket
no-trade rule while preserving protection against adverse carry.
## Decision pipeline

DecisionSnapshot -> HardRiskDecision -> StrategyDecision -> OverlayDecision ->
ExecutionValidation -> ActionCandidate

Each layer is pure and testable. The runtime composes the layers once per
snapshot; the event hot path does not perform registry or filesystem lookups.

The observability plane records:

- the snapshot and version identifiers;
- each layer's state and reason code;
- the final action;
- the counterfactual action before each overlay;
- latency and data age;
- whether the rejection was hard safety, strategy admission, overlay, or
  execution validation.
## Over-gating diagnostics

A high rejection count is not sufficient evidence of a risk problem. Reports
must partition it into:

- hard_data_*: stale/missing/contradictory market state;
- hard_anchor_*: invalid or expired external anchor;
- hard_session_*: equity session or flatten deadline;
- strategy_threshold_*: signal/evidence threshold;
- overlay_funding_*: funding-specific veto;
- execution_*: maker/filter/lifecycle validation.

The simulation engine must report counts by reason and preserve the last reason per
symbol. A strategy is not promoted merely because it increases order count;
orders must survive conservative fill, markout, latency and tail tests.
## Acceptance tests

The minimum regression suite includes:

- zero funding: M8 delegates to M7 and does not synthesize a funding exit;
- favorable funding: M8 may collect without weakening hard safety;
- adverse funding: M8 can veto a new position and reduce an existing one;
- unknown/special funding: fail-closed behavior is explicit;
- stale market/anchor: no new risk in every method;
- method-label metamorphism: changing labels does not change the world ledger;
- recorder failure: strategy state remains unchanged and failure is surfaced;
- atomic metric snapshots: transient Windows file locks are retried;
- replay and simulation produce the same typed decision for the same snapshot.

## Live anchor bootstrap contract

A live simulation lab must never use a local anchor file as a fallback. The
`--index-anchors` path is mandatory: startup obtains the latest Binance
index/FX-derived snapshot before admitting market events, and transient REST
failures retry without changing the run inputs. If the authoritative
source cannot be obtained, the process remains blocked and reports the failure;
it must not silently continue with an old cache.

Once bootstrapped, the anchor remains immutable through the current closure
episode. Periodic refresh may install only a newer authoritative snapshot when
the exchange calendar permits a new completed close. This preserves the
closed-session invariant without sacrificing live freshness.

Offline CSV anchors remain available only to explicit replay/backtest tools;
they are prohibited for production simulation-batch execution.
