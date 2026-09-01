#[derive(Debug, Default)]
pub struct OrderBook {
    best_bid: i64,
    best_ask: i64,
}

impl OrderBook {
    pub fn update(&mut self, bid: i64, ask: i64) {
        self.best_bid = bid;
        self.best_ask = ask;
    }

    pub fn bid(&self) -> i64 {
        self.best_bid
    }

    pub fn ask(&self) -> i64 {
        self.best_ask
    }

    pub fn mid(&self) -> i64 {
        (self.best_bid + self.best_ask) / 2
    }

    pub fn spread(&self) -> i64 {
        self.best_ask - self.best_bid
    }
}
