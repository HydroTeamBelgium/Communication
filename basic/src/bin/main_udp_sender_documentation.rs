#![no_std]
#![no_main]

use defmt_rtt as _;
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
use embassy_time::{Timer, Duration};

// =============================================
//              CONFIGURATION
// =============================================
// Grouped all constants into logical sections with documentation.


// --- Network Configuration ---
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61); // IP of sender
const DESTINATION_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 60); // IP of receiver
const DESTINATION_PORT: u16 = 12345; // chosen arbitrarily

// Buffer Sizes

// RX: receiving side, TX: sender side
// These are buffers used to store udp/tcp packets before recv_from or send_to is called
// buffer size defines how many bytes can be in the buffer before dropping new packets
const RX_BUFFER_SIZE: usize = 1; // 1 is minimal valid value
const TX_BUFFER_SIZE: usize = 2048;
const MAX_PACKET_SIZE: usize = 1536; // a little above max size for ethernet packet (1530 or actually 1542 it seems, but already overkill)
// Socket Configuration
// Metadata is things like length of packet, ip addr...
// The metadata count defines how many packets can sit in the buffer before dropping new packets
const RX_METADATA_COUNT: usize = 1; // 1 is minimal valid value 
const TX_METADATA_COUNT: usize = 32;
// Validate configuration
// Packets should always be able to fit inside the buffer when the buffer is empty
fn validate_config() {
	assert!(RX_BUFFER_SIZE >= 0, "RX buffer too small");    
	assert!(TX_BUFFER_SIZE >= MAX_PACKET_SIZE, "TX buffer too small");
}


// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Network Buffers
static PACKETS: StaticCell<PacketQueue<8, 2>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();


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

// =============================================
//              TASKS
// =============================================
/// UDP Network Task
/// Sends the message Hello world to the chosen ip address
#[embassy_executor::task]
async fn udp_task(stack: &'static Stack<'static>) -> () {
    // rx/tx buffers are per socket (rx/tx_meta is for the metadata of incoming/outgoing packets) 
    let mut rx_meta = [PacketMetadata::EMPTY; RX_METADATA_COUNT]; // You could make buffers of different sizes for each socket
    let mut rx_buffer = [0; RX_BUFFER_SIZE];    
    let mut tx_meta = [PacketMetadata::EMPTY; TX_METADATA_COUNT];    
    let mut tx_buffer = [0; TX_BUFFER_SIZE];
	// creates a socket with the created network stack, that is shared over all sockets
    let mut socket = UdpSocket::new(*stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer,);
    // listen to a port for incoming messages (not needed)
    socket.bind(0).unwrap(); // 0, because we don't want to receive data
    let data = b"Hello world";    
    let endpoint = IpEndpoint::new(DESTINATION_IP.into(), DESTINATION_PORT);
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(200)).await;
    }

    match socket.send_to(data, endpoint).await {
		    Ok(_) => info!("data is sent"), 
		    Err(_) => warn!("data isn't sent"),    
		}
    Timer::after(Duration::from_millis(10)).await;
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericPhy>>) -> ! {
    runner.run().await
}


// =============================================
//              MAIN
// =============================================
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    validate_config();
    configure_clock(&mut config);

    // initializes the primary core and returns the peripherals (GPIO pins, adc, rng,...
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 2>::new()),
        p.ETH,
        Irqs,
        p.PA1, p.PA2, p.PC1, p.PA7,
        p.PC4, p.PC5, p.PG13, p.PB13, p.PG11,
        GenericPhy::new_auto(),
        mac_addr,
);

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24), // our ip address and /24 as subnet mask
        dns_servers: heapless::Vec::new(), // No DNS is needed for us
        gateway: None, // No gateway is needed on local network
    });

    // create a random number and let it get handled by the Irqs interrupt handler
    let mut rng = Rng::new(p.RNG, Irqs);
    // create a random array of 8 bytes
    let mut seed = [0; 8];
    rng.try_fill_bytes(&mut seed).unwrap();
    // converts 8 bytes into a 64-bit unsigned integer in little-endian order
    let seed = u64::from_le_bytes(seed);

    // the random seed is used for creating a random port when needed, random time-out when a collision happened... (less predictable and attackable by hackers)
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let stack = STACK.init(stack);

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");

}