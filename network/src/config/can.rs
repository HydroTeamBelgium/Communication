//! CAN configuration and types

/// Supported ECU protocol families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EcuType {
    ScsDelta,
    MaxxEcu,
}

impl EcuType {
    pub const fn primary_can_id(self) -> u32 {
        match self {
            Self::ScsDelta => 0x300,
            Self::MaxxEcu => 0x520,
        }
    }
}

/// Default ECU selection for each board role.
/// Keep these centralized so the binaries do not carry hidden local policy.
pub const NUCLEO_1_ECU_MODE: EcuType = EcuType::MaxxEcu;
pub const NUCLEO_2_ECU_MODE: EcuType = EcuType::MaxxEcu;

/// Centralized timing defaults for the ECU test setups.
pub const SCS_TEST_FRAME_INTERVAL_MS: u64 = 250;
pub const MAXX_TEST_FRAME_INTERVAL_MS: u64 = 100;
pub const DEFAULT_CAN_RX_TIMEOUT_MS: u64 = 200;

/// Configuration for CAN communication
#[derive(Clone, Copy, Debug)]
pub struct CanConfig {
    /// Selected ECU protocol family.
    pub ecu_type: EcuType,
    /// CAN bitrate in Hz
    pub bitrate: u32,
    /// TX interval in milliseconds (for periodic senders)
    pub tx_interval_ms: u64,
    /// RX timeout in milliseconds (watchdog for reader)
    pub rx_timeout_ms: u64,
    /// Enable hardware CAN filtering
    pub enable_filtering: bool,
}

impl Default for CanConfig {
    fn default() -> Self {
        Self {
            ecu_type: NUCLEO_1_ECU_MODE,
            bitrate: 500_000,
            tx_interval_ms: SCS_TEST_FRAME_INTERVAL_MS,
            rx_timeout_ms: 5_000,
            enable_filtering: true,
        }
    }
}

impl CanConfig {
    /// Create a new CAN configuration
    pub const fn new(ecu_type: EcuType, bitrate: u32, tx_interval_ms: u64, rx_timeout_ms: u64) -> Self {
        Self {
            ecu_type,
            bitrate,
            tx_interval_ms,
            rx_timeout_ms,
            enable_filtering: true,
        }
    }

    /// Get the primary CAN ID associated with the selected ECU type.
    pub const fn primary_can_id(&self) -> u32 {
        self.ecu_type.primary_can_id()
    }
}

/// CAN statistics for diagnostics
#[derive(Clone, Copy, Debug, Default)]
pub struct CanStats {
    pub tx_count: u32,
    pub tx_errors: u32,
    pub rx_count: u32,
    pub rx_errors: u32,
    pub timeout_count: u32,
}
