//! Cost-aware, volatility-aware maker admission policy.
use super::PriceTicks;
use crate::execution::{OrderIntent, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveThreshold {
    pub floor_bps: i64,
    pub residual_volatility_bps: i64,
    pub cost_bps: i64,
    pub uncertainty_bps: i64,
    pub deadline_risk_bps: i64,
    pub safety_margin_bps: i64,
}

impl AdaptiveThreshold {
    pub fn required_bps(self) -> Option<i64> {
        let values = [
            self.floor_bps,
            self.residual_volatility_bps,
            self.cost_bps,
            self.uncertainty_bps,
            self.deadline_risk_bps,
            self.safety_margin_bps,
        ];
        if values.iter().any(|value| *value < 0) {
            return None;
        }
        values.into_iter().max()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalInput {
    pub symbol: u32,
    pub anchor: PriceTicks,
    pub best_bid: PriceTicks,
    pub best_ask: PriceTicks,
    pub index_price: PriceTicks,
    pub mark_price: PriceTicks,
    pub position: i64,
    pub max_position: i64,
    pub requested_quantity: i64,
    pub threshold: AdaptiveThreshold,
    pub max_mark_index_gap_bps: i64,
    pub signal_age_ms: u64,
    pub max_signal_age_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalBlockReason {
    InvalidPrices,
    InvalidPositionLimit,
    StaleSignal,
    MarkIndexDisagreement,
    ThresholdUnavailable,
    NoEdge,
    PositionLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDecision {
    BuyMaker { price: PriceTicks, quantity: i64 },
    SellMaker { price: PriceTicks, quantity: i64 },
    Blocked(SignalBlockReason),
}

pub fn decide(input: SignalInput) -> SignalDecision {
    if input.anchor.0 <= 0
        || input.best_bid.0 <= 0
        || input.best_ask.0 < input.best_bid.0
        || input.index_price.0 <= 0
        || input.mark_price.0 <= 0
    {
        return SignalDecision::Blocked(SignalBlockReason::InvalidPrices);
    }
    if input.max_position <= 0 || input.requested_quantity <= 0 {
        return SignalDecision::Blocked(SignalBlockReason::InvalidPositionLimit);
    }
    if input.signal_age_ms > input.max_signal_age_ms {
        return SignalDecision::Blocked(SignalBlockReason::StaleSignal);
    }
    if input.max_mark_index_gap_bps < 0 {
        return SignalDecision::Blocked(SignalBlockReason::MarkIndexDisagreement);
    }
    let mark_index_gap_numerator =
        (i128::from(input.mark_price.0) - i128::from(input.index_price.0)).abs() * 10_000;
    let mark_index_limit_numerator =
        i128::from(input.max_mark_index_gap_bps) * i128::from(input.index_price.0);
    if mark_index_gap_numerator > mark_index_limit_numerator {
        return SignalDecision::Blocked(SignalBlockReason::MarkIndexDisagreement);
    }
    let required_bps = match input.threshold.required_bps() {
        Some(value) => value,
        None => return SignalDecision::Blocked(SignalBlockReason::ThresholdUnavailable),
    };
    let mid = (i128::from(input.best_bid.0) + i128::from(input.best_ask.0)) / 2;
    let deviation_numerator = (mid - i128::from(input.anchor.0)) * 10_000;
    let threshold_numerator = i128::from(required_bps) * i128::from(input.anchor.0);
    let quantity = input.requested_quantity.min(input.max_position);
    if quantity <= 0 {
        return SignalDecision::Blocked(SignalBlockReason::PositionLimit);
    }
    if deviation_numerator <= -threshold_numerator {
        let remaining = (i128::from(input.max_position) - i128::from(input.position)).max(0);
        let capped_quantity = i128::from(quantity).min(remaining);
        if capped_quantity > 0 {
            return SignalDecision::BuyMaker {
                price: input.best_bid,
                quantity: capped_quantity as i64,
            };
        }
        return SignalDecision::Blocked(SignalBlockReason::PositionLimit);
    }
    if deviation_numerator >= threshold_numerator {
        let remaining = (i128::from(input.max_position) + i128::from(input.position)).max(0);
        let capped_quantity = i128::from(quantity).min(remaining);
        if capped_quantity > 0 {
            return SignalDecision::SellMaker {
                price: input.best_ask,
                quantity: capped_quantity as i64,
            };
        }
        return SignalDecision::Blocked(SignalBlockReason::PositionLimit);
    }
    SignalDecision::Blocked(SignalBlockReason::NoEdge)
}

impl SignalDecision {
    pub fn into_intent(self, symbol: u32) -> Option<OrderIntent> {
        match self {
            SignalDecision::BuyMaker { price, quantity } => Some(OrderIntent {
                symbol,
                side: Side::Buy,
                price: price.0,
                quantity,
                post_only: true,
            }),
            SignalDecision::SellMaker { price, quantity } => Some(OrderIntent {
                symbol,
                side: Side::Sell,
                price: price.0,
                quantity,
                post_only: true,
            }),
            SignalDecision::Blocked(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SignalInput {
        SignalInput {
            symbol: 7,
            anchor: PriceTicks(100_000),
            best_bid: PriceTicks(98_000),
            best_ask: PriceTicks(98_100),
            index_price: PriceTicks(98_050),
            mark_price: PriceTicks(98_050),
            position: 0,
            max_position: 1_000,
            requested_quantity: 100,
            threshold: AdaptiveThreshold {
                floor_bps: 50,
                residual_volatility_bps: 25,
                cost_bps: 10,
                uncertainty_bps: 10,
                deadline_risk_bps: 0,
                safety_margin_bps: 10,
            },
            max_mark_index_gap_bps: 20,
            signal_age_ms: 10,
            max_signal_age_ms: 100,
        }
    }

    #[test]
    fn requires_edge_above_all_dynamic_components() {
        assert_eq!(
            decide(input()),
            SignalDecision::BuyMaker {
                price: PriceTicks(98_000),
                quantity: 100
            }
        );
    }

    #[test]
    fn blocks_when_mark_and_index_disagree() {
        let mut value = input();
        value.mark_price = PriceTicks(99_000);
        assert_eq!(
            decide(value),
            SignalDecision::Blocked(SignalBlockReason::MarkIndexDisagreement)
        );
    }

    #[test]
    fn caps_both_sides_by_remaining_position() {
        let mut value = input();
        value.position = 950;
        assert_eq!(
            decide(value),
            SignalDecision::BuyMaker {
                price: PriceTicks(98_000),
                quantity: 50
            }
        );
        value.position = -950;
        value.best_bid = PriceTicks(102_000);
        value.best_ask = PriceTicks(102_100);
        assert_eq!(
            decide(value),
            SignalDecision::SellMaker {
                price: PriceTicks(102_100),
                quantity: 50
            }
        );
    }

    #[test]
    fn extreme_prices_are_evaluated_without_overflow() {
        let mut value = input();
        value.anchor = PriceTicks(i64::MAX);
        value.best_bid = PriceTicks(i64::MAX - 100);
        value.best_ask = PriceTicks(i64::MAX);
        value.index_price = PriceTicks(i64::MAX);
        value.mark_price = PriceTicks(i64::MAX);
        assert_eq!(
            decide(value),
            SignalDecision::Blocked(SignalBlockReason::NoEdge)
        );
    }

    #[test]
    fn stale_and_invalid_inputs_fail_closed() {
        let mut value = input();
        value.signal_age_ms = 101;
        assert_eq!(
            decide(value),
            SignalDecision::Blocked(SignalBlockReason::StaleSignal)
        );
        value = input();
        value.threshold.cost_bps = -1;
        assert_eq!(
            decide(value),
            SignalDecision::Blocked(SignalBlockReason::ThresholdUnavailable)
        );
    }
}
