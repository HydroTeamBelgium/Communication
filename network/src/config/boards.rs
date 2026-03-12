//! Board-specific configurations
//!
//! Each Nucleo board has its own configuration struct with IP, MAC, and port settings.

use embassy_net::Ipv4Address;

/// Configuration for a single board
#[derive(Clone, Copy)]
pub struct BoardConfig {
    /// Local IP address of this board
    pub ip: Ipv4Address,
    /// MAC address (last byte typically matches IP last octet for easy identification)
    pub mac: [u8; 6],
    /// UDP port this board listens on
    pub listen_port: u16,
}

// =============================================
//              NUCLEO BOARD CONFIGS
// =============================================

/// Nucleo 1 - Sender board
pub const NUCLEO_1: BoardConfig = BoardConfig {
    ip: Ipv4Address::new(10, 42, 0, 61),
    mac: [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0x61],
    listen_port: 4321,
};

/// Nucleo 2 - Primary receiver board
pub const NUCLEO_2: BoardConfig = BoardConfig {
    ip: Ipv4Address::new(10, 42, 0, 60),
    mac: [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0x60],
    listen_port: 12345,
};

/// Nucleo 3 - Secondary receiver board
pub const NUCLEO_3: BoardConfig = BoardConfig {
    ip: Ipv4Address::new(10, 42, 0, 62),
    mac: [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0x62],
    listen_port: 12346,
};

// =============================================
//              DESTINATION HELPERS
// =============================================

/// Get destination endpoint for sending to another board
impl BoardConfig {
    pub const fn endpoint(&self) -> (Ipv4Address, u16) {
        (self.ip, self.listen_port)
    }
}

// =============================================
//              COMMON DESTINATIONS
// =============================================

/// Default destination for Nucleo 1 (sends to Nucleo 2)
pub const NUCLEO_1_DESTINATION: (Ipv4Address, u16) = NUCLEO_2.endpoint();

/// Default destination for Nucleo 2 (sends to Nucleo 1)  
pub const NUCLEO_2_DESTINATION: (Ipv4Address, u16) = NUCLEO_1.endpoint();

/// Default destination for Nucleo 3 (sends to Nucleo 1)
pub const NUCLEO_3_DESTINATION: (Ipv4Address, u16) = NUCLEO_1.endpoint();
