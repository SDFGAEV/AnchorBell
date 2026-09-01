#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderIntent {
    pub symbol: u32,
    pub side: Side,
    pub price: i64,
    pub quantity: i64,
    pub post_only: bool,
}

impl OrderIntent {
    pub fn maker_buy(symbol: u32, price: i64, quantity: i64) -> Self {
        Self {
            symbol,
            side: Side::Buy,
            price,
            quantity,
            post_only: true,
        }
    }

    pub fn maker_sell(symbol: u32, price: i64, quantity: i64) -> Self {
        Self {
            symbol,
            side: Side::Sell,
            price,
            quantity,
            post_only: true,
        }
    }
}
