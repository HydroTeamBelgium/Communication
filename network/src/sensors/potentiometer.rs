//! Potentiometer / Analog sensor driver
//!
//! Provides voltage reading from ADC with calibration.

use embassy_stm32::adc::Adc;
use embassy_stm32::peripherals;
use embassy_stm32::Peri;
use embassy_time::{Duration, Timer};
use crate::protocol::{Message, PotReading};
use super::adc::get_vrefint_cal;

/// Potentiometer driver configuration
#[derive(Copy, Clone)]
pub struct PotConfig {
    /// Sample interval in milliseconds
    pub sample_interval_ms: u64,
    /// Minimum change in raw value to trigger a new reading
    pub threshold: u16,
    /// Enable VREFINT calibration for accurate voltage
    pub use_vrefint: bool,
}

impl Default for PotConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 100,
            threshold: 10,
            use_vrefint: true,
        }
    }
}

/// Potentiometer driver
pub struct PotDriver {
    config: PotConfig,
    vrefint_cal: u16,
    last_raw: u16,
}

impl PotDriver {
    pub fn new(config: PotConfig) -> Self {
        Self {
            config,
            vrefint_cal: get_vrefint_cal(),
            last_raw: 0,
        }
    }
    
    /// Read raw ADC value (blocking)
    pub fn read_raw(
        &mut self,
        adc: &mut Adc<'static, peripherals::ADC2>,
        pin: &mut Peri<'static, peripherals::PA3>,
    ) -> u16 {
        adc.blocking_read(pin)
    }
    
    /// Read with VREFINT calibration for accurate voltage
    pub fn read_calibrated(
        &mut self,
        adc: &mut Adc<'static, peripherals::ADC2>,
        pin: &mut Peri<'static, peripherals::PA3>,
        vrefint_channel: &mut impl embassy_stm32::adc::AdcChannel<peripherals::ADC2>,
    ) -> PotReading {
        let vrefint = adc.blocking_read(vrefint_channel);
        let raw = adc.blocking_read(pin);
        
        // Calculate VDDA from VREFINT
        let vdda = 3.3 * self.vrefint_cal as f32 / vrefint as f32;
        
        // Calculate voltage (14-bit ADC)
        let voltage = (raw as f32 / 16383.0) * vdda;
        
        self.last_raw = raw;
        PotReading::new(raw, voltage)
    }
    
    /// Simple read without VREFINT (assumes 3.3V reference)
    pub fn read_simple(
        &mut self,
        adc: &mut Adc<'static, peripherals::ADC2>,
        pin: &mut Peri<'static, peripherals::PA3>,
    ) -> PotReading {
        let raw = adc.blocking_read(pin);
        let voltage = (raw as f32 / 16383.0) * 3.3;
        
        self.last_raw = raw;
        PotReading::new(raw, voltage)
    }
    
    /// Check if value changed enough to report
    pub fn value_changed(&self, new_raw: u16) -> bool {
        let diff = if new_raw > self.last_raw {
            new_raw - self.last_raw
        } else {
            self.last_raw - new_raw
        };
        diff >= self.config.threshold
    }
    
    /// Wait for the configured sample interval
    pub async fn wait_interval(&self) {
        Timer::after(Duration::from_millis(self.config.sample_interval_ms)).await;
    }
    
    /// Create a Message from a reading
    pub fn to_message(&self, reading: PotReading) -> Message {
        Message::Pot(reading)
    }
}

/// Macro to generate a potentiometer task with simple reading (no VREFINT)
#[macro_export]
macro_rules! pot_task_simple {
    ($name:ident, $channel:expr, $config:expr) => {
        #[embassy_executor::task]
        async fn $name(
            mut adc: ::embassy_stm32::adc::Adc<'static, ::embassy_stm32::peripherals::ADC2>,
            mut pin: ::embassy_stm32::Peri<'static, ::embassy_stm32::peripherals::PA3>,
        ) -> ! {
            let mut driver = $crate::sensors::PotDriver::new($config);
            loop {
                let reading = driver.read_simple(&mut adc, &mut pin);
                if driver.value_changed(reading.measured) {
                    $channel.send(driver.to_message(reading)).await;
                }
                driver.wait_interval().await;
            }
        }
    };
}
