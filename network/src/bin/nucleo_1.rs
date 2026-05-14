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

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_stm32::{
    SharedData, 
    can,
    bind_interrupts,
    Config,
    peripherals,
    eth::{self, Ethernet},
    rng::InterruptHandler as RngInterruptHandler,
};
use core::mem::MaybeUninit;

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
    can.set_bitrate(basis::hal::can::config::CAN_BITRATE_DEFAULT);

    // MaxxECU default protocol uses classic CAN frames (11-bit IDs, 500 kbps).
    // Enable edge filtering to reduce sensitivity to short noise spikes on the bus.
    let fd_cfg = can
        .config()
        .set_frame_transmit(can::config::FrameTransmissionConfig::ClassicCanOnly)
        .set_non_iso_mode(false)
        .set_edge_filtering(true);
    can.set_config(fd_cfg);

    let can = can.into_normal_mode();
    info!("CAN Configured at {} kbps", basis::hal::can::config::CAN_BITRATE_DEFAULT / 1000);
    // DIAGNOSTIC: Check FDCAN peripheral state (why is it entering Error Passive?)
    {
        let fdcan = &p.FDCAN1;
        let cccr = fdcan.cccr.read();
        let ecr = fdcan.ecr.read();
        let psr = fdcan.psr.read();
        
        info!("FDCAN1 Diagnostic State:");
        info!("  CCCR init={}, cce={}, ase={}, brse={}, fdoe={}, test={}", 
            cccr.init(), cccr.cce(), cccr.ase(), cccr.brse(), cccr.fdoe(), cccr.test());
        info!("  ECR tec={}, rec={}, rp={}, cel={}",
            ecr.tec(), ecr.rec(), ecr.rp(), ecr.cel());
        info!("  PSR lec={}, act={}, ep={}, ew={}, bo={}, dlec={}",
            psr.lec(), psr.act(), psr.ep(), psr.ew(), psr.bo(), psr.dlec());
        info!("  FDCAN in state: act={} ep={} ew={} bo={}", 
            match psr.act() {
                0 => "Synchronizing",
                1 => "IdleWaiting",  
                2 => "Receiver",
                3 => "Transmitter",
                _ => "Unknown",
            },
            psr.ep(), psr.ew(), psr.bo()
        );
    }


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

    // Create CAN configuration with 200ms timeout (industry standard)
    let can_config = CanConfig::new(0x520, 500_000, 250, 200)
        .with_rx_timeout(200);

    // Spawn network runner task
    match spawner.spawn(net_task(runner)) {
        Ok(_) => info!("Network task spawned"),
        Err(_) => {
            error!("CRITICAL: Cannot spawn network task - out of memory!");
            loop { defmt::flush(); }
        }
    }

    // Spawn CAN reader task with channel broadcasting
    match spawner.spawn(can_read_task_with_channel(can, can_config, CAN_CHANNEL.sender())) {
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
