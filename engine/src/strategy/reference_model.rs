//! Deterministic external-close reference construction.
use super::PriceTicks;

pub const PPM_SCALE: i128 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceQuality {
    Confirmed,
    Stale,
    Missing,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceInputs {
    pub close_price: PriceTicks,
    /// Quote currency per one unit of the external close, in parts per million.
    pub quote_per_unit_ppm: i64,
    /// Corporate-action factor in parts per million.
    pub corporate_action_factor_ppm: i64,
    /// Optional carry adjustment in parts per million.
    pub carry_ppm: i64,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjustedReference {
    pub price: PriceTicks,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub quality: ReferenceQuality,
}

impl AdjustedReference {
    pub fn from_inputs(inputs: ReferenceInputs) -> Option<Self> {
        if inputs.close_price.0 <= 0
            || inputs.quote_per_unit_ppm <= 0
            || inputs.corporate_action_factor_ppm <= 0
            || inputs.valid_until_ms <= inputs.observed_at_ms
            || inputs.carry_ppm <= -1_000_000
        {
            return None;
        }

        let close = i128::from(inputs.close_price.0);
        let fx = i128::from(inputs.quote_per_unit_ppm);
        let action = i128::from(inputs.corporate_action_factor_ppm);
        let carry = PPM_SCALE + i128::from(inputs.carry_ppm);
        let scaled = close
            .checked_mul(fx)?
            .checked_mul(action)?
            .checked_mul(carry)?
            .checked_div(PPM_SCALE)?
            .checked_div(PPM_SCALE)?
            .checked_div(PPM_SCALE)?;
        if scaled <= 0 || scaled > i128::from(i64::MAX) {
            return None;
        }

        Some(Self {
            price: PriceTicks(scaled as i64),
            observed_at_ms: inputs.observed_at_ms,
            valid_until_ms: inputs.valid_until_ms,
            quality: ReferenceQuality::Confirmed,
        })
    }

    pub fn is_valid_at(&self, now_ms: u64) -> bool {
        self.quality == ReferenceQuality::Confirmed
            && now_ms >= self.observed_at_ms
            && now_ms < self.valid_until_ms
    }

    pub fn with_quality(self, quality: ReferenceQuality) -> Self {
        Self { quality, ..self }
    }
}

/// Robust online fair-value estimate used by adaptive strategy variants.
///
/// The external close remains the primary coordinate system.  Binance index,
/// mark, and mid are evidence about the current tradable state, not a
/// replacement anchor.  Weighting is integer-only and is reduced when the
/// venues disagree, which prevents a transient mark/index dislocation from
/// becoming a false mean-reversion signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FairValueRegime {
    Calm,
    Normal,
    Stressed,
    Dislocated,
}

impl FairValueRegime {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Normal => "normal",
            Self::Stressed => "stressed",
            Self::Dislocated => "dislocated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FairValueEstimate {
    pub price: PriceTicks,
    pub confidence_bps: i64,
    pub dispersion_bps: i64,
    pub regime: FairValueRegime,
}

impl FairValueEstimate {
    pub fn from_market(
        anchor: PriceTicks,
        index: PriceTicks,
        mark: PriceTicks,
        mid: PriceTicks,
        volatility_bps: i64,
        spread_bps: i64,
    ) -> Option<Self> {
        if [anchor.0, index.0, mark.0, mid.0].iter().any(|value| *value <= 0) {
            return None;
        }
        let index_gap = distance_bps(index.0, anchor.0);
        let mark_gap = distance_bps(mark.0, index.0);
        let mid_gap = distance_bps(mid.0, mark.0);
        let dispersion_bps = index_gap.max(mark_gap).max(mid_gap);
        let regime = if dispersion_bps >= 100 || volatility_bps >= 75 {
            FairValueRegime::Dislocated
        } else if dispersion_bps >= 50 || volatility_bps >= 35 {
            FairValueRegime::Stressed
        } else if dispersion_bps <= 5 && volatility_bps <= 8 {
            FairValueRegime::Calm
        } else {
            FairValueRegime::Normal
        };
        let (anchor_weight, index_weight, mark_weight) = match regime {
            FairValueRegime::Calm => (500_000_i128, 300_000_i128, 200_000_i128),
            FairValueRegime::Normal => (600_000, 250_000, 150_000),
            FairValueRegime::Stressed => (750_000, 175_000, 75_000),
            FairValueRegime::Dislocated => (900_000, 75_000, 25_000),
        };
        let weighted = i128::from(anchor.0) * anchor_weight
            + i128::from(index.0) * index_weight
            + i128::from(mark.0) * mark_weight;
        let price = weighted.checked_div(PPM_SCALE)?;
        if price <= 0 || price > i128::from(i64::MAX) {
            return None;
        }
        let confidence_bps = dispersion_bps
            .saturating_add(volatility_bps.max(0))
            .saturating_add(spread_bps.max(0) / 2)
            .clamp(0, i64::MAX);
        Some(Self {
            price: PriceTicks(price as i64),
            confidence_bps,
            dispersion_bps,
            regime,
        })
    }
}

fn distance_bps(left: i64, right: i64) -> i64 {
    (((i128::from(left) - i128::from(right)).abs() * 10_000)
        / i128::from(right.max(1)))
    .clamp(0, i128::from(i64::MAX)) as i64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceFreshness {
    pub now_ms: u64,
    pub max_age_ms: u64,
    pub observed_at_ms: u64,
}

impl ReferenceFreshness {
    pub fn is_fresh(&self) -> bool {
        self.now_ms >= self.observed_at_ms && self.now_ms - self.observed_at_ms <= self.max_age_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_reference_with_currency_and_action_adjustment() {
        let reference = AdjustedReference::from_inputs(ReferenceInputs {
            close_price: PriceTicks(100_000),
            quote_per_unit_ppm: 1_000_000,
            corporate_action_factor_ppm: 1_000_000,
            carry_ppm: 5_000,
            observed_at_ms: 10,
            valid_until_ms: 20,
        })
        .unwrap();
        assert_eq!(reference.price, PriceTicks(100_500));
        assert!(reference.is_valid_at(10));
        assert!(!reference.is_valid_at(20));
    }
    #[test]
    fn applies_cny_or_hkd_fx_without_floating_point() {
        let reference = AdjustedReference::from_inputs(ReferenceInputs {
            close_price: PriceTicks(100_000),
            quote_per_unit_ppm: 7_500,
            corporate_action_factor_ppm: 1_000_000,
            carry_ppm: 0,
            observed_at_ms: 0,
            valid_until_ms: 100,
        })
        .unwrap();
        assert_eq!(reference.price, PriceTicks(750));
    }

    #[test]
    fn rejects_missing_or_invalid_reference_inputs() {
        assert!(AdjustedReference::from_inputs(ReferenceInputs {
            close_price: PriceTicks(0),
            quote_per_unit_ppm: 1_000_000,
            corporate_action_factor_ppm: 1_000_000,
            carry_ppm: 0,
            observed_at_ms: 0,
            valid_until_ms: 10,
        })
        .is_none());
        assert!(AdjustedReference::from_inputs(ReferenceInputs {
            close_price: PriceTicks(100),
            quote_per_unit_ppm: 1_000_000,
            corporate_action_factor_ppm: 1_000_000,
            carry_ppm: -1_000_000,
            observed_at_ms: 0,
            valid_until_ms: 10,
        })
        .is_none());
    }

    #[test]
    fn freshness_is_explicit() {
        assert!(ReferenceFreshness {
            now_ms: 100,
            max_age_ms: 10,
            observed_at_ms: 95,
        }
        .is_fresh());
        assert!(!ReferenceFreshness {
            now_ms: 100,
            max_age_ms: 4,
            observed_at_ms: 95,
        }
        .is_fresh());
    }

    #[test]
    fn fair_value_keeps_anchor_primary_during_dislocation() {
        let estimate = FairValueEstimate::from_market(
            PriceTicks(100_000),
            PriceTicks(120_000),
            PriceTicks(125_000),
            PriceTicks(124_000),
            80,
            10,
        )
        .unwrap();
        assert_eq!(estimate.regime, FairValueRegime::Dislocated);
        assert!(estimate.price.0 < 110_000);
        assert!(estimate.confidence_bps >= 80);
    }

    #[test]
    fn fair_value_reports_calm_consensus() {
        let estimate = FairValueEstimate::from_market(
            PriceTicks(100_000),
            PriceTicks(100_020),
            PriceTicks(100_025),
            PriceTicks(100_024),
            2,
            2,
        )
        .unwrap();
        assert_eq!(estimate.regime, FairValueRegime::Calm);
        assert!(estimate.confidence_bps < 10);
    }
}
