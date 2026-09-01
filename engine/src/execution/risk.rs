//! Execution-side safety gate for session, anchor, and market freshness.

use crate::strategy::{ClosedSession, Quantity, StaticAnchor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskAction {
    NoAction,
    AllowEntry { quantity: Quantity },
    Flatten,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskInput {
    pub now_ms: u64,
    pub requested_quantity: Quantity,
    pub position: Quantity,
    pub market_event_at_ms: u64,
    pub max_market_age_ms: u64,
    pub anchor: Option<StaticAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRiskGate {
    pub session: ClosedSession,
    pub max_position: Quantity,
}

impl SessionRiskGate {
    pub fn new(session: ClosedSession, max_position: Quantity) -> Option<Self> {
        if max_position.0 <= 0 {
            return None;
        }
        Some(Self { session, max_position })
    }

    pub fn evaluate(&self, input: RiskInput) -> RiskAction {
        if input.position.0 != 0 && self.session.must_flatten(input.now_ms) {
            return RiskAction::Flatten;
        }
        let absolute_position = input.position.0.checked_abs().unwrap_or(i64::MAX);
        if absolute_position > self.max_position.0 {
            return RiskAction::Halt;
        }
        if !self.session.entry_allowed(input.now_ms) {
            return RiskAction::NoAction;
        }
        if input.market_event_at_ms > input.now_ms
            || input.now_ms.saturating_sub(input.market_event_at_ms) > input.max_market_age_ms
        {
            return RiskAction::Halt;
        }
        if input.anchor.is_none_or(|anchor| !anchor.is_valid_at(input.now_ms)) {
            return RiskAction::Halt;
        }
        if input.requested_quantity.0 <= 0 {
            return RiskAction::NoAction;
        }
        let remaining = self.max_position.0 - absolute_position;
        if remaining <= 0 {
            return RiskAction::NoAction;
        }
        RiskAction::AllowEntry {
            quantity: Quantity(input.requested_quantity.0.min(remaining)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> SessionRiskGate {
        SessionRiskGate::new(
            ClosedSession::new(100, 900, 1_000).unwrap(),
            Quantity(1_000),
        ).unwrap()
    }

    fn input() -> RiskInput {
        RiskInput {
            now_ms: 200,
            requested_quantity: Quantity(250),
            position: Quantity(0),
            market_event_at_ms: 190,
            max_market_age_ms: 20,
            anchor: StaticAnchor::new(PriceTicks(100_000), 100, 500),
        }
    }

    #[test]
    fn allows_fresh_entry_inside_closed_session() {
        assert_eq!(
            gate().evaluate(input()),
            RiskAction::AllowEntry { quantity: Quantity(250) }
        );
    }

    #[test]
    fn forces_flatten_at_flatten_deadline() {
        let mut value = input();
        value.now_ms = 900;
        value.position = Quantity(100);
        assert_eq!(gate().evaluate(value), RiskAction::Flatten);
    }

    #[test]
    fn stale_market_or_anchor_halts_new_entries() {
        let mut value = input();
        value.market_event_at_ms = 100;
        assert_eq!(gate().evaluate(value), RiskAction::Halt);
        value.market_event_at_ms = 190;
        value.anchor = None;
        assert_eq!(gate().evaluate(value), RiskAction::Halt);
    }

    #[test]
    fn caps_entry_by_remaining_position_limit() {
        let mut value = input();
        value.position = Quantity(900);
        assert_eq!(
            gate().evaluate(value),
            RiskAction::AllowEntry { quantity: Quantity(100) }
        );
    }

    #[test]
    fn does_not_open_after_session_deadline() {
        let mut value = input();
        value.now_ms = 900;
        assert_eq!(gate().evaluate(value), RiskAction::NoAction);
    }
}
