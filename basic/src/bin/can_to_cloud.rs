#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use core::mem::MaybeUninit;

use core::fmt::Write as _;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{
    Ipv4Address, Ipv4Cidr, Stack, StackResources,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_stm32::{
    Config, SharedData, bind_interrupts,
    eth::{self, Ethernet, PacketQueue},
    peripherals,
    rcc::*,
    rng::{InterruptHandler as RngInterruptHandler, Rng},
};
use basis::prelude::*;

use basis::tasks::can::{can_read_maxx_task_with_channel, can_read_scs_task_with_channel};
use basis::config::can::{DEFAULT_CAN_RX_TIMEOUT_MS, NUCLEO_1_ECU_MODE};

use heapless::String;
use reqwless::{
    client::{HttpClient, TlsConfig},
    headers::ContentType,
    request::{Method, RequestBuilder},
};
use static_cell::StaticCell;

const SUPABASE_ENDPOINT: &str = "https://ctrbjkapatxjuogfewae.supabase.co/rest/v1/SensorData";
const SUPABASE_KEY: &str = "sb_publishable_aNJfm7INlJE9qMvkMWLnIw_hOWNyYv7";

const NETWORK_LOCAL_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 5);
const MODEM_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 1);

static PACKETS: StaticCell<PacketQueue<8, 2>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();

#[unsafe(link_section = ".ram_d3.shared_data")]
static SHARED_DATA: MaybeUninit<SharedData> = MaybeUninit::uninit();

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
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



async fn write_sensor_data(
    stack: Stack<'_>, 
    tls_seed: u64,
    rx: embassy_sync::channel::Receiver<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    // Replace TlsVerify::None with certificate validation before production use.
    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut response_buffer = [0u8; 1024];
    let mut authorization = String::<128>::new();
    authorization
        .write_fmt(format_args!("Bearer {}", SUPABASE_KEY))
        .unwrap();
    let headers = [
        ("apikey", SUPABASE_KEY),
        ("Authorization", authorization.as_str()),
        ("Prefer", "return=minimal"),
    ];

    loop {
        
        let msg = rx.receive().await;
        info!("msg received is: " {:?}, msg);
        let payload = br#"{"temperature":30.0,"humidity":80.0}"#;

        let mut request = client
            .request(Method::POST, SUPABASE_ENDPOINT)
            .await
            .unwrap()
            .headers(&headers)
            .content_type(ContentType::ApplicationJson)
            .accept(ContentType::ApplicationJson)
            .body(payload.as_slice());
    
        let response = request.send(&mut response_buffer).await.unwrap();
        if response.status.is_successful() {
            info!("Sensor data successfully sent to Supabase");
        } else {
            error!("Supabase returned HTTP {}", response.status.0);
            let body = response.body().read_to_end().await.unwrap_or(&mut []);
            if let Ok(message) = core::str::from_utf8(body) {
                error!("Supabase response: {}", message);
            }
        }

    }

}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    configure_clock(&mut config);
    let p = embassy_stm32::init_primary(config, &SHARED_DATA);


    // Can config
    let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
    can.set_bitrate(500_000);
    let ecu_label = "ScsDelta";
    
    let nominal = can.config().nbtr;
    let sample_point_permille = (1000 * (1 + nominal.seg1.get() as u16))
        / (1 + nominal.seg1.get() as u16 + nominal.seg2.get() as u16);
    info!(
        "CAN debug: board=Nucleo1, ECU mode={}, bitrate={} bps, rx_timeout={} ms, pins=PD0/PD1",
        ecu_label,
        basis::hal::can::config::CAN_BITRATE_DEFAULT,
        DEFAULT_CAN_RX_TIMEOUT_MS
    );
    info!(
        "CAN timing: prescaler={}, seg1={}, seg2={}, sjw={}, sample_point={} permille",
        nominal.prescaler.get(),
        nominal.seg1.get(),
        nominal.seg2.get(),
        nominal.sync_jump_width.get(),
        sample_point_permille
    );
    let can = can.into_normal_mode();
    info!("CAN Configured at {} kbps", basis::hal::can::config::CAN_BITRATE_DEFAULT / 1000);


    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<8, 2>::new()),
        p.ETH,
        Irqs,
        p.PA1,
        p.PA7,
        p.PC4,
        p.PC5,
        p.PG13,
        p.PB13,
        p.PG11,
        mac_addr,
        p.ETH_SMA,
        p.PA2,
        p.PC1,
    );

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24),
        dns_servers: heapless::Vec::from_slice(&[MODEM_IP]).unwrap(),
        gateway: Some(MODEM_IP),
    });

    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);
    let (stack, runner) =
        embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    let tls_seed = rng.next_u64();

    spawner
        .spawn(net_task(runner))
        .expect("Failed to spawn net task");
    spawner.spawn(can_read_scs_task_with_channel(can, can_config, CAN_CHANNEL.sender()));
    write_sensor_data(stack, tls_seed, CAN_CHANNEL.receiver()).await;
}
