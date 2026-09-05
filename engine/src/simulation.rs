//! Operational simulation facade.
//!
//! Simulation is an execution environment, not a research-paper concept.
//! The implementation is still being migrated from the legacy internal module
//! names; new callers should depend on this facade and its typed contracts.

#[path = "simulation/contract.rs"]
pub mod contract;

pub mod runtime {
    pub use crate::paper::*;
}

pub mod batch {
    pub use crate::paper_lab::*;
}

pub use crate::paper::{
    allocate_positions, load_anchors, load_binance_index_anchor_set, run_live,
    BinanceIndexAnchorSet, PaperAnchor as AnchorSnapshot, PaperEngine as SimulationEngine,
    PaperError as SimulationError, PaperRunConfig as SimulationConfig, PaperRunConfig,
    PaperRunResult as SimulationResult, PaperStrategyVariant as PolicyVariant,
    PaperStrategyVariant, PositionAllocation, PositionMode,
};
pub use crate::paper_lab::{
    PaperLabConfig as BatchSimulationConfig, PaperLabResult as BatchSimulationResult,
    PaperLabSpec as SimulationSpec,
};
pub use contract::{SimulationRunManifest, SIMULATION_MANIFEST_SCHEMA_VERSION};
