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
use rand_core::RngCore;
use static_cell::StaticCell;
use heapless::{spsc::Producer, Vec};
use defmt::*;
use core::mem::MaybeUninit;

use {defmt_rtt as _, panic_probe as _};
// Network Config
const MY_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61);
const LAPTOP_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 100);
const PORT: u16 = 4321;  // Both devices use same port for simplicity

// initialise the shared data section in RAM
// this is used by the embassy executor to store data that needs to be shared between tasks
#[link_section = ".ram_d3.shared_data"]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

// define the interrupt handlers for the ethernet and rng peripherals
bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
});

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>) -> ! {
    runner.run().await
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
        address: Ipv4Cidr::new(MY_IP, 24),
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
    info!("Network initialized. IP: {}", stack.config_v4().unwrap().address);

    // Then we can use it!
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 1024];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 1024];
    
    
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer
    );

    
    let remote_endpoint = IpEndpoint::new(embassy_net::IpAddress::Ipv4(LAPTOP_IP), PORT);  
    socket.bind(PORT).unwrap();
    let mut counter: i32 = 0;
    let mut buf = [0; 4];

    loop {
        // Send a message to laptop
        // let counter_bytes = counter.to_le_bytes();
        // socket.send_to(&counter_bytes, remote_endpoint).await.unwrap();
        // info!("Sent: {}", counter);


        // counter += 1;
        // Timer::after_millis(1000).await;

        // Wait for incoming data
        match embassy_time::with_timeout(
            embassy_time::Duration::from_secs(5),
            socket.recv_from(&mut buf)
        ).await {
            Ok(Ok((len, remote))) => {
                let received_num = i32::from_le_bytes(buf);
                info!("Received {} from {}", received_num, remote);
            }
            Ok(Err(e)) => {
                warn!("Receive error: {:?}", e);
            }
            Err(_) => {
                info!("Timeout waiting for packet");
            }
        }
    }
    
    
}