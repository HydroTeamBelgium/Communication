//! Nucleo 1 - Read CAN frames and broadcast over UDP with proper error handling
//!
//! Receives CAN messages from the ECU and broadcasts them via UDP to 0.0.0.0:9999 with:
//! - Timeout protection (5 second watchdog)
//! - Frame validation
//! - Sequence number tracking
//! - UDP broadcast to remote logging client
//! - Error diagnostics

#![no_std]
#![no_main]

// ============================================================================
//                              IMPORTS
// ============================================================================
use defmt_rtt as _;
use panic_probe as _;

use basis::prelude::*;
use basis::hal::create_net_config;
use basis::tasks::can::{can_read_maxx_task_with_channel, can_read_scs_task_with_channel};
use basis::config::can::{DEFAULT_CAN_RX_TIMEOUT_MS, NUCLEO_1_ECU_MODE};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_stm32::{
    SharedData, 
    can,
    bind_interrupts,
    can::config::NominalBitTiming,
    Config,
    peripherals,
    eth::{self, Ethernet},
    rng::InterruptHandler as RngInterruptHandler,
};
use core::mem::MaybeUninit;
use core::num::{NonZeroU16, NonZeroU8};

// ============================================================================
//                         BOARD CONFIGURATION
// ============================================================================
const THIS_BOARD: BoardConfig = NUCLEO_1;

// ============================================================================
//                         STATIC ALLOCATIONS  
// ============================================================================
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Channel for CAN frames → UDP broadcast (size 256 to prevent frame loss)
static CAN_CHANNEL: Channel<CriticalSectionRawMutex, Message, 256> = Channel::new();

#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// ============================================================================
//                            INTERRUPTS
// ============================================================================
bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => can::IT0InterruptHandler<peripherals::FDCAN1>;
    FDCAN1_IT1 => can::IT1InterruptHandler<peripherals::FDCAN1>;
});

embassy_stm32::bind_interrupts!(struct EthIrqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

// ============================================================================
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks
    let mut config = Config::default();
    configure_clock_full(&mut config);
    
    info!("Nucleo 1 (CAN reader + UDP broadcast) starting...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Setup CAN with standard ECU parameters (see hal::can::config for constants)
    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    let ecu_label = match NUCLEO_1_ECU_MODE {
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
        "CAN debug: board=Nucleo1, ECU mode={}, bitrate={} bps, rx_timeout={} ms, pins=PD0/PD1",
        ecu_label,
        basis::hal::can::config::CAN_BITRATE_DEFAULT,
        DEFAULT_CAN_RX_TIMEOUT_MS
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

    // Setup Ethernet for UDP broadcast
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 8>::new()),
        p.ETH, EthIrqs,
        p.PA1, p.PA2, p.PC1, p.PA7,
        p.PC4, p.PC5, p.PG13, p.PB13, p.PG11,
        GenericPhy::new_auto(),
        THIS_BOARD.mac,
    );

    // Setup network stack
    let net_config = create_net_config(&THIS_BOARD);
    let seed = {
        let mut rng = Rng::new(p.RNG, EthIrqs);
        let mut seed = [0u8; 8];
        rng.fill_bytes(&mut seed);
        u64::from_le_bytes(seed)
    };
    let (stack, runner) = embassy_net::new(
        device,
        net_config,
        RESOURCES.init(StackResources::new()),
        seed,
    );
    let stack = STACK.init(stack);

    // Create CAN configuration with centralized timeout settings.
    let can_config = CanConfig::new(
        NUCLEO_1_ECU_MODE,
        basis::hal::can::config::CAN_BITRATE_DEFAULT,
        0,
        DEFAULT_CAN_RX_TIMEOUT_MS,
    );

    // Spawn network runner task
    match spawner.spawn(net_task(runner)) {
        Ok(_) => info!("Network task spawned"),
        Err(_) => {
            error!("CRITICAL: Cannot spawn network task - out of memory!");
            loop { defmt::flush(); }
        }
    }

    // Spawn CAN reader task with ECU-specific parser selection
    let spawn_result = match NUCLEO_1_ECU_MODE {
        basis::config::can::EcuType::ScsDelta => spawner.spawn(can_read_scs_task_with_channel(can, can_config, CAN_CHANNEL.sender())),
        basis::config::can::EcuType::MaxxEcu => spawner.spawn(can_read_maxx_task_with_channel(can, can_config, CAN_CHANNEL.sender())),
    };

    match spawn_result {
        Ok(_) => info!("CAN read task spawned"),
        Err(_) => {
            error!("CRITICAL: Cannot spawn CAN reader - out of memory!");
            loop { defmt::flush(); }
        }
    }

    // Spawn UDP broadcast task for remote logging
    match spawner.spawn(can_udp_broadcast_task(stack, CAN_CHANNEL.receiver())) {
        Ok(_) => info!("UDP broadcast task spawned"),
        Err(_) => {
            error!("CRITICAL: Cannot spawn UDP broadcast - out of memory!");
            loop { defmt::flush(); }
        }
    }
    
    info!("All tasks spawned successfully - CAN data broadcasting to UDP 255.255.255.255:9999");
}
