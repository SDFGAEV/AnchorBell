use std::{
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tokio::io::AsyncWriteExt;

use super::control_plane::HealthTransition;

pub const RUNTIME_AUDIT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Serialize)]
pub struct RuntimeAuditEvent {
    pub schema_version: u16,
    pub sequence: u64,
    pub event: &'static str,
    pub recorded_at_ms: u64,
    pub transition: HealthTransition,
}

#[derive(Debug)]
pub struct AuditSink {
    path: PathBuf,
    next_sequence: u64,
}

impl AuditSink {
    pub fn from_environment(default_path: impl Into<PathBuf>) -> Self {
        let path = std::env::var_os("ANCHORBELL_AUDIT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_path.into());
        let next_sequence = std::fs::read_to_string(&path)
            .ok()
            .and_then(|body| {
                body.lines().rev().find_map(|line| {
                    serde_json::from_str::<serde_json::Value>(line)
                        .ok()?
                        .get("sequence")?
                        .as_u64()
                        .and_then(|sequence| sequence.checked_add(1))
                })
            })
            .unwrap_or(0);
        Self {
            path,
            next_sequence,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub async fn append_health_transition(
        &mut self,
        transition: HealthTransition,
        recorded_at_ms: u64,
    ) -> Result<(), io::Error> {
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        let event = RuntimeAuditEvent {
            schema_version: RUNTIME_AUDIT_SCHEMA_VERSION,
            sequence: self.next_sequence,
            event: "system_health_transition",
            recorded_at_ms,
            transition,
        };
        let mut bytes =
            serde_json::to_vec(&event).map_err(|error| io::Error::other(error.to_string()))?;
        bytes.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(&bytes).await?;
        file.flush().await?;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::SystemState;
    #[tokio::test]
    async fn audit_sink_writes_versioned_transition() {
        let path = std::env::temp_dir().join(format!(
            "anchorbell-runtime-audit-{}.jsonl",
            std::process::id()
        ));
        let mut sink = AuditSink::from_environment(path.clone());
        sink.append_health_transition(
            HealthTransition {
                system_id: "control.registry".to_owned(),
                from: SystemState::Discovered,
                to: SystemState::Ready,
                stale: false,
                observed_at_ms: 10,
                diagnostics: Vec::new(),
            },
            11,
        )
        .await
        .unwrap();
        let mut restarted = AuditSink::from_environment(path.clone());
        restarted
            .append_health_transition(
                HealthTransition {
                    system_id: "control.registry".to_owned(),
                    from: SystemState::Ready,
                    to: SystemState::Halted,
                    stale: false,
                    observed_at_ms: 12,
                    diagnostics: vec!["test".to_owned()],
                },
                13,
            )
            .await
            .unwrap();
        let body = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(body.contains(r#""schema_version":1"#));
        assert!(body.contains(r#""system_id":"control.registry""#));
        let sequences = body
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["sequence"]
                    .as_u64()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(sequences, vec![0, 1]);
        let _ = tokio::fs::remove_file(path).await;
    }
}
