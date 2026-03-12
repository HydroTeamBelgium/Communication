//! Network configuration constants
//!
//! All network-related constants in one place for easy modification.

use embassy_net::Ipv4Address;

// =============================================
//              GATEWAY / COMMON
// =============================================
pub const NETWORK_GATEWAY_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
pub const NETWORK_SUBNET_PREFIX: u8 = 24;

// =============================================
//              BUFFER SIZES
// =============================================
/// RX buffer size - must be >= MAX_PACKET_SIZE
pub const RX_BUFFER_SIZE: usize = 2048;
/// TX buffer size - must be >= MAX_PACKET_SIZE
pub const TX_BUFFER_SIZE: usize = 2048;
/// Maximum UDP packet size (MTU - headers)
pub const MAX_PACKET_SIZE: usize = 1536;

// =============================================
//              SOCKET CONFIGURATION
// =============================================
pub const RX_METADATA_COUNT: usize = 32;
pub const TX_METADATA_COUNT: usize = 32;

// =============================================
//              VALIDATION
// =============================================
/// Compile-time validation of buffer sizes
pub const fn validate_config() {
    assert!(RX_BUFFER_SIZE >= MAX_PACKET_SIZE, "RX buffer too small");
    assert!(TX_BUFFER_SIZE >= MAX_PACKET_SIZE, "TX buffer too small");
}
