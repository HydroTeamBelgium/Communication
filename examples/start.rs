// initialise code without standard library, no main function
// and use embassy executor to run the main function
#![no_std]
#![no_main]

//all the necessary imports
use embassy_executor::Spawner;
use embassy_net::{
    udp::{UdpSocket, PacketMetadata},
    Ipv4Address,
    StackResources,
    IpEndpoint,
    IpAddress::Ipv4,
    Ipv4Cidr
};

use embassy_stm32::{
    eth::{
        Ethernet,
        PacketQueue,
        generic_smi::GenericSMI
    },
    eth,
    rng::Rng,
    rng,
    bind_interrupts,
    SharedData,
    peripherals::ETH,
    peripherals,
};

use embassy_time::Timer;
//use embedded_io_async::Write;
use rand_core::RngCore;
use static_cell::StaticCell;
use heapless::Vec;
use defmt::*;
use core::{
    mem::MaybeUninit,
    str::from_utf8,
};

use {defmt_rtt as _, panic_probe as _};

// initialise the shared data section in RAM
// this is used by the embassy executor to store data that needs to be shared between tasks
#[link_section = ".ram_d3.shared_data"]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// define the interrupt handlers for the ethernet and rng peripherals
bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
});

// define the ethernet device type
type Device = Ethernet<'static, ETH, GenericSMI>;

// define the ethernet task that will run in the background
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
    runner.run().await
}


// Creates ethernet frame: currently combines header and message content
fn create_mssg_frame(dest_addr: [u8; 4], mssg_content: [u8; 4]) -> [u8; 8] {
    let mut frame = [0u8; 8];
    frame[..4].copy_from_slice(&dest_addr);
    frame[4..].copy_from_slice(&mssg_content);
    frame
}


#[embassy_executor::main]
async fn main(spawner: Spawner) {

    defmt::info!("Hello World!");

    // configure the reset and clock control (RCC) peripheral
    // configer phase-locked loop (PLL) to use the high-speed internal (HSI) clock
    // configer system and bus clocks
    // configer power and supply voltage
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV8), // 100mhz
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P; // 400 Mhz
        config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 Mhz
        config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb2_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb3_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb4_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.voltage_scale = VoltageScale::Scale1;
        config.rcc.supply_config = SupplyConfig::DirectSMPS;
    }

    // initialise the embassy STM32 library with the config and shared data
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);
    info!("Hello World!");

    // Generate random seed. This is used to generate a random MAC address.
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);

    // Generate a random MAC address using the seed.
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    // Create a new ethernet device using the generated MAC address and the ethernet peripheral. Pinout is checked agianst the datasheet.
    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new();
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
        p.ETH,
        Irqs,
        p.PA1,  // ref_clk
        p.PA2,  // mdio
        p.PC1,  // eth_mdc
        p.PA7,  // CRS_DV: Carrier Sense
        p.PC4,  // RX_D0: Received Bit 0
        p.PC5,  // RX_D1: Received Bit 1
        p.PG13, // TX_D0: Transmit Bit 0
        p.PB13, // TX_D1: Transmit Bit 1
        p.PG11, // TX_EN: Transmit Enable
        GenericSMI::new(0),
        mac_addr,
    );

    info!("Device created");

    // hard coded IP address for now (commented line underneath is for dynamic adress assignment)
    //let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    });

    // Init network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    
    // Launch network task
    unwrap!(spawner.spawn(net_task(runner)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;
    info!("Network task initialized");

    // Then we can use it!
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 1024];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 1024];
    let mut msg_buffer = [0; 128];

    //store the hardcoded IP's of the other devices
    let stm_1 = Ipv4Address::new(192, 168, 1, 100);  // needs to be changed to the hardcoded IP of the other stm32
    // let stm_2 = Ipv4Address::new(192, 168, 1, 101);
    // let stm_3 = Ipv4Address::new(192, 168, 1, 102);

    const COMMS_PORT: u16 = 12345;
    
    let mut udp_socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer
    );
    match udp_socket.bind(0) {
        Ok(_) => {
            info!("UDP server ready!");
            loop {
                info!("sending UDP packet");

                let message: [u8; 4] = [0u8; 4];

                udp_socket
                    .send_to(&create_mssg_frame(stm_1.octets(), message), IpEndpoint::new(Ipv4(stm_1), COMMS_PORT))
                    .await
                    .unwrap();
                Timer::after_millis(1000).await;
                    
                if udp_socket.may_recv() {
                    let (rx_size, from_addr) = udp_socket.recv_from(&mut msg_buffer).await.unwrap();
                    let response = from_utf8(&msg_buffer[..rx_size]).unwrap();
                    info!("Server replied with {} from {}", response, from_addr);
                }
                Timer::after_millis(1000).await;
            }
        }
        Err(err) => {
            warn!("UDP bind error: {:?}", err);
        }
    };
    
}