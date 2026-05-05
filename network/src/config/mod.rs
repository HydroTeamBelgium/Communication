//! Configuration module - all constants and board configs in one place
//!
//! This module provides:
//! - Network configuration (IPs, ports, buffer sizes)
//! - Board-specific configurations
//! - CAN configuration and types

pub mod network;
pub mod boards;
pub mod can;

pub use network::*;
pub use boards::*;
pub use can::*;
