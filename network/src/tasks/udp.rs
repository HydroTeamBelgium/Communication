//! UDP communication helpers and task macros
//!
//! Provides reusable UDP sending and receiving functionality.
//! Use the macros to generate tasks in your binaries.

use embassy_net::udp::{UdpMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address};
use defmt::{info, warn};

use crate::config::MAX_PACKET_SIZE;
use crate::protocol::Message;

// ============================================================================
//                            HELPER FUNCTIONS
// ============================================================================

/// Create destination endpoint from IP and port
pub const fn endpoint(ip: Ipv4Address, port: u16) -> IpEndpoint {
    IpEndpoint::new(embassy_net::IpAddress::Ipv4(ip), port)
}

/// Log a received message
pub fn log_message(msg: &Message, sender: &UdpMetadata, counter: u32) {
    match msg {
        Message::Bytes(data) => {
            match core::str::from_utf8(data) {
                Ok(s) => info!("[{}] Bytes: {} from {} data: {}", counter, s, sender, data),
                Err(_) => info!("[{}] Bytes (non-UTF8) from {} data {}", counter, sender, data),
            }
        }
        Message::Pot(reading) => {
            info!("[{}] Pot: {=f32} V from {}", counter, reading.voltage, sender);
        }
        Message::CanFrame(can_data) => {
            info!("[{}] CAN ID={=u32} DLC={} from {}", counter, can_data.can_id, can_data.dlc, sender);
        }
        Message::EcuJson(ecu_data) => {
            info!("[{}] ECU JSON: {} from {}", counter, ecu_data.json.as_str(), sender);
        }
    }
}

// ============================================================================
//                         UDP TASK MACROS
// ============================================================================

/// Generate a UDP sender task that reads Messages from a channel and sends them.
///
/// # Usage
/// ```ignore
/// basis::udp_send_task!(udp_send_task, CHANNEL, THIS_BOARD, DESTINATION);
/// ```
#[macro_export]
macro_rules! udp_send_task {
    ($name:ident, $channel:ident, $this_board:expr, $destination:expr) => {
        #[embassy_executor::task]
        async fn $name(stack: &'static embassy_net::Stack<'static>) {
            use embassy_net::udp::{PacketMetadata, UdpSocket};
            use embassy_net::IpEndpoint;
            use defmt::{info, warn};
            use basis::config::{RX_BUFFER_SIZE, TX_BUFFER_SIZE, RX_METADATA_COUNT, TX_METADATA_COUNT};
            
            let mut rx_meta = [PacketMetadata::EMPTY; RX_METADATA_COUNT];
            let mut rx_buffer = [0; RX_BUFFER_SIZE];
            let mut tx_meta = [PacketMetadata::EMPTY; TX_METADATA_COUNT];
            let mut tx_buffer = [0; TX_BUFFER_SIZE];

            let mut socket = UdpSocket::new(
                *stack,
                &mut rx_meta, &mut rx_buffer,
                &mut tx_meta, &mut tx_buffer,
            );
            socket.bind($this_board.listen_port).unwrap();

            let endpoint = IpEndpoint::new($destination.ip.into(), $destination.listen_port);
            
            info!("UDP sender ready, target: {}:{}", $destination.ip, $destination.listen_port);

            loop {
                let msg = $channel.receive().await;
                let mut buf = [0u8; 32];
                let len = msg.serialize(&mut buf);
                
                match socket.send_to(&buf[..len], endpoint).await {
                    Ok(_) => info!("Sent: {:?}", msg.message_type()),
                    Err(e) => warn!("Send error: {:?}", e),
                }
            }
        }
    };
}

/// Generate a UDP receiver task that receives Messages and logs them.
///
/// # Usage
/// ```ignore
/// basis::udp_recv_task!(udp_recv_task, THIS_BOARD);
/// ```
#[macro_export]
macro_rules! udp_recv_task {
    ($name:ident, $this_board:expr) => {
        #[embassy_executor::task]
        async fn $name(stack: &'static embassy_net::Stack<'static>) -> ! {
            use embassy_net::udp::{PacketMetadata, UdpSocket};
            use defmt::{info, warn};
            use basis::config::{RX_BUFFER_SIZE, TX_BUFFER_SIZE, RX_METADATA_COUNT, TX_METADATA_COUNT, MAX_PACKET_SIZE};
            use basis::protocol::Message;
            
            let mut rx_meta = [PacketMetadata::EMPTY; RX_METADATA_COUNT];
            let mut rx_buffer = [0; RX_BUFFER_SIZE];
            let mut tx_meta = [PacketMetadata::EMPTY; TX_METADATA_COUNT];
            let mut tx_buffer = [0; TX_BUFFER_SIZE];

            let mut socket = UdpSocket::new(
                *stack,
                &mut rx_meta, &mut rx_buffer,
                &mut tx_meta, &mut tx_buffer,
            );
            socket.bind($this_board.listen_port).unwrap();
            
            info!("Listening on port {}", $this_board.listen_port);

            let mut rx_buf = [0u8; MAX_PACKET_SIZE];
            let mut counter: u32 = 0;

            loop {
                match socket.recv_from(&mut rx_buf).await {
                    Ok((n, sender)) => {
                        if let Some(msg) = Message::deserialize(&rx_buf[..n]) {
                            basis::tasks::udp::log_message(&msg, &sender, counter);
                            counter += 1;
                        } else {
                            warn!("Failed to parse message: {:?}", &rx_buf[..n]);
                        }
                    }
                    Err(e) => warn!("Recv error: {:?}", e),
                }
            }
        }
    };
}

// ============================================================================
//                         UDP HELPER STRUCT
// ============================================================================

/// UDP socket wrapper with pre-allocated buffers (for advanced use)
pub struct UdpHelper<'a> {
    socket: UdpSocket<'a>,
}

impl<'a> UdpHelper<'a> {
    /// Send a message to an endpoint
    pub async fn send_message(
        &mut self,
        msg: &Message,
        endpoint: IpEndpoint,
    ) -> Result<(), embassy_net::udp::SendError> {
        let mut buf = [0u8; 32];
        let len = msg.serialize(&mut buf);
        self.socket.send_to(&buf[..len], endpoint).await
    }

    /// Receive a message, returns (message, sender metadata) or None if parse failed
    pub async fn recv_message(&mut self) -> Option<(Message, UdpMetadata)> {
        let mut buf = [0u8; MAX_PACKET_SIZE];
        match self.socket.recv_from(&mut buf).await {
            Ok((n, sender)) => {
                let msg = Message::deserialize(&buf[..n])?;
                Some((msg, sender))
            }
            Err(e) => {
                warn!("UDP receive error: {:?}", e);
                None
            }
        }
    }
}
