#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_executor::Spawner;

use embassy_stm32::{
    Config,
};


mod config1;
mod usb1;
mod eth1;

use config1::*;
use usb1::*;
use eth1::*;



// =============================================
//              MAIN
// =============================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    validate_config();
    configure_clock(&mut config);

    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Initialize Hardware
    let (class, mut usb) = setup_usb(p.USB_OTG_FS, p.PA12, p.PA11);
    let (stack, runner) = setup_ethernet(
    p.ETH, p.PA1, p.PA2, p.PC1, p.PA7,
    p.PC4, p.PC5, p.PG13, p.PB13, p.PG11, p.RNG,
    [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]
    );
    let stack = STACK.init(stack);

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");
    spawner.spawn(usb_task(class)).expect("Failed to spawn USB task");

    // Run USB Device
    usb.run().await;
}