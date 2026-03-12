//! Message types for inter-board communication
//!
//! All boards use these same message types to ensure compatibility.

use defmt::Format;

/// Message type discriminator byte
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Format)]
pub enum MessageType {
    /// Raw byte message (16 bytes payload)
    Bytes = 0x01,
    /// Potentiometer reading
    Pot = 0x02,
}

impl MessageType {
    /// Try to parse a message type from a byte
    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Bytes),
            0x02 => Some(Self::Pot),
            _ => None,
        }
    }
}

/// Potentiometer/ADC reading data
#[derive(Copy, Clone, Debug, Format)]
pub struct PotReading {
    /// Raw ADC value (0-16383 for 14-bit ADC)
    pub measured: u16,
    /// Calculated voltage in volts
    pub voltage: f32,
}

impl PotReading {
    /// Create a new reading
    pub const fn new(measured: u16, voltage: f32) -> Self {
        Self { measured, voltage }
    }

    /// Serialize to bytes (4 bytes: voltage as f32 big-endian)
    pub fn to_bytes(&self) -> [u8; 4] {
        self.voltage.to_bits().to_be_bytes()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let voltage = f32::from_bits(u32::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]));
        Some(Self {
            measured: 0, // Not transmitted, only voltage
            voltage,
        })
    }
}

/// Message variants for channel communication
#[derive(Copy, Clone, Debug, Format)]
pub enum Message {
    /// Raw bytes (16 byte fixed payload)
    Bytes([u8; 16]),
    /// Potentiometer reading
    Pot(PotReading),
}

impl Message {
    /// Get the message type byte
    pub const fn message_type(&self) -> MessageType {
        match self {
            Self::Bytes(_) => MessageType::Bytes,
            Self::Pot(_) => MessageType::Pot,
        }
    }

    /// Serialize message to a buffer, returns number of bytes written
    /// Format: [type_byte][payload...]
    pub fn serialize(&self, buf: &mut [u8]) -> usize {
        match self {
            Self::Bytes(data) => {
                if buf.len() < 17 {
                    return 0;
                }
                buf[0] = MessageType::Bytes as u8;
                buf[1..17].copy_from_slice(data);
                17
            }
            Self::Pot(reading) => {
                if buf.len() < 5 {
                    return 0;
                }
                buf[0] = MessageType::Pot as u8;
                buf[1..5].copy_from_slice(&reading.to_bytes());
                5
            }
        }
    }

    /// Deserialize from buffer
    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.is_empty() {
            return None;
        }
        match MessageType::from_byte(buf[0])? {
            MessageType::Bytes => {
                if buf.len() < 17 {
                    return None;
                }
                let mut data = [0u8; 16];
                data.copy_from_slice(&buf[1..17]);
                Some(Self::Bytes(data))
            }
            MessageType::Pot => {
                let reading = PotReading::from_bytes(&buf[1..])?;
                Some(Self::Pot(reading))
            }
        }
    }
}

// =============================================
//              HELPER MACROS
// =============================================

/// Create a Bytes message from a string literal (pads/truncates to 16 bytes)
#[macro_export]
macro_rules! msg_bytes {
    ($s:literal) => {{
        let bytes = $s.as_bytes();
        let mut arr = [0u8; 16];
        let len = if bytes.len() > 16 { 16 } else { bytes.len() };
        let mut i = 0;
        while i < len {
            arr[i] = bytes[i];
            i += 1;
        }
        $crate::protocol::Message::Bytes(arr)
    }};
}
