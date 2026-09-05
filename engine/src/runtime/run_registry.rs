use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const RUN_REGISTRY_SCHEMA_VERSION: u16 = 2;

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
    pub checkpoint_digest: Option<String>,
    pub last_error: Option<String>,
    pub recovery_reason: Option<String>,
    pub attempt_id: u64,
    pub owner_id: Option<String>,
    pub fencing_token: Option<String>,
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
    #[error("run is owned by another active runtime")]
    LeaseConflict,
    #[error("run registry lock could not be acquired")]
    LockTimeout,
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

#[derive(Debug)]
struct RegistryLock {
    path: PathBuf,
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl RunRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn acquire_lock(&self) -> Result<RegistryLock, RunRegistryError> {
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(".registry.lock");
        for _ in 0..40 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(unix_ms().to_string().as_bytes())?;
                    return Ok(RegistryLock { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age > Duration::from_secs(30));
                    if stale {
                        let _ = fs::remove_file(&path);
                    } else {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
                Err(error) => return Err(RunRegistryError::Io(error)),
            }
        }
        Err(RunRegistryError::LockTimeout)
    }

    fn record_path(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id).join("run.json")
    }

    pub fn create(&self, spec: RunSpec, now_ms: u64) -> Result<RunRecord, RunRegistryError> {
        let _lock = self.acquire_lock()?;
        spec.validate()?;
        let record = RunRecord {
            spec,
            status: RunStatus::Created,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            restart_count: 0,
            last_heartbeat_ms: now_ms,
            last_checkpoint: None,
            checkpoint_digest: None,
            last_error: None,
            recovery_reason: None,
            attempt_id: 0,
            owner_id: None,
            fencing_token: None,
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
        let _lock = self.acquire_lock()?;
        let mut record = self.read(run_id)?;
        record.status = status;
        record.updated_at_ms = now_ms;
        if matches!(record.status, RunStatus::Recovering) {
            record.restart_count = record.restart_count.saturating_add(1);
        }
        self.write(&record)?;
        Ok(record)
    }

    pub fn claim(
        &self,
        run_id: &str,
        owner_id: impl Into<String>,
        now_ms: u64,
    ) -> Result<RunRecord, RunRegistryError> {
        let _lock = self.acquire_lock()?;
        let mut record = self.read(run_id)?;
        let owner_id = owner_id.into();
        if owner_id.trim().is_empty() {
            return Err(RunRegistryError::InvalidSpec("owner identity is required"));
        }
        if record
            .owner_id
            .as_deref()
            .is_some_and(|owner| owner != owner_id)
        {
            return Err(RunRegistryError::LeaseConflict);
        }
        record.attempt_id = record.attempt_id.saturating_add(1);
        record.owner_id = Some(owner_id.clone());
        record.fencing_token = Some(format!(
            "{}-{}-{}",
            record.spec.run_id, owner_id, record.attempt_id
        ));
        record.status = RunStatus::Starting;
        record.updated_at_ms = now_ms;
        record.recovery_reason = None;
        self.write(&record)?;
        Ok(record)
    }

    pub fn heartbeat(&self, run_id: &str, now_ms: u64) -> Result<RunRecord, RunRegistryError> {
        let _lock = self.acquire_lock()?;
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
        let _lock = self.acquire_lock()?;
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
        record.recovery_reason = Some("heartbeat_expired".into());
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
        let _lock = self.acquire_lock()?;
        let mut record = self.read(run_id)?;
        let checkpoint = checkpoint.into();
        let mut digest = Sha256::new();
        digest.update(checkpoint.as_bytes());
        record.last_checkpoint = Some(checkpoint);
        record.checkpoint_digest = Some(format!("sha256:{}", hex::encode(digest.finalize())));
        record.updated_at_ms = now_ms;
        self.write(&record)?;
        Ok(record)
    }

    pub fn fail(
        &self,
        run_id: &str,
        error: impl Into<String>,
        now_ms: u64,
    ) -> Result<RunRecord, RunRegistryError> {
        let _lock = self.acquire_lock()?;
        let mut record = self.read(run_id)?;
        record.status = RunStatus::Failed;
        record.last_error = Some(error.into());
        record.recovery_reason = Some("runtime_failed".into());
        record.updated_at_ms = now_ms;
        self.write(&record)?;
        Ok(record)
    }

    fn write(&self, record: &RunRecord) -> Result<(), RunRegistryError> {
        record.spec.validate()?;
        let path = self.record_path(&record.spec.run_id);
        fs::create_dir_all(path.parent().expect("record has parent"))
            .map_err(|error| io_context("create run registry directory", &path, error))?;
        let tmp = path.with_extension(format!("{}.tmp", now_ms()));
        let bytes = serde_json::to_vec_pretty(record)?;
        fs::write(&tmp, bytes)
            .map_err(|error| io_context("write run registry temporary file", &tmp, error))?;
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

fn io_context(operation: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{operation} '{}': {error}", path.display()),
    )
}

fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    let mut last_error = None;
    for attempt in 0..5 {
        match fs::rename(source, target) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if target.exists() {
                    let _ = fs::remove_file(target);
                }
                if attempt < 4 {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        }
    }
    let error = last_error.unwrap_or_else(|| io::Error::other("unknown replace failure"));
    Err(io_context(
        &format!(
            "replace run registry file '{}' -> '{}'",
            source.display(),
            target.display()
        ),
        target,
        error,
    ))
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
        let claimed = registry.claim("run-1", "owner-a", 40).unwrap();
        assert_eq!(claimed.attempt_id, 1);
        assert!(matches!(
            registry.claim("run-1", "owner-b", 50),
            Err(RunRegistryError::LeaseConflict)
        ));
        let checkpoint = registry
            .checkpoint("run-1", "{\"state_version\":1}", 60)
            .unwrap();
        assert!(checkpoint
            .checkpoint_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        let _ = fs::remove_dir_all(root);
    }
}
