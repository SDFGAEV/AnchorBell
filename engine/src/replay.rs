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
}

pub trait ReplaySink {
    fn on_event(&mut self, event: &HistoricalMarketEvent);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    OutOfOrder { previous_ms: i64, current_ms: i64 },
}

pub struct EventReplay {
    events: Vec<HistoricalMarketEvent>,
    cursor: usize,
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
    fn rejects_out_of_order_events() {
        assert!(matches!(
            EventReplay::new(vec![mark(2), mark(1)]),
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
