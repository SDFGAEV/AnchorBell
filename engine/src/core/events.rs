#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSequence(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTimestamp {
    pub exchange_ms: u64,
    pub received_ms: u64,
}

impl EventTimestamp {
    pub fn is_stale_at(self, now_ms: u64, max_age_ms: u64) -> bool {
        now_ms < self.received_ms || now_ms.saturating_sub(self.received_ms) > max_age_ms
    }
}
