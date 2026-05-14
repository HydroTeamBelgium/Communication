//! Nucleo 2 - Sends CAN messages with real sensor data
//!
//! Sends periodic CAN messages with engine data using:
//! - All 17 SCS message types (0x300-0x310)
//! - Professional test data generators from protocol module
//! - Rotating through each message type
//! - Realistic test data with incrementing values
//! - Statistics and error tracking

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
    
    info!("Nucleo 2 (CAN sender - all message types) starting...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Setup CAN with standard ECU parameters (see hal::can::config for constants)
    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    can.set_bitrate(basis::hal::can::config::CAN_BITRATE_DEFAULT);
    let can = can.into_normal_mode();
    info!("CAN Configured at {} kbps", basis::hal::can::config::CAN_BITRATE_DEFAULT / 1000);

    // Spawn CAN writer task that cycles through all message types
    match spawner.spawn(can_send_all_messages(can)) {
        Ok(_) => info!("CAN multi-message writer task spawned successfully"),
        Err(_) => {
            error!("CRITICAL: Cannot spawn CAN writer - out of memory!");
            loop { defmt::flush(); }
        }
    }
}

// ============================================================================
//                      MULTI-MESSAGE TASK
// ============================================================================

/// Task that cycles through all 17 message types (0x300-0x310)
/// 
/// Uses professional test data generators from the protocol module.
#[embassy_executor::task]
async fn can_send_all_messages(
    mut can: embassy_stm32::can::Can<'static>,
) {
    use embassy_time::Timer;
    use basis::protocol::{ScsTestGenerator, SCS_MESSAGE_IDS};
    
    let mut generator = ScsTestGenerator::new();
    let mut msg_count = 0u32;
    
    info!("Starting CAN message cycle: 0x300 → 0x310 ({} message types)", SCS_MESSAGE_IDS.len());
    
    loop {
        for &msg_id in SCS_MESSAGE_IDS {
            let (can_id, frame_data) = generator.generate_frame(msg_id);
            
            // Create CAN frame
            let frame = match embassy_stm32::can::frame::Frame::new_extended(can_id, &frame_data) {
                Ok(f) => f,
                Err(e) => {
                    error!("CAN TX: Failed to create frame for ID 0x{:03X}: {}", can_id, defmt::Debug2Format(&e));
                    continue;
                }
            };
            
            // Send frame
            match can.write(&frame).await {
                None => {
                    trace!("CAN TX: ID=0x{:03X}, counter={}", can_id, generator.counter());
                    msg_count += 1;
                }
                Some(_) => {
                    warn!("CAN TX: Queue full for ID 0x{:03X}", can_id);
                }
            }
            
            // Print progress every cycle
            if msg_count % (SCS_MESSAGE_IDS.len() as u32) == 0 && msg_count > 0 {
                info!("CAN Cycle #{}: sent {} messages total", msg_count / (SCS_MESSAGE_IDS.len() as u32), msg_count);
            }
            
            // Small delay between messages (250ms per message)
            Timer::after_millis(250).await;
        }
        
        generator.next_cycle();
    }
}
