use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FreshnessClass {
    Stream,
    Quote,
    Reference,
    Metadata,
    Funding,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessPolicy {
    pub class: FreshnessClass,
    pub refresh_after_ms: u64,
    pub fallback_after_ms: u64,
}

impl FreshnessPolicy {
    pub const fn new(class: FreshnessClass, refresh_after_ms: u64, fallback_after_ms: u64) -> Self {
        Self {
            class,
            refresh_after_ms,
            fallback_after_ms,
        }
    }

    pub fn validate(self, observed_at_ms: u64, now_ms: u64) -> FreshnessState {
        if observed_at_ms == 0 || observed_at_ms > now_ms {
            return FreshnessState::Invalid;
        }
        let age = now_ms.saturating_sub(observed_at_ms);
        if age <= self.refresh_after_ms {
            FreshnessState::Fresh
        } else if age <= self.fallback_after_ms {
            FreshnessState::Reusable
        } else {
            FreshnessState::Expired
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FreshnessState {
    Fresh,
    Reusable,
    Expired,
    Invalid,
}
