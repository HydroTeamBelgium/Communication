//! Protocol module - message types and serialization
//!
//! Defines all message types used for communication between boards.

pub mod messages;
pub mod can_scs;
pub mod engine_data;

pub use messages::*;
pub use can_scs::*;
pub use engine_data::*;
