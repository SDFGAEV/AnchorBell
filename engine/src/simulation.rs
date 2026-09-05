//! Operational simulation facade.
//!
//! Simulation is an execution environment with explicit runtime, batch,
//! replay, accounting, and validation boundaries.

#[path = "simulation/contract.rs"]
pub mod contract;
#[path = "simulation/experiment_plan.rs"]
pub mod experiment_plan;

pub mod runtime {
    pub use crate::simulation_runtime::*;
}

pub mod batch {
    pub use crate::simulation_batch::*;
}

pub use crate::runtime::reference_authority::fetch as load_index_anchor_set;
pub use crate::simulation_batch::{
    SimulationBatchConfig, SimulationBatchResult, SimulationBatchSpec,
};
pub use crate::simulation_runtime::{
    allocate_positions, load_anchor_file, replay_jsonl_with_realism, run_simulation,
    AnchorSnapshot, BinanceIndexAnchorSet, PositionAllocation, PositionMode, SimulationConfig,
    SimulationEngine, SimulationError, SimulationPolicyVariant, SimulationResult,
};
pub use contract::{SimulationRunManifest, SIMULATION_MANIFEST_SCHEMA_VERSION};
pub use experiment_plan::{ExperimentPlan, ExperimentSpec};
