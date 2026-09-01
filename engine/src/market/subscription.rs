#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceSubscription {
    pub symbol: String,
    pub book_ticker: bool,
    pub mark_price_1s: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    EmptySymbol,
    InvalidSymbol,
    NoStreams,
}

impl BinanceSubscription {
    pub fn new(symbol: impl Into<String>) -> Result<Self, SubscriptionError> {
        let symbol = symbol.into().to_ascii_lowercase();
        if symbol.is_empty() {
            return Err(SubscriptionError::EmptySymbol);
        }
        if !symbol
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(SubscriptionError::InvalidSymbol);
        }
        Ok(Self {
            symbol,
            book_ticker: true,
            mark_price_1s: true,
        })
    }

    pub fn stream_names(&self) -> Result<Vec<String>, SubscriptionError> {
        let mut streams = Vec::new();
        if self.book_ticker {
            streams.push(format!("{}@bookTicker", self.symbol));
        }
        if self.mark_price_1s {
            streams.push(format!("{}@markPrice@1s", self.symbol));
        }
        if streams.is_empty() {
            return Err(SubscriptionError::NoStreams);
        }
        Ok(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_symbol_and_builds_public_streams() {
        let subscription = BinanceSubscription::new("ABCUSDT").unwrap();
        assert_eq!(
            subscription.stream_names().unwrap(),
            vec!["abcusdt@bookTicker", "abcusdt@markPrice@1s"]
        );
    }

    #[test]
    fn rejects_invalid_symbol() {
        assert_eq!(
            BinanceSubscription::new("ABC-USDT"),
            Err(SubscriptionError::InvalidSymbol)
        );
    }
}
