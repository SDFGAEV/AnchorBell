use super::lifecycle_contract::{LifecycleContractError, UnifiedOrderEvent, UnifiedOrderState};

pub use super::lifecycle_contract::OrderLifecycleState as OrderState;

/// The single order-lifecycle projection used by simulation, replay, and live execution.
#[derive(Debug, Default)]
pub struct OrderManager {
    state: Option<UnifiedOrderState>,
}

impl OrderManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> Option<&UnifiedOrderState> {
        self.state.as_ref()
    }

    pub fn apply_event(&mut self, event: &UnifiedOrderEvent) -> Result<(), LifecycleContractError> {
        match self.state.as_mut() {
            Some(state) => state.apply(event),
            None => {
                self.state = Some(UnifiedOrderState::from_event(event)?);
                Ok(())
            }
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            matches!(
                state.state,
                OrderState::Filled
                    | OrderState::Canceled
                    | OrderState::Rejected
                    | OrderState::Unknown
            )
        })
    }
}
