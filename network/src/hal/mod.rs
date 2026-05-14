//! Hardware Abstraction Layer
//!
//! Provides reusable hardware setup functions for:
//! - Clock configuration
//! - Ethernet initialization
//! - CAN communication (unified interface)

pub mod clock;
pub mod ethernet;
pub mod can;

pub use clock::*;
pub use ethernet::*;
pub use can::*;
