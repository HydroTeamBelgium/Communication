//! Protocol module - message types and serialization
//!
//! Defines all message types used for communication between boards.

pub mod messages;
#[path = "can_scs.rs"]
pub mod scs;
#[path = "constants.rs"]
pub mod scs_constants;
pub mod ecu;
pub mod maxx;

pub use messages::*;
pub use scs::*;
pub use scs_constants::*;
pub use ecu::*;
pub use maxx::*;
