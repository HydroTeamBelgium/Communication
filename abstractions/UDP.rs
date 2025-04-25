use embassy_net::{
    udp::UdpSocket, 
    IpEndpoint, 
    IpAddress::Ipv4, Ipv4Address
}

use defmt;
use embassy_time::Timer;

pub struct UdpSender<'a> {
    socket: &'a mut UdpSocket<'a>,
    destination: IpEndpoint,
}

impl<'a> UdpSender<'a> {
    pub fn new(
        stack: &'a Stack<Device>,
        rx_meta: &'a mut [PacketMetadata],
        rx_buffer: &'a mut [u8],
        tx_meta: &'a mut [PacketMetadata],
        tx_buffer: &'a mut [u8],
        destination: IpEndpoint,
    ) -> Self {
        let socket = UdpSocket::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        
        Self {
            socket,
            destination,
        }
    }

    pub async fn send_message(&mut self, message: impl AsRef<[u8]>) -> Result<(), embassy_net::udp::Error> {
        info!("Sending UDP message to {:?}", self.destination);
        self.socket.sent_to(&message.as_ref(), self.destination).await?;
        Ok(())
    }
}