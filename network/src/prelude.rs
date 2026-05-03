//! Prelude module - common imports for all nucleo binaries
//!
//! Usage: `use basis::prelude::*;`

// Re-export commonly needed items
pub use defmt::{info, warn, error, debug, trace};
pub use embassy_executor::Spawner;
pub use embassy_net::{Stack, StackResources, Ipv4Cidr};
pub use embassy_net::udp::{UdpSocket, PacketMetadata};
pub use embassy_stm32::Config;
pub use embassy_stm32::eth::{Ethernet, GenericPhy, PacketQueue};
pub use embassy_stm32::rng::Rng;
pub use rand_core::RngCore;
pub use static_cell::StaticCell;

// Re-export our modules
pub use crate::config::*;
pub use crate::protocol::*;
pub use crate::hal::*;
pub use crate::{can_write_task, can_read_task};
pub use crate::tasks::net_task;

// Re-export sensor drivers
pub use crate::sensors::{ButtonDriver, ButtonConfig, PotDriver, PotConfig, AdcConfig, AdcSensor};
