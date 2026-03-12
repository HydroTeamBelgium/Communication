//! Sensor drivers and helpers
//!
//! This module provides reusable sensor reading logic.
//! Tasks must be defined per-binary, but they can call these helpers.

pub mod adc;
pub mod button;
pub mod potentiometer;

pub use adc::*;
pub use button::*;
pub use potentiometer::*;
