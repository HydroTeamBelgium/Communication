//! CAN Communication Hardware Abstraction
//!
//! Provides CAN configuration constants and helpers.
//! Single source of truth for CAN communication parameters.
//!
//! Note: Each binary must define its own interrupt handler binding:
//! ```ignore
//! bind_interrupts!(struct CanIrqs {
//!     FDCAN1_IT0 => can::IT0InterruptHandler<peripherals::FDCAN1>;
//!     FDCAN1_IT1 => can::IT1InterruptHandler<peripherals::FDCAN1>;
//! });
//! ```

/// CAN Configuration Constants
/// 
/// These are the standard ECU communication parameters.
/// Used across all Nucleo boards for consistent CAN behavior.
pub mod config {
    /// Standard CAN bitrate for ECU communication: 500 kbps
    /// 
    /// Supports:
    /// - 250 kbps for reduced bandwidth (legacy ECU)
    /// - 500 kbps for standard ECU
    /// - 1 Mbps for high-speed diagnostics
    pub const CAN_BITRATE_500K: u32 = 500_000;
    pub const CAN_BITRATE_250K: u32 = 250_000;
    pub const CAN_BITRATE_1M: u32 = 1_000_000;

    /// Default bitrate used by all boards
    pub const CAN_BITRATE_DEFAULT: u32 = CAN_BITRATE_500K;

    /// CAN read timeout in milliseconds
    pub const CAN_RX_TIMEOUT_MS: u32 = 200;

    /// Pin assignments for FDCAN1 (standard on STM32H755)
    pub const CAN_RX_PIN: &str = "PD0";  // FDCAN1_RX
    pub const CAN_TX_PIN: &str = "PD1";  // FDCAN1_TX
}

/// CAN Setup Template for Binary Files
///
/// Each binary should use this pattern in its main setup:
/// ```ignore
/// let mut can = can::CanConfigurator::new(p.FDCAN1, p.PD0, p.PD1, CanIrqs);
/// can.set_bitrate(crate::hal::can::config::CAN_BITRATE_DEFAULT);
/// let can = can.into_normal_mode();
/// ```
pub fn setup_can_config_comment() {
    // This is documentation only - see above for usage pattern
}
