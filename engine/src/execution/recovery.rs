#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    Healthy,
    RiskStopped,
    Synchronizing,
    CancelingUnknownOrders,
    Flattening,
    Resumed,
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryEvent {
    ConnectionLost,
    ReconnectSucceeded,
    SnapshotLoaded,
    ReconciliationClean,
    UnknownOrdersFound,
    PositionMismatch,
    CancelComplete,
    FlattenComplete,
    OperatorHalt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    InvalidTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryMachine {
    state: RecoveryState,
}

impl Default for RecoveryMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl RecoveryMachine {
    pub fn new() -> Self {
        Self {
            state: RecoveryState::Healthy,
        }
    }

    pub fn state(&self) -> RecoveryState {
        self.state
    }

    pub fn apply(&mut self, event: RecoveryEvent) -> Result<(), RecoveryError> {
        self.state = match (self.state, event) {
            (RecoveryState::Healthy, RecoveryEvent::ConnectionLost)
            | (RecoveryState::Resumed, RecoveryEvent::ConnectionLost) => RecoveryState::RiskStopped,
            (RecoveryState::RiskStopped, RecoveryEvent::ReconnectSucceeded) => {
                RecoveryState::Synchronizing
            }
            (RecoveryState::Synchronizing, RecoveryEvent::SnapshotLoaded) => {
                RecoveryState::Synchronizing
            }
            (RecoveryState::Synchronizing, RecoveryEvent::ReconciliationClean) => {
                RecoveryState::Resumed
            }
            (RecoveryState::Synchronizing, RecoveryEvent::UnknownOrdersFound) => {
                RecoveryState::CancelingUnknownOrders
            }
            (RecoveryState::Synchronizing, RecoveryEvent::PositionMismatch) => {
                RecoveryState::Flattening
            }
            (RecoveryState::CancelingUnknownOrders, RecoveryEvent::CancelComplete) => {
                RecoveryState::Synchronizing
            }
            (RecoveryState::Flattening, RecoveryEvent::FlattenComplete) => RecoveryState::Halted,
            (_, RecoveryEvent::OperatorHalt) => RecoveryState::Halted,
            _ => return Err(RecoveryError::InvalidTransition),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_reconciles_before_resuming() {
        let mut machine = RecoveryMachine::new();
        machine.apply(RecoveryEvent::ConnectionLost).unwrap();
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::SnapshotLoaded).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        assert_eq!(machine.state(), RecoveryState::Resumed);
    }

    #[test]
    fn position_mismatch_requires_flatten_and_halts() {
        let mut machine = RecoveryMachine::new();
        machine.apply(RecoveryEvent::ConnectionLost).unwrap();
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::PositionMismatch).unwrap();
        machine.apply(RecoveryEvent::FlattenComplete).unwrap();
        assert_eq!(machine.state(), RecoveryState::Halted);
    }

    #[test]
    fn unknown_order_cancel_requires_a_second_reconciliation() {
        let mut machine = RecoveryMachine::new();
        machine.apply(RecoveryEvent::ConnectionLost).unwrap();
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::UnknownOrdersFound).unwrap();
        machine.apply(RecoveryEvent::CancelComplete).unwrap();
        assert_eq!(machine.state(), RecoveryState::Synchronizing);
        machine.apply(RecoveryEvent::SnapshotLoaded).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        assert_eq!(machine.state(), RecoveryState::Resumed);
    }

    #[test]
    fn resumed_connection_loss_stops_risk_before_recovery() {
        let mut machine = RecoveryMachine::new();
        machine.apply(RecoveryEvent::ConnectionLost).unwrap();
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        machine.apply(RecoveryEvent::ConnectionLost).unwrap();
        assert_eq!(machine.state(), RecoveryState::RiskStopped);
    }

    #[test]
    fn invalid_resume_path_is_rejected() {
        let mut machine = RecoveryMachine::new();
        assert_eq!(
            machine.apply(RecoveryEvent::ReconciliationClean),
            Err(RecoveryError::InvalidTransition)
        );
    }
}
