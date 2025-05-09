// basic test between 2 nucleo's. Turns on LED on both nucleos when button is pressed. 
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
    gpio::{Input, Pull, Output, Speed, Level},
    exti::ExtiInput,
};

use embassy_time::{Timer, Duration};
use rand_core::RngCore;
use static_cell::StaticCell;
use heapless::Vec;
use defmt::*;
use core::mem::MaybeUninit;

use {defmt_rtt as _, panic_probe as _};

// Network Config
const NUCLEO1_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 61);
const NUCLEO2_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 62);
const PORT: u16 = 4321;

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

    let mut button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    
    let mut led1 = Output::new(p.PB14, Level::High, Speed::Low);
    let mut led2 = Output::new(p.PE1, Level::High, Speed::Low);

    // Generate random seed. This is used to generate a random MAC address.
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut mac_addr = [0u8; 6];
    let mut seed = [0;8];
    mac_addr[0] = 0x02; // Locally administered unicast MAC
    rng.fill_bytes(&mut mac_addr[1..]);
    let seed = u64::from_le_bytes(seed);


    // Generate a random MAC address using the seed.
    // let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

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
    let config = embassy_net::Config::dhcpv4(Default::default());
    // let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //     address: Ipv4Cidr::new(NUCLEO1_IP, 24),
    //     dns_servers: Vec::new(),
    //     gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    // });

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

    
    let remote_endpoint = IpEndpoint::new(embassy_net::IpAddress::Ipv4(NUCLEO2_IP), 1234);  
    socket.bind(PORT).unwrap();
    let mut buf = [0; 1]; // 1-byte messages
    let mut prev_button = false;
    let mut led_state = false;

    loop {
        // Check button state with debouncing
        // let pressed = button.is_high();
        // if pressed && !prev_button {
        //     // Toggle local LED
        //     led_state = !led_state;
        
        //     if led_state {
        //         led1.set_high();
        //         buf[0] = 1; // LED on command
        //     } else {
        //         led1.set_low();
        //         buf[0] = 0; // LED off command
        //     }
            
        //     // Send command to other Nucleo
        //     match socket.send_to(&buf, (NUCLEO2_IP, PORT)).await {
        //         Ok(_) => info!("Sent command: {}", buf[0]),
        //         Err(e) => error!("Send failed: {:?}", e),
        //     }
            
        //     // Simple debounce
        //     Timer::after_millis(50).await;
        // }
        stack.wait_config_up().await;
        info!("DHCP IP acquired: {}", stack.config_v4().unwrap().address);

        // prev_button = pressed;

        // Check for incoming commands (non-blocking with timeout)
        match embassy_time::with_timeout(
            Duration::from_millis(10),
            socket.recv_from(&mut buf)
        ).await {
            Ok(Ok((len, remote))) => {
                info!("Received command: {} from {}", buf[0], remote);
                match buf[0] {
                    1 => led2.set_high(),
                    0 => led2.set_low(),
                    _ => warn!("Unknown command"),
                }
            }
            Ok(Err(e)) => warn!("Receive error: {:?}", e),
            Err(_) => (), // Timeout is normal
        }

        Timer::after_millis(10).await;
    }

}