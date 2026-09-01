#[path = "market/binance.rs"]
pub mod binance;
#[path = "market/connection.rs"]
pub mod connection;
#[path = "market/live.rs"]
pub mod live;
#[path = "market/recorder.rs"]
pub mod recorder;
#[path = "market/subscription.rs"]
pub mod subscription;
pub use connection::{ConnectionAction, ConnectionState, ConnectionSupervisor, ReconnectPolicy};
pub use live::{BinanceMarketConfig, BinanceMarketStream, MarketStreamError};
pub use subscription::{
    BinanceSubscription, SubscriptionError, SubscriptionPlan, SubscriptionPlanError,
};

#[derive(Debug)]
pub struct MarketState {
    pub symbol: String,
    pub index_price: f64,
    pub bid: f64,
    pub ask: f64,
}

impl MarketState {
    pub fn mid(&self) -> f64 {
        (self.bid + self.ask) * 0.5
    }
}
