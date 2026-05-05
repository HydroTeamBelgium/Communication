//! Engine data structures for CAN transmission

/// Real engine sensor data
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineData {
    /// Engine RPM (0-8000)
    pub rpm: u16,
    /// Throttle position (0-100%)
    pub throttle: u8,
    /// Manifold absolute pressure (0-300 kPa)
    pub map: u16,
    /// Lambda / air-fuel ratio (0.5-2.0, stored as u8 with scaling)
    pub lambda_scaled: u8,
}

impl EngineData {
    /// Create new engine data with all zeros
    pub const fn new() -> Self {
        Self {
            rpm: 0,
            throttle: 0,
            map: 0,
            lambda_scaled: 100, // 1.0 lambda (stoich)
        }
    }

    /// Serialize to 8-byte CAN frame format
    /// Format:
    /// - Bytes 0-1: RPM (big-endian u16)
    /// - Byte 2: Throttle (0-100)
    /// - Bytes 3-4: MAP (big-endian u16)
    /// - Byte 5: Lambda scaled (value/100 = actual lambda)
    /// - Bytes 6-7: Reserved
    pub fn to_can_frame(&self) -> [u8; 8] {
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&self.rpm.to_be_bytes());
        data[2] = self.throttle;
        data[3..5].copy_from_slice(&self.map.to_be_bytes());
        data[5] = self.lambda_scaled;
        // Bytes 6-7 reserved
        data
    }

    /// Deserialize from CAN frame with validation
    pub fn from_can_frame(frame_data: &[u8]) -> Result<Self, CanParseError> {
        if frame_data.len() < 6 {
            return Err(CanParseError::InvalidLength);
        }

        let rpm = u16::from_be_bytes([frame_data[0], frame_data[1]]);
        let throttle = frame_data[2];
        let map = u16::from_be_bytes([frame_data[3], frame_data[4]]);
        let lambda_scaled = frame_data[5];

        // Validation: throttle should be 0-100
        if throttle > 100 {
            return Err(CanParseError::InvalidThrottle(throttle));
        }

        Ok(Self {
            rpm,
            throttle,
            map,
            lambda_scaled,
        })
    }

    /// Simulate reading from real sensors
    /// For testing: incrementally updates RPM
    pub fn simulate_sensor_read(seq: u8) -> Self {
        Self {
            rpm: 1000 + (seq as u16 * 50), // Simulate RPM increase
            throttle: 50,                  // 50% throttle
            map: 100,                      // 100 kPa
            lambda_scaled: 100,            // 1.0 lambda
        }
    }
}

/// CAN frame parsing errors
#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum CanParseError {
    InvalidLength,
    InvalidThrottle(u8),
    ChecksumMismatch,
}
