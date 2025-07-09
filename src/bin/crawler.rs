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
    bind_interrupts,
    eth::{self, generic_smi::GenericSMI, Ethernet, PacketQueue},
    gpio::{Level, Output, Speed},
    peripherals::{self, ETH},
    rcc::*,
    rng::{InterruptHandler as RngInterruptHandler, Rng},
    usb::{self, Driver},
    Config,
    SharedData
};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex, 
    pipe::Pipe, signal::Signal,
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use static_cell::StaticCell;
use defmt::{*, assert};
use core::mem::MaybeUninit;
use embassy_net::EthernetAddress;
// =============================================
//              CONFIGURATION
// =============================================

// --- Network Configuration ---
// Now using MAC-based IP generation instead of hardcoded IPs
const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(169, 254, 1, 1);
const NETWORK_GATEWAY_UDP_PORT: u16 = 4321;

// --- Buffers ---
const USB_BUFFER_SIZE: usize = 512;
const NETWORK_RX_BUFFER_SIZE: usize = 1024;

// -- PC to Nucleo USB Command Protocol --
const CMD_LED_ON: u8 = 0x01;
const CMD_LED_OFF: u8 = 0x02;

// -- Network Crawler Protocol --
const DISCOVERY_PORT: u16 = 12345;
const DATA_FORWARDING_PORT: u16 = 4321;

const DISCOVERY_REQUEST: u8 = 0xA0;
const DISCOVERY_RESPONSE: u8 = 0xA1;

fn validate_config() {
    assert!(USB_BUFFER_SIZE >= 64, "USB_BUFFER_SIZE must be at least 64");
    assert!(NETWORK_RX_BUFFER_SIZE >= 512, "NETWORK_RX_BUFFER_SIZE must be at least 512");
}

// =============================================
//              STATIC ALLOCATIONS
// =============================================

// USB Buffers
static EP_OUT_BUFFER: StaticCell<[u8; USB_BUFFER_SIZE]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static STATE: StaticCell<State> = StaticCell::new();

// Network Buffers
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();

// Inter-task Communication
static USB_TO_ETH_PIPE: Pipe<ThreadModeRawMutex, 4096> = Pipe::new();
static LED_SIGNAL: Signal<ThreadModeRawMutex, bool> = Signal::new();

// Hardware Shared Data
#[link_section = ".ram_d3.shared_data"]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// =============================================
//              HARDWARE SETUP
// =============================================
bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

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

/// Generate MAC address from STM32 unique ID - always available regardless of network state
fn generate_mac_from_serial() -> [u8; 6] {
    let uid_base = 0x1FF1_E800 as *const u32;
    let uid0 = unsafe { core::ptr::read_volatile(uid_base.offset(0)) };
    let uid1 = unsafe { core::ptr::read_volatile(uid_base.offset(1)) };

    [
        0x02, // Locally administered MAC (bit 1 set), unicast (bit 0 cleared)
        (uid0 >> 0) as u8,
        (uid0 >> 8) as u8,
        (uid0 >> 16) as u8,
        (uid0 >> 24) as u8,
        (uid1 >> 0) as u8,
    ]
}

// Still doing statically at the moment, should happne dynamically in the future,
// Trouble with generating unique addresses from mac address
// Mac addres is assigned dynamically however
fn generate_link_local_ip() -> Ipv4Address {
    // Ipv4Address::new(169, 254, 1, 1) // First address
    Ipv4Address::new(169, 254, 1, 2) // Second address
    // Ipv4Address::new(169, 254, 1, 3) // Third address
    // Ipv4Address::new(169, 254, 1, 4) // Fourth address

}

fn setup_usb(
    p: peripherals::USB_OTG_FS,
    pa12: peripherals::PA12,
    pa11: peripherals::PA11,
) -> (CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>, UsbDevice<'static, Driver<'static, peripherals::USB_OTG_FS>>) {
    let ep_out_buffer = EP_OUT_BUFFER.init([0u8; USB_BUFFER_SIZE]);
    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;

    let driver = Driver::new_fs(p, Irqs, pa12, pa11, &mut *ep_out_buffer, usb_config);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB Serial");
    config.serial_number = Some("12345678");

    let config_descriptor = CONFIG_DESCRIPTOR.init([0; 256]);
    let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 64]);
    let state = STATE.init(State::new());

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor[..],
        &mut bos_descriptor[..],
        &mut [],
        &mut control_buf[..],
    );

    let class = CdcAcmClass::new(&mut builder, state, 64);
    let usb = builder.build();
    (class, usb)
}

/// Setup Ethernet with MAC-based static IP - no DHCP dependency
fn setup_ethernet(
    eth: peripherals::ETH,
    pa1: peripherals::PA1,
    pa2: peripherals::PA2,
    pc1: peripherals::PC1,
    pa7: peripherals::PA7,
    pc4: peripherals::PC4,
    pc5: peripherals::PC5,
    pg13: peripherals::PG13,
    pb13: peripherals::PB13,
    pg11: peripherals::PG11,
    rng: peripherals::RNG,
    mac_addr: [u8; 6],
) -> (Stack<'static>, embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>) {
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 8>::new()),
        eth,
        Irqs,
        pa1,
        pa2,
        pc1,
        pa7,
        pc4,
        pc5,
        pg13,
        pb13,
        pg11,
        GenericSMI::new(0),
        mac_addr,
    );

    // Generate link-local IP based on MAC address
    let ip = generate_link_local_ip();
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(ip, 16),
        gateway: None,
        dns_servers: Default::default(),
    });

    let mut rng = Rng::new(rng, Irqs);
    let mut seed = [0; 8];
    rng.try_fill_bytes(&mut seed).unwrap();
    let seed = u64::from_le_bytes(seed);

    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    (stack, runner)
}

// =============================================
//              TASKS
// =============================================

/// Discovery sender - only runs when network is ready, program continues without it
#[embassy_executor::task]
async fn discovery_sender_task(stack: &'static Stack<'static>) -> ! {
    // Wait for network readiness, but don't block main program
    wait_for_network_ready(stack).await;

    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buf = [0; 256];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buf = [0; 256];

    let mut socket = UdpSocket::new(*stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);

    match socket.bind(DISCOVERY_PORT + 1) {
        Ok(_) => info!("Discovery sender bound to port {}", DISCOVERY_PORT + 1),
        Err(e) => {
            error!("Failed to bind discovery sender: {:?}", e);
        }
    }

    let discovery_msg = [DISCOVERY_REQUEST];

    loop {
        // Use broadcast in link-local range
        let broadcast_ip = Ipv4Address::new(169, 254, 255, 255);
        let broadcast_endpoint = IpEndpoint::new(broadcast_ip.into(), DISCOVERY_PORT);

        // Try to get the current IP from stack
        let current_ip = stack.config_v4().map(|c| c.address.address());
        if let Some(ip) = current_ip {
            info!("Sending discovery from IP {} to broadcast", ip);
        } else {
            info!("Sending discovery to broadcast (IP not yet assigned)");
        }

        match socket.send_to(&discovery_msg, broadcast_endpoint).await {
            Ok(_) => info!("Sent discovery to broadcast"),
            Err(e) => warn!("Failed to send broadcast: {:?}", e),
        }

        // Wait for response with timeout
        let mut payload_buf = [0u8; 256];
        match embassy_time::with_timeout(
            embassy_time::Duration::from_secs(2),
            socket.recv_from(&mut payload_buf)
        ).await {
            Ok(Ok((n, sender))) => {
                if n > 0 && payload_buf[0] == DISCOVERY_RESPONSE {
                    info!("*** DISCOVERY SUCCESS! Found MCU at {:?} ***", sender);
                } else {
                    info!("Unexpected response from {:?}: {:02X}", sender, payload_buf[0]);
                }
            }
            Ok(Err(e)) => {
                warn!("Discovery receive error: {:?}", e);
            }
            Err(_) => {
                info!("Discovery timeout - no response received");
            }
        }

        embassy_time::Timer::after_secs(5).await;
    }
}

/// Discovery responder - only runs when network is ready
#[embassy_executor::task]
async fn discovery_responder_task(stack: &'static Stack<'static>) -> ! {
    wait_for_network_ready(stack).await;

    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buf = [0; 256];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buf = [0; 256];

    let mut socket = UdpSocket::new(*stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);

    // Try to get the current IP from stack
    let current_ip = stack.config_v4().map(|c| c.address.address());
    match socket.bind(DISCOVERY_PORT) {
        Ok(_) => {
            if let Some(ip) = current_ip {
                info!("Discovery responder bound to port {} with IP {}", DISCOVERY_PORT, ip);
            } else {
                info!("Discovery responder bound to port {} (IP not yet assigned)", DISCOVERY_PORT);
            }
        }
        Err(e) => {
            error!("Failed to bind discovery responder: {:?}", e);
        }
    }

    let response = [DISCOVERY_RESPONSE];

    loop {
        let mut payload_buf = [0u8; 256];
        match socket.recv_from(&mut payload_buf).await {
            Ok((n, sender)) => {
                info!("Received {} bytes from {:?}", n, sender);
                if n > 0 && payload_buf[0] == DISCOVERY_REQUEST {
                    info!("*** DISCOVERY REQUEST RECEIVED from {:?} ***", sender);
                    match socket.send_to(&response, sender).await {
                        Ok(_) => info!("Sent discovery response to {:?}", sender),
                        Err(e) => error!("Failed to send response: {:?}", e),
                    }
                } else {
                    info!("Unexpected packet: {:02X} from {:?}", payload_buf[0], sender);
                }
            }
            Err(e) => {
                warn!("Discovery responder receive error: {:?}", e);
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}

/// LED task - works independently of network state
#[embassy_executor::task]
async fn led_task(mut led: Output<'static>, mac_addr: [u8; 6], stack: &'static Stack<'static>) -> ! {
    loop {
        let led_state = LED_SIGNAL.wait().await;
        if led_state {
            led.set_high();
            info!("LED ON");
        } else {
            led.set_low();
            info!("LED OFF");
        }

        // MAC and IP are always available, regardless of network state
        let ip = stack.config_v4().map(|c| c.address.address());
        if let Some(ip) = ip {
            defmt::info!(
                "MAC: {:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}, IP: {}",
                mac_addr[0], mac_addr[1], mac_addr[2], mac_addr[3], mac_addr[4], mac_addr[5],
                ip
            );
        } else {
            defmt::info!(
                "MAC: {:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}, IP: (not assigned)",
                mac_addr[0], mac_addr[1], mac_addr[2], mac_addr[3], mac_addr[4], mac_addr[5]
            );
        }

        // Network status - informational only, doesn't block functionality
        if stack.is_link_up() {
            info!("Network link is UP");
            if let Some(_config) = stack.config_v4() {
                info!("Network fully configured and ready");
            } else {
                info!("Link up but configuration pending");
            }
        } else {
            info!("Network link is DOWN (cable disconnected)");
        }

        embassy_time::Timer::after_millis(200).await;
    }
}

/// Helper function - waits for network readiness but doesn't block main program
async fn wait_for_network_ready(stack: &'static Stack<'static>) {
    info!("Waiting for network to be ready...");
    
    // Wait for physical link (cable connected)
    loop {
        if stack.is_link_up() {
            info!("Physical link is up");
            break;
        }
        embassy_time::Timer::after_millis(100).await;
    }
    
    // Wait for IP configuration to be applied
    loop {
        if stack.config_v4().is_some() {
            if let Some(config) = stack.config_v4() {
                let ip = config.address.address();
                info!("Network ready with IP: {}", ip);
                break;
            }
        }
        embassy_time::Timer::after_millis(100).await;
    }
}

/// USB task - works independently of network state
#[embassy_executor::task]
async fn usb_task(
    mut class: CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) -> ! {
    let mut buf = [0; 512];
    loop {
        info!("Waiting for USB connection");
        class.wait_connection().await;
        info!("USB connected");

        loop {
            match class.read_packet(&mut buf).await {
                Ok(n) => {
                    if n > 0 {
                        info!("Received {} bytes over USB", n);

                        if buf[0] == CMD_LED_ON {
                            info!("LED ON command received");
                            LED_SIGNAL.signal(true);
                        } else if buf[0] == CMD_LED_OFF {
                            info!("LED OFF command received");
                            LED_SIGNAL.signal(false);
                        } else {
                            // Forward to network if available
                            USB_TO_ETH_PIPE.write(&buf[..n]).await;
                        }
                    }
                }
                Err(_) => {
                    info!("USB disconnected");
                    break;
                }
            }
        }
    }
}

/// UDP task - only active when network is ready
#[embassy_executor::task]
async fn udp_task(stack: &'static Stack<'static>) -> ! {
    wait_for_network_ready(stack).await;

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 1024];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 1024];
    
    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer
    );

    match socket.bind(DATA_FORWARDING_PORT + 100) {
        Ok(_) => info!("UDP data forwarding bound to port {}", DATA_FORWARDING_PORT + 100),
        Err(e) => {
            error!("Failed to bind UDP data socket: {:?}", e);
        }
    }
    
    let remote_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(NETWORK_GATEWAY_IP), 
        DATA_FORWARDING_PORT
    );

    let mut buf = [0; 512];
    loop {
        let n = USB_TO_ETH_PIPE.read(&mut buf).await;
        info!("Forwarding {} bytes over UDP", n);

        match socket.send_to(&buf[..n], remote_endpoint).await {
            Ok(_) => info!("UDP send successful"),
            Err(e) => error!("UDP send error: {:?}", e),
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>) -> ! {
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

    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // Initialize LED - works regardless of network/USB state
    let led = Output::new(p.PB14, Level::Low, Speed::Low);

    // Initialize USB - works independently
    let (class, mut usb) = setup_usb(p.USB_OTG_FS, p.PA12, p.PA11);

    // Generate MAC - always available
    let mac = generate_mac_from_serial();
    let (stack, runner) = setup_ethernet(
        p.ETH, p.PA1, p.PA2, p.PC1, p.PA7,
        p.PC4, p.PC5, p.PG13, p.PB13, p.PG11, p.RNG,
        mac
    );
    let stack = STACK.init(stack);

    info!("Device initialized with MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", 
          mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

    // Spawn all tasks - they handle their own readiness requirements
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(led_task(led, mac, stack)).expect("Failed to spawn LED task");
    spawner.spawn(usb_task(class)).expect("Failed to spawn USB task");

    // Network-dependent tasks (will wait for network internally)
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");

    // Choose one based on device role:
    // spawner.spawn(discovery_sender_task(stack)).expect("Failed to spawn discovery sender");
    // OR
    spawner.spawn(discovery_responder_task(stack)).expect("Failed to spawn discovery responder");

    // USB device runs independently
    usb.run().await;
}