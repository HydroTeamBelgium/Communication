//! CAN communication helpers and task functions
//!
//! Provides robust CAN read/write task implementations with proper error handling,
//! validation, and timeout protection. Includes UDP broadcast support for remote logging.

use embassy_stm32::can::{enums::BusError, Can};
use embassy_time::Timer;
use embassy_sync::channel::Sender;
use crate::config::{CanConfig, CanStats};
use crate::protocol::Message;
use defmt::*;

// ============================================================================
//                           ERROR TYPES
// ============================================================================

#[derive(Default)]
struct BusErrorStats {
    form: u32,
    stuff: u32,
    crc: u32,
    ack: u32,
    bit_recessive: u32,
    bit_dominant: u32,
    software: u32,
    bus_warning: u32,
    bus_passive: u32,
    bus_off: u32,
}

impl BusErrorStats {
    fn total(&self) -> u32 {
        self.form
            + self.stuff
            + self.crc
            + self.ack
            + self.bit_recessive
            + self.bit_dominant
            + self.software
            + self.bus_warning
            + self.bus_passive
            + self.bus_off
    }

    fn record(&mut self, err: &BusError) {
        match err {
            BusError::Form => self.form += 1,
            BusError::Stuff => self.stuff += 1,
            BusError::Crc => self.crc += 1,
            BusError::Acknowledge => self.ack += 1,
            BusError::BitRecessive => self.bit_recessive += 1,
            BusError::BitDominant => self.bit_dominant += 1,
            BusError::Software => self.software += 1,
            BusError::BusWarning => self.bus_warning += 1,
            BusError::BusPassive => self.bus_passive += 1,
            BusError::BusOff => self.bus_off += 1,
        }
    }
}

/// Generic CAN reader that delegates parsing to an `EcuParser` implementation.
pub async fn can_read_generic<P: crate::protocol::ecu::EcuParser + 'static>(
    parser: &'static P,
    mut can: Can<'static>,
    config: CanConfig,
    channel_tx: Sender<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    use embedded_can::Id;

    let mut stats = CanStats::default();
    let mut bus_error_stats = BusErrorStats::default();
    let mut consecutive_bus_errors = 0u32;
    let mut physical_hint_logged = false;
    let mut ever_seen_valid_frame = false;

    let ecu_label = match config.ecu_type {
        crate::config::can::EcuType::ScsDelta => "ScsDelta",
        crate::config::can::EcuType::MaxxEcu => "MaxxEcu",
    };

    info!(
        "CAN Reader (generic): Starting with {} ms timeout, bitrate={} bps, ECU={}",
        config.rx_timeout_ms,
        config.bitrate,
        ecu_label
    );

    loop {
        let timeout = Timer::after_millis(config.rx_timeout_ms);

        match embassy_futures::select::select(can.read(), timeout).await {
            embassy_futures::select::Either::First(Ok(envelope)) => {
                let frame = &envelope.frame;
                consecutive_bus_errors = 0;
                ever_seen_valid_frame = true;

                // Extract CAN ID
                let (id_val, is_standard): (u32, bool) = match frame.id() {
                    Id::Standard(id) => (id.as_raw() as u32, true),
                    Id::Extended(id) => (id.as_raw(), false),
                };

                info!("CAN RX: Raw frame ID={} [STD={}] DLC={}", id_val, is_standard, frame.data().len());

                if !is_standard {
                    debug!("CAN RX: Ignoring extended frame ID={}", id_val);
                    continue;
                }

                let frame_data = frame.data();

                if !parser.matches_id(id_val) {
                    debug!("CAN RX: Parser does not own ID {}, forwarding raw frame", id_val);
                }

                match parser.parse(id_val, frame_data) {
                    Ok(msg) => {
                        // Send to channel with backpressure (50ms timeout before dropping)
                        match embassy_futures::select::select(
                            channel_tx.send(msg),
                            Timer::after_millis(50)
                        ).await {
                            embassy_futures::select::Either::First(_) => {
                                trace!("CAN->UDP: ID={} sent", id_val);
                                stats.rx_count += 1;
                            }
                            embassy_futures::select::Either::Second(_) => {
                                error!("CAN->UDP pipeline stalled > 50ms, dropping frame ID={}", id_val);
                                stats.rx_errors += 1;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("CAN RX: Parser error for ID={} {}", id_val, e);
                        stats.rx_errors += 1;
                    }
                }
            }

            embassy_futures::select::Either::First(Err(e)) => {
                bus_error_stats.record(&e);
                consecutive_bus_errors = consecutive_bus_errors.saturating_add(1);
                match e {
                    BusError::BusPassive => {
                        if bus_error_stats.bus_passive % 50 == 1 {
                            warn!(
                                "CAN RX: BusPassive repeated (count={}). Controller is in Error Passive, but that does not only mean no ACK. If no valid frame has been seen yet, prioritize bitrate/clock mismatch, wrong bus, ECU power, transceiver, and termination checks.",
                                bus_error_stats.bus_passive
                            );
                        }

                        if bus_error_stats.bus_passive >= 500 && bus_error_stats.bus_passive % 500 == 1 {
                            error!("CAN RX: ERROR PASSIVE RECOVERY ATTEMPT - Controller stuck in Error Passive ({}+ errors)", bus_error_stats.bus_passive);
                            error!("CAN RX: Likely causes: (1) wrong bitrate or FDCAN kernel clock (2) ECU on another bus (3) ECU not powered (4) transceiver / wiring issue");
                        }
                    }
                    BusError::BusOff => {
                        error!("CAN RX: BusOff - controller disconnected from bus");
                    }
                    _ => {
                        error!("CAN RX: Read failed: {}", defmt::Debug2Format(&e));
                    }
                }
                stats.rx_errors += 1;

                if !physical_hint_logged {
                    warn!("CAN: physical-layer error detected - check bus wiring and termination");
                    physical_hint_logged = true;
                }

                if !ever_seen_valid_frame && bus_error_stats.total() % 25 == 1 {
                    warn!(
                        "CAN RX: no valid frame decoded yet; this strongly suggests clock/bitrate mismatch, wrong bus, or a filter/setup problem rather than a parser issue"
                    );
                }

                if bus_error_stats.total() % 100 == 0 {
                    warn!(
                        "CAN bus errors: total={}, form={}, stuff={}, crc={}, ack={}, bit0={}, bit1={}, warn={}, passive={}, off={}",
                        bus_error_stats.total(),
                        bus_error_stats.form,
                        bus_error_stats.stuff,
                        bus_error_stats.crc,
                        bus_error_stats.ack,
                        bus_error_stats.bit_recessive,
                        bus_error_stats.bit_dominant,
                        bus_error_stats.bus_warning,
                        bus_error_stats.bus_passive,
                        bus_error_stats.bus_off,
                    );
                }

                if consecutive_bus_errors >= 25 {
                    Timer::after_millis(50).await;
                }
            }

            embassy_futures::select::Either::Second(_) => {
                warn!("CAN RX: Timeout ({}ms) - possible bus fault or no messages", config.rx_timeout_ms);
                stats.timeout_count += 1;
            }
        }

        // Periodic diagnostics
        if stats.rx_count % 100 == 0 && stats.rx_count > 0 {
            info!(
                "CAN Stats: RX={}, RX_ERR={}, TX_TO={}",
                stats.rx_count,
                stats.rx_errors,
                stats.timeout_count
            );
        }
    }
}

/// CAN reader task with channel broadcasting for remote logging.
/// This is the SCS Delta entrypoint and delegates to the generic reader.
#[embassy_executor::task]
pub async fn can_read_scs_task_with_channel(
    can: Can<'static>,
    config: CanConfig,
    channel_tx: Sender<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    use crate::protocol::scs::SCS_PARSER;
    can_read_generic(&SCS_PARSER, can, config, channel_tx).await;
}

/// CAN reader task for MaxxECU setups.
#[embassy_executor::task]
pub async fn can_read_maxx_task_with_channel(
    can: Can<'static>,
    config: CanConfig,
    channel_tx: Sender<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    use crate::protocol::maxx::MAXX_PARSER;
    can_read_generic(&MAXX_PARSER, can, config, channel_tx).await;
}

// ============================================================================
//                   CAN UDP BROADCAST TASK
// ============================================================================

/// UDP broadcast task for CAN frames
///
/// Receives CAN frame messages from a channel and broadcasts them over UDP
/// to 255.255.255.255 on port 9999 (configurable).
///
/// # Example
/// ```ignore
/// let channel: Channel<Message, 256> = Channel::new();
/// spawner.spawn(can_udp_broadcast_task(stack, channel.receiver())).unwrap();
/// ```
#[embassy_executor::task]
pub async fn can_udp_broadcast_task(
    stack: &'static embassy_net::Stack<'static>,
    rx: embassy_sync::channel::Receiver<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    use embassy_net::udp::{PacketMetadata, UdpSocket};
    use embassy_net::IpEndpoint;

    const BROADCAST_PORT: u16 = 9999;
    const BROADCAST_ADDR: [u8; 4] = [255, 255, 255, 255];

    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buffer = [0; 256];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_buffer = [0; 2048];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    // Bind to local port for broadcast
    match socket.bind(BROADCAST_PORT) {
        Ok(_) => info!("CAN UDP broadcast ready on port {}", BROADCAST_PORT),
        Err(e) => {
            error!("UDP broadcast: Failed to bind: {}", defmt::Debug2Format(&e));
            return;
        }
    }

    let broadcast_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(embassy_net::Ipv4Address::new(BROADCAST_ADDR[0], BROADCAST_ADDR[1], BROADCAST_ADDR[2], BROADCAST_ADDR[3])),
        BROADCAST_PORT,
    );

    let mut msg_count = 0u32;
    let mut link_down_logged = false;

    loop {
        // Check if network link is up (issue #4)
        if !stack.is_link_up() {
            if !link_down_logged {
                warn!("Network link DOWN - UDP broadcast waiting for connection");
                link_down_logged = true;
            }
            Timer::after_millis(500).await;
            continue;
        }
        
        if link_down_logged {
            info!("Network link UP - resuming UDP broadcast");
            link_down_logged = false;
        }

        let msg = rx.receive().await;
        msg_count += 1;

        let mut buf = [0u8; 512];  // Increased from 32 to accommodate JSON payloads (1 byte type + 1 byte length + up to 256 bytes JSON)
        let len = msg.serialize(&mut buf);

        if len == 0 {
            error!("CAN UDP broadcast: failed to serialize {:?}", msg.message_type());
            continue;
        }

        match socket.send_to(&buf[..len], broadcast_endpoint).await {
            Ok(_) => {
                trace!("CAN broadcast #{}: {:?}", msg_count, msg.message_type());
            }
            Err(e) => {
                warn!("CAN UDP broadcast failed: {}", defmt::Debug2Format(&e));
            }
        }

        // Periodic info
        if msg_count % 100 == 0 {
            info!("CAN UDP broadcast: {} frames sent", msg_count);
        }
    }
}
