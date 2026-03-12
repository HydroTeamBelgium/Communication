//! Configuration module - all constants and board configs in one place
//!
//! This module provides:
//! - Network configuration (IPs, ports, buffer sizes)
//! - Board-specific configurations

pub mod network;
pub mod boards;

pub use network::*;
pub use boards::*;
