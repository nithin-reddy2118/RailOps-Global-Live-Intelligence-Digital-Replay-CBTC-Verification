//! Core domain types shared by every CBTC platform service.
//!
//! This crate has no dependency on any specific service, transport, or
//! storage technology beyond `sqlx::Type` derives (needed so the same
//! newtypes can be used directly as query parameters). Business logic
//! that is specific to a single service (e.g. interlocking conflict
//! resolution) lives in that service, not here — this crate only holds
//! the vocabulary every service needs to agree on.

pub mod block;
pub mod ids;
pub mod movement_authority;
pub mod signal;
pub mod station;
pub mod switch;
pub mod topology;
pub mod train;

pub use block::*;
pub use ids::*;
pub use movement_authority::*;
pub use signal::*;
pub use station::*;
pub use switch::*;
pub use topology::*;
pub use train::*;
