#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use rand_core::RngCore;
use embassy_executor::Spawner;
use embassy_net::{
    tcp::TcpSocket,
    StackResources,
    Ipv4Address,
    Ipv4Cidr,
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
use defmt::*;
use core::mem::MaybeUninit;

// =============================================
//              CONFIGURATION
// =============================================
// Grouped all constants into logical sections with documentation.


// --- Network Configuration ---
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 60); // IP of sender
const TCP_LISTEN_PORT: u16 = 12345; // chosen arbitrarily

// Buffer Sizes

// RX: receiving side, TX: sender side
// These are buffers used to store udp/tcp packets before write or read is called
// buffer size defines how many bytes can be in the buffer before dropping new packets
const RX_BUFFER_SIZE: usize = 2048;
const TX_BUFFER_SIZE: usize = 1; // 1 is minimal valid value
// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Network Buffers
// The amount of packets that can be stored in the network buffer. This space is shared over all sockets
// receiving side: packet goes into packetQueue and then in RX_buffer
// Sender side: packet goes into TX_buffer and gets assembled, then into packetQueue
static PACKETS: StaticCell<PacketQueue<2, 8>> = StaticCell::new();
// The max amount of sockets
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
// Coordinates the network
static STACK: StaticCell<Stack<'static>> = StaticCell::new();


// Hardware Shared Data
// This is for sharing the memory between the two cores of the stm32
#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// =============================================
//              HARDWARE SETUP
// =============================================
bind_interrupts!(struct Irqs {
	// This makes sure that when an ethernet event happens, it is handled properly (signals packetQueue and the socket)
    ETH => eth::InterruptHandler;
	// when a new random number is generated, wake the required eventhandler
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

#[embassy_executor::task]
async fn tcp_task(stack: &'static Stack<'static>) {
    let mut rx_buffer = [0; RX_BUFFER_SIZE];
    let mut tx_buffer = [0; TX_BUFFER_SIZE];

    let mut socket = TcpSocket::new(*stack, &mut rx_buffer, &mut tx_buffer);

    socket.set_timeout(None);

    socket.accept(TCP_LISTEN_PORT).await.unwrap();
    info!("Client connected!");

    let mut buf = [0u8; 512];

    loop {
        match socket.read(&mut buf).await {
            Ok(0) => {
                info!("Connection closed");
                break;
            }
            Ok(n) => {
                info!("Received {} bytes: {:?}", n, &buf[..n]);
            }
            Err(e) => {
                warn!("Read error: {:?}", e);
                break;
            }
        }
    }

    socket.close();
    socket.abort();
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
    configure_clock(&mut config);

		// initializes the primary core and returns the peripherals (GPIO pins, adc, rng,...
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);
    // mac address of this stm32 (chosen arbitrarily)
    // mac address of the receiving stm32 is received by ARP with the ip address of that stm32
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<2, 8>::new()),
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
    spawner.spawn(tcp_task(stack)).expect("Failed to spawn UDP task");

}