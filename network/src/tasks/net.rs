//! Network runner task
//!
//! This task runs the embassy-net stack.

use embassy_stm32::eth::{Ethernet, GenericPhy, Sma};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::peripherals::ETH_SMA;

/// Network stack runner task
///
/// Must be spawned once per stack. Runs forever.
#[embassy_executor::task]
pub async fn net_task(
    mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericPhy<Sma<'static, ETH_SMA>>>>
) -> ! {
    runner.run().await
}
