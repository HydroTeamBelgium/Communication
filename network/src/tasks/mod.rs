//! Reusable Embassy tasks
//!
//! These tasks can be spawned from any main binary.

pub mod net;
pub mod udp;

pub use net::*;
pub use udp::*;
