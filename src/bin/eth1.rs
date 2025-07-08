use embassy_net::{
    udp::{UdpSocket, PacketMetadata},
    StackResources,  
    Ipv4Cidr,
    IpEndpoint,
    Stack
};
use embassy_stm32::{
    eth::{Ethernet, PacketQueue, generic_smi::GenericSMI},
    rng::Rng,
    peripherals::ETH,
    peripherals,
};

use embassy_futures::select::{select, Either};
use static_cell::StaticCell;
use rand_core::RngCore;
use defmt::{info, error};


use crate::config1::{
    USB_TO_ETH_PIPE, 
    ETH_TO_USB_PIPE,
    NETWORK_LOCAL_IP,
    NETWORK_GATEWAY_IP,
    NETWORK_UDP_PORT,
    Irqs
};



// Network Buffers
static PACKETS: StaticCell<PacketQueue<8, 8>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
pub static STACK: StaticCell<Stack<'static>> = StaticCell::new();


/// Initializes the Ethernet interface with static IP.
pub fn setup_ethernet(
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

/// UDP Network Task
/// 
/// - Reads data from the USB pipe.
/// - Sends data over UDP.
#[embassy_executor::task]
pub async fn udp_task(stack: &'static Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 1024];

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
                info!("Forwarding {} bytes over UDP", n);
                match socket.send_to(&buf_usb_to_udp[..n], remote_endpoint).await {
                    Ok(_) => info!("UDP send successful"),
                    Err(e) => error!("UDP send error: {:?}", e),
                }
            }
            Either::Second(Ok((n, _addr))) => {
                info!("Received {} bytes from UDP", n);
                ETH_TO_USB_PIPE.write(&buf_udp_to_usb[..n]).await;
            }
            Either::Second(Err(e)) => {
                error!("UDP receive error: {:?}", e);
            }
        }
    }
}


#[embassy_executor::task]
pub async fn net_task(mut runner: embassy_net::Runner<'static, Ethernet<'static, ETH, GenericSMI>>) -> ! {
    runner.run().await
}