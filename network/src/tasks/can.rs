//! CAN communication helpers and task functions
//!
//! Provides robust CAN read/write task implementations with proper error handling,
//! validation, and timeout protection. Includes UDP broadcast support for remote logging.

use embassy_stm32::can::Can;
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

        // Create CAN frame
        let frame = match embassy_stm32::can::frame::Frame::new_extended(config.can_id, &frame_data) {
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
                error!("CAN RX: Read failed: {}", defmt::Debug2Format(&e));
                stats.rx_errors += 1;
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

    info!("CAN Reader with UDP broadcast (SCS protocol): Starting with {} ms timeout", config.rx_timeout_ms);

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
                error!("CAN RX: Read failed: {}", defmt::Debug2Format(&e));
                stats.rx_errors += 1;
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
    // Minimum length check
    if frame_data.len() < 6 {
        return Err("Frame too short");
    }

    // Maximum standard CAN frame length
    if frame_data.len() > 8 {
        return Err("Frame too long");
    }

    // Validate throttle field (byte 2)
    if frame_data[2] > 100 {
        return Err("Throttle out of range");
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
