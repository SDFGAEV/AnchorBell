//! Risk-plane contracts and funding overlay decisions.
//!
//! This module owns risk interpretation only. It does not read exchange
//! state, create orders, write logs, or calculate strategy signals.

use serde::Serialize;

use crate::m8::{FundingAction, FundingRateStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FundingRiskState {
    Neutral,
    Favorable,
    Adverse,
    ReduceOnly,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FundingOverlayDecision {
    pub state: FundingRiskState,
    /// Whether the base strategy may independently decide on a new entry.
    pub allow_base_strategy: bool,
    /// Whether an existing position must be handled reduce-only.
    pub reduce_only: bool,
    pub reason: &'static str,
}
/// Interpret funding as an incremental risk overlay.
///
/// Zero or favorable funding is neutral to the base M1-M7 signal. It must not
/// turn a weekend/no-carry window into a blanket entry block. Adverse funding
/// can veto new risk when the M8 solver has already classified the opportunity
/// as Avoid. Unknown and special metadata remain fail-closed.
pub fn evaluate_funding_overlay(
    action: FundingAction,
    status: FundingRateStatus,
    carry_bps: i64,
    position: i64,
) -> FundingOverlayDecision {
    if matches!(
        status,
        FundingRateStatus::Unknown | FundingRateStatus::Missing | FundingRateStatus::Stale
    ) {
        return FundingOverlayDecision {
            state: FundingRiskState::Halt,
            allow_base_strategy: false,
            reduce_only: position != 0,
            reason: "funding_metadata_missing",
        };
    }
    if status == FundingRateStatus::Special {
        return FundingOverlayDecision {
            state: if position != 0 {
                FundingRiskState::ReduceOnly
            } else {
                FundingRiskState::Halt
            },
            allow_base_strategy: false,
            reduce_only: position != 0,
            reason: if position != 0 {
                "special_funding_reduce_only"
            } else {
                "special_funding_requires_explicit_policy"
            },
        };
    }
    match action {
        FundingAction::Exit => FundingOverlayDecision {
            state: FundingRiskState::ReduceOnly,
            allow_base_strategy: false,
            reduce_only: position != 0,
            reason: "funding_exit",
        },
        FundingAction::Avoid if carry_bps < 0 => FundingOverlayDecision {
            state: FundingRiskState::Adverse,
            allow_base_strategy: false,
            reduce_only: position != 0,
            reason: "adverse_funding_not_covered",
        },
        FundingAction::Collect => FundingOverlayDecision {
            state: FundingRiskState::Favorable,
            allow_base_strategy: true,
            reduce_only: false,
            reason: "favorable_funding_overlay",
        },
        FundingAction::Tolerate | FundingAction::NoAction | FundingAction::Avoid => {
            FundingOverlayDecision {
                state: if carry_bps > 0 {
                    FundingRiskState::Favorable
                } else {
                    FundingRiskState::Neutral
                },
                allow_base_strategy: true,
                reduce_only: false,
                reason: "funding_neutral_base_strategy_decides",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_funding_does_not_blanket_block_base_strategy() {
        let decision =
            evaluate_funding_overlay(FundingAction::Avoid, FundingRateStatus::Observed, 0, 0);
        assert!(decision.allow_base_strategy);
        assert_eq!(decision.state, FundingRiskState::Neutral);
    }

    #[test]
    fn adverse_funding_can_veto_new_risk() {
        let decision =
            evaluate_funding_overlay(FundingAction::Avoid, FundingRateStatus::Observed, -2, 0);
        assert!(!decision.allow_base_strategy);
        assert_eq!(decision.state, FundingRiskState::Adverse);
    }
    #[test]
    fn exit_is_reduce_only() {
        let decision =
            evaluate_funding_overlay(FundingAction::Exit, FundingRateStatus::Observed, -4, 10);
        assert!(decision.reduce_only);
        assert!(!decision.allow_base_strategy);
    }

    #[test]
    fn unknown_funding_fails_closed() {
        let decision =
            evaluate_funding_overlay(FundingAction::NoAction, FundingRateStatus::Missing, 0, 0);
        assert!(!decision.allow_base_strategy);
        assert_eq!(decision.state, FundingRiskState::Halt);
    }
}
