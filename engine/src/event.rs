use crate::runtime::{DataQuality, EventEnvelope, EventSource};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum EngineEvent {
    MarketTick(EventEnvelope<MarketTick>),
    OrderUpdate(EventEnvelope<OrderUpdate>),
}

#[derive(Debug, Clone)]
pub struct MarketTick {
    pub symbol: u32,
    pub timestamp_ns: u64,
    pub bid: i64,
    pub ask: i64,
    pub index_price: i64,
    pub mark_price: i64,
}

#[derive(Debug, Clone)]
pub struct OrderUpdate {
    pub order_id: u64,
    pub filled_qty: i64,
}

impl MarketTick {
    pub fn enveloped(self, run_id: impl Into<String>, sequence: u64) -> EventEnvelope<Self> {
        EventEnvelope {
            event_id: format!("market-{}-{sequence}", self.symbol).into(),
            run_id: run_id.into().into(),
            causality_id: format!("market-cause-{sequence}").into(),
            source: EventSource::BinancePublic,
            observed_at_ms: self.timestamp_ns / 1_000_000,
            received_at_ms: self.timestamp_ns / 1_000_000,
            sequence,
            state_version: sequence,
            quality: DataQuality::Trusted,
            payload: self,
        }
    }
}

pub type EventSender = broadcast::Sender<EngineEvent>;
pub type EventReceiver = broadcast::Receiver<EngineEvent>;
