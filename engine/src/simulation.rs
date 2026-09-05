//! Operational simulation facade.
//!
//! Simulation is an execution environment with explicit runtime, batch,
//! replay, accounting, and validation boundaries.

#[path = "simulation/contract.rs"]
pub mod contract;

pub mod runtime {
    pub use crate::simulation_runtime::*;
}

pub mod batch {
    pub use crate::simulation_batch::*;
}

pub use crate::simulation_batch::{
    SimulationBatchConfig, SimulationBatchResult, SimulationBatchSpec,
};
pub use crate::simulation_runtime::{
    allocate_positions, load_anchor_file, load_index_anchor_set, replay_jsonl_with_realism,
    run_simulation, AnchorSnapshot, BinanceIndexAnchorSet, PositionAllocation, PositionMode,
    SimulationConfig, SimulationEngine, SimulationError, SimulationPolicyVariant, SimulationResult,
};
pub use contract::{SimulationRunManifest, SIMULATION_MANIFEST_SCHEMA_VERSION};
