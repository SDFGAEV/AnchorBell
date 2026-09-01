//! Execution risk gate for equity-open and funding-settlement flattening.
//!
//! This adapter turns the pure strategy flatten plan into execution decisions.
//! It does not submit orders and cannot bypass maker-only validation.

use crate::strategy::{DualFlattenPlan, FlattenPhase, FlattenReason, Quantity, StaticAnchor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FundingRiskAction {
    NoAction,
    AllowEntry { quantity: Quantity },
    StopNewRisk { reason: FlattenReason },
    Flatten { reason: FlattenReason },
    ResidualExposure { reason: FlattenReason },
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FundingRiskInput {
    pub now_ms: u64,
    pub requested_quantity: Quantity,
    pub position: Quantity,
    pub market_event_at_ms: u64,
    pub max_market_age_ms: u64,
    pub anchor: Option<StaticAnchor>,
    pub flatten_plan: DualFlattenPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FundingAwareRiskGate {
    pub max_position: Quantity,
}

impl FundingAwareRiskGate {
    pub fn new(max_position: Quantity) -> Option<Self> {
        (max_position.0 > 0).then_some(Self { max_position })
    }

    pub fn evaluate(&self, input: FundingRiskInput) -> FundingRiskAction {
        let absolute_position = input.position.0.checked_abs().unwrap_or(i64::MAX);
        if absolute_position > self.max_position.0 {
            return FundingRiskAction::Halt;
        }

        let reason = input.flatten_plan.reason_at(input.now_ms);
        match input
            .flatten_plan
            .phase_at(input.now_ms, input.position.0 != 0)
        {
            FlattenPhase::ResidualExposure if input.position.0 != 0 => {
                return FundingRiskAction::ResidualExposure { reason };
            }
            FlattenPhase::ReduceOnly if input.position.0 != 0 => {
                return FundingRiskAction::Flatten { reason };
            }
            FlattenPhase::ReduceOnly => {
                return FundingRiskAction::StopNewRisk { reason };
            }
            FlattenPhase::Flat => {}
            FlattenPhase::Trading => {}
            FlattenPhase::ResidualExposure => {}
        }

        if !input.flatten_plan.entry_allowed_at(input.now_ms) {
            return if input.position.0 == 0 {
                FundingRiskAction::StopNewRisk { reason }
            } else {
                FundingRiskAction::Flatten { reason }
            };
        }

        if input.market_event_at_ms > input.now_ms
            || input.now_ms.saturating_sub(input.market_event_at_ms) > input.max_market_age_ms
        {
            return FundingRiskAction::Halt;
        }

        if input
            .anchor
            .is_none_or(|anchor| !anchor.is_valid_at(input.now_ms))
        {
            return FundingRiskAction::Halt;
        }

        if input.requested_quantity.0 <= 0 {
            return FundingRiskAction::NoAction;
        }

        let remaining = self.max_position.0 - absolute_position;
        if remaining <= 0 {
            return FundingRiskAction::NoAction;
        }

        FundingRiskAction::AllowEntry {
            quantity: Quantity(input.requested_quantity.0.min(remaining)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{FundingRateKind, FundingSchedule, PriceTicks};

    fn anchor() -> StaticAnchor {
        StaticAnchor::new(PriceTicks(100_000), 100, 20_000).unwrap()
    }

    fn schedule(next_funding_at_ms: Option<u64>) -> FundingSchedule {
        FundingSchedule::new(
            next_funding_at_ms,
            next_funding_at_ms.map(|_| 8),
            Some(100),
            FundingRateKind::Regular,
            100,
        )
        .unwrap()
    }

    fn input(plan: DualFlattenPlan) -> FundingRiskInput {
        FundingRiskInput {
            now_ms: 1_000,
            requested_quantity: Quantity(250),
            position: Quantity(0),
            market_event_at_ms: 990,
            max_market_age_ms: 20,
            anchor: Some(anchor()),
            flatten_plan: plan,
        }
    }

    fn gate() -> FundingAwareRiskGate {
        FundingAwareRiskGate::new(Quantity(1_000)).unwrap()
    }

    #[test]
    fn allows_entry_when_no_flatten_event_is_active() {
        let plan = DualFlattenPlan::new(1_000, None, schedule(None), 1_800_000, 300_000).unwrap();

        assert_eq!(
            gate().evaluate(input(plan)),
            FundingRiskAction::AllowEntry {
                quantity: Quantity(250)
            }
        );
    }

    #[test]
    fn funding_window_stops_new_risk_and_flattens_position() {
        let plan =
            DualFlattenPlan::new(1_000, None, schedule(Some(301_000)), 1_800_000, 300_000).unwrap();

        let mut no_position = input(plan);
        assert_eq!(
            gate().evaluate(no_position),
            FundingRiskAction::StopNewRisk {
                reason: FlattenReason::FundingSettlement
            }
        );

        no_position.position = Quantity(100);
        assert_eq!(
            gate().evaluate(no_position),
            FundingRiskAction::Flatten {
                reason: FlattenReason::FundingSettlement
            }
        );
    }

    #[test]
    fn equity_open_and_funding_expose_both_when_windows_overlap() {
        let plan = DualFlattenPlan::new(
            1_000,
            Some(301_000),
            schedule(Some(301_000)),
            300_000,
            300_000,
        )
        .unwrap();

        let mut value = input(plan);
        value.position = Quantity(100);
        assert_eq!(
            gate().evaluate(value),
            FundingRiskAction::Flatten {
                reason: FlattenReason::Both
            }
        );
    }

    #[test]
    fn hard_deadline_reports_residual_exposure() {
        let plan =
            DualFlattenPlan::new(1_000, None, schedule(Some(1_000)), 1_800_000, 300_000).unwrap();

        let mut value = input(plan);
        value.now_ms = 1_001;
        value.position = Quantity(100);
        assert_eq!(
            gate().evaluate(value),
            FundingRiskAction::ResidualExposure {
                reason: FlattenReason::FundingSettlement
            }
        );
    }

    #[test]
    fn stale_state_halts_only_when_not_already_flattening() {
        let plan = DualFlattenPlan::new(1_000, None, schedule(None), 1_800_000, 300_000).unwrap();

        let mut value = input(plan);
        value.market_event_at_ms = 900;
        assert_eq!(gate().evaluate(value), FundingRiskAction::Halt);
    }

    #[test]
    fn entry_is_capped_by_absolute_position_limit() {
        let plan = DualFlattenPlan::new(1_000, None, schedule(None), 1_800_000, 300_000).unwrap();

        let mut value = input(plan);
        value.position = Quantity(900);
        assert_eq!(
            gate().evaluate(value),
            FundingRiskAction::AllowEntry {
                quantity: Quantity(100)
            }
        );
    }

    #[test]
    fn rejects_invalid_max_position() {
        assert!(FundingAwareRiskGate::new(Quantity(0)).is_none());
    }
}
