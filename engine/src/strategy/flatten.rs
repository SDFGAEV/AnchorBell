//! Pure schedule and state transitions for dual-deadline flattening.
//!
//! This module has no exchange, clock, network, or persistence dependency.
//! Live and backtest runtimes provide the timestamps and consume the same plan.

use super::FundingSchedule;

/// Why a contract entered its flatten window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlattenReason {
    None,
    EquityOpen,
    FundingSettlement,
    Both,
}

/// Position-aware phase of the flatten lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlattenPhase {
    Trading,
    ReduceOnly,
    ResidualExposure,
    Flat,
}

/// Independent equity-open and funding-settlement deadlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualFlattenPlan {
    pub now_ms: u64,
    pub equity_open_at_ms: Option<u64>,
    pub funding: FundingSchedule,
    pub equity_window_ms: u64,
    pub funding_window_ms: u64,
}

impl DualFlattenPlan {
    pub fn new(
        now_ms: u64,
        equity_open_at_ms: Option<u64>,
        funding: FundingSchedule,
        equity_window_ms: u64,
        funding_window_ms: u64,
    ) -> Option<Self> {
        if equity_window_ms == 0 || funding_window_ms == 0 {
            return None;
        }
        Some(Self {
            now_ms,
            equity_open_at_ms,
            funding,
            equity_window_ms,
            funding_window_ms,
        })
    }

    pub fn equity_open_deadline_ms(&self) -> Option<u64> {
        self.equity_open_at_ms
            .map(|open| open.saturating_sub(self.equity_window_ms))
    }

    pub fn funding_deadline_ms(&self) -> Option<u64> {
        self.funding.deadline(self.funding_window_ms)
    }

    pub fn effective_flatten_start_ms(&self) -> Option<u64> {
        match (self.equity_open_deadline_ms(), self.funding_deadline_ms()) {
            (Some(equity), Some(funding)) => Some(equity.min(funding)),
            (Some(equity), None) => Some(equity),
            (None, Some(funding)) => Some(funding),
            (None, None) => None,
        }
    }

    pub fn hard_deadline_ms(&self) -> Option<u64> {
        match (self.equity_open_at_ms, self.funding.next_funding_at_ms) {
            (Some(equity), Some(funding)) => Some(equity.min(funding)),
            (Some(equity), None) => Some(equity),
            (None, Some(funding)) => Some(funding),
            (None, None) => None,
        }
    }

    pub fn reason_at(&self, now_ms: u64) -> FlattenReason {
        let equity_active = self
            .equity_open_deadline_ms()
            .is_some_and(|deadline| now_ms >= deadline);
        let funding_active = self
            .funding_deadline_ms()
            .is_some_and(|deadline| now_ms >= deadline);
        match (equity_active, funding_active) {
            (true, true) => FlattenReason::Both,
            (true, false) => FlattenReason::EquityOpen,
            (false, true) => FlattenReason::FundingSettlement,
            (false, false) => FlattenReason::None,
        }
    }

    pub fn phase_at(&self, now_ms: u64, has_position: bool) -> FlattenPhase {
        if !has_position {
            return FlattenPhase::Flat;
        }
        if self
            .hard_deadline_ms()
            .is_some_and(|deadline| now_ms >= deadline)
        {
            return FlattenPhase::ResidualExposure;
        }
        if self
            .effective_flatten_start_ms()
            .is_some_and(|start| now_ms >= start)
        {
            return FlattenPhase::ReduceOnly;
        }
        FlattenPhase::Trading
    }

    pub fn entry_allowed_at(&self, now_ms: u64) -> bool {
        self.phase_at(now_ms, true) == FlattenPhase::Trading
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::FundingRateKind;

    fn no_funding(now_ms: u64) -> FundingSchedule {
        FundingSchedule::new(None, None, None, FundingRateKind::Unknown, now_ms).unwrap()
    }

    fn funding(now_ms: u64) -> FundingSchedule {
        FundingSchedule::new(
            Some(600_000),
            Some(8),
            Some(100),
            FundingRateKind::Regular,
            now_ms,
        )
        .unwrap()
    }

    #[test]
    fn equity_open_starts_reduction_thirty_minutes_early() {
        let plan =
            DualFlattenPlan::new(0, Some(2_000_000), no_funding(0), 1_800_000, 300_000).unwrap();

        assert_eq!(plan.equity_open_deadline_ms(), Some(200_000));
        assert_eq!(plan.effective_flatten_start_ms(), Some(200_000));
        assert_eq!(plan.phase_at(199_999, true), FlattenPhase::Trading);
        assert_eq!(plan.phase_at(200_000, true), FlattenPhase::ReduceOnly);
        assert_eq!(
            plan.phase_at(2_000_000, true),
            FlattenPhase::ResidualExposure
        );
    }

    #[test]
    fn funding_starts_reduction_five_minutes_early() {
        let plan = DualFlattenPlan::new(0, None, funding(0), 1_800_000, 300_000).unwrap();

        assert_eq!(plan.funding_deadline_ms(), Some(300_000));
        assert_eq!(plan.reason_at(300_000), FlattenReason::FundingSettlement);
        assert_eq!(plan.phase_at(299_999, true), FlattenPhase::Trading);
        assert_eq!(plan.phase_at(300_000, true), FlattenPhase::ReduceOnly);
        assert_eq!(plan.phase_at(600_000, true), FlattenPhase::ResidualExposure);
    }

    #[test]
    fn earliest_deadline_wins_and_both_reasons_are_visible() {
        let short_funding = FundingSchedule::new(
            Some(10_000),
            Some(8),
            Some(100),
            FundingRateKind::Regular,
            0,
        )
        .unwrap();
        let plan = DualFlattenPlan::new(0, Some(9_800), short_funding, 1_000, 300).unwrap();

        assert_eq!(plan.effective_flatten_start_ms(), Some(8_800));
        assert_eq!(plan.reason_at(8_800), FlattenReason::EquityOpen);
        assert_eq!(plan.reason_at(9_700), FlattenReason::Both);
        assert_eq!(plan.hard_deadline_ms(), Some(9_800));
    }

    #[test]
    fn no_funding_does_not_create_a_weekend_deadline() {
        let plan = DualFlattenPlan::new(0, None, no_funding(0), 1_800_000, 300_000).unwrap();

        assert_eq!(plan.effective_flatten_start_ms(), None);
        assert_eq!(plan.hard_deadline_ms(), None);
        assert_eq!(plan.reason_at(86_400_000), FlattenReason::None);
        assert!(plan.entry_allowed_at(86_400_000));
    }

    #[test]
    fn flat_position_is_always_flat_even_after_a_deadline() {
        let plan = DualFlattenPlan::new(0, None, funding(0), 1_800_000, 300_000).unwrap();

        assert_eq!(plan.phase_at(10_000, false), FlattenPhase::Flat);
    }

    #[test]
    fn zero_windows_are_rejected() {
        assert!(DualFlattenPlan::new(0, None, no_funding(0), 0, 300_000).is_none());
        assert!(DualFlattenPlan::new(0, None, no_funding(0), 1_800_000, 0).is_none());
    }
}
