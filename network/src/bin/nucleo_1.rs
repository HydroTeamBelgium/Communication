//! Nucleo 1 - Sender board
//!
//! Sends button presses and potentiometer readings to Nucleo 2.

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
    adc::Adc,
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
const THIS_BOARD: BoardConfig = NUCLEO_1;
const DESTINATION: BoardConfig = NUCLEO_2;

// ============================================================================
//                         STATIC ALLOCATIONS  
// ============================================================================
// Network statics (required per-binary)
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Inter-task communication
static CHANNEL: Channel<CriticalSectionRawMutex, Message, 4> = Channel::new();

// Hardware shared data for dual-core STM32H755
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
basis::button_task!(button_task, CHANNEL, ButtonConfig::for_button(1));

// Generate potentiometer task using the library macro
basis::pot_task_simple!(pot_task, CHANNEL, PotConfig::default());

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
    configure_clock_full(&mut config);
    
    info!("Nucleo 1 (sender) starting...");
    
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

    // Setup peripherals
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    let adc = Adc::new(p.ADC2);

    // Spawn tasks
    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(udp_send_task(stack)).unwrap();
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(pot_task(adc, p.PA3)).unwrap();
}
