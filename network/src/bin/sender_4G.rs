#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use panic_probe as _;
use rand_core::RngCore;
use embassy_executor::Spawner;
use embassy_net::{
    tcp::{TcpSocket},
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
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 50);
const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 1);

// const NETWORK_UDP_PORT: u16 = 4321;

// const DESTINATION_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 60);
// const DESTINATION_PORT: u16 = 12345;


// Buffer sizes
const TCP_RX_BUFFER_SIZE: usize = 2048;
const TCP_TX_BUFFER_SIZE: usize = 2048;

// Socket Configuration
// const RX_METADATA_COUNT: usize = 32;  
// const TX_METADATA_COUNT: usize = 32;

// // Validate configuration
// fn validate_config() {
//     assert!(RX_BUFFER_SIZE >= MAX_PACKET_SIZE, "RX buffer too small");
//     assert!(TX_BUFFER_SIZE >= MAX_PACKET_SIZE, "TX buffer too small");
// }


// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Network Buffers
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Network: your laptop IP (hotspot) and chosen port
const SERVER_IP: Ipv4Address = Ipv4Address::new(192, 168, 24, 70);
const SERVER_PORT: u16 = 5000;

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


// =============================================
//              TASKS
// =============================================

/// TCP client task: repeatedly connects to SERVER_IP:SERVER_PORT,
/// sends a greeting, attempts to read a reply, then disconnects and retries.
#[embassy_executor::task]
async fn tcp_task(stack: &'static Stack<'static>) {
    // local buffers for the TCP socket
    let mut rx_buf = [0u8; TCP_RX_BUFFER_SIZE];
    let mut tx_buf = [0u8; TCP_TX_BUFFER_SIZE];

    loop {
        // Create a socket
        let mut socket = TcpSocket::new(*stack, &mut rx_buf, &mut tx_buf);
        // Optional: set a timeout (SmolDuration) if you want
        // socket.set_timeout(Some(embassy_net::SmolDuration::from_secs(10)));

        info!("Connecting to server {}:{}", SERVER_IP, SERVER_PORT);
        let endpoint = IpEndpoint::new(SERVER_IP.into(), SERVER_PORT);

        if let Err(e) = socket.connect(endpoint).await {
            warn!("TCP connect failed: {:?}", e);
            // wait and retry
            // Timer::after(Duration::from_secs(5)).await;
            // continue;
        }

        info!("Connected to server: {:?}", socket.remote_endpoint());

        // Prepare message
        let msg = b"Hello from STM32!\n";

        // Write the message (may write less than msg.len())
        match socket.write(msg).await {
            Ok(n) => info!("Wrote {} bytes", n),
            Err(e) => {
                warn!("Write error: {:?}", e);
                // Close socket and retry
                let _ = socket.close();
                // Timer::after(Duration::from_secs(2)).await;
                // continue;
            }
        }

        // Try to read a reply (blocking read until data or EOF)
        let mut read_buf = [0u8; 512];
        match socket.read(&mut read_buf).await {
            Ok(0) => {
                // EOF / connection closed by peer
                info!("Read 0 bytes (EOF)");
            }
            Ok(n) => {
                if let Ok(s) = core::str::from_utf8(&read_buf[..n]) {
                    info!("Received reply ({} bytes): {}", n, s);
                } else {
                    info!("Received reply ({} bytes, non-UTF8)", n);
                }
            }
            Err(e) => {
                warn!("Read error: {:?}", e);
            }
        }

        // Close connection gracefully
        let _ = socket.close();

        // Wait before reconnecting
        // Timer::after(Duration::from_secs(5)).await;
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
    // validate_config();
    configure_clock(&mut config);
    info!("blabla");
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0x62];

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
    spawner.spawn(tcp_task(stack)).expect("Failed to spawn UDP task");
    spawner.spawn(button_task(button)).expect("Failed to spawn button task");

}