#[derive(Debug, Clone, Copy)]
pub struct MakerPriceEngine {
    pub offset_bps: i64,
}

impl MakerPriceEngine {
    pub fn new(offset_bps: i64) -> Self {
        Self { offset_bps }
    }

    pub fn buy_price(&self, index_price: i64) -> i64 {
        index_price - (index_price * self.offset_bps / 10000)
    }

    pub fn sell_price(&self, index_price: i64) -> i64 {
        index_price + (index_price * self.offset_bps / 10000)
    }
}
