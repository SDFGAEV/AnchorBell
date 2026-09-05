use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionMode {
    Simulation,
    Live,
    Replay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrderLifecycleState {
    Intent,
    Submitted,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelPending,
    Canceled,
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LifecycleAuthority {
    Local,
    Exchange,
    Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnifiedOrderEvent {
    pub event_id: String,
    pub run_id: String,
    pub client_order_id: String,
    pub symbol: String,
    pub mode: ExecutionMode,
    pub state: OrderLifecycleState,
    pub authority: LifecycleAuthority,
    pub event_at_ms: u64,
    pub cumulative_quantity: i64,
    pub average_price_ticks: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnifiedOrderState {
    pub client_order_id: String,
    pub symbol: String,
    pub state: OrderLifecycleState,
    pub cumulative_quantity: i64,
    pub average_price_ticks: Option<i64>,
    pub last_event_at_ms: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LifecycleContractError {
    #[error("order identity is empty")]
    EmptyIdentity,
    #[error("order quantity cannot be negative")]
    NegativeQuantity,
    #[error("order lifecycle regressed")]
    StateRegression,
    #[error("order event timestamp regressed")]
    TimestampRegression,
}

impl UnifiedOrderEvent {
    pub fn validate(&self) -> Result<(), LifecycleContractError> {
        if self.run_id.trim().is_empty()
            || self.client_order_id.trim().is_empty()
            || self.symbol.trim().is_empty()
        {
            return Err(LifecycleContractError::EmptyIdentity);
        }
        if self.cumulative_quantity < 0 {
            return Err(LifecycleContractError::NegativeQuantity);
        }
        Ok(())
    }
}

impl UnifiedOrderState {
    pub fn from_event(event: &UnifiedOrderEvent) -> Result<Self, LifecycleContractError> {
        event.validate()?;
        Ok(Self {
            client_order_id: event.client_order_id.clone(),
            symbol: event.symbol.clone(),
            state: event.state,
            cumulative_quantity: event.cumulative_quantity,
            average_price_ticks: event.average_price_ticks,
            last_event_at_ms: event.event_at_ms,
        })
    }

    pub fn apply(&mut self, event: &UnifiedOrderEvent) -> Result<(), LifecycleContractError> {
        event.validate()?;
        if event.client_order_id != self.client_order_id || event.symbol != self.symbol {
            return Err(LifecycleContractError::EmptyIdentity);
        }
        if event.event_at_ms < self.last_event_at_ms {
            return Err(LifecycleContractError::TimestampRegression);
        }
        if event.state < self.state && event.state != OrderLifecycleState::Unknown {
            return Err(LifecycleContractError::StateRegression);
        }
        if event.cumulative_quantity < self.cumulative_quantity {
            return Err(LifecycleContractError::NegativeQuantity);
        }
        self.state = event.state;
        self.cumulative_quantity = event.cumulative_quantity;
        self.average_price_ticks = event.average_price_ticks;
        self.last_event_at_ms = event.event_at_ms;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(state: OrderLifecycleState, at: u64, qty: i64) -> UnifiedOrderEvent {
        UnifiedOrderEvent {
            event_id: format!("event-{at}"),
            run_id: "run".into(),
            client_order_id: "client-1".into(),
            symbol: "BTCUSDT".into(),
            mode: ExecutionMode::Simulation,
            state,
            authority: LifecycleAuthority::Local,
            event_at_ms: at,
            cumulative_quantity: qty,
            average_price_ticks: Some(100),
        }
    }
    #[test]
    fn simulation_and_live_share_monotonic_lifecycle() {
        let mut state =
            UnifiedOrderState::from_event(&event(OrderLifecycleState::Submitted, 1, 0)).unwrap();
        state
            .apply(&event(OrderLifecycleState::Filled, 2, 10))
            .unwrap();
        assert_eq!(state.state, OrderLifecycleState::Filled);
        assert_eq!(
            state.apply(&event(OrderLifecycleState::Accepted, 3, 10)),
            Err(LifecycleContractError::StateRegression)
        );
    }
}
