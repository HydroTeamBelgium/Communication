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
    eth::{Ethernet, PacketQueue, generic_smi::GenericSMI},
    eth,
    rng::{Rng, InterruptHandler as RngInterruptHandler},
    peripherals::ETH,
    bind_interrupts,
    SharedData,
    peripherals,
    usb::Driver,
    usb,
    Config,
    rcc::*,
};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex, 
    pipe::Pipe,
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use embassy_futures::select::{select, Either};
use static_cell::StaticCell;
use defmt::{*, assert};
use core::{mem::MaybeUninit};
use embassy_time::Timer;

// =============================================
//              CONFIGURATION
// =============================================
// Grouped all constants into logical sections with documentation.


// --- Network Configuration ---
const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61);
const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
const NETWORK_UDP_PORT: u16 = 4321;


// --- Buffers ---
const USB_BUFFER_SIZE: usize = 2048;
const NETWORK_RX_BUFFER_SIZE: usize = 2048;

const USB_DELAY: u64 = 1;


// Ensure buffers meet minimum requirements
fn validate_config() {
    assert!(USB_BUFFER_SIZE >= 64, "USB_BUFFER_SIZE must be at least 64");
    assert!(NETWORK_RX_BUFFER_SIZE >= 512, "NETWORK_RX_BUFFER_SIZE must be at least 512");
}


// =============================================
//              STATIC ALLOCATIONS
// =============================================
// Grouped buffers by purpose (USB vs. Network)

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
static ETH_TO_USB_PIPE: Pipe<ThreadModeRawMutex, 131072> = Pipe::new();

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

/// Initializes the USB CDC ACM (Serial) interface.
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

/// Initializes the Ethernet interface with static IP.
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
) -> (Stack<'static>, embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>)
 {
    
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

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(NETWORK_GATEWAY_IP),
    });

    let mut rng = Rng::new(rng, Irqs);
    let mut seed = [0; 8];
    rng.try_fill_bytes(&mut seed).unwrap();
    let seed = u64::from_le_bytes(seed);

    embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed)
}



// =============================================
//              TASKS
// =============================================

/// USB Communication Task
/// 
/// - Reads data from USB serial port.
/// - Writes data to a pipe for the UDP task.

#[embassy_executor::task]
async fn usb_task(
    mut class: CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) -> ! {
    const CHUNK_SIZE: usize = 64; // Match your USB endpoint size
    let mut usb_rx_buf = [0u8; CHUNK_SIZE];
    let mut pipe_rx_buf = [0u8; 512]; // Keep UDP-sized buffer

    loop {
        class.wait_connection().await;
        trace!("USB connected");

        loop {
            match select(
                class.read_packet(&mut usb_rx_buf),
                ETH_TO_USB_PIPE.read(&mut pipe_rx_buf),
            )
            .await
            {
                Either::First(Ok(n)) => {
                    if n > 0 {
                        trace!("Received {} bytes over USB", n);
                        USB_TO_ETH_PIPE.write(&usb_rx_buf[..n]).await;
                    }
                }
                Either::First(Err(_)) => {
                    trace!("USB disconnected");
                    break;
                }
                Either::Second(n) => {
                    if n > 0 {
                        trace!("Forwarding {} bytes over USB (chunked)", n);
                        // Chunk the data to match USB endpoint size
                        for chunk in pipe_rx_buf[..n].chunks(CHUNK_SIZE) {
                            let mut attempts = 0;
                            let mut success = false;
                            while attempts < 10 {
                                match class.write_packet(chunk).await {
                                    Ok(_) => { 
                                        success = true; 
                                        Timer::after_nanos(USB_DELAY).await;  // Small delay
                                        break; }
                                    Err(_e) => {
                                        attempts += 1;
                                        Timer::after_nanos(USB_DELAY * attempts).await;  // Backoff
                                    }
                                }
                            }
                            if !success {
                                error!("USB write failed after retries");
                            }
                        }
                    }
                }
            }
        }
    }
}

/// UDP Network Task
/// 
/// - Reads data from the USB pipe.
/// - Sends data over UDP.
#[embassy_executor::task]
async fn udp_task(stack: &'static Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 16384];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 8192];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    socket.bind(NETWORK_UDP_PORT).unwrap();
    let remote_endpoint =
        IpEndpoint::new(embassy_net::IpAddress::Ipv4(NETWORK_GATEWAY_IP), NETWORK_UDP_PORT);

    loop {
        let mut buf_usb_to_udp = [0; 512];
        let mut buf_udp_to_usb = [0; 512];

        match select(
            USB_TO_ETH_PIPE.read(&mut buf_usb_to_udp),
            socket.recv_from(&mut buf_udp_to_usb),
        )
        .await {
            Either::First(n) => {
                trace!("Forwarding {} bytes over UDP", n);
                match socket.send_to(&buf_usb_to_udp[..n], remote_endpoint).await {
                    Ok(_) => trace!("UDP send successful"),
                    Err(e) => error!("UDP send error: {:?}", e),
                }
            }
            Either::Second(Ok((n, _addr))) => {
                trace!("Received {} bytes from UDP", n);
                ETH_TO_USB_PIPE.write(&buf_udp_to_usb[..n]).await;
            }
            Either::Second(Err(e)) => {
                error!("UDP receive error: {:?}", e);
            }
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

    // Initialize Hardware
    let (class, mut usb) = setup_usb(p.USB_OTG_FS, p.PA12, p.PA11);
    let (stack, runner) = setup_ethernet(
    p.ETH, p.PA1, p.PA2, p.PC1, p.PA7,
    p.PC4, p.PC5, p.PG13, p.PB13, p.PG11, p.RNG,
    [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]
    );
    let stack = STACK.init(stack);

    // Spawn Tasks
    spawner.spawn(net_task(runner)).expect("Failed to spawn net task");
    spawner.spawn(udp_task(stack)).expect("Failed to spawn UDP task");
    spawner.spawn(usb_task(class)).expect("Failed to spawn USB task");

    // Run USB Device
    usb.run().await;
}