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

    #[inline]
    pub fn mid(&self) -> i64 {
        self.best_bid + self.best_ask.saturating_sub(self.best_bid) / 2
    }

    #[inline]
    pub fn spread(&self) -> i64 {
        if self.best_ask >= self.best_bid {
            self.best_ask - self.best_bid
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrderBook;

    #[test]
    fn midpoint_avoids_bid_ask_sum_overflow() {
        let mut book = OrderBook::default();
        book.update(i64::MAX - 2, i64::MAX);
        assert_eq!(book.mid(), i64::MAX - 1);
        assert_eq!(book.spread(), 2);
    }

    #[test]
    fn invalid_reversed_book_does_not_underflow_spread() {
        let mut book = OrderBook::default();
        book.update(i64::MAX, i64::MIN);
        assert_eq!(book.spread(), 0);
    }
}
