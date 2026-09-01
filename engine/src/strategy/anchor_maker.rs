use crate::execution::{OrderIntent, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    BuyMaker,
    SellMaker,
    NoAction,
}

#[derive(Debug, Clone, Copy)]
pub struct AnchorMakerStrategy {
    pub entry_threshold_bps: i64,
    pub exit_threshold_bps: i64,
}

impl AnchorMakerStrategy {
    pub fn new(entry_threshold_bps: i64, exit_threshold_bps: i64) -> Self {
        Self {
            entry_threshold_bps,
            exit_threshold_bps,
        }
    }

    pub fn generate_intent(
        &self,
        symbol: u32,
        bid: i64,
        ask: i64,
        index_price: i64,
        quantity: i64,
    ) -> Option<OrderIntent> {
        if index_price == 0 {
            return None;
        }

        let mid = (bid + ask) / 2;
        let deviation_bps = (mid - index_price) * 10000 / index_price;

        if deviation_bps <= -self.entry_threshold_bps {
            Some(OrderIntent {
                symbol,
                side: Side::Buy,
                price: bid,
                quantity,
                post_only: true,
            })
        } else if deviation_bps >= self.entry_threshold_bps {
            Some(OrderIntent {
                symbol,
                side: Side::Sell,
                price: ask,
                quantity,
                post_only: true,
            })
        } else {
            None
        }
    }
}
