# M8 Funding-Aware Robust Anchor Control

Status: implemented in the Rust paper engine and production public-feed paper lab; no live-order authority. The implementation is deliberately fail-closed when funding metadata is missing, stale, special, or contradictory.

M8 extends M7 with a funding-aware carry controller. The immutable research object remains the official close anchor and the falsifiable closed-session mean-reversion hypothesis. Funding is a secondary cash-flow layer, never a replacement for anchor evidence.

## Immutable core

- Official close anchor is fixed per closure episode and carries source/time/currency lineage.
- Binance contract rules, PriceMode, FX/Quanto transform, funding schedule and settlement mark are versioned inputs.
- Strategy receives only the decision-time filtration; no opening label or future event may enter online state.
- Normal orders are maker-only, post-only, and reduce-only semantics are explicit.
- M1-M7 and the shared hypothesis accumulator are unchanged; each public event is sampled once.
- Unknown, stale or contradictory contract/market state fails closed.

## Mutable research layer

Funding forecast, latent fair value, regime model, queue model, robust radius, CVaR budget, candidate quotes, holding horizon and capital allocation are replaceable components. They are calibrated by predictive and execution losses, never by lockbox PnL.

## State decomposition

Let a_e=log(A_e), p_t=log(P_t), and v_t be latent next-open fair value:

p_t-a_e = (v_t-a_e) + (p_t-v_t) = information_revaluation + microstructure_mispricing.

M8 may trade only when the conservative posterior value of the second term remains positive after costs and tail error.

## Funding controller

For signed position q (positive long), exact settlement cash flow is:

CF_funding = -q * Mark_at_settlement * (RegularRate + SpecialRate).

The Binance funding history contract exposes funding time, associated mark price and Regular/Special rate type. FundingInfo supplies adjusted cap, floor and interval where applicable. Missing is not zero.

Actions are Collect, Tolerate, Avoid, Exit, and NoAction. A five-minute blanket close is not used by M8; the action is selected from anchor edge, funding carry, spread, volatility, model uncertainty, liquidity and liquidation buffer.
## Mathematical objective

At each decision time, M8 evaluates a finite candidate-action robust control problem:

max_pi inf_Q E_Q[log(W_T)] - lambda*CVaR_alpha(L) - gamma*Drawdown

The current production-safe implementation is the closed-form one-step solver in `engine/src/m8.rs`: it prices anchor edge, funding carry, fees, spread, volatility, model uncertainty and liquidation buffer, then applies the fail-closed viability shield. This is mathematically equivalent to a conservative upper bound over the pre-registered tail-cost set; a future multi-step MPC/SOCP implementation must prove parity against this baseline before promotion.

If the conservative advantage over NO_ACTION is not larger than solver, simulation, execution and calibration error, M8 abstains.

## World separation

FactualReplay replays real external events; StructuralQueue models queue and counterfactual order effects; GenerativeStress produces only robustness scenarios. None can alter the immutable ledger. Strategy logs decisions separately from market facts and account settlement.

The simulator includes Binance event timestamps and receipt timestamps, depth sequence gaps, queue ahead, partial fills, cancel races, latency bursts, 418/429/503 unknown execution states, funding settlement, special funding, mark/index divergence, margin, liquidation and ADL.

## M8 experiment matrix

- M8-A0: exact funding accounting only.
- M8-A1: remove blanket funding deadline with known regular funding.
- M8-A2: probabilistic funding forecast.
- M8-A3: funding-aware holding/exit MPC.
- M8-A4: special funding and contract-change guard.
- M8-A5: DRO/CVaR/viability shield.
- M8-Full: all components.

Every variant uses identical events, anchors, rules, random tape and hypothesis evidence. M8-Full is compared with M7 on paired episodes and lockbox sessions.
## Metrics and promotion gates

Economic metrics separate anchor PnL, funding PnL, maker rebate, fees, adverse selection, exit cost and failure cost. Scientific metrics separately report closure-episode recovery, information displacement, funding forecast calibration, fill/markout calibration and simulator fidelity. Operational metrics are not strategy PnL.

Promotion requires contract replay, factual/structural/stress separation, no-lookahead audit, full cost accounting, positive conservative economic certificate, drawdown/ruin coverage and unseen production-paper shadow validation. No paper or model can guarantee profit or survival against an unbounded market halt; guarantees are only for the pre-registered threat set and solvency region.

## Method lineage and reuse

M8 is a strict child of M7, not a parallel strategy. The reusable control-plane registry is
`strategy::method_graph`: it stores parent methods, overlays, required features and
immutable contracts, resolves a deterministic lineage, and rejects attempts to override
the immutable core. The event hot path receives the resolved plan once; it does not perform
registry lookups.

The canonical lineage is M1 -> M2 -> M3 -> M4 -> M5 -> M6 -> M7 -> M8. Each ablation
adds one overlay to the same M7 parent and consumes the same event tape, feature cache,
simulator and evidence accumulator. This keeps performance comparisons paired and makes
the changed component auditable.

Research references: Binance TradFi price-index notice; Binance USD-M funding-rate API; He, Manela, Ross and von Wachter, Fundamentals of Perpetual Futures; Huang, Lehalle and Rosenbaum, queue-reactive LOB; Esfahani and Kuhn, Wasserstein DRO; Rockafellar and Uryasev, CVaR optimization.
