//! Simulation orchestration facade. Network adapters stay outside the domain engine.
pub use crate::simulation_batch::{
    run, SimulationBatchConfig, SimulationBatchResult, SimulationBatchSpec,
};
