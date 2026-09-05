use crate::platform::{HealthSnapshot, ReadinessReport, RegistryError, SystemRegistry};

/// The live control plane is the only admission surface between runtime
/// observations and new exchange risk. It is deliberately independent of
/// strategy and exchange clients so every live entrypoint can reuse it.
#[derive(Debug)]
pub struct LiveControlPlane {
    registry: SystemRegistry,
}

impl Default for LiveControlPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveControlPlane {
    pub fn new() -> Self {
        let mut registry = SystemRegistry::default();
        registry.bootstrap_health(now_ms());
        Self { registry }
    }

    pub fn registry(&self) -> &SystemRegistry {
        &self.registry
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<String> {
        self.registry.mark_stale_at(now_ms)
    }

    pub fn readiness(&mut self, now_ms: u64) -> ReadinessReport {
        self.registry.mark_stale_at(now_ms);
        self.registry
            .readiness_for_capability("execution.submit", now_ms)
    }

    pub fn execution_ready(&mut self, now_ms: u64) -> bool {
        self.readiness(now_ms).ready
    }
    /// Report the minimum live bootstrap contract after credentials, market
    /// metadata and the initial exchange reconciliation have succeeded.
    pub fn bootstrap_ready(&mut self, observed_at_ms: u64) -> Result<(), RegistryError> {
        for id in [
            "control.registry",
            "observability.telemetry",
            "decision.risk",
            "execution.gateway",
            "execution.lifecycle",
        ] {
            self.ready(id, observed_at_ms)?;
        }
        Ok(())
    }

    pub fn observe_market(&mut self, observed_at_ms: u64) -> Result<(), RegistryError> {
        self.ready("market.binance", observed_at_ms)?;
        self.ready("market.anchor", observed_at_ms)?;
        // Risk evaluation and the local gateway are event-loop participants.
        // This is a liveness heartbeat, never an exchange acknowledgement.
        self.ready("decision.risk", observed_at_ms)?;
        self.ready("execution.gateway", observed_at_ms)
    }

    pub fn observe_reference(&mut self, observed_at_ms: u64) -> Result<(), RegistryError> {
        self.ready("market.reference", observed_at_ms)
    }

    pub fn observe_user_data(&mut self, observed_at_ms: u64) -> Result<(), RegistryError> {
        self.ready("execution.lifecycle", observed_at_ms)
    }

    pub fn degrade(
        &mut self,
        id: &str,
        observed_at_ms: u64,
        reason: &str,
    ) -> Result<(), RegistryError> {
        let mut snapshot = HealthSnapshot::ready(id, observed_at_ms);
        snapshot.state = crate::platform::SystemState::Degraded;
        snapshot.diagnostics.push(reason.to_owned());
        self.registry.report_health(snapshot)
    }

    fn ready(&mut self, id: &str, observed_at_ms: u64) -> Result<(), RegistryError> {
        self.registry
            .report_health(HealthSnapshot::ready(id, observed_at_ms))
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_admission_is_closed_until_observations_arrive() {
        let mut plane = LiveControlPlane::new();
        assert!(!plane.execution_ready(1_000));
        plane.bootstrap_ready(1_000).unwrap();
        assert!(!plane.execution_ready(1_000));
        plane.observe_reference(1_000).unwrap();
        plane.observe_market(1_000).unwrap();
        assert!(plane.execution_ready(1_000));
    }

    #[test]
    fn silence_expires_market_admission_automatically() {
        let mut plane = LiveControlPlane::new();
        plane.bootstrap_ready(1_000).unwrap();
        plane.observe_reference(1_000).unwrap();
        plane.observe_market(1_000).unwrap();
        assert!(!plane.execution_ready(3_001));
    }
}
