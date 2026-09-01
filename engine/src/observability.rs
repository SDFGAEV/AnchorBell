use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AuditKind {
    MarketAccepted,
    MarketRejected,
    Decision,
    RiskRejected,
    OrderIntent,
    ExchangeAcknowledgement,
    Lifecycle,
    Recovery,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub kind: AuditKind,
    pub symbol: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
}

impl AuditRecord {
    pub fn new(sequence: u64, timestamp_ms: u64, kind: AuditKind) -> Self {
        Self {
            sequence,
            timestamp_ms,
            kind,
            symbol: None,
            reason: None,
            correlation_id: None,
        }
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }
}

#[derive(Debug, Default)]
pub struct AuditSequence {
    next: u64,
}

impl AuditSequence {
    pub fn next(&mut self) -> u64 {
        let value = self.next;
        self.next = self.next.saturating_add(1);
        value
    }

    pub fn record(&mut self, timestamp_ms: u64, kind: AuditKind) -> AuditRecord {
        AuditRecord::new(self.next(), timestamp_ms, kind)
    }
}

pub fn redact_secret(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    "[REDACTED]".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_sequence_is_monotonic() {
        let mut sequence = AuditSequence::default();
        assert_eq!(sequence.record(10, AuditKind::Decision).sequence, 0);
        assert_eq!(sequence.record(11, AuditKind::Lifecycle).sequence, 1);
    }

    #[test]
    fn secret_redaction_never_returns_input() {
        assert_eq!(redact_secret("api-secret"), "[REDACTED]");
        assert_eq!(redact_secret(""), "");
    }
}
