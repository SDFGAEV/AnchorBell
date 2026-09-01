#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalMarketEvent {
    BookTicker {
        event_time_ms: i64,
        symbol: String,
        bid_price_ticks: i64,
        bid_quantity: i64,
        ask_price_ticks: i64,
        ask_quantity: i64,
    },
    MarkPrice {
        event_time_ms: i64,
        symbol: String,
        mark_price_ticks: i64,
        index_price_ticks: i64,
    },
    Anchor {
        effective_at_ms: i64,
        symbol: String,
        close_price_ticks: i64,
    },
}

impl HistoricalMarketEvent {
    pub fn timestamp_ms(&self) -> i64 {
        match self {
            Self::BookTicker { event_time_ms, .. }
            | Self::MarkPrice { event_time_ms, .. } => *event_time_ms,
            Self::Anchor { effective_at_ms, .. } => *effective_at_ms,
        }
    }

    pub fn from_binance(event: crate::market::binance::BinanceMarketEvent) -> Self {
        match event {
            crate::market::binance::BinanceMarketEvent::BookTicker(ticker) => {
                Self::BookTicker {
                    event_time_ms: ticker.event_time_ms as i64,
                    symbol: ticker.symbol,
                    bid_price_ticks: ticker.bid_price.0,
                    bid_quantity: ticker.bid_quantity.0,
                    ask_price_ticks: ticker.ask_price.0,
                    ask_quantity: ticker.ask_quantity.0,
                }
            }
            crate::market::binance::BinanceMarketEvent::MarkPrice(mark) => Self::MarkPrice {
                event_time_ms: mark.event_time_ms as i64,
                symbol: mark.symbol,
                mark_price_ticks: mark.mark_price.0,
                index_price_ticks: mark.index_price.0,
            },
        }
    }
}

pub trait ReplaySink {
    fn on_event(&mut self, event: &HistoricalMarketEvent);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    OutOfOrder { previous_ms: i64, current_ms: i64 },
    InvalidMarketMessage(crate::market::binance::ParseError),
}

impl HistoricalMarketEvent {
    pub fn from_binance_json(
        payload: &[u8],
        price_scale: u32,
        quantity_scale: u32,
    ) -> Result<Self, ReplayError> {
        let event = crate::market::binance::parse_market_message(
            payload,
            price_scale,
            quantity_scale,
        )
        .map_err(ReplayError::InvalidMarketMessage)?;
        Ok(Self::from_binance(event))
    }
}

pub struct EventReplay {
    events: Vec<HistoricalMarketEvent>,
    cursor: usize,
}

pub struct ReplayBuilder {
    events: Vec<HistoricalMarketEvent>,
    last_timestamp_ms: Option<i64>,
}

impl ReplayBuilder {
    pub fn new() -> Self {
        Self { events: Vec::new(), last_timestamp_ms: None }
    }

    pub fn push(&mut self, event: HistoricalMarketEvent) -> Result<(), ReplayError> {
        if let Some(previous_ms) = self.last_timestamp_ms {
            let current_ms = event.timestamp_ms();
            if current_ms < previous_ms {
                return Err(ReplayError::OutOfOrder { previous_ms, current_ms });
            }
        }
        self.last_timestamp_ms = Some(event.timestamp_ms());
        self.events.push(event);
        Ok(())
    }

    pub fn push_binance_json(
        &mut self,
        payload: &[u8],
        price_scale: u32,
        quantity_scale: u32,
    ) -> Result<(), ReplayError> {
        self.push(HistoricalMarketEvent::from_binance_json(
            payload, price_scale, quantity_scale,
        )?)
    }

    pub fn finish(self) -> Result<EventReplay, ReplayError> {
        EventReplay::new(self.events)
    }
}

impl Default for ReplayBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventReplay {
    pub fn new(events: Vec<HistoricalMarketEvent>) -> Result<Self, ReplayError> {
        for pair in events.windows(2) {
            let previous_ms = pair[0].timestamp_ms();
            let current_ms = pair[1].timestamp_ms();
            if current_ms < previous_ms {
                return Err(ReplayError::OutOfOrder { previous_ms, current_ms });
            }
        }
        Ok(Self { events, cursor: 0 })
    }

    pub fn remaining(&self) -> usize {
        self.events.len().saturating_sub(self.cursor)
    }

    pub fn run<S: ReplaySink>(&mut self, sink: &mut S) {
        while self.cursor < self.events.len() {
            sink.on_event(&self.events[self.cursor]);
            self.cursor += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter(usize);
    impl ReplaySink for Counter {
        fn on_event(&mut self, _event: &HistoricalMarketEvent) {
            self.0 += 1;
        }
    }

    fn mark(time: i64) -> HistoricalMarketEvent {
        HistoricalMarketEvent::MarkPrice {
            event_time_ms: time,
            symbol: "BTCUSDT".to_string(),
            mark_price_ticks: 100,
            index_price_ticks: 100,
        }
    }

    #[test]
    fn converts_parsed_binance_events() {
        let raw = br#"{"e":"markPriceUpdate","E":1000,"s":"ABCUSDT","p":"12.3456","i":"12.3000","T":2000}"#;
        let parsed = crate::market::binance::parse_market_message(raw, 4, 2).unwrap();
        assert_eq!(
            HistoricalMarketEvent::from_binance(parsed).timestamp_ms(),
            1000
        );
    }

    #[test]
    fn decodes_a_recorded_binance_line() {
        let raw = br#"{"stream":"abcusdt@markPrice@1s","data":{"e":"markPriceUpdate","E":1000,"s":"ABCUSDT","p":"12.3456","i":"12.3000","T":2000}}"#;
        let event = HistoricalMarketEvent::from_binance_json(raw, 4, 2).unwrap();
        assert!(matches!(
            event,
            HistoricalMarketEvent::MarkPrice { event_time_ms: 1000, .. }
        ));
    }

    #[test]
    fn rejects_out_of_order_events() {
        assert!(matches!(
            EventReplay::new(vec![mark(2), mark(1)]),
            Err(ReplayError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn builder_rejects_late_timestamp_before_finish() {
        let mut builder = ReplayBuilder::new();
        builder.push(mark(10)).unwrap();
        assert!(matches!(
            builder.push(mark(9)),
            Err(ReplayError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn replays_each_event_once() {
        let mut replay = EventReplay::new(vec![mark(1), mark(2)]).unwrap();
        let mut counter = Counter(0);
        replay.run(&mut counter);
        assert_eq!(counter.0, 2);
        assert_eq!(replay.remaining(), 0);
    }
}
