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

    pub fn quote(&self, ctx: QuoteContext) -> MakerQuote {
        let micro_adjustment = ctx.imbalance_bps / 10;
        let inventory_adjustment =
            ctx.inventory * self.inventory_skew_bps / self.inventory_limit.max(1);

        let center = ctx.fair_price + micro_adjustment - inventory_adjustment;
        let half_spread = ctx.spread.max(1) / 2;

        MakerQuote {
            bid: center - half_spread,
            ask: center + half_spread,
        }
    }
}
