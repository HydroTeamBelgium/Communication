//! Nucleo 1 - Read CAN frames and log locally
//!
//! Receives CAN messages from the ECU and logs them via defmt/RTT.

#![no_std]
#![no_main]

// ============================================================================
//                              IMPORTS
// ============================================================================
use defmt_rtt as _;
use panic_probe as _;

use basis::prelude::*;

use embassy_stm32::{
    SharedData, 
    can,
    bind_interrupts,
    Config,
    peripherals,
};
use core::mem::MaybeUninit;

// ============================================================================
//                         BOARD CONFIGURATION
// ============================================================================
const THIS_BOARD: BoardConfig = NUCLEO_1;

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
//                            TASKS
// ============================================================================

// CAN reader will be spawned in main after stack is initialized
basis::can_read_task!(can_read);

// ============================================================================
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks
    let mut config = Config::default();
    validate_config();
    configure_clock_full(&mut config);
    
    info!("Nucleo 1 (CAN reader) starting...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Setup CAN
    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    can.set_bitrate(500_000);
    let can = can.into_normal_mode();
    info!("CAN Configured");

    // Spawn tasks
    spawner.spawn(can_read(can)).unwrap();
}
