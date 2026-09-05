use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const RUN_REGISTRY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunMode {
    Simulation,
    Live,
    Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunStatus {
    Created,
    Starting,
    Running,
    Degraded,
    Recovering,
    Completed,
    Failed,
    Halted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSpec {
    pub schema_version: u16,
    pub run_id: String,
    pub mode: RunMode,
    pub policy_id: String,
    pub capital_currency: String,
    pub capital_minor_units: i64,
    pub universe: String,
    pub strategies: Vec<String>,
    pub ablations: Vec<String>,
    pub checkpoint_interval_ms: u64,
    pub max_stale_ms: u64,
    pub auto_restart: bool,
    pub build_identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRecord {
    pub spec: RunSpec,
    pub status: RunStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub restart_count: u32,
    pub last_heartbeat_ms: u64,
    pub last_checkpoint: Option<String>,
    pub last_error: Option<String>,
}
#[derive(Debug, thiserror::Error)]
pub enum RunRegistryError {
    #[error("run registry I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("run registry encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
    #[error("invalid run specification: {0}")]
    InvalidSpec(&'static str),
    #[error("run record does not exist")]
    MissingRecord,
}

impl RunSpec {
    pub fn validate(&self) -> Result<(), RunRegistryError> {
        if self.schema_version != RUN_REGISTRY_SCHEMA_VERSION {
            return Err(RunRegistryError::InvalidSpec("unsupported schema"));
        }
        if self.run_id.trim().is_empty() || self.policy_id.trim().is_empty() {
            return Err(RunRegistryError::InvalidSpec(
                "run_id and policy_id are required",
            ));
        }
        if self.run_id.contains(['\\', '/', ':']) {
            return Err(RunRegistryError::InvalidSpec(
                "run_id contains a path separator",
            ));
        }
        if self.capital_minor_units <= 0 || self.capital_currency.trim().is_empty() {
            return Err(RunRegistryError::InvalidSpec("capital must be positive"));
        }
        if self.universe.trim().is_empty() || self.strategies.is_empty() {
            return Err(RunRegistryError::InvalidSpec(
                "universe and strategies are required",
            ));
        }
        if self.checkpoint_interval_ms == 0 || self.max_stale_ms == 0 {
            return Err(RunRegistryError::InvalidSpec(
                "runtime intervals must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RunHeartbeat {
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl RunHeartbeat {
    pub fn abort(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Drop for RunHeartbeat {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunRegistry {
    root: PathBuf,
}

impl RunRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn record_path(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id).join("run.json")
    }

    pub fn create(&self, spec: RunSpec, now_ms: u64) -> Result<RunRecord, RunRegistryError> {
        spec.validate()?;
        let record = RunRecord {
            spec,
            status: RunStatus::Created,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            restart_count: 0,
            last_heartbeat_ms: now_ms,
            last_checkpoint: None,
            last_error: None,
        };
        self.write(&record)?;
        Ok(record)
    }

    pub fn read(&self, run_id: &str) -> Result<RunRecord, RunRegistryError> {
        let path = self.record_path(run_id);
        if !path.exists() {
            return Err(RunRegistryError::MissingRecord);
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    pub fn transition(
        &self,
        run_id: &str,
        status: RunStatus,
        now_ms: u64,
    ) -> Result<RunRecord, RunRegistryError> {
        let mut record = self.read(run_id)?;
        record.status = status;
        record.updated_at_ms = now_ms;
        if matches!(record.status, RunStatus::Recovering) {
            record.restart_count = record.restart_count.saturating_add(1);
        }
        self.write(&record)?;
        Ok(record)
    }

    pub fn heartbeat(&self, run_id: &str, now_ms: u64) -> Result<RunRecord, RunRegistryError> {
        let mut record = self.read(run_id)?;
        record.last_heartbeat_ms = now_ms;
        record.updated_at_ms = now_ms;
        self.write(&record)?;
        Ok(record)
    }

    pub fn spawn_heartbeat(&self, run_id: impl Into<String>, interval_ms: u64) -> RunHeartbeat {
        let registry = self.clone();
        let run_id = run_id.into();
        RunHeartbeat {
            handle: Some(tokio::spawn(async move {
                let interval_ms = interval_ms.max(100);
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
                    if registry.heartbeat(&run_id, unix_ms()).is_err() {
                        break;
                    }
                }
            })),
        }
    }

    pub fn recover_stale(
        &self,
        run_id: &str,
        now_ms: u64,
    ) -> Result<Option<RunRecord>, RunRegistryError> {
        let mut record = self.read(run_id)?;
        let stale = now_ms.saturating_sub(record.last_heartbeat_ms) > record.spec.max_stale_ms;
        if !stale || !matches!(record.status, RunStatus::Running | RunStatus::Degraded) {
            return Ok(None);
        }
        if !record.spec.auto_restart {
            record.status = RunStatus::Halted;
        } else {
            record.status = RunStatus::Recovering;
            record.restart_count = record.restart_count.saturating_add(1);
        }
        record.last_error = Some("heartbeat expired; runtime recovery required".into());
        record.updated_at_ms = now_ms;
        self.write(&record)?;
        Ok(Some(record))
    }

    pub fn checkpoint(
        &self,
        run_id: &str,
        checkpoint: impl Into<String>,
        now_ms: u64,
    ) -> Result<RunRecord, RunRegistryError> {
        let mut record = self.read(run_id)?;
        record.last_checkpoint = Some(checkpoint.into());
        record.updated_at_ms = now_ms;
        self.write(&record)?;
        Ok(record)
    }

    fn write(&self, record: &RunRecord) -> Result<(), RunRegistryError> {
        record.spec.validate()?;
        let path = self.record_path(&record.spec.run_id);
        fs::create_dir_all(path.parent().expect("record has parent"))?;
        let tmp = path.with_extension(format!("{}.tmp", now_ms()));
        fs::write(&tmp, serde_json::to_vec_pretty(record)?)?;
        replace_file(&tmp, &path)?;
        Ok(())
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_ms() -> u64 {
    unix_ms()
}

fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if target.exists() => {
            fs::remove_file(target)?;
            fs::rename(source, target).map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_persists_status_heartbeat_and_recovery_count() {
        let root = std::env::temp_dir().join(format!("anchorbell-run-{}", now_ms()));
        let registry = RunRegistry::new(&root);
        let spec = RunSpec {
            schema_version: RUN_REGISTRY_SCHEMA_VERSION,
            run_id: "run-1".into(),
            mode: RunMode::Simulation,
            policy_id: "policy-1".into(),
            capital_currency: "USDT".into(),
            capital_minor_units: 1_000,
            universe: "frozen-close-ah".into(),
            strategies: vec!["m1".into()],
            ablations: vec![],
            checkpoint_interval_ms: 1_000,
            max_stale_ms: 5_000,
            auto_restart: true,
            build_identity: "test".into(),
        };
        registry.create(spec, 10).unwrap();
        registry
            .transition("run-1", RunStatus::Recovering, 20)
            .unwrap();
        let record = registry.heartbeat("run-1", 30).unwrap();
        assert_eq!(record.restart_count, 1);
        assert_eq!(record.last_heartbeat_ms, 30);
        let _ = fs::remove_dir_all(root);
    }
}
