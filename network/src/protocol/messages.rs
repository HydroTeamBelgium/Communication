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
    /// CAN frame data (8 byte payload + CAN ID)
    CanFrame = 0x03,
    /// ECU SCS JSON logging data
    EcuJson = 0x04,
}

impl MessageType {
    /// Try to parse a message type from a byte
    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Bytes),
            0x02 => Some(Self::Pot),
            0x03 => Some(Self::CanFrame),
            0x04 => Some(Self::EcuJson),
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

/// CAN frame data for remote logging
#[derive(Copy, Clone, Debug, Format)]
pub struct CanFrameData {
    /// CAN identifier (extended)
    pub can_id: u32,
    /// Frame data (8 bytes)
    pub data: [u8; 8],
    /// Data length code (0-8)
    pub dlc: u8,
}

impl CanFrameData {
    /// Create a new CAN frame data
    pub const fn new(can_id: u32, data: [u8; 8], dlc: u8) -> Self {
        Self { can_id, data, dlc }
    }

    /// Serialize to bytes (13 bytes: 4 CAN ID + 8 data + 1 DLC)
    pub fn to_bytes(&self) -> [u8; 13] {
        let mut buf = [0u8; 13];
        buf[0..4].copy_from_slice(&self.can_id.to_be_bytes());
        buf[4..12].copy_from_slice(&self.data);
        buf[12] = self.dlc;
        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 13 {
            return None;
        }
        let can_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let mut data = [0u8; 8];
        data.copy_from_slice(&bytes[4..12]);
        let dlc = bytes[12];

        if dlc > 8 {
            return None;
        }

        Some(Self { can_id, data, dlc })
    }
}

/// ECU JSON data for remote logging (variable length string)
#[derive(Clone, Debug, Format)]
pub struct EcuJsonData {
    /// JSON string (up to 256 bytes)
    pub json: heapless::String<256>,
}

impl EcuJsonData {
    /// Create new ECU JSON data
    pub fn new(json: heapless::String<256>) -> Self {
        Self { json }
    }

    /// Serialize to bytes (1 byte length + JSON data)
    pub fn to_bytes(&self, buf: &mut [u8]) -> usize {
        let json_bytes = self.json.as_bytes();
        if buf.len() < 1 + json_bytes.len() {
            return 0;
        }
        buf[0] = json_bytes.len() as u8;
        buf[1..1 + json_bytes.len()].copy_from_slice(json_bytes);
        1 + json_bytes.len()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }
        let len = bytes[0] as usize;
        if bytes.len() < 1 + len {
            return None;
        }
        let json_str = core::str::from_utf8(&bytes[1..1 + len]).ok()?;
        let json: heapless::String<256> = heapless::String::try_from(json_str).ok()?;
        Some(Self { json })
    }
}

/// Message variants for channel communication
#[derive(Clone, Debug, Format)]
pub enum Message {
    /// Raw bytes (16 byte fixed payload)
    Bytes([u8; 16]),
    /// Potentiometer reading
    Pot(PotReading),
    /// CAN frame data for remote logging
    CanFrame(CanFrameData),
    /// ECU SCS JSON logging data
    EcuJson(EcuJsonData),
}

impl Message {
    /// Get the message type byte
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::Bytes(_) => MessageType::Bytes,
            Self::Pot(_) => MessageType::Pot,
            Self::CanFrame(_) => MessageType::CanFrame,
            Self::EcuJson(_) => MessageType::EcuJson,
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
            Self::CanFrame(can_data) => {
                if buf.len() < 14 {
                    return 0;
                }
                buf[0] = MessageType::CanFrame as u8;
                buf[1..14].copy_from_slice(&can_data.to_bytes());
                14
            }
            Self::EcuJson(ecu_data) => {
                if buf.len() < 2 {
                    return 0;
                }
                buf[0] = MessageType::EcuJson as u8;
                ecu_data.to_bytes(&mut buf[1..]) + 1
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
            MessageType::CanFrame => {
                let can_data = CanFrameData::from_bytes(&buf[1..])?;
                Some(Self::CanFrame(can_data))
            }
            MessageType::EcuJson => {
                let ecu_data = EcuJsonData::from_bytes(&buf[1..])?;
                Some(Self::EcuJson(ecu_data))
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
