//! Protocol module - message types and serialization
//!
//! Defines all message types used for communication between boards.

pub mod messages;
pub mod can_scs;

pub use messages::*;
pub use can_scs::*;
