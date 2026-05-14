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
pub use crate::tasks::can::{can_write_task, can_read_task, can_read_task_with_channel, can_udp_broadcast_task};
pub use crate::tasks::net_task;

// Re-export sensor drivers
pub use crate::sensors::{ButtonDriver, ButtonConfig, PotDriver, PotConfig, AdcConfig, AdcSensor};

// ============================================================================
//                        UTILITY MACROS
// ============================================================================

/// Spawn a critical task that must succeed
///
/// Panics the board if task spawning fails (out of memory).
/// Automatically logs success/failure.
///
/// # Example
/// ```ignore
/// spawn_critical!(spawner, my_task(), "My Critical Task");
/// ```
#[macro_export]
macro_rules! spawn_critical {
    ($spawner:expr, $task:expr, $name:expr) => {
        match $spawner.spawn($task) {
            Ok(_) => $crate::prelude::info!("{} spawned successfully", $name),
            Err(_) => {
                $crate::prelude::error!("CRITICAL: Cannot spawn {} - out of memory!", $name);
                loop { defmt::flush(); }
            }
        }
    };
}
