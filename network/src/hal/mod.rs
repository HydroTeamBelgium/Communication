//! Hardware Abstraction Layer
//!
//! Provides reusable hardware setup functions for:
//! - Clock configuration
//! - Ethernet initialization

pub mod clock;
pub mod ethernet;

pub use clock::*;
pub use ethernet::*;
