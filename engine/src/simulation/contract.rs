use serde::Serialize;

pub const SIMULATION_MANIFEST_SCHEMA_VERSION: u16 = 1;

/// Stable identity shared by replay, single-run simulation and batch runs.
/// The manifest is operational evidence, not a research notebook artifact.
#[derive(Debug, Clone, Serialize)]
pub struct SimulationRunManifest {
    pub schema_version: u16,
    pub run_id: String,
    pub mode: String,
    pub policy_version: String,
    pub created_at_ms: u64,
    pub symbols: Vec<String>,
    pub policy_variants: Vec<String>,
}

impl SimulationRunManifest {
    pub fn new(
        run_id: impl Into<String>,
        mode: impl Into<String>,
        policy_version: impl Into<String>,
        created_at_ms: u64,
        symbols: Vec<String>,
        policy_variants: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SIMULATION_MANIFEST_SCHEMA_VERSION,
            run_id: run_id.into(),
            mode: mode.into(),
            policy_version: policy_version.into(),
            created_at_ms,
            symbols,
            policy_variants,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_identity_is_explicit_and_versioned() {
        let manifest = SimulationRunManifest::new(
            "run-001",
            "batch",
            "policy-v1",
            42,
            vec!["CXMTUSDT".to_owned()],
            vec!["baseline".to_owned()],
        );
        assert_eq!(manifest.schema_version, SIMULATION_MANIFEST_SCHEMA_VERSION);
        assert_eq!(manifest.mode, "batch");
        assert_eq!(manifest.policy_variants, vec!["baseline"]);
    }
}
