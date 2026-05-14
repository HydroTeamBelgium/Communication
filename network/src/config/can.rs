//! CAN configuration and types

/// Configuration for CAN communication
#[derive(Clone, Copy, Debug)]
pub struct CanConfig {
    /// CAN message ID (extended frame)
    pub can_id: u32,
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
            can_id: 0x300,
            bitrate: 500_000,
            tx_interval_ms: 250,
            rx_timeout_ms: 5000,
            enable_filtering: true,
        }
    }
}

impl CanConfig {
    /// Create a new CAN configuration
    pub const fn new(can_id: u32, bitrate: u32, tx_interval_ms: u64, rx_timeout_ms: u64) -> Self {
        Self {
            can_id,
            bitrate,
            tx_interval_ms,
            rx_timeout_ms,
            enable_filtering: true,
        }
    }

    /// Set custom RX timeout
    pub const fn with_rx_timeout(mut self, ms: u64) -> Self {
        self.rx_timeout_ms = ms;
        self
    }

    /// Disable hardware filtering
    pub const fn without_filtering(mut self) -> Self {
        self.enable_filtering = false;
        self
    }
}

/// CAN frame validation error types
#[derive(Clone, Copy, Debug)]
pub enum CanValidationError {
    /// Data length code mismatch
    DlcMismatch { expected: usize, got: usize },
    /// Sequence number gap detected
    SequenceGap { expected: u8, got: u8 },
    /// Invalid checksum
    ChecksumInvalid,
    /// Frame timeout
    Timeout,
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
