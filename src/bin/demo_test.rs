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
};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex, 
    pipe::Pipe,
    mutex::Mutex
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    UsbDevice,
};
use static_cell::StaticCell;
use defmt::*;
use core::mem::MaybeUninit;
use embassy_time::{Timer, Duration, with_timeout};

// Network config
const NUCLEO_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61);
const GATEWAY: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
const UDP_PORT: u16 = 4321;
const CHUNK_SIZE: usize = 512;

// Static buffers
static EP_OUT_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static STATE: StaticCell<State> = StaticCell::new();
static STACK: StaticCell<Stack<'static>> = StaticCell::new();
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
static USB_TO_ETH_PIPE: Pipe<ThreadModeRawMutex, 4096> = Pipe::new();
static USB_MUTEX: Mutex<ThreadModeRawMutex, ()> = Mutex::new(());

#[link_section = ".ram_d3.shared_data"]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
    HASH_RNG => RngInterruptHandler<peripherals::RNG>;
});

#[embassy_executor::task]
async fn usb_task(
    mut class: CdcAcmClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) -> ! {
    let mut buf = [0; CHUNK_SIZE];
    loop {
        class.wait_connection().await;
        info!("USB connected");

        loop {
            let _guard = USB_MUTEX.lock().await;
            match with_timeout(
                Duration::from_secs(1),
                class.read_packet(&mut buf)
            ).await {
                Ok(Ok(n)) if n > 0 => {
                    let _ = USB_TO_ETH_PIPE.write(&buf[..n]).await;
                },
                _ => {
                    info!("USB disconnected or timeout");
                    break;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn udp_task(stack: &'static Stack<'static>) -> ! {
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

    socket.bind(UDP_PORT).unwrap();
    let remote_endpoint = IpEndpoint::new(embassy_net::IpAddress::Ipv4(GATEWAY), UDP_PORT);

    let mut usb_buf = [0u8; CHUNK_SIZE];
    
    loop {
        // Read from USB pipe with timeout
        let data_len = match with_timeout(
            Duration::from_secs(2),
            USB_TO_ETH_PIPE.read(&mut usb_buf)
        ).await {
            Ok(len) => len,
            Err(_) => {
                warn!("USB pipe read timeout");
                continue;
            }
        };

        // Validate data length
        if data_len == 0 || data_len > CHUNK_SIZE {
            warn!("Invalid data length: {}", data_len);
            continue;
        }

        // Send with retries
        for attempt in 0..3 {
            match socket.send_to(&usb_buf[..data_len], remote_endpoint).await {
                Ok(_) => {
                    info!("Sent {} bytes", data_len);
                    break;
                },
                Err(e) if attempt == 2 => {
                    error!("Failed to send data: {:?}", e);
                },
                Err(_) => {
                    Timer::after_millis(100 * (attempt + 1)).await;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn monitor() {
    loop {
        info!("System operational");
        Timer::after_secs(5).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
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

    let p = embassy_stm32::init_primary(config, &SHARED_DATA);

    // USB Setup
    let ep_out_buffer = EP_OUT_BUFFER.init([0u8; CHUNK_SIZE]);
    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;

    let driver = Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut *ep_out_buffer,
        usb_config,
    );

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
    let mut usb = builder.build();

    // Ethernet Setup
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 8>::new()),
        p.ETH,
        Irqs,
        p.PA1,
        p.PA2,
        p.PC1,
        p.PA7,
        p.PC4,
        p.PC5,
        p.PG13,
        p.PB13,
        p.PG11,
        GenericSMI::new(0),
        mac_addr,
    );

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NUCLEO_IP, 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(GATEWAY),
    });

    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.try_fill_bytes(&mut seed).unwrap();
    let seed = u64::from_le_bytes(seed);

    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let stack = STACK.init(stack);

    // Spawn tasks
    spawner.spawn(monitor()).unwrap();
    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(udp_task(stack)).unwrap();
    spawner.spawn(usb_task(class)).unwrap();

    // Run USB device
    usb.run().await;
}