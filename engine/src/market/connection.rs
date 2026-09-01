use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Draining,
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionAction {
    Connect,
    SendPing,
    Reconnect,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            max_attempts: None,
        }
    }
}

impl ReconnectPolicy {
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let multiplier = 1u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
        let delay = self.initial_delay.saturating_mul(multiplier);
        delay.min(self.max_delay)
    }

    pub fn allows_attempt(&self, attempt: u32) -> bool {
        self.max_attempts.is_none_or(|limit| attempt < limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionSupervisor {
    state: ConnectionState,
    attempts: u32,
    policy: ReconnectPolicy,
}

impl ConnectionSupervisor {
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            attempts: 0,
            policy,
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn on_connecting(&mut self) -> ConnectionAction {
        self.state = ConnectionState::Connecting;
        ConnectionAction::Connect
    }

    pub fn on_connected(&mut self) {
        self.state = ConnectionState::Connected;
        self.attempts = 0;
    }

    pub fn on_disconnect(&mut self) -> Option<(ConnectionAction, Duration)> {
        if self.state == ConnectionState::Halted {
            return Some((ConnectionAction::Stop, Duration::ZERO));
        }
        self.state = ConnectionState::Disconnected;
        if !self.policy.allows_attempt(self.attempts) {
            self.state = ConnectionState::Halted;
            return Some((ConnectionAction::Stop, Duration::ZERO));
        }
        let delay = self.policy.delay_for(self.attempts);
        self.attempts = self.attempts.saturating_add(1);
        Some((ConnectionAction::Reconnect, delay))
    }

    pub fn begin_draining(&mut self) {
        self.state = ConnectionState::Draining;
    }

    pub fn halt(&mut self) {
        self.state = ConnectionState::Halted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_backoff_is_bounded() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.delay_for(0), Duration::from_millis(50));
        assert_eq!(policy.delay_for(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for(10), Duration::from_secs(5));
    }

    #[test]
    fn successful_connection_resets_attempts() {
        let mut supervisor = ConnectionSupervisor::new(ReconnectPolicy::default());
        supervisor.on_connecting();
        supervisor.on_disconnect();
        assert_eq!(supervisor.attempts(), 1);
        supervisor.on_connected();
        assert_eq!(supervisor.attempts(), 0);
    }

    #[test]
    fn finite_policy_halts_after_limit() {
        let policy = ReconnectPolicy {
            max_attempts: Some(1),
            ..ReconnectPolicy::default()
        };
        let mut supervisor = ConnectionSupervisor::new(policy);
        assert_eq!(
            supervisor.on_disconnect(),
            Some((ConnectionAction::Reconnect, Duration::from_millis(50)))
        );
        assert_eq!(
            supervisor.on_disconnect(),
            Some((ConnectionAction::Stop, Duration::ZERO))
        );
        assert_eq!(supervisor.state(), ConnectionState::Halted);
    }
}
