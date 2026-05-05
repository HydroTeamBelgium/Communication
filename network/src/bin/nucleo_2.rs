//! Nucleo 2 - Sends CAN messages with real sensor data
//!
//! Sends periodic CAN messages with engine data using:
//! - Proper error handling for transmission failures
//! - Real sensor data simulation (not dummy increment)
//! - Configurable CAN ID and transmission interval
//! - Statistics and error tracking

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
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks (no ADC needed, only CAN)
    let mut config = Config::default();
    configure_clock_full(&mut config);
    
    info!("Nucleo 2 (CAN sender) starting with improved error handling...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Setup CAN
    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    can.set_bitrate(500_000);
    let can = can.into_normal_mode();
    info!("CAN Configured at 500 kbps");

    // Create CAN configuration for periodic transmission
    let can_config = CanConfig::new(0x520, 500_000, 250, 5000)
        .without_filtering(); // Sender doesn't need filtering

    // Spawn CAN writer task with error handling and statistics
    spawner.spawn(can_write_task(can, can_config))
        .expect("Failed to spawn CAN write task");
    
    info!("CAN writer task spawned successfully");
}
