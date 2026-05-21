//! Nucleo 2 - Sends CAN messages with real sensor data
//!
//! Sends periodic CAN messages with engine data using:
//! - ECU-specific test data generators from the protocol module
//! - Rotating through each documented CAN message for the selected ECU
//! - Standard 11-bit CAN frames matching the ECU documentation
//! - Statistics and error tracking

#![no_std]
#![no_main]

// ============================================================================
//                              IMPORTS
// ============================================================================
use defmt_rtt as _;
use panic_probe as _;

use basis::prelude::*;
use basis::config::can::{EcuType, NUCLEO_2_ECU_MODE};

use embassy_stm32::{
    SharedData, 
    Config,
    can,
    bind_interrupts,
    can::config::NominalBitTiming,
    peripherals, 
};
use embassy_time::Timer;
use core::mem::MaybeUninit;
use core::num::{NonZeroU16, NonZeroU8};

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
    let ecu_label = match NUCLEO_2_ECU_MODE {
        basis::config::can::EcuType::ScsDelta => "ScsDelta",
        basis::config::can::EcuType::MaxxEcu => "MaxxEcu",
    };

    let fd_cfg = can
        .config()
        .set_frame_transmit(can::config::FrameTransmissionConfig::ClassicCanOnly)
        .set_non_iso_mode(false)
        .set_edge_filtering(false)
        .set_nominal_bit_timing(NominalBitTiming {
            prescaler: NonZeroU16::new(5).unwrap(),
            seg1: NonZeroU8::new(7).unwrap(),
            seg2: NonZeroU8::new(2).unwrap(),
            sync_jump_width: NonZeroU8::new(1).unwrap(),
        });
    can.set_config(fd_cfg);

    let nominal = can.config().nbtr;
    let sample_point_permille = (1000 * (1 + nominal.seg1.get() as u16))
        / (1 + nominal.seg1.get() as u16 + nominal.seg2.get() as u16);
    info!(
        "CAN debug: board=Nucleo2, ECU mode={}, bitrate={} bps, pins=PD0/PD1",
        ecu_label,
        basis::hal::can::config::CAN_BITRATE_DEFAULT
    );
    info!(
        "CAN timing: prescaler={}, seg1={}, seg2={}, sjw={}, sample_point={} permille",
        nominal.prescaler.get(),
        nominal.seg1.get(),
        nominal.seg2.get(),
        nominal.sync_jump_width.get(),
        sample_point_permille
    );
    let can = can.into_normal_mode();
    info!("CAN Configured at {} kbps", basis::hal::can::config::CAN_BITRATE_DEFAULT / 1000);

    // Spawn CAN writer task that cycles through the ECU-specific message set
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

/// Task that cycles through the selected ECU's documented CAN message set.
#[embassy_executor::task]
async fn can_send_all_messages(
    mut can: embassy_stm32::can::Can<'static>,
) {
    match NUCLEO_2_ECU_MODE {
        EcuType::ScsDelta => send_scs_cycle(&mut can).await,
        EcuType::MaxxEcu => send_maxx_cycle(&mut can).await,
    }
}

async fn send_scs_cycle(can: &mut embassy_stm32::can::Can<'static>) {
    use basis::protocol::{ScsTestGenerator, SCS_MESSAGE_IDS};

    let mut generator = ScsTestGenerator::new();
    let mut msg_count = 0u32;

    info!("Starting CAN message cycle: SCS 0x300 → 0x310 ({} message types)", SCS_MESSAGE_IDS.len());

    loop {
        for &msg_id in SCS_MESSAGE_IDS {
            let (can_id, frame_data) = generator.generate_frame(msg_id);
            let frame = match embassy_stm32::can::frame::Frame::new_standard(can_id as u16, &frame_data) {
                Ok(f) => f,
                Err(e) => {
                    error!("CAN TX: Failed to create SCS frame for ID 0x{:03X}: {}", can_id, defmt::Debug2Format(&e));
                    continue;
                }
            };

            match can.write(&frame).await {
                None => {
                    trace!("CAN TX: SCS ID=0x{:03X}, counter={}", can_id, generator.counter());
                    msg_count += 1;
                }
                Some(_) => {
                    warn!("CAN TX: Queue full for SCS ID 0x{:03X}", can_id);
                }
            }

            if msg_count % (SCS_MESSAGE_IDS.len() as u32) == 0 && msg_count > 0 {
                info!("CAN Cycle #{}: sent {} SCS messages total", msg_count / (SCS_MESSAGE_IDS.len() as u32), msg_count);
            }

            Timer::after_millis(SCS_TEST_FRAME_INTERVAL_MS).await;
        }

        generator.next_cycle();
    }
}

async fn send_maxx_cycle(can: &mut embassy_stm32::can::Can<'static>) {
    use basis::protocol::{MaxxTestGenerator, MAXX_MESSAGE_IDS};

    let mut generator = MaxxTestGenerator::new();
    let mut msg_count = 0u32;

    info!("Starting CAN message cycle: MaxxECU documented message set ({} message types)", MAXX_MESSAGE_IDS.len());

    loop {
        for &msg_id in MAXX_MESSAGE_IDS {
            let (can_id, frame_data) = generator.generate_frame(msg_id);
            let frame = match embassy_stm32::can::frame::Frame::new_standard(can_id as u16, &frame_data) {
                Ok(f) => f,
                Err(e) => {
                    error!("CAN TX: Failed to create MaxxECU frame for ID 0x{:03X}: {}", can_id, defmt::Debug2Format(&e));
                    continue;
                }
            };

            match can.write(&frame).await {
                None => {
                    trace!("CAN TX: MaxxECU ID=0x{:03X}, counter={}", can_id, generator.counter());
                    msg_count += 1;
                }
                Some(_) => {
                    warn!("CAN TX: Queue full for MaxxECU ID 0x{:03X}", can_id);
                }
            }

            if msg_count % (MAXX_MESSAGE_IDS.len() as u32) == 0 && msg_count > 0 {
                info!("CAN Cycle #{}: sent {} MaxxECU messages total", msg_count / (MAXX_MESSAGE_IDS.len() as u32), msg_count);
            }

            Timer::after_millis(MAXX_TEST_FRAME_INTERVAL_MS).await;
        }

        generator.next_cycle();
    }
}
