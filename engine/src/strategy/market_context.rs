#[derive(Debug, Clone, Copy)]
pub struct MarketContext {
    pub index_price: i64,
    pub best_bid: i64,
    pub best_ask: i64,
    pub inventory: i64,
    pub imbalance_bps: i64,
}

impl MarketContext {
    pub fn mid_price(&self) -> i64 {
        (self.best_bid + self.best_ask) / 2
    }

    pub fn spread(&self) -> i64 {
        self.best_ask - self.best_bid
    }

    pub fn deviation_bps(&self) -> i64 {
        if self.index_price == 0 {
            return 0;
        }

        (self.mid_price() - self.index_price) * 10000 / self.index_price
    }
}
