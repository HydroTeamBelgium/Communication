//! Button sensor driver
//!
//! Provides button reading with debouncing and edge detection.

use embassy_stm32::exti::ExtiInput;
use embassy_time::{Duration, Timer};
use crate::protocol::Message;

/// Button event types
#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum ButtonEvent {
    Pressed,
    Released,
    LongPress,
}

/// Configuration for button behavior
#[derive(Copy, Clone)]
pub struct ButtonConfig {
    /// Debounce time in milliseconds
    pub debounce_ms: u64,
    /// Long press threshold in milliseconds (0 = disabled)
    pub long_press_ms: u64,
    /// Message to send on press (16 bytes, will be padded/truncated)
    pub press_message: [u8; 16],
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 20,
            long_press_ms: 0, // disabled by default
            press_message: *b"button pressed  ",
        }
    }
}

impl ButtonConfig {
    /// Create config with custom message
    pub const fn with_message(msg: &[u8; 16]) -> Self {
        Self {
            debounce_ms: 20,
            long_press_ms: 0,
            press_message: *msg,
        }
    }
    
    /// Create config for a specific button number
    pub fn for_button(num: u8) -> Self {
        let mut msg = *b"button X pressed";
        msg[7] = b'0' + num;
        Self {
            debounce_ms: 20,
            long_press_ms: 0,
            press_message: msg,
        }
    }
}

/// Button driver - handles edge detection and creates messages
pub struct ButtonDriver {
    config: ButtonConfig,
}

impl ButtonDriver {
    pub const fn new(config: ButtonConfig) -> Self {
        Self { config }
    }
    
    /// Wait for button press with debouncing, returns message to send
    pub async fn wait_for_press(&self, button: &mut ExtiInput<'static>) -> Message {
        button.wait_for_rising_edge().await;
        
        // Debounce
        if self.config.debounce_ms > 0 {
            Timer::after(Duration::from_millis(self.config.debounce_ms)).await;
        }
        
        Message::Bytes(self.config.press_message)
    }
    
    /// Wait for release (call after wait_for_press)
    pub async fn wait_for_release(&self, button: &mut ExtiInput<'static>) {
        button.wait_for_falling_edge().await;
        
        // Debounce
        if self.config.debounce_ms > 0 {
            Timer::after(Duration::from_millis(self.config.debounce_ms)).await;
        }
    }
    
    /// Full button cycle - returns message on press, waits for release
    pub async fn wait_for_press_release(&self, button: &mut ExtiInput<'static>) -> Message {
        let msg = self.wait_for_press(button).await;
        self.wait_for_release(button).await;
        msg
    }
}

/// Macro to generate a button task
/// 
/// Usage:
/// ```ignore
/// button_task!(my_button_task, CHANNEL, ButtonConfig::for_button(1));
/// ```
#[macro_export]
macro_rules! button_task {
    ($name:ident, $channel:expr, $config:expr) => {
        #[embassy_executor::task]
        async fn $name(mut button: ::embassy_stm32::exti::ExtiInput<'static>) -> ! {
            let driver = $crate::sensors::ButtonDriver::new($config);
            loop {
                let msg = driver.wait_for_press(&mut button).await;
                ::defmt::info!("Button pressed");
                $channel.send(msg).await;
                driver.wait_for_release(&mut button).await;
            }
        }
    };
}
