// ethernet.rs
//! Ethernet Communication Module
//! Handles static IP configuration and UDP bridging to USB.

use defmt::{error, info};
use embassy_futures::select::{select, Either};
use embassy_net::{
    Config as NetConfig, Ipv4Cidr, Stack, StackResources,
    IpEndpoint, Runner, udp::{UdpSocket, PacketMetadata},
};
use embassy_stm32::{
    eth::{Ethernet, PacketQueue, generic_smi::GenericSMI},
    peripherals::{self, ETH, RNG},
    rng::Rng,
};
use rand_core::RngCore;
use static_cell::StaticCell;

use crate::config::{
    USB_TO_ETH_PIPE, ETH_TO_USB_PIPE,
    NETWORK_LOCAL_IP, NETWORK_GATEWAY_IP, NETWORK_UDP_PORT,
    Irqs
};

pub static STACK: StaticCell<Stack<'static>> = StaticCell::new();


/// Number of UDP RX and TX packets to buffer
const UDP_PACKET_META: usize = 16;
/// Maximum UDP packet size (UDP MTU is usually <=1500, but you’re bridging USB)
const UDP_BUFFER_SIZE: usize = 512;
/// UDP Receive buffer size
const UDP_RX_BUF_SIZE: usize = UDP_PACKET_META * UDP_BUFFER_SIZE;
/// UDP Transmit buffer size
const UDP_TX_BUF_SIZE: usize = UDP_PACKET_META * UDP_BUFFER_SIZE;

/// Initializes the Ethernet interface and network stack.
///
/// Returns the [`Stack`] and [`Runner`] used to run the network.
pub fn setup_ethernet(
    eth: ETH,
    pa1: peripherals::PA1,
    pa2: peripherals::PA2,
    pc1: peripherals::PC1,
    pa7: peripherals::PA7,
    pc4: peripherals::PC4,
    pc5: peripherals::PC5,
    pg13: peripherals::PG13,
    pb13: peripherals::PB13,
    pg11: peripherals::PG11,
    rng: RNG,
    mac_addr: [u8; 6],
) -> (Stack<'static>, Runner<'static, Ethernet<'static, ETH, GenericSMI>>) {
    static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();

    let eth_dev = Ethernet::new(
        PACKETS.init(PacketQueue::new()),
        eth,
        Irqs,
        pa1, pa2, pc1, pa7, pc4, pc5, pg13, pb13, pg11,
        GenericSMI::new(0),
        mac_addr,
    );

    let net_config = NetConfig::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(NETWORK_LOCAL_IP, 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(NETWORK_GATEWAY_IP),
    });

    let mut rng = Rng::new(rng, Irqs);
    let mut seed_bytes = [0; 8];
    if rng.try_fill_bytes(&mut seed_bytes).is_err() {
        error!("RNG failed; using fallback seed");
        seed_bytes = 0xDEADBEEFCAFEBABE_u64.to_le_bytes();
    }
    let seed = u64::from_le_bytes(seed_bytes);

    embassy_net::new(
        eth_dev,
        net_config,
        RESOURCES.init(StackResources::new()),
        seed,
    )
}

/// Task that drives the networking stack.
///
/// This must be spawned to make the network stack function.
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Ethernet<'static, ETH, GenericSMI>>) -> ! {
    runner.run().await
}

/// UDP Task that bridges data from USB to Ethernet and back.
#[embassy_executor::task]
pub async fn udp_task(stack: &'static Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; UDP_PACKET_META];
    let mut tx_meta = [PacketMetadata::EMPTY; UDP_PACKET_META];
    let mut rx_buf = [0u8; UDP_RX_BUF_SIZE];
    let mut tx_buf = [0u8; UDP_TX_BUF_SIZE];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buf,
        &mut tx_meta,
        &mut tx_buf,
    );

    socket.bind(NETWORK_UDP_PORT).unwrap();

    // In production, this would be dynamic — this is a test fixture.
    let remote = IpEndpoint::new(NETWORK_GATEWAY_IP.into(), NETWORK_UDP_PORT);

    let mut buf_usb_to_udp = [0u8; UDP_BUFFER_SIZE];
    let mut buf_udp_to_usb = [0u8; UDP_BUFFER_SIZE];

    loop {
        match select(
            USB_TO_ETH_PIPE.read(&mut buf_usb_to_udp),
            socket.recv_from(&mut buf_udp_to_usb),
        ).await {
            Either::First(n) => {
                if let Err(e) = socket.send_to(&buf_usb_to_udp[..n], remote).await {
                    error!("UDP send error: {:?}", e);
                } else {
                    info!("Forwarded {} bytes to network", n);
                }
            }
            Either::Second(Ok((n, _src))) => {
                ETH_TO_USB_PIPE.write(&buf_udp_to_usb[..n]).await;
                info!("Forwarded {} bytes to USB", n);
            }
            Either::Second(Err(e)) => {
                error!("UDP receive error: {:?}", e);
            }
        }
    }
}
