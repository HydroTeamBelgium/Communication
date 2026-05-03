//! Nucleo 2 - Sends CAN message
//!
//! Receives and logs UDP messages from other boards.

#![no_std]
#![no_main]

// ============================================================================
//                              IMPORTS
// ============================================================================
use defmt_rtt as _;
use panic_probe as _;

use basis::prelude::*;
// use basis::hal::create_net_config;

use embassy_stm32::{
    SharedData, 
    Config,
    can,
    bind_interrupts,
    peripherals, 
};
use core::mem::MaybeUninit;



// ============================================================================
//                         STATIC ALLOCATIONS  
// ============================================================================


#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// ============================================================================
//                            INTERRUPTS
// ============================================================================

bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => can::IT0InterruptHandler<peripherals::FDCAN1>;
    FDCAN1_IT1 => can::IT1InterruptHandler<peripherals::FDCAN1>;
});

// ============================================================================
//                              TASKS
// ============================================================================


basis::can_write_task!(can_write, 0x520, 250);


// ============================================================================
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks (no ADC needed)
    let mut config = Config::default();
    validate_config();
    configure_clock_full(&mut config);
    
    info!("Nucleo 2 (receiver) starting...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);


    //can setup

    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    can.set_bitrate(500_000);
    let can = can.into_normal_mode();
    info!("CAN Configured");

    // Spawn tasks
    spawner.spawn(can_write(can)).unwrap();
}
