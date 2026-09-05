pub mod audit;
pub mod channels;
pub mod control_plane;
pub mod event_envelope;
pub mod event_loop;
pub mod health_reporter;
pub mod io;
pub mod reference_authority;
pub mod run_registry;
#[path = "supervisor.rs"]
pub mod supervisor;

pub use channels::RuntimeChannels;
pub use control_plane::RuntimeControlPlane;
pub use event_envelope::{CausalLedger, DataQuality, EventEnvelope, EventSource};
pub use event_loop::{DispatchError, RuntimeEventHandler, TradingRuntime};
pub use run_registry::{RunMode, RunRecord, RunRegistry, RunRegistryError, RunSpec, RunStatus};
pub use supervisor::{RuntimeBus, RuntimeCapacities, RuntimeHandles, RuntimeSignal};
