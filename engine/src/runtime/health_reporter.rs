use super::{audit::AuditSink, RuntimeControlPlane};

pub struct RuntimeHealthReporter {
    control: RuntimeControlPlane,
    audit: AuditSink,
}

pub fn timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

impl RuntimeHealthReporter {
    pub fn new(default_audit_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            control: RuntimeControlPlane::new(),
            audit: AuditSink::from_environment(default_audit_path),
        }
    }

    pub async fn start(&mut self, systems: &[&str], observed_at_ms: u64) -> Result<(), String> {
        for system in systems {
            self.control
                .mark_ready(system, observed_at_ms)
                .map_err(|error| error.to_string())?;
        }
        self.flush(observed_at_ms).await
    }

    pub async fn ready(&mut self, system: &str, observed_at_ms: u64) -> Result<(), String> {
        self.control
            .mark_ready(system, observed_at_ms)
            .map_err(|error| error.to_string())?;
        self.flush(observed_at_ms).await
    }
    pub async fn degraded(
        &mut self,
        system: &str,
        observed_at_ms: u64,
        reason: &str,
    ) -> Result<(), String> {
        self.control
            .degrade(system, observed_at_ms, reason)
            .map_err(|error| error.to_string())?;
        self.flush(observed_at_ms).await
    }

    pub async fn halted(
        &mut self,
        system: &str,
        observed_at_ms: u64,
        reason: &str,
    ) -> Result<(), String> {
        self.control
            .mark_halted(system, observed_at_ms, reason)
            .map_err(|error| error.to_string())?;
        self.flush(observed_at_ms).await
    }

    async fn flush(&mut self, recorded_at_ms: u64) -> Result<(), String> {
        for transition in self.control.drain_health_events() {
            self.audit
                .append_health_transition(transition, recorded_at_ms)
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_start_and_halt_transitions() {
        let path = std::env::temp_dir().join(format!(
            "anchorbell-health-reporter-{}.jsonl",
            std::process::id()
        ));
        let mut reporter = RuntimeHealthReporter::new(path.clone());
        reporter.start(&["control.registry"], 1_000).await.unwrap();
        reporter
            .halted("control.registry", 2_000, "test_failure")
            .await
            .unwrap();
        let body = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(body.contains("system_health_transition"));
        assert!(body.contains("test_failure"));
        let _ = tokio::fs::remove_file(path).await;
    }
}
