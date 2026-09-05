use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventSource {
    BinancePublic,
    BinanceUser,
    Simulation,
    Replay,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataQuality {
    Trusted,
    Degraded,
    GapDetected,
    Stale,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventEnvelope<T> {
    pub event_id: String,
    pub run_id: String,
    pub causality_id: String,
    pub source: EventSource,
    pub observed_at_ms: u64,
    pub received_at_ms: u64,
    pub sequence: u64,
    pub state_version: u64,
    pub quality: DataQuality,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn validate(&self) -> Result<(), EnvelopeError> {
        if self.event_id.trim().is_empty() || self.run_id.trim().is_empty() {
            return Err(EnvelopeError::MissingIdentity);
        }
        if self.causality_id.trim().is_empty() {
            return Err(EnvelopeError::MissingCausality);
        }
        if self.received_at_ms < self.observed_at_ms {
            return Err(EnvelopeError::ClockRegression);
        }
        if matches!(self.quality, DataQuality::Invalid) {
            return Err(EnvelopeError::InvalidQuality);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvelopeError {
    #[error("event identity is empty")]
    MissingIdentity,
    #[error("event causality identity is empty")]
    MissingCausality,
    #[error("received timestamp precedes observed timestamp")]
    ClockRegression,
    #[error("invalid event cannot enter the kernel")]
    InvalidQuality,
    #[error("event sequence or state version regressed")]
    SequenceRegression,
    #[error("event id was already committed")]
    DuplicateEvent,
}

#[derive(Debug, Default)]
pub struct CausalLedger {
    last_sequence: u64,
    last_state_version: u64,
    committed: BTreeSet<String>,
}

impl CausalLedger {
    pub fn commit<T>(&mut self, event: &EventEnvelope<T>) -> Result<(), EnvelopeError> {
        event.validate()?;
        if !self.committed.insert(event.event_id.clone()) {
            return Err(EnvelopeError::DuplicateEvent);
        }
        if event.sequence < self.last_sequence || event.state_version < self.last_state_version {
            return Err(EnvelopeError::SequenceRegression);
        }
        self.last_sequence = event.sequence;
        self.last_state_version = event.state_version;
        Ok(())
    }

    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub fn last_state_version(&self) -> u64 {
        self.last_state_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(sequence: u64) -> EventEnvelope<&'static str> {
        EventEnvelope {
            event_id: format!("event-{sequence}"),
            run_id: "run-1".into(),
            causality_id: "cause-1".into(),
            source: EventSource::Simulation,
            observed_at_ms: 10,
            received_at_ms: 11,
            sequence,
            state_version: sequence,
            quality: DataQuality::Trusted,
            payload: "tick",
        }
    }

    #[test]
    fn ledger_rejects_duplicate_and_regressed_events() {
        let mut ledger = CausalLedger::default();
        ledger.commit(&event(2)).unwrap();
        assert_eq!(ledger.commit(&event(2)), Err(EnvelopeError::DuplicateEvent));
        assert_eq!(
            ledger.commit(&event(1)),
            Err(EnvelopeError::SequenceRegression)
        );
    }
}
