use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum EngineEvent {
    MarketTick(MarketTick),
    OrderUpdate(OrderUpdate),
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

pub type EventSender = broadcast::Sender<EngineEvent>;
pub type EventReceiver = broadcast::Receiver<EngineEvent>;
