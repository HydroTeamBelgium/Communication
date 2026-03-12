//! Nucleo Network Communication Library
//!
//! This crate provides modular components for UDP communication between
//! multiple Nucleo-H755ZI boards.
//!
//! # Modules
//!
//! - [`config`]: Network and board configuration constants
//! - [`protocol`]: Message types and serialization
//! - [`hal`]: Hardware abstraction (clock, ethernet, sensors)
//! - [`tasks`]: Reusable Embassy async tasks
//! - [`sensors`]: Sensor drivers (button, potentiometer, ADC)
//! - [`prelude`]: Common imports for binaries
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use basis::prelude::*;
//! use basis::sensors::{ButtonDriver, ButtonConfig};
//! 
//! // Use macros for common patterns
//! basis::button_task!(my_button_task, CHANNEL, ButtonConfig::for_button(1));
//! ```

#![no_std]

pub mod config;
pub mod protocol;
pub mod hal;
pub mod tasks;
pub mod sensors;
pub mod prelude;

// Re-export commonly used items at crate root
pub use config::{BoardConfig, NUCLEO_1, NUCLEO_2, NUCLEO_3};
pub use protocol::{Message, MessageType, PotReading};
