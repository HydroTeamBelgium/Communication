//! SCS Protocol Constants - Single Source of Truth
//!
//! Centralized definitions for all CAN message IDs, types, and ranges.
//! Used across Rust binaries and Python receiver.

#![allow(non_upper_case_globals)]

// ============================================================================
//                         CAN MESSAGE IDs
// ============================================================================

/// SCS message IDs (0x300 - 0x310 range)
pub const SCS_MESSAGE_IDS: &[u32] = &[
    0x300, 0x301, 0x302, 0x303, 0x304, 0x305, 0x306,
    0x307, 0x308, 0x309, 0x30A, 0x30B, 0x30C, 0x30D,
    0x30E, 0x30F, 0x310,
];

pub const MSG_ID_0x300: u32 = 0x300;  // Engine Control
pub const MSG_ID_0x301: u32 = 0x301;  // Fuel & Lambda
pub const MSG_ID_0x302: u32 = 0x302;  // Speed & Idle Control
pub const MSG_ID_0x303: u32 = 0x303;  // Cam Control
pub const MSG_ID_0x304: u32 = 0x304;  // Injection & Spark
pub const MSG_ID_0x305: u32 = 0x305;  // Lambda & Fuel Trim
pub const MSG_ID_0x306: u32 = 0x306;  // Boost & Pressures
pub const MSG_ID_0x307: u32 = 0x307;  // Barometric Pressure & Boost Control
pub const MSG_ID_0x308: u32 = 0x308;  // Battery & Ignition
pub const MSG_ID_0x309: u32 = 0x309;  // Raw Throttle & Pedal
pub const MSG_ID_0x30A: u32 = 0x30A;  // Scaled Throttle & Pedal
pub const MSG_ID_0x30B: u32 = 0x30B;  // Temperatures
pub const MSG_ID_0x30C: u32 = 0x30C;  // Run Timer & Corrections
pub const MSG_ID_0x30D: u32 = 0x30D;  // Crank Counter & Learn Values
pub const MSG_ID_0x30E: u32 = 0x30E;  // Wheel Speeds
pub const MSG_ID_0x30F: u32 = 0x30F;  // Fuel Learn & Level
pub const MSG_ID_0x310: u32 = 0x310;  // Knock Retard

pub const TOTAL_MESSAGE_TYPES: usize = 17;

// ============================================================================
//                      MESSAGE TYPE CONSTANTS (for Python)
// ============================================================================

/// Message type identifiers for UDP broadcast protocol
/// Must match Python receiver expectations
pub const MSG_TYPE_BYTES: u8 = 0x01;
pub const MSG_TYPE_POT: u8 = 0x02;
pub const MSG_TYPE_CAN_FRAME: u8 = 0x03;
pub const MSG_TYPE_ECU_JSON: u8 = 0x04;

// ============================================================================
//                     GENERATOR RANGE CONSTANTS
// ============================================================================

/// Valid ranges for test data generation
/// Ensures generated data stays within ECU specification bounds

// 0x300 - Engine Control
pub const RPM_MIN: u16 = 500;
pub const RPM_MAX: u16 = 8000;
pub const RPM_BASE: u16 = 2000;
pub const RPM_STEP: u16 = 100;

pub const TPS_MIN: u8 = 0;
pub const TPS_MAX: u8 = 255;     // 0-100%
pub const TPS_BASE: u8 = 50;

pub const KFUEL_MAP_MIN: u8 = 0;
pub const KFUEL_MAP_MAX: u8 = 255;  // 0-400%
pub const KFUEL_MAP_BASE: u8 = 100;

pub const MAP_MIN: i16 = 0;
pub const MAP_MAX: i16 = 2000;
pub const MAP_BASE: i16 = 1000;
pub const MAP_STEP: i16 = 50;

pub const IDLE_LEARN_BASE: u16 = 100;

// 0x301 - Fuel & Lambda
pub const LAMBDA_MIN: u8 = 0;      // 0.00
pub const LAMBDA_MAX: u8 = 255;    // 2.00
pub const LAMBDA_BASE: u8 = 128;

pub const INJ_H_PERC_MIN: u8 = 0;
pub const INJ_H_PERC_MAX: u8 = 255;  // 0-100%
pub const INJ_H_PERC_BASE: u8 = 200;

// 0x302 - Speed & Idle Control
pub const KMH_MIN: u16 = 0;
pub const KMH_MAX: u16 = 3000;   // 0.1 kmh/bit resolution
pub const SLIP_MIN: u8 = 0;
pub const SLIP_MAX: u8 = 255;    // 0-100%

// 0x303 - Cam Control
pub const CAM_ANGLE_MIN: i16 = -180;
pub const CAM_ANGLE_MAX: i16 = 180;
pub const CAM_ANGLE_BASE: i16 = -50;

pub const DBW_TPS_MIN: u16 = 0;
pub const DBW_TPS_MAX: u16 = 1023;  // 0-100%

// 0x308 - Battery & Ignition
pub const VBATT_MIN: u16 = 0;
pub const VBATT_MAX: u16 = 1023;   // 0-18V (VBatt max)
pub const VBATT_BASE: u16 = 12000 / 18;  // ~667 for 12V

pub const DWELL_MIN: u16 = 1000;
pub const DWELL_MAX: u16 = 5000;   // microseconds

// Generic scale factors
pub const PERCENT_SCALE_U8: f32 = 100.0 / 255.0;      // Convert u8 to %
pub const PERCENT_SCALE_U16: f32 = 100.0 / 1023.0;    // Convert u16 to %

// ============================================================================
//                      HELPER FUNCTIONS
// ============================================================================

/// Check if a message ID is a valid SCS message
#[inline]
pub fn is_valid_scs_message(id: u32) -> bool {
    id >= 0x300 && id <= 0x310
}

/// Get human-readable name for message ID
pub fn message_name(id: u32) -> &'static str {
    match id {
        0x300 => "Engine Control",
        0x301 => "Fuel & Lambda",
        0x302 => "Speed & Idle Control",
        0x303 => "Cam Control",
        0x304 => "Injection & Spark",
        0x305 => "Lambda & Fuel Trim",
        0x306 => "Boost & Pressures",
        0x307 => "Barometric Pressure & Boost Control",
        0x308 => "Battery & Ignition",
        0x309 => "Raw Throttle & Pedal",
        0x30A => "Scaled Throttle & Pedal",
        0x30B => "Temperatures",
        0x30C => "Run Timer & Corrections",
        0x30D => "Crank Counter & Learn Values",
        0x30E => "Wheel Speeds",
        0x30F => "Fuel Learn & Level",
        0x310 => "Knock Retard",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_ids_valid_range() {
        for &id in SCS_MESSAGE_IDS {
            assert!(is_valid_scs_message(id), "ID 0x{:03X} should be valid", id);
        }
    }

    #[test]
    fn test_message_ids_count() {
        assert_eq!(SCS_MESSAGE_IDS.len(), TOTAL_MESSAGE_TYPES);
    }

    #[test]
    fn test_all_message_ids_sequential() {
        for i in 0..TOTAL_MESSAGE_TYPES {
            assert_eq!(
                SCS_MESSAGE_IDS[i],
                0x300 + (i as u32),
                "Message IDs should be sequential from 0x300"
            );
        }
    }
}
