#![no_std]
#![no_main]


use defmt_rtt as _;
use panic_probe as _;
use embassy_executor::Spawner;

mod config;
mod usb1;
mod ethernet;

use config::{
    configure_clock, 
    SHARED_DATA,
};
use usb1::{setup_usb};
use ethernet::{setup_ethernet, STACK};

use crate::ethernet::{net_task, udp_task};
use crate::usb1::usb_task;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    configure_clock(&mut config);

    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Initialize Hardware
    let (class, mut usb) = setup_usb(p.USB_OTG_FS, p.PA12, p.PA11);
    let (stack, runner) = setup_ethernet(
    p.ETH, p.PA1, p.PA2, p.PC1, p.PA7,
    p.PC4, p.PC5, p.PG13, p.PB13, p.PG11, p.RNG,
    [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEA]
    );
    let stack = STACK.init(stack);

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");
    spawner.spawn(usb_task(class)).expect("Failed to spawn USB task");

    // Run USB Device
    usb.run().await;
}