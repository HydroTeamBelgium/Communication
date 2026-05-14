//! CAN communication helpers and task functions
//!
//! Provides robust CAN read/write task implementations with proper error handling,
//! validation, and timeout protection. Includes UDP broadcast support for remote logging.

use embassy_stm32::can::{enums::BusError, Can};
use embassy_time::Timer;
use embassy_sync::channel::Sender;
use crate::config::{CanConfig, CanStats};
use crate::protocol::{EngineData, Message, CanFrameData, EcuJsonData, CanMessage};
use defmt::*;

// ============================================================================
//                           ERROR TYPES
// ============================================================================

/// CAN operation error
#[derive(Debug, Clone, Copy)]
pub enum CanError {
    WriteQueueFull,
    WriteFailed,
    ReadTimeout,
    ReadFailed,
    ValidationFailed,
}

const MAXXECU_ID_MIN: u32 = 0x520;
const MAXXECU_ID_MAX: u32 = 0x542;

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

fn is_maxxecu_standard_id(id: u32) -> bool {
    (MAXXECU_ID_MIN..=MAXXECU_ID_MAX).contains(&id)
}

// ============================================================================
//                      CAN WRITER TASK (improved)
// ============================================================================

/// CAN writer task with proper error handling
///
/// Sends engine data periodically on the CAN bus with:
/// - Error logging for each failed transmission
/// - Configurable interval and CAN ID
/// - Real sensor data simulation
///
/// # Example
/// ```ignore
/// let config = CanConfig::new(0x520, 500_000, 250, 5000);
/// spawner.spawn(can_write_task(can, config)).unwrap();
/// ```
#[embassy_executor::task]
pub async fn can_write_task(
    mut can: Can<'static>,
    config: CanConfig,
) {
    let mut stats = CanStats::default();
    let mut seq: u8 = 0;

    loop {
        // Get real sensor data (simulated)
        let engine_data = EngineData::simulate_sensor_read(seq);
        let frame_data = engine_data.to_can_frame();

        // Create CAN frame (MaxxECU uses standard 11-bit IDs, not extended)
        let frame = match embassy_stm32::can::frame::Frame::new_standard(config.can_id as u16, &frame_data) {
            Ok(f) => f,
            Err(e) => {
                error!("CAN TX: Failed to create frame: {}", defmt::Debug2Format(&e));
                stats.tx_errors += 1;
                Timer::after_millis(config.tx_interval_ms).await;
                continue;
            }
        };

        // Attempt transmission
        match can.write(&frame).await {
            None => {
                trace!("CAN TX: ID={=u32}, seq={}, RPM={}, Throttle={}, Map={}, lambda={}", config.can_id, seq, engine_data.rpm, engine_data.throttle, engine_data.map, engine_data.lambda_scaled);
                stats.tx_count += 1;
            }
            Some(_frame) => {
                warn!("CAN TX: Queue full, frame dropped (seq={})", seq);
                stats.tx_errors += 1;
            }
        }

        seq = seq.wrapping_add(1);
        Timer::after_millis(config.tx_interval_ms).await;
    }
}

// ============================================================================
//                      CAN READER TASK (improved)
// ============================================================================

/// CAN reader task with timeout protection and validation
///
/// Reads CAN frames with:
/// - Configurable timeout (watchdog)
/// - Frame validation
/// - Sequence number tracking
/// - Error recovery logging
///
/// # Example
/// ```ignore
/// let config = CanConfig::new(0x520, 500_000, 250, 5000);
/// spawner.spawn(can_read_task(can, config)).unwrap();
/// ```
#[embassy_executor::task]
pub async fn can_read_task(
    mut can: Can<'static>,
    config: CanConfig,
) {
    use embedded_can::Id;

    let mut stats = CanStats::default();
    let mut expected_seq: u8 = 0;
    let mut bus_error_stats = BusErrorStats::default();
    let mut physical_hint_logged = false;

    info!("CAN Reader: Starting with {} ms timeout", config.rx_timeout_ms);

    loop {
        let timeout = Timer::after_millis(config.rx_timeout_ms);

        match embassy_futures::select::select(can.read(), timeout).await {
            embassy_futures::select::Either::First(Ok(envelope)) => {
                let frame = &envelope.frame;

                // Extract CAN ID
                let id_val: u32 = match frame.id() {
                    Id::Standard(id) => id.as_raw() as u32,
                    Id::Extended(id) => id.as_raw(),
                };

                let frame_data = frame.data();

                // Validate frame
                match validate_frame(frame_data, &config) {
                    Ok(_) => {
                        // Try to parse as engine data
                        match EngineData::from_can_frame(frame_data) {
                            Ok(data) => {
                                debug!(
                                    "CAN RX: ID={=u32}, RPM={}, Throttle={}%",
                                    id_val,
                                    data.rpm,
                                    data.throttle
                                );
                                stats.rx_count += 1;

                                // Check sequence number if frame has one
                                if frame_data.len() > 6 {
                                    let seq = frame_data[6];
                                    if seq != expected_seq {
                                        warn!(
                                            "CAN RX: Seq gap detected: expected {}, got {}",
                                            expected_seq,
                                            seq
                                        );
                                    }
                                    expected_seq = seq.wrapping_add(1);
                                }
                            }
                            Err(e) => {
                                debug!(
                                    "CAN RX: Parse error for ID={=u32}: {:?}",
                                    id_val,
                                    e
                                );
                                stats.rx_errors += 1;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("CAN RX: Validation error for ID={=u32}: {}", id_val, e);
                        stats.rx_errors += 1;
                    }
                }
            }

            embassy_futures::select::Either::First(Err(e)) => {
                bus_error_stats.record(&e);
                match e {
                    BusError::BusPassive => {
                        if bus_error_stats.bus_passive % 50 == 1 {
                            warn!(
                                "CAN RX: BusPassive repeated (count={})",
                                bus_error_stats.bus_passive
                            );
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
                    warn!(
                        "CAN diagnostics: protocol errors usually mean physical-layer mismatch (CANH/CANL swap, missing 120R termination, wrong CAN bus selection, or baud mismatch)."
                    );
                    physical_hint_logged = true;
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

// ============================================================================
//                      VALIDATION HELPERS
// ============================================================================

/// CAN reader task with channel broadcasting for remote logging
///
/// Reads CAN frames and sends them over a channel for UDP broadcast with:
/// - Configurable timeout (watchdog)
/// - Frame validation
/// - SCS protocol parsing with JSON serialization
/// - Channel-based message delivery
///
/// # Example
/// ```ignore
/// let channel: Channel<Message, 256> = Channel::new();
/// let config = CanConfig::new(0x520, 500_000, 250, 200);
/// spawner.spawn(can_read_task_with_channel(can, config, channel.sender())).unwrap();
/// ```
#[embassy_executor::task]
pub async fn can_read_task_with_channel(
    mut can: Can<'static>,
    config: CanConfig,
    channel_tx: Sender<'static, embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Message, 256>,
) {
    use embedded_can::Id;
    use heapless::String;

    let mut stats = CanStats::default();
    let mut bus_error_stats = BusErrorStats::default();
    let mut consecutive_bus_errors = 0u32;
    let mut physical_hint_logged = false;

    info!("CAN Reader with UDP broadcast (SCS protocol): Starting with {} ms timeout", config.rx_timeout_ms);

    loop {
        let timeout = Timer::after_millis(config.rx_timeout_ms);

        match embassy_futures::select::select(can.read(), timeout).await {
            embassy_futures::select::Either::First(Ok(envelope)) => {
                let frame = &envelope.frame;
                consecutive_bus_errors = 0;

                // Extract CAN ID
                let (id_val, is_standard): (u32, bool) = match frame.id() {
                    Id::Standard(id) => (id.as_raw() as u32, true),
                    Id::Extended(id) => (id.as_raw(), false),
                };

                // DEBUG: Log all received frames regardless of filtering
                info!("CAN RX: Raw frame ID=0x{:03X} [STD={}] DLC={}", id_val, is_standard, frame.data().len());

                // MaxxECU default protocol uses 11-bit standard IDs only.
                if !is_standard {
                    debug!("CAN RX: Ignoring extended frame ID=0x{:03X}", id_val);
                    continue;
                }

                if !is_maxxecu_standard_id(id_val) {
                    debug!("CAN RX: Ignoring non-MaxxECU ID=0x{:03X} (outside 0x520-0x542 range)", id_val);
                    continue;
                }

                let frame_data = frame.data();

                // MaxxECU default protocol transmits fixed 8-byte payloads.
                if frame_data.len() != 8 {
                    warn!("CAN RX: Dropping ID=0x{:03X} with DLC={} (expected 8)", id_val, frame_data.len());
                    stats.rx_errors += 1;
                    continue;
                }

                info!("CAN RX: Accepted MaxxECU frame ID=0x{:03X} DLC=8", id_val);

                // Try to parse using SCS protocol
                if let Some(can_msg) = CanMessage::from_frame(id_val, frame_data) {
                    // Convert to JSON
                    let json_bytes = can_msg.to_json_bytes();
                    let json_str = match core::str::from_utf8(&json_bytes) {
                        Ok(s) => s,
                        Err(_) => {
                            warn!("CAN RX: Invalid UTF8 in JSON for ID={=u32}", id_val);
                            stats.rx_errors += 1;
                            continue;
                        }
                    };

                    // Create JSON message
                    let json_string: String<256> = match String::try_from(json_str) {
                        Ok(s) => s,
                        Err(_) => {
                            warn!("CAN RX: JSON string too long for ID={=u32}", id_val);
                            stats.rx_errors += 1;
                            continue;
                        }
                    };

                    let ecu_json = EcuJsonData::new(json_string);
                    let msg = Message::EcuJson(ecu_json);

                    // Send to channel with backpressure (50ms timeout before dropping)
                    match embassy_futures::select::select(
                        channel_tx.send(msg),
                        Timer::after_millis(50)
                    ).await {
                        embassy_futures::select::Either::First(_) => {
                            trace!("CAN broadcast to UDP: ID={=u32}, JSON sent, data: {:?}", id_val, json_str);
                            stats.rx_count += 1;
                        }
                        embassy_futures::select::Either::Second(_) => {
                            error!("CAN→UDP pipeline stalled > 50ms, dropping JSON frame ID={=u32}", id_val);
                            stats.rx_errors += 1;
                        }
                    }

                    debug!("CAN RX: ID={=u32}, parsed and sent JSON", id_val);
                } else {
                    // Fallback: send as raw CAN frame if not recognized SCS message
                    debug!("CAN RX: ID={=u32}, not recognized SCS message, falling back to raw frame, frame data: {:?}", id_val, frame_data);
                    
                    let can_msg = CanFrameData::new(id_val, {
                        let mut d = [0u8; 8];
                        d.copy_from_slice(&frame_data[0..8.min(frame_data.len())]);
                        d
                    }, frame_data.len() as u8);

                    let msg = Message::CanFrame(can_msg);

                    // Send to channel with backpressure (50ms timeout before dropping)
                    match embassy_futures::select::select(
                        channel_tx.send(msg),
                        Timer::after_millis(50)
                    ).await {
                        embassy_futures::select::Either::First(_) => {
                            trace!("CAN broadcast raw frame to UDP: ID={=u32}", id_val);
                            stats.rx_count += 1;
                        }
                        embassy_futures::select::Either::Second(_) => {
                            error!("CAN→UDP pipeline stalled > 50ms, dropping raw frame ID={=u32}", id_val);
                            stats.rx_errors += 1;
                        }
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
                                "CAN RX: BusPassive repeated (count={}). Controller stuck in Error Passive state (TEC≥128). Per Bosch CAN spec this occurs when no ACK received. Check: 1) Is MaxxECU powered? 2) Transmitting on CAN1? 3) Bus termination 60Ω?",
                                bus_error_stats.bus_passive
                            );
                        }
                        
                        // Recovery strategy: If stuck in BusPassive for too long, attempt controlled reset
                        // Per STM32 FDCAN errata: Error Passive state can persist without reaching Bus Off
                        // if the error counters are pegged at 128. Hard reset of FDCAN clears this.
                        if bus_error_stats.bus_passive >= 500 && bus_error_stats.bus_passive % 500 == 1 {
                            error!("CAN RX: ERROR PASSIVE RECOVERY ATTEMPT - Controller stuck in Error Passive ({}+ errors)", bus_error_stats.bus_passive);
                            error!("CAN RX: Likely causes: (1) MaxxECU not connected/powered (2) No ACK from peer (3) Baud mismatch (4) Transceiver issue");
                            error!("CAN RX: Physical layer diagnostic needed - no software fix will help if hardware is disconnected");
                            // Note: Full reset would require dropping and reinitializing Can peripheral, which is not async-safe here
                            // Recommend hardware power cycle of MaxxECU and Nucleo if this persists
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
                    warn!(
                        "CAN: physical-layer error detected - check bus wiring and termination"
                    );
                    physical_hint_logged = true;
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

                // Throttle log storm if the controller stays in a bad bus state.
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

// ============================================================================
//                      VALIDATION HELPERS
// ============================================================================

/// Validate CAN frame data
fn validate_frame(frame_data: &[u8], _config: &CanConfig) -> Result<(), &'static str> {
    // MaxxECU default protocol transmits fixed-size 8-byte CAN frames.
    if frame_data.len() != 8 {
        return Err("Expected DLC=8");
    }

    Ok(())
}

// ============================================================================
//                    LEGACY MACRO SUPPORT (backward compat)
// ============================================================================

/// Legacy macro support - converts to function calls
/// 
/// Kept for backward compatibility but should migrate to function calls
#[macro_export]
macro_rules! can_write_task {
    ($name:ident, $can_id:expr, $interval_ms:expr) => {
        #[embassy_executor::task]
        pub async fn $name(can: embassy_stm32::can::Can<'static>) {
            let config = $crate::config::CanConfig::new($can_id, 500_000, $interval_ms, 5000);
            $crate::tasks::can_write_task(can, config).await;
        }
    };
}

/// Legacy macro support - converts to function calls
#[macro_export]
macro_rules! can_read_task {
    ($name:ident) => {
        #[embassy_executor::task]
        pub async fn $name(can: embassy_stm32::can::Can<'static>) {
            let config = $crate::config::CanConfig::default();
            $crate::tasks::can_read_task(can, config).await;
        }
    };
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
