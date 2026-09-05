pub mod clock;
pub mod events;
pub mod ids;

pub use clock::{Clock, ManualClock, SystemClock};
pub use ids::{
    CausalityId, CheckpointId, EventId, IdentifierError, InstrumentId, OrderId, PolicyId, RunId,
};

#[derive(Debug, Clone, Copy)]
pub struct PricePoint {
    pub value: f64,
    pub timestamp_ns: u64,
}
