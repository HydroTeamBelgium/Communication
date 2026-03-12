//! Generic ADC sensor driver
//!
//! Provides flexible ADC reading for various analog sensors,
//! including factory calibration support for accurate voltage measurement.

use embassy_time::{Duration, Timer};
use crate::protocol::{Message, PotReading};

// ============================================================================
//                    FACTORY CALIBRATION (STM32H7)
// ============================================================================

/// VREFINT calibration address for STM32H7
const VREFINT_CAL_ADDR: *const u16 = 0x1FF1E860 as *const u16;

/// Get VREFINT calibration value from factory calibration
/// 
/// # Safety
/// This reads from a known factory calibration address.
pub fn get_vrefint_cal() -> u16 {
    unsafe { core::ptr::read(VREFINT_CAL_ADDR) }
}

/// Calculate actual VDDA voltage from VREFINT reading
///
/// Uses factory calibration for accurate voltage reference.
pub fn calculate_vdda(vrefint_reading: u16, vrefint_cal: u16) -> f32 {
    0.33 * vrefint_cal as f32 / vrefint_reading as f32
}

/// Helper for factory-calibrated voltage readings
pub struct VoltageReader {
    vrefint_cal: u16,
}

impl VoltageReader {
    pub fn new() -> Self {
        Self { vrefint_cal: get_vrefint_cal() }
    }

    /// Calculate voltage from raw reading and vrefint sample
    pub fn calculate(&self, raw: u16, vrefint: u16, resolution_bits: u8) -> f32 {
        let vdda = calculate_vdda(vrefint, self.vrefint_cal);
        let max_value = (1u32 << resolution_bits) - 1;
        (raw as f32 / max_value as f32) * vdda
    }
}

impl Default for VoltageReader {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
//                         ADC CONFIGURATION
// ============================================================================

/// ADC reading configuration
#[derive(Copy, Clone)]
pub struct AdcConfig {
    /// Sample interval in milliseconds
    pub sample_interval_ms: u64,
    /// Minimum change to trigger report
    pub threshold: u16,
    /// Reference voltage (typically 3.3V)
    pub vref: f32,
    /// ADC resolution in bits (e.g., 14 for STM32H7)
    pub resolution_bits: u8,
    /// Sensor ID for identification in messages
    pub sensor_id: u8,
}

impl Default for AdcConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 100,
            threshold: 10,
            vref: 3.3,
            resolution_bits: 14,
            sensor_id: 0,
        }
    }
}

impl AdcConfig {
    /// Create config for a specific sensor
    pub const fn for_sensor(id: u8, interval_ms: u64) -> Self {
        Self {
            sample_interval_ms: interval_ms,
            threshold: 10,
            vref: 3.3,
            resolution_bits: 14,
            sensor_id: id,
        }
    }
    
    /// Calculate voltage from raw ADC value
    pub fn raw_to_voltage(&self, raw: u16) -> f32 {
        let max_value = (1u32 << self.resolution_bits) - 1;
        (raw as f32 / max_value as f32) * self.vref
    }
    
    /// Create a PotReading from raw value
    pub fn to_reading(&self, raw: u16) -> PotReading {
        PotReading::new(raw, self.raw_to_voltage(raw))
    }
}

/// Generic ADC sensor state tracker
pub struct AdcSensor {
    config: AdcConfig,
    last_raw: u16,
}

impl AdcSensor {
    pub const fn new(config: AdcConfig) -> Self {
        Self {
            config,
            last_raw: 0,
        }
    }
    
    /// Process a new raw reading, returns Some(Message) if should be sent
    pub fn process_reading(&mut self, raw: u16) -> Option<Message> {
        let diff = if raw > self.last_raw {
            raw - self.last_raw
        } else {
            self.last_raw - raw
        };
        
        if diff >= self.config.threshold {
            self.last_raw = raw;
            Some(Message::Pot(self.config.to_reading(raw)))
        } else {
            None
        }
    }
    
    /// Force create a message (ignoring threshold)
    pub fn force_reading(&mut self, raw: u16) -> Message {
        self.last_raw = raw;
        Message::Pot(self.config.to_reading(raw))
    }
    
    /// Wait for sample interval
    pub async fn wait_interval(&self) {
        Timer::after(Duration::from_millis(self.config.sample_interval_ms)).await;
    }
    
    /// Get sensor ID
    pub fn sensor_id(&self) -> u8 {
        self.config.sensor_id
    }
}
