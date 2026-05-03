//! CAN communication helpers and task macros

// ============================================================================
//                         CAN WRITER TASK MACRO
// ============================================================================

/// Generate a CAN writer task that sends data periodically.
///
/// # Usage
/// ```ignore
/// basis::can_write_task!(can_write_rpm, 0x300, 250);
/// ```
#[macro_export]
macro_rules! can_write_task {
    ($name:ident, $can_id:expr, $interval_ms:expr) => {
        #[embassy_executor::task]
        async fn $name(mut can: embassy_stm32::can::Can<'static>) {
            let mut rpm: i16 = 0;
            let mut throttle: i16 = 0;
            let mut map: i16 = 0;
            let mut lambda: i16 = 0;

            loop {
                let mut data = [0u8; 8];
                data[0..2].copy_from_slice(&rpm.to_be_bytes());
                data[2..4].copy_from_slice(&throttle.to_be_bytes());
                data[4..6].copy_from_slice(&map.to_be_bytes());
                data[6..8].copy_from_slice(&lambda.to_be_bytes());
                
                let frame = embassy_stm32::can::frame::Frame::new_extended($can_id, &data).unwrap();
                _ = can.write(&frame).await;
                
                rpm = rpm.wrapping_add(1);
                throttle = throttle.wrapping_add(1);
                map = map.wrapping_add(1);
                lambda = lambda.wrapping_add(1);
                
                embassy_time::Timer::after_millis($interval_ms).await;
            }
        }
    };
}

// ============================================================================
//                         CAN READER TASK MACRO
// ============================================================================

/// Read CAN frames and parse them using the SCS protocol
///
/// # Usage
/// ```ignore
/// basis::can_read_task!(can_read);
/// ```
#[macro_export]
macro_rules! can_read_task {
    ($name:ident) => {
        #[embassy_executor::task]
        async fn $name(mut can: embassy_stm32::can::Can<'static>) {
            use basis::protocol::CanMessage;
            use embedded_can::Id;
            
            loop {
                match can.read().await {
                    Ok(envelope) => {
                        let frame = &envelope.frame;
                        let id_val: u32 = match frame.id() {
                            Id::Standard(id) => id.as_raw() as u32,
                            Id::Extended(id) => id.as_raw(),
                        };
                        let frame_data = frame.data();
                        
                        if let Some(msg) = CanMessage::from_frame(id_val, frame_data) {
                            defmt::info!("CAN: {}", defmt::Debug2Format(&msg));
                        } else {
                            defmt::debug!("CAN ID: {=u32}", id_val);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}
