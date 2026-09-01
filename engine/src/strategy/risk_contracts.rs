//! Typed contracts for funding-aware, queue-aware maker decisions.
//!
//! These values are deliberately independent from exchange clients and
//! persistence. They can be used by live, paper, replay, and backtest paths.

use super::{PriceTicks, Quantity};

/// Funding event classification reported by the exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FundingRateKind {
    Regular,
    Special,
    Unknown,
}

/// Per-contract funding schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FundingSchedule {
    pub next_funding_at_ms: Option<u64>,
    pub funding_interval_hours: Option<u32>,
    pub estimated_rate_ppm: Option<i64>,
    pub rate_kind: FundingRateKind,
    pub observed_at_ms: u64,
}

impl FundingSchedule {
    pub fn new(
        next_funding_at_ms: Option<u64>,
        funding_interval_hours: Option<u32>,
        estimated_rate_ppm: Option<i64>,
        rate_kind: FundingRateKind,
        observed_at_ms: u64,
    ) -> Option<Self> {
        if let Some(next) = next_funding_at_ms {
            if next <= observed_at_ms {
                return None;
            }
        }
        if let Some(interval) = funding_interval_hours {
            if interval == 0 {
                return None;
            }
        }
        Some(Self {
            next_funding_at_ms,
            funding_interval_hours,
            estimated_rate_ppm,
            rate_kind,
            observed_at_ms,
        })
    }

    pub fn has_event(&self) -> bool {
        self.next_funding_at_ms.is_some()
    }

    pub fn is_within(&self, now_ms: u64, window_ms: u64) -> bool {
        self.next_funding_at_ms
            .is_some_and(|next| next >= now_ms && next - now_ms <= window_ms)
    }

    pub fn deadline(&self, window_ms: u64) -> Option<u64> {
        self.next_funding_at_ms
            .map(|next| next.saturating_sub(window_ms))
    }

    pub fn is_fresh_at(&self, now_ms: u64, max_age_ms: u64) -> bool {
        now_ms >= self.observed_at_ms && now_ms - self.observed_at_ms <= max_age_ms
    }
}

/// A probabilistic estimate of the queue around a maker order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueEstimate {
    pub price: PriceTicks,
    pub quantity_ahead: Quantity,
    pub quantity_ahead_lower: Quantity,
    pub quantity_ahead_upper: Quantity,
    pub confidence_bps: u16,
}

impl QueueEstimate {
    pub fn new(
        price: PriceTicks,
        quantity_ahead: Quantity,
        quantity_ahead_lower: Quantity,
        quantity_ahead_upper: Quantity,
        confidence_bps: u16,
    ) -> Option<Self> {
        if price.0 <= 0
            || quantity_ahead.0 < 0
            || quantity_ahead_lower.0 < 0
            || quantity_ahead_upper.0 < quantity_ahead_lower.0
            || quantity_ahead.0 < quantity_ahead_lower.0
            || quantity_ahead.0 > quantity_ahead_upper.0
            || confidence_bps > 10_000
        {
            return None;
        }
        Some(Self {
            price,
            quantity_ahead,
            quantity_ahead_lower,
            quantity_ahead_upper,
            confidence_bps,
        })
    }

    pub fn conservative_ahead(&self) -> Quantity {
        self.quantity_ahead_upper
    }
}

/// A bounded estimate used when point estimates are unsafe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfidenceInterval {
    pub lower: i64,
    pub estimate: i64,
    pub upper: i64,
    pub sample_count: u64,
    pub confidence_bps: u16,
}

impl ConfidenceInterval {
    pub fn new(
        lower: i64,
        estimate: i64,
        upper: i64,
        sample_count: u64,
        confidence_bps: u16,
    ) -> Option<Self> {
        if lower > estimate || estimate > upper || sample_count == 0 || confidence_bps > 10_000 {
            return None;
        }
        Some(Self {
            lower,
            estimate,
            upper,
            sample_count,
            confidence_bps,
        })
    }
}

/// Conservative economic value of one candidate maker order.
///
/// All *_ppm fields are parts-per-million of quote notional. Probability and
/// confidence are basis points in [0, 10_000].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionalOrderValue {
    pub gross_edge_ppm: ConfidenceInterval,
    pub fill_probability_bps: u16,
    pub confidence_bps: u16,
    pub cost_ppm: i64,
    pub inventory_penalty_ppm: i64,
    pub deadline_penalty_ppm: i64,
}

impl ConditionalOrderValue {
    pub fn new(
        gross_edge_ppm: ConfidenceInterval,
        fill_probability_bps: u16,
        confidence_bps: u16,
        cost_ppm: i64,
        inventory_penalty_ppm: i64,
        deadline_penalty_ppm: i64,
    ) -> Option<Self> {
        if fill_probability_bps > 10_000
            || confidence_bps > 10_000
            || cost_ppm < 0
            || inventory_penalty_ppm < 0
            || deadline_penalty_ppm < 0
        {
            return None;
        }
        Some(Self {
            gross_edge_ppm,
            fill_probability_bps,
            confidence_bps,
            cost_ppm,
            inventory_penalty_ppm,
            deadline_penalty_ppm,
        })
    }

    pub fn conservative_net_value_ppm(&self) -> i128 {
        let gross = i128::from(self.gross_edge_ppm.lower);
        let probability = i128::from(self.fill_probability_bps);
        let confidence = i128::from(self.confidence_bps);
        let expected_gross = gross * probability * confidence / 100_000_000;
        expected_gross
            - i128::from(self.cost_ppm)
            - i128::from(self.inventory_penalty_ppm)
            - i128::from(self.deadline_penalty_ppm)
    }

    pub fn is_admissible(&self, safety_margin_ppm: i64) -> bool {
        safety_margin_ppm >= 0 && self.conservative_net_value_ppm() > i128::from(safety_margin_ppm)
    }
}

/// Estimated ability to flatten a live position before a risk deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlattenFeasibility {
    pub now_ms: u64,
    pub deadline_ms: u64,
    pub estimated_time_to_flat_ms: u64,
    pub fillable_quantity: Quantity,
    pub target_quantity: Quantity,
    pub confidence_bps: u16,
    pub safety_buffer_ms: u64,
}

impl FlattenFeasibility {
    pub fn new(
        now_ms: u64,
        deadline_ms: u64,
        estimated_time_to_flat_ms: u64,
        fillable_quantity: Quantity,
        target_quantity: Quantity,
        confidence_bps: u16,
        safety_buffer_ms: u64,
    ) -> Option<Self> {
        if deadline_ms < now_ms
            || fillable_quantity.0 < 0
            || target_quantity.0 < 0
            || confidence_bps > 10_000
        {
            return None;
        }
        Some(Self {
            now_ms,
            deadline_ms,
            estimated_time_to_flat_ms,
            fillable_quantity,
            target_quantity,
            confidence_bps,
            safety_buffer_ms,
        })
    }

    pub fn deadline_slack_ms(&self) -> i128 {
        i128::from(self.deadline_ms)
            - i128::from(self.now_ms)
            - i128::from(self.estimated_time_to_flat_ms)
            - i128::from(self.safety_buffer_ms)
    }

    pub fn is_feasible(&self) -> bool {
        self.fillable_quantity.0 >= self.target_quantity.0 && self.deadline_slack_ms() >= 0
    }

    pub fn is_low_slack(&self, warning_ms: u64) -> bool {
        self.deadline_slack_ms() <= i128::from(warning_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funding_schedule_is_symbol_specific_and_window_aware() {
        let schedule = FundingSchedule::new(
            Some(20_000),
            Some(8),
            Some(125),
            FundingRateKind::Regular,
            10_000,
        )
        .unwrap();

        assert!(schedule.has_event());
        assert!(schedule.is_within(15_000, 5_000));
        assert!(!schedule.is_within(15_001, 4_998));
        assert_eq!(schedule.deadline(5_000), Some(15_000));
        assert!(schedule.is_fresh_at(12_000, 2_000));
    }

    #[test]
    fn funding_schedule_rejects_expired_event_and_zero_interval() {
        assert!(
            FundingSchedule::new(Some(10), Some(8), None, FundingRateKind::Unknown, 10).is_none()
        );
        assert!(
            FundingSchedule::new(Some(20), Some(0), None, FundingRateKind::Unknown, 10).is_none()
        );
    }

    #[test]
    fn weekend_does_not_synthesize_a_funding_event() {
        let schedule =
            FundingSchedule::new(None, None, None, FundingRateKind::Unknown, 10).unwrap();

        assert!(!schedule.has_event());
        assert!(!schedule.is_within(10_000, 5_000));
        assert_eq!(schedule.deadline(5_000), None);
    }

    #[test]
    fn queue_estimate_uses_upper_bound_for_conservative_decisions() {
        let queue = QueueEstimate::new(
            PriceTicks(100),
            Quantity(10),
            Quantity(5),
            Quantity(20),
            7_500,
        )
        .unwrap();

        assert_eq!(queue.conservative_ahead(), Quantity(20));
        assert!(QueueEstimate::new(
            PriceTicks(100),
            Quantity(10),
            Quantity(20),
            Quantity(10),
            7_500
        )
        .is_none());
    }

    #[test]
    fn conditional_order_value_requires_conservative_positive_edge() {
        let edge = ConfidenceInterval::new(1_000, 1_500, 2_000, 100, 9_000).unwrap();
        let value = ConditionalOrderValue::new(edge, 8_000, 9_000, 50, 20, 10).unwrap();

        assert!(value.conservative_net_value_ppm() > 0);
        assert!(value.is_admissible(1));
        assert!(!value.is_admissible(10_000));
    }

    #[test]
    fn conditional_order_value_rejects_negative_costs() {
        let edge = ConfidenceInterval::new(1, 2, 3, 1, 5_000).unwrap();
        assert!(ConditionalOrderValue::new(edge, 1_000, 1_000, -1, 0, 0).is_none());
    }

    #[test]
    fn flatten_feasibility_models_time_and_quantity() {
        let feasible =
            FlattenFeasibility::new(1_000, 10_000, 5_000, Quantity(10), Quantity(8), 9_000, 500)
                .unwrap();

        assert_eq!(feasible.deadline_slack_ms(), 3_500);
        assert!(feasible.is_feasible());
        assert!(!feasible.is_low_slack(1_000));
    }

    #[test]
    fn flatten_feasibility_rejects_when_deadline_is_past() {
        assert!(FlattenFeasibility::new(
            10_000,
            9_000,
            1_000,
            Quantity(10),
            Quantity(10),
            9_000,
            0
        )
        .is_none());
    }
}
