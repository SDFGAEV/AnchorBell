//! Non-authoritative analytics and validation facade.
//!
//! Analytics consumes immutable market, decision, lifecycle, and simulation
//! evidence. It cannot create order intents, access credentials, or call an
//! exchange adapter. These modules are the canonical non-authoritative
//! analytics implementations and never create order authority.

pub mod validation {
    pub use crate::analytics_validation::*;
}

pub mod evidence {
    pub use crate::analytics_evidence::*;
}
