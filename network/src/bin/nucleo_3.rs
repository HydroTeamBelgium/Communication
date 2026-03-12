//! Nucleo 3 - Secondary sender board
//!
//! Sends button presses to Nucleo 2.
//! Uses the same Message protocol as Nucleo 1.

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
    eth::{self, Ethernet},
    exti::ExtiInput,
    gpio::Pull,
    peripherals,
    rng::InterruptHandler as RngInterruptHandler,
};
use core::mem::MaybeUninit;

// ============================================================================
//                         BOARD CONFIGURATION
// ============================================================================
const THIS_BOARD: BoardConfig = NUCLEO_3;
const DESTINATION: BoardConfig = NUCLEO_2;

// ============================================================================
//                         STATIC ALLOCATIONS  
// ============================================================================
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Inter-task communication (uses Message protocol)
static CHANNEL: Channel<CriticalSectionRawMutex, Message, 4> = Channel::new();

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
//                         SENSOR TASKS (using macros)
// ============================================================================

// Generate button task using the library macro
basis::button_task!(button_task, CHANNEL, ButtonConfig::for_button(3));

// ============================================================================
//                              NETWORK TASKS
// ============================================================================

// Generate UDP sender task using the library macro
basis::udp_send_task!(udp_send_task, CHANNEL, THIS_BOARD, DESTINATION);

// ============================================================================
//                               MAIN
// ============================================================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize clocks
    let mut config = Config::default();
    validate_config();
    configure_clock_network_only(&mut config);
    
    info!("Nucleo 3 (sender) starting...");
    
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

    // Setup button
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);

    // Spawn tasks
    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(udp_send_task(stack)).unwrap();
    spawner.spawn(button_task(button)).unwrap();
}
