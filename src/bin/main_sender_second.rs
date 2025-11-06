#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use panic_probe as _;
use rand_core::RngCore;
use embassy_executor::Spawner;
use embassy_net::{
    udp::{UdpSocket, PacketMetadata},
    StackResources, 
    Ipv4Address, 
    Ipv4Cidr,
    IpEndpoint,
    Stack
};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::Pull,
    eth::{Ethernet, PacketQueue, GenericPhy},
    eth,
    rng::{Rng, InterruptHandler as RngInterruptHandler},
    peripherals::ETH,
    bind_interrupts,
    SharedData,
    peripherals,

    Config,
    rcc::*,
};

use static_cell::StaticCell;
use defmt::{*, assert};
use core::mem::MaybeUninit;

// =============================================
//              CONFIGURATION
// =============================================
// Grouped all constants into logical sections with documentation.


// --- Network Configuration ---
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 62);
const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
const NETWORK_UDP_PORT: u16 = 4321;

const DESTINATION_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 60);
const DESTINATION_PORT: u16 = 12345;


// Buffer Sizes (optimized for image transfer)
const RX_BUFFER_SIZE: usize = 2048;  
const TX_BUFFER_SIZE: usize = 2048;
const MAX_PACKET_SIZE: usize = 1536;

// Socket Configuration
const RX_METADATA_COUNT: usize = 32;  
const TX_METADATA_COUNT: usize = 32;

// Validate configuration
fn validate_config() {
    assert!(RX_BUFFER_SIZE >= MAX_PACKET_SIZE, "RX buffer too small");
    assert!(TX_BUFFER_SIZE >= MAX_PACKET_SIZE, "TX buffer too small");
}


// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Network Buffers
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Shared channel for messages from button_task to udp_task
static CHANNEL: Channel<CriticalSectionRawMutex, &'static [u8], 4> = Channel::new();

// Hardware Shared Data
#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// =============================================
//              HARDWARE SETUP
// =============================================
bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

/// Configures the STM32 clock tree for optimal performance.
fn configure_clock(config: &mut Config) {
    
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    config.rcc.csi = true;
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV8),
        divr: None,
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV2;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.apb3_pre = APBPrescaler::DIV2;
    config.rcc.apb4_pre = APBPrescaler::DIV2;
    config.rcc.voltage_scale = VoltageScale::Scale1;
    config.rcc.supply_config = SupplyConfig::DirectSMPS;
    config.rcc.mux.usbsel = mux::Usbsel::HSI48;

}
// /// Initializes the Ethernet interface with static IP.
// fn setup_ethernet(
//     eth: peripherals::ETH,
//     pa1: peripherals::PA1,
//     pa2: peripherals::PA2,
//     pc1: peripherals::PC1,
//     pa7: peripherals::PA7,
//     pc4: peripherals::PC4,
//     pc5: peripherals::PC5,
//     pg13: peripherals::PG13,
//     pb13: peripherals::PB13,
//     pg11: peripherals::PG11,
//     rng: peripherals::RNG,
//     mac_addr: [u8; 6],
// ) -> (Stack<'static>, embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>)
//  {
    
    
// }



// =============================================
//              TASKS
// =============================================

/// UDP Network Task
///
/// - Receives UDP packets that contain a 6-byte header: [dst_ip (4 bytes)][dst_port (2 bytes)]
/// - Forwards the rest of the data to the destination.
/// - If dst_ip == sender_ip, behaves like echo.
#[embassy_executor::task]
async fn udp_task(stack: &'static Stack<'static>) -> () {
    let mut rx_meta = [PacketMetadata::EMPTY; RX_METADATA_COUNT];
    let mut rx_buffer = [0; RX_BUFFER_SIZE];
    let mut tx_meta = [PacketMetadata::EMPTY; TX_METADATA_COUNT];
    let mut tx_buffer = [0; TX_BUFFER_SIZE];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    socket.bind(NETWORK_UDP_PORT).unwrap();
    let data = b"koekoek";
    let endpoint = IpEndpoint::new(DESTINATION_IP.into(), DESTINATION_PORT);
    match socket.send_to(data, endpoint).await {
        Ok(_) => info!("data is sent"),
        Err(_) => warn!("data isn't sent"),
    }
    let data = CHANNEL.receive().await;
    match socket.send_to(data, endpoint).await {
        Ok(_) => {match core::str::from_utf8(data) {
                    Ok(s) => info!("UDP sent: {}", s),
                    Err(_) => info!("UDP sent: (non-UTF8 data)")}},
        Err(e) => warn!("UDP send error: {:?}", e),
    }
        
}


#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericPhy>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) -> ! {
    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");
        CHANNEL.send(b"button 2 pressed").await;
        button.wait_for_falling_edge().await;
        info!("Released!");
    }
}

// =============================================
//              MAIN
// =============================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    validate_config();
    configure_clock(&mut config);
    info!("blabla");
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 8>::new()),
        p.ETH,
        Irqs,
        p.PA1, p.PA2, p.PC1, p.PA7,
        p.PC4, p.PC5, p.PG13, p.PB13, p.PG11,
        GenericPhy::new_auto(),
        mac_addr,
);

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(NETWORK_GATEWAY_IP),
    });

    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.try_fill_bytes(&mut seed).unwrap();
    let seed = u64::from_le_bytes(seed);

    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let stack = STACK.init(stack);

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");
    spawner.spawn(button_task(button)).expect("Failed to spawn button task");

}