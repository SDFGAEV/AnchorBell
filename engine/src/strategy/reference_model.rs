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
}
