pub mod orderbook;

#[derive(Debug)]
pub struct MarketState {
    pub symbol: String,
    pub index_price: f64,
    pub bid: f64,
    pub ask: f64,
}

impl MarketState {
    pub fn mid(&self) -> f64 {
        (self.bid + self.ask) * 0.5
    }
}
