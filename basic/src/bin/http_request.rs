#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_executor::Spawner;
use embassy_net::{
    tcp::client::{TcpClient, TcpClientState},
    StackResources,
    Ipv4Address,
    Ipv4Cidr,
    Stack,
    dns::DnsSocket,

};
use reqwless::client::{HttpClient, TlsConfig};

use embassy_stm32::{
    eth::{Ethernet, PacketQueue},
    eth,
    rng::{Rng, InterruptHandler as RngInterruptHandler},
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
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 5); // IP of sender
const MODEM_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 1);

// Buffer Sizes

// RX: receiving side, TX: sender side
// These are buffers used to store udp/tcp packets before write or read is called
// buffer size defines how many bytes can be in the buffer before dropping new packets
const RX_BUFFER_SIZE: usize = 1; // 1 is minimal valid value
const TX_BUFFER_SIZE: usize = 2048;
// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Network Buffers
// The amount of packets that can be stored in the network buffer. This space is shared over all sockets
// receiving side: packet goes into packetQueue and then in RX_buffer
// Sender side: packet goes into TX_buffer and gets assembled, then into packetQueue
static PACKETS: StaticCell<PacketQueue<8, 2>> = StaticCell::new();
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



async fn access_website(stack: Stack<'_>, tls_seed: u64) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 4096];
    let mut http_req = client
        .request(
            reqwless::request::Method::GET,
            "https://jsonplaceholder.typicode.com/posts/1",
        )
        .await
        .unwrap();
    let response = http_req.send(&mut buffer).await.unwrap();

    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    println!("{}", content);
}



type EthernetDevice = embassy_stm32::eth::Ethernet<
    'static,
    embassy_stm32::peripherals::ETH,
    embassy_stm32::eth::GenericPhy<
        embassy_stm32::eth::Sma<'static, embassy_stm32::peripherals::ETH_SMA>,
    >,
>;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, EthernetDevice>) -> ! {
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
        PACKETS.init(PacketQueue::<8, 2>::new()),
        p.ETH,
        Irqs,
        p.PA1, p.PA7, p.PC4, p.PC5,
        p.PG13, p.PB13, p.PG11,
        mac_addr,
        p.ETH_SMA, p.PA2, p.PC1,
    );

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24), // our ip address and /24 as subnet mask
        dns_servers: heapless::Vec::from_slice(&[MODEM_IP]).unwrap(),
        gateway: Some(MODEM_IP),
    });

    // create a random number and let it get handled by the Irqs interrupt handler
    let mut rng = Rng::new(p.RNG, Irqs);
    // create a random array of 8 bytes
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    // converts 8 bytes into a 64-bit unsigned integer in little-endian order
    let seed = u64::from_le_bytes(seed);

		// the random seed is used for creating a random port when needed, random time-out when a collision happened... (less predictable and attackable by hackers)
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    //let stack = STACK.init(stack);
    let tls_seed = rng.next_u64();

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");

    access_website(stack, tls_seed).await;
}