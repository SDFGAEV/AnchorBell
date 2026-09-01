use crate::backtest::{FillDecision, MakerQuote, TopOfBook};
use crate::execution::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyModel {
    pub market_to_decision_ms: u64,
    pub decision_to_exchange_ms: u64,
    pub cancel_to_exchange_ms: u64,
}

impl LatencyModel {
    pub const fn total_entry_ms(self) -> u64 {
        self.market_to_decision_ms + self.decision_to_exchange_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueModel {
    pub visible_ahead: i64,
    pub trade_through: i64,
}

impl QueueModel {
    pub fn fill_quantity(self, quote: MakerQuote, book: TopOfBook, aggressed_quantity: i64) -> i64 {
        if aggressed_quantity <= self.visible_ahead {
            return 0;
        }
        let available = aggressed_quantity - self.visible_ahead;
        let displayed = match quote.side {
            Side::Buy => book.bid_quantity,
            Side::Sell => book.ask_quantity,
        };
        available.min(displayed).min(quote.quantity).max(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostModel {
    pub maker_fee_numerator: i64,
    pub fee_denominator: i64,
    pub funding_ticks: i64,
}

impl CostModel {
    pub fn fee_ticks(self, notional_ticks: i64) -> i64 {
        if self.fee_denominator <= 0 {
            return i64::MAX;
        }
        notional_ticks.saturating_mul(self.maker_fee_numerator) / self.fee_denominator
    }

    pub fn net_ticks(self, gross_ticks: i64, notional_ticks: i64) -> i64 {
        gross_ticks
            .saturating_sub(self.fee_ticks(notional_ticks))
            .saturating_sub(self.funding_ticks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealisticFillModel {
    pub queue: QueueModel,
    pub latency: LatencyModel,
}

impl RealisticFillModel {
    pub fn evaluate_after_latency(
        self,
        quote: MakerQuote,
        book: TopOfBook,
        aggressed_quantity: i64,
    ) -> FillDecision {
        let quantity = self.queue.fill_quantity(quote, book, aggressed_quantity);
        if quantity <= 0 {
            FillDecision::NoFill
        } else {
            FillDecision::Fill { quantity }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_position_reduces_visible_fill() {
        let model = RealisticFillModel {
            queue: QueueModel {
                visible_ahead: 5,
                trade_through: 0,
            },
            latency: LatencyModel {
                market_to_decision_ms: 2,
                decision_to_exchange_ms: 3,
                cancel_to_exchange_ms: 4,
            },
        };
        let decision = model.evaluate_after_latency(
            MakerQuote {
                side: Side::Buy,
                price_ticks: 100,
                quantity: 10,
            },
            TopOfBook {
                bid_price_ticks: 100,
                ask_price_ticks: 101,
                bid_quantity: 7,
                ask_quantity: 8,
            },
            9,
        );
        assert_eq!(decision, FillDecision::Fill { quantity: 4 });
        assert_eq!(model.latency.total_entry_ms(), 5);
    }

    #[test]
    fn cost_model_subtracts_fee_and_funding() {
        let model = CostModel {
            maker_fee_numerator: 1,
            fee_denominator: 1000,
            funding_ticks: 2,
        };
        assert_eq!(model.net_ticks(20, 1000), 17);
    }
}
