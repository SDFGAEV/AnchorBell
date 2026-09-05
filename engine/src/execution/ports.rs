use crate::{
    execution::{
        lifecycle_contract::{UnifiedOrderEvent, UnifiedOrderState},
        OrderIntent,
    },
    market::StandardMarketEvent,
    runtime::EventEnvelope,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct VenueId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub venue: VenueId,
    pub account: AccountId,
    pub observed_at_ms: u64,
    pub positions: Vec<UnifiedOrderState>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdapterError {
    #[error("adapter identity is empty")]
    EmptyIdentity,
    #[error("adapter rejected an invalid event")]
    InvalidEvent,
    #[error("adapter operation is unavailable")]
    Unavailable,
}

pub trait MarketDataAdapter {
    fn venue(&self) -> &VenueId;
    fn normalize(
        &self,
        event: EventEnvelope<StandardMarketEvent>,
    ) -> Result<EventEnvelope<StandardMarketEvent>, AdapterError>;
}

pub trait ExecutionAdapter {
    fn venue(&self) -> &VenueId;
    fn submit(&self, intent: &OrderIntent) -> Result<UnifiedOrderEvent, AdapterError>;
    fn cancel(&self, client_order_id: &str) -> Result<UnifiedOrderEvent, AdapterError>;
}

pub trait AccountAuthority {
    fn venue(&self) -> &VenueId;
    fn snapshot(&self) -> Result<AccountSnapshot, AdapterError>;
}

pub struct ReadOnlyMarketAdapter {
    venue: VenueId,
}

impl ReadOnlyMarketAdapter {
    pub fn new(venue: impl Into<String>) -> Result<Self, AdapterError> {
        let venue = venue.into();
        if venue.trim().is_empty() {
            return Err(AdapterError::EmptyIdentity);
        }
        Ok(Self {
            venue: VenueId(venue),
        })
    }
}

impl MarketDataAdapter for ReadOnlyMarketAdapter {
    fn venue(&self) -> &VenueId {
        &self.venue
    }
    fn normalize(
        &self,
        event: EventEnvelope<StandardMarketEvent>,
    ) -> Result<EventEnvelope<StandardMarketEvent>, AdapterError> {
        event.validate().map_err(|_| AdapterError::InvalidEvent)?;
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_ports_preserve_standard_market_identity() {
        let adapter = ReadOnlyMarketAdapter::new("binance").unwrap();
        assert_eq!(adapter.venue().0, "binance");
        assert!(ReadOnlyMarketAdapter::new("").is_err());
    }
}
