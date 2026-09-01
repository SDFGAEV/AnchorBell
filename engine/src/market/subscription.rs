use std::collections::BTreeSet;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionPlanError {
    Empty,
    InvalidShardCapacity,
    InvalidSubscription(SubscriptionError),
    DuplicateSymbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionPlan {
    shards: Vec<Vec<BinanceSubscription>>,
    total_streams: usize,
}

impl SubscriptionPlan {
    pub fn new(
        mut subscriptions: Vec<BinanceSubscription>,
        max_subscriptions_per_shard: usize,
    ) -> Result<Self, SubscriptionPlanError> {
        if subscriptions.is_empty() {
            return Err(SubscriptionPlanError::Empty);
        }
        if max_subscriptions_per_shard == 0 {
            return Err(SubscriptionPlanError::InvalidShardCapacity);
        }

        subscriptions.sort_by(|left, right| left.symbol.cmp(&right.symbol));
        let mut seen = BTreeSet::new();
        let mut total_streams = 0_usize;
        for subscription in &subscriptions {
            let stream_count = subscription
                .stream_names()
                .map_err(SubscriptionPlanError::InvalidSubscription)?
                .len();
            if !seen.insert(subscription.symbol.clone()) {
                return Err(SubscriptionPlanError::DuplicateSymbol(
                    subscription.symbol.clone(),
                ));
            }
            total_streams += stream_count;
        }

        let shards = subscriptions
            .chunks(max_subscriptions_per_shard)
            .map(|chunk| chunk.to_vec())
            .collect();
        Ok(Self {
            shards,
            total_streams,
        })
    }

    pub fn shards(&self) -> &[Vec<BinanceSubscription>] {
        &self.shards
    }

    pub fn total_streams(&self) -> usize {
        self.total_streams
    }

    pub fn total_subscriptions(&self) -> usize {
        self.shards.iter().map(Vec::len).sum()
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

    #[test]
    fn plans_large_universe_deterministically_into_bounded_shards() {
        let plan = SubscriptionPlan::new(
            vec![
                BinanceSubscription::new("CCCUSDT").unwrap(),
                BinanceSubscription::new("AAAUSDT").unwrap(),
                BinanceSubscription::new("BBBUSDT").unwrap(),
            ],
            2,
        )
        .unwrap();

        assert_eq!(plan.total_subscriptions(), 3);
        assert_eq!(plan.total_streams(), 6);
        assert_eq!(
            plan.shards()[0]
                .iter()
                .map(|subscription| subscription.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["aaausdt", "bbbusdt"]
        );
        assert_eq!(
            plan.shards()[1]
                .iter()
                .map(|subscription| subscription.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["cccusdt"]
        );
    }

    #[test]
    fn rejects_duplicate_symbols_before_network_creation() {
        let duplicate = vec![
            BinanceSubscription::new("ABCUSDT").unwrap(),
            BinanceSubscription::new("abcusdt").unwrap(),
        ];
        assert_eq!(
            SubscriptionPlan::new(duplicate, 10),
            Err(SubscriptionPlanError::DuplicateSymbol("abcusdt".into()))
        );
    }

    #[test]
    fn rejects_unbounded_plan_configuration() {
        assert_eq!(
            SubscriptionPlan::new(Vec::new(), 10),
            Err(SubscriptionPlanError::Empty)
        );
        assert_eq!(
            SubscriptionPlan::new(vec![BinanceSubscription::new("ABCUSDT").unwrap()], 0),
            Err(SubscriptionPlanError::InvalidShardCapacity)
        );
    }
}
