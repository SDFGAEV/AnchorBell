//! Deterministic policy for passive trading around a static close anchor.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PriceTicks(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BasisPoints(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorDecision {
    BuyMaker { price: PriceTicks, quantity: Quantity },
    SellMaker { price: PriceTicks, quantity: Quantity },
    NoAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorPolicy {
    pub entry_threshold: BasisPoints,
    pub max_position: Quantity,
}

impl AnchorPolicy {
    pub fn new(entry_threshold: BasisPoints, max_position: Quantity) -> Option<Self> {
        if entry_threshold.0 <= 0 || max_position.0 <= 0 {
            return None;
        }
        Some(Self { entry_threshold, max_position })
    }

    pub fn decide(
        &self,
        anchor: PriceTicks,
        best_bid: PriceTicks,
        best_ask: PriceTicks,
        position: Quantity,
        requested_quantity: Quantity,
    ) -> AnchorDecision {
        if anchor.0 <= 0
            || best_bid.0 <= 0
            || best_ask.0 < best_bid.0
            || requested_quantity.0 <= 0
        {
            return AnchorDecision::NoAction;
        }
        let mid = (best_bid.0 + best_ask.0) / 2;
        let deviation_bps = (mid - anchor.0) * 10_000 / anchor.0;
        let quantity = Quantity(requested_quantity.0.min(self.max_position.0));
        if deviation_bps <= -self.entry_threshold.0
            && position.0 < self.max_position.0
        {
            AnchorDecision::BuyMaker { price: best_bid, quantity }
        } else if deviation_bps >= self.entry_threshold.0
            && position.0 > -self.max_position.0
        {
            AnchorDecision::SellMaker { price: best_ask, quantity }
        } else {
            AnchorDecision::NoAction
        }
    }
}
 
#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AnchorPolicy {
        AnchorPolicy::new(BasisPoints(100), Quantity(10_000)).unwrap()
    }

    #[test]
    fn buys_below_static_anchor_at_best_bid() {
        assert_eq!(
            policy().decide(PriceTicks(100_000), PriceTicks(98_000), PriceTicks(98_100),
                Quantity(0), Quantity(500)),
            AnchorDecision::BuyMaker { price: PriceTicks(98_000), quantity: Quantity(500) }
        );
    }

    #[test]
    fn sells_above_static_anchor_at_best_ask() {
        assert_eq!(
            policy().decide(PriceTicks(100_000), PriceTicks(102_000), PriceTicks(102_100),
                Quantity(0), Quantity(500)),
            AnchorDecision::SellMaker { price: PriceTicks(102_100), quantity: Quantity(500) }
        );
    }

    #[test]
    fn refuses_invalid_market_and_zero_anchor() {
        assert_eq!(
            policy().decide(PriceTicks(0), PriceTicks(98_000), PriceTicks(98_100),
                Quantity(0), Quantity(500)),
            AnchorDecision::NoAction
        );
        assert_eq!(
            policy().decide(PriceTicks(100_000), PriceTicks(99_000), PriceTicks(98_900),
                Quantity(0), Quantity(500)),
            AnchorDecision::NoAction
        );
    }

    #[test]
    fn enforces_position_limit() {
        assert_eq!(
            policy().decide(PriceTicks(100_000), PriceTicks(98_000), PriceTicks(98_100),
                Quantity(10_000), Quantity(500)),
            AnchorDecision::NoAction
        );
    }
}
