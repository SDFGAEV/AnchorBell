use crate::execution::Side;
#[path = "backtest_realism.rs"]
pub mod realism;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MakerQuote {
    pub side: Side,
    pub price_ticks: i64,
    pub quantity: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopOfBook {
    pub bid_price_ticks: i64,
    pub ask_price_ticks: i64,
    pub bid_quantity: i64,
    pub ask_quantity: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillDecision {
    NoFill,
    Fill { quantity: i64 },
}

pub trait FillModel {
    fn evaluate(&self, quote: MakerQuote, book: TopOfBook) -> FillDecision;
}

#[derive(Debug, Clone, Copy)]
pub struct ConservativeTopOfBook;

impl FillModel for ConservativeTopOfBook {
    fn evaluate(&self, quote: MakerQuote, book: TopOfBook) -> FillDecision {
        match quote.side {
            Side::Buy if quote.price_ticks < book.ask_price_ticks => FillDecision::NoFill,
            Side::Sell if quote.price_ticks > book.bid_price_ticks => FillDecision::NoFill,
            Side::Buy => FillDecision::Fill {
                quantity: quote.quantity.min(book.ask_quantity),
            },
            Side::Sell => FillDecision::Fill {
                quantity: quote.quantity.min(book.bid_quantity),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservative_model_does_not_fill_when_quote_is_behind_ask() {
        let decision = ConservativeTopOfBook.evaluate(
            MakerQuote {
                side: Side::Buy,
                price_ticks: 99,
                quantity: 5,
            },
            TopOfBook {
                bid_price_ticks: 98,
                ask_price_ticks: 100,
                bid_quantity: 7,
                ask_quantity: 3,
            },
        );
        assert_eq!(decision, FillDecision::NoFill);
    }

    #[test]
    fn fill_is_capped_by_visible_top_level_quantity() {
        let decision = ConservativeTopOfBook.evaluate(
            MakerQuote {
                side: Side::Sell,
                price_ticks: 100,
                quantity: 5,
            },
            TopOfBook {
                bid_price_ticks: 100,
                ask_price_ticks: 101,
                bid_quantity: 2,
                ask_quantity: 7,
            },
        );
        assert_eq!(decision, FillDecision::Fill { quantity: 2 });
    }
}
