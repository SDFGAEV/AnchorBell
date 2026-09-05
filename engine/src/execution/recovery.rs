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
    EventGapDetected,
    ReconciliationClean,
    UnknownOrdersFound,
    PositionMismatch,
    ExternalStateAdopted,
    CancelComplete,
    FlattenComplete,
    OperatorHalt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryEpoch {
    pub number: u64,
    pub started_at_ms: u64,
    pub last_event_at_ms: u64,
    pub snapshot_at_ms: u64,
    pub event_gap_detected: bool,
    pub external_adjustments: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    InvalidTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryMachine {
    state: RecoveryState,
    next_epoch: u64,
    epoch: Option<RecoveryEpoch>,
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
            next_epoch: 0,
            epoch: None,
        }
    }

    pub fn state(&self) -> RecoveryState {
        self.state
    }

    pub fn epoch(&self) -> Option<RecoveryEpoch> {
        self.epoch
    }

    pub fn resume_gate_open(&self) -> bool {
        matches!(self.state, RecoveryState::Resumed | RecoveryState::Healthy)
            && self.epoch.is_none_or(|epoch| !epoch.event_gap_detected)
    }

    pub fn start_epoch(&mut self, started_at_ms: u64) -> RecoveryEpoch {
        self.next_epoch = self.next_epoch.saturating_add(1);
        let epoch = RecoveryEpoch {
            number: self.next_epoch,
            started_at_ms,
            last_event_at_ms: 0,
            snapshot_at_ms: 0,
            event_gap_detected: false,
            external_adjustments: 0,
        };
        self.epoch = Some(epoch);
        self.state = RecoveryState::RiskStopped;
        epoch
    }

    pub fn record_event(&mut self, event_at_ms: u64) {
        if let Some(epoch) = self.epoch.as_mut() {
            epoch.last_event_at_ms = epoch.last_event_at_ms.max(event_at_ms);
        }
    }

    pub fn record_snapshot(&mut self, snapshot_at_ms: u64) {
        if let Some(epoch) = self.epoch.as_mut() {
            epoch.snapshot_at_ms = epoch.snapshot_at_ms.max(snapshot_at_ms);
        }
    }

    pub fn record_gap(&mut self) {
        if let Some(epoch) = self.epoch.as_mut() {
            epoch.event_gap_detected = true;
        }
    }

    pub fn record_external_adjustment(&mut self) {
        if let Some(epoch) = self.epoch.as_mut() {
            epoch.external_adjustments = epoch.external_adjustments.saturating_add(1);
        }
    }

    pub fn mark_reconciled(&mut self) {
        if let Some(epoch) = self.epoch.as_mut() {
            epoch.event_gap_detected = false;
        }
    }

    pub fn apply(&mut self, event: RecoveryEvent) -> Result<(), RecoveryError> {
        self.state = match (self.state, event) {
            (RecoveryState::Healthy, RecoveryEvent::ConnectionLost)
            | (RecoveryState::Resumed, RecoveryEvent::ConnectionLost) => RecoveryState::RiskStopped,
            (RecoveryState::RiskStopped, RecoveryEvent::ReconnectSucceeded) => {
                RecoveryState::Synchronizing
            }
            (RecoveryState::Synchronizing, RecoveryEvent::SnapshotLoaded)
            | (RecoveryState::Synchronizing, RecoveryEvent::EventGapDetected)
            | (RecoveryState::Synchronizing, RecoveryEvent::ExternalStateAdopted) => {
                RecoveryState::Synchronizing
            }
            (RecoveryState::Synchronizing, RecoveryEvent::ReconciliationClean) => {
                RecoveryState::Resumed
            }
            (RecoveryState::Synchronizing, RecoveryEvent::UnknownOrdersFound) => {
                RecoveryState::CancelingUnknownOrders
            }
            // A remote position is authoritative after the snapshot has been
            // verified. It is not an operator-only failure anymore.
            (RecoveryState::Synchronizing, RecoveryEvent::PositionMismatch) => {
                RecoveryState::Synchronizing
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
        machine.start_epoch(100);
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::SnapshotLoaded).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        assert!(machine.resume_gate_open());
        assert_eq!(machine.state(), RecoveryState::Resumed);
    }
    #[test]
    fn authoritative_position_mismatch_does_not_require_manual_halt() {
        let mut machine = RecoveryMachine::new();
        machine.start_epoch(100);
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::PositionMismatch).unwrap();
        machine.apply(RecoveryEvent::ExternalStateAdopted).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        assert_eq!(machine.state(), RecoveryState::Resumed);
    }

    #[test]
    fn unknown_order_cancel_requires_a_second_reconciliation() {
        let mut machine = RecoveryMachine::new();
        machine.start_epoch(100);
        machine.apply(RecoveryEvent::ReconnectSucceeded).unwrap();
        machine.apply(RecoveryEvent::UnknownOrdersFound).unwrap();
        machine.apply(RecoveryEvent::CancelComplete).unwrap();
        assert_eq!(machine.state(), RecoveryState::Synchronizing);
        machine.apply(RecoveryEvent::SnapshotLoaded).unwrap();
        machine.apply(RecoveryEvent::ReconciliationClean).unwrap();
        assert_eq!(machine.state(), RecoveryState::Resumed);
    }

    #[test]
    fn reconnect_epoch_records_gap_and_external_adjustment() {
        let mut machine = RecoveryMachine::new();
        let epoch = machine.start_epoch(100);
        machine.record_gap();
        machine.record_external_adjustment();
        machine.record_snapshot(200);
        assert_eq!(epoch.number, 1);
        assert_eq!(machine.epoch().unwrap().external_adjustments, 1);
        assert_eq!(machine.epoch().unwrap().snapshot_at_ms, 200);
        assert!(!machine.resume_gate_open());
    }

    #[test]
    fn operator_halt_is_terminal() {
        let mut machine = RecoveryMachine::new();
        machine.apply(RecoveryEvent::OperatorHalt).unwrap();
        assert_eq!(machine.state(), RecoveryState::Halted);
        assert!(!machine.resume_gate_open());
    }
}
