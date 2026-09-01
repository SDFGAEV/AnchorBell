//! Static close anchors and closed-session admission rules.

use super::{PriceTicks, Quantity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticAnchor {
    pub price: PriceTicks,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}

impl StaticAnchor {
    pub fn new(
        price: PriceTicks,
        observed_at_ms: u64,
        valid_until_ms: u64,
    ) -> Option<Self> {
        if price.0 <= 0 || valid_until_ms <= observed_at_ms {
            return None;
        }
        Some(Self { price, observed_at_ms, valid_until_ms })
    }

    pub fn is_valid_at(&self, now_ms: u64) -> bool {
        now_ms >= self.observed_at_ms && now_ms < self.valid_until_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedSession {
    pub closed_at_ms: u64,
    pub flatten_at_ms: u64,
    pub reopen_at_ms: u64,
}

impl ClosedSession {
    pub fn new(
        closed_at_ms: u64,
        flatten_at_ms: u64,
        reopen_at_ms: u64,
    ) -> Option<Self> {
        if closed_at_ms < flatten_at_ms && flatten_at_ms < reopen_at_ms {
            Some(Self { closed_at_ms, flatten_at_ms, reopen_at_ms })
        } else {
            None
        }
    }

    pub fn entry_allowed(&self, now_ms: u64) -> bool {
        now_ms >= self.closed_at_ms && now_ms < self.flatten_at_ms
    }

    pub fn must_flatten(&self, now_ms: u64) -> bool {
        now_ms >= self.flatten_at_ms
    }

    pub fn is_closed_session(&self, now_ms: u64) -> bool {
        now_ms >= self.closed_at_ms && now_ms < self.reopen_at_ms
    }

    pub fn entry_quantity(
        &self,
        now_ms: u64,
        anchor: Option<StaticAnchor>,
        requested: Quantity,
    ) -> Quantity {
        if !self.entry_allowed(now_ms)
            || requested.0 <= 0
            || anchor.is_none_or(|a| !a.is_valid_at(now_ms))
        {
            Quantity(0)
        } else {
            requested
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_anchor_is_valid_only_inside_its_interval() {
        let anchor = StaticAnchor::new(PriceTicks(100_000), 10, 20).unwrap();
        assert!(!anchor.is_valid_at(9));
        assert!(anchor.is_valid_at(10));
        assert!(!anchor.is_valid_at(20));
    }

    #[test]
    fn rejects_invalid_anchor_interval() {
        assert!(StaticAnchor::new(PriceTicks(0), 10, 20).is_none());
        assert!(StaticAnchor::new(PriceTicks(100), 20, 20).is_none());
    }

    #[test]
    fn entries_stop_before_reopen() {
        let session = ClosedSession::new(100, 900, 1_000).unwrap();
        assert!(!session.entry_allowed(99));
        assert!(session.entry_allowed(100));
        assert!(session.entry_allowed(899));
        assert!(!session.entry_allowed(900));
        assert!(session.must_flatten(900));
        assert!(session.is_closed_session(999));
        assert!(!session.is_closed_session(1_000));
    }

    #[test]
    fn entry_quantity_requires_valid_anchor_and_window() {
        let session = ClosedSession::new(100, 900, 1_000).unwrap();
        let anchor = StaticAnchor::new(PriceTicks(100_000), 90, 500).unwrap();
        assert_eq!(session.entry_quantity(200, Some(anchor), Quantity(5)), Quantity(5));
        assert_eq!(session.entry_quantity(600, Some(anchor), Quantity(5)), Quantity(0));
        assert_eq!(session.entry_quantity(200, None, Quantity(5)), Quantity(0));
        assert_eq!(session.entry_quantity(900, Some(anchor), Quantity(5)), Quantity(0));
    }
}
