#[derive(Debug, Clone, Copy)]
pub struct QuoteContext {
    pub fair_price: i64,
    pub spread: i64,
    pub imbalance_bps: i64,
    pub inventory: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct MakerQuote {
    pub bid: i64,
    pub ask: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct QuoteEngine {
    pub inventory_limit: i64,
    pub inventory_skew_bps: i64,
}

impl QuoteEngine {
    pub fn new(inventory_limit: i64, inventory_skew_bps: i64) -> Self {
        Self {
            inventory_limit,
            inventory_skew_bps,
        }
    }

    #[inline]
    pub fn quote(&self, ctx: QuoteContext) -> MakerQuote {
        let micro_adjustment = i128::from(ctx.imbalance_bps / 10);
        let inventory_limit = i128::from(self.inventory_limit.max(1));
        let inventory_adjustment =
            i128::from(ctx.inventory) * i128::from(self.inventory_skew_bps) / inventory_limit;

        let center = i128::from(ctx.fair_price) + micro_adjustment - inventory_adjustment;
        let half_spread = i128::from(ctx.spread.max(2).saturating_add(1) / 2);

        MakerQuote {
            bid: clamp_i128(center - half_spread),
            ask: clamp_i128(center + half_spread),
        }
    }
}

#[inline]
fn clamp_i128(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

#[cfg(test)]
mod tests {
    use super::{QuoteContext, QuoteEngine};

    #[test]
    fn keeps_a_minimum_two_tick_symmetric_quote_width() {
        let quote = QuoteEngine::new(100, 100).quote(QuoteContext {
            fair_price: 100,
            spread: 1,
            imbalance_bps: 0,
            inventory: 0,
        });
        assert_eq!((quote.bid, quote.ask), (99, 101));
    }

    #[test]
    fn applies_inventory_skew_without_overflow() {
        let quote = QuoteEngine::new(100, 100).quote(QuoteContext {
            fair_price: 100,
            spread: 4,
            imbalance_bps: 0,
            inventory: 50,
        });
        assert_eq!((quote.bid, quote.ask), (48, 52));

        let extreme = QuoteEngine::new(i64::MAX, i64::MAX).quote(QuoteContext {
            fair_price: i64::MAX,
            spread: i64::MAX,
            imbalance_bps: i64::MAX,
            inventory: i64::MAX,
        });
        assert!(extreme.bid <= extreme.ask);
    }
}
