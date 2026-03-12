//! Nucleo 2 - Receiver board
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
use basis::hal::create_net_config;

use embassy_stm32::{
    SharedData,
    eth::{self, Ethernet},
    peripherals,
    rng::InterruptHandler as RngInterruptHandler,
};
use core::mem::MaybeUninit;

// ============================================================================
//                         BOARD CONFIGURATION
// ============================================================================
const THIS_BOARD: BoardConfig = NUCLEO_2;

// ============================================================================
//                         STATIC ALLOCATIONS  
// ============================================================================
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// ============================================================================
//                            INTERRUPTS
// ============================================================================
embassy_stm32::bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

// ============================================================================
//                              TASKS
// ============================================================================

// Generate UDP receiver task using the library macro
basis::udp_recv_task!(udp_recv_task, THIS_BOARD);

// ============================================================================
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks (no ADC needed)
    let mut config = Config::default();
    validate_config();
    configure_clock_network_only(&mut config);
    
    info!("Nucleo 2 (receiver) starting...");
    
    // Initialize hardware
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Setup Ethernet
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 8>::new()),
        p.ETH, Irqs,
        p.PA1, p.PA2, p.PC1, p.PA7,
        p.PC4, p.PC5, p.PG13, p.PB13, p.PG11,
        GenericPhy::new_auto(),
        THIS_BOARD.mac,
    );

    // Setup network stack
    let net_config = create_net_config(&THIS_BOARD);
    let seed = {
        let mut rng = Rng::new(p.RNG, Irqs);
        let mut seed = [0u8; 8];
        rng.fill_bytes(&mut seed);
        u64::from_le_bytes(seed)
    };
    let (stack, runner) = embassy_net::new(
        device, 
        net_config, 
        RESOURCES.init(StackResources::new()), 
        seed
    );
    let stack = STACK.init(stack);

    // Spawn tasks
    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(udp_recv_task(stack)).unwrap();
}
