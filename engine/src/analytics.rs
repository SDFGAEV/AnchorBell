//! Non-authoritative analytics and validation facade.
//!
//! Analytics consumes immutable market, decision, lifecycle, and simulation
//! evidence. It cannot create order intents, access credentials, or call an
//! exchange adapter. The legacy implementation modules remain internal
//! migration sources while downstream code adopts these operational names.

pub mod validation {
    pub use crate::research_methods::*;
}

pub mod evidence {
    pub use crate::hypothesis::*;
}
