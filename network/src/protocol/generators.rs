//! Test Data Generators for SCS CAN Protocol
//!
//! Professional, modular test data generation for all SCS message types (0x300-0x310).
//! Provides realistic sensor data simulation with configurable parameters.
//!
//! Uses constants from `protocol::constants` to ensure compliance with ECU specifications.
//! All generated values stay within valid ranges to prevent silent data corruption.
//!
//! # Example
//! ```ignore
//! let generator = ScsTestGenerator::new();
//! let (can_id, frame_data) = generator.generate_frame(0x300);
//! ```

use defmt::Format;
use super::constants::*;

// ============================================================================
//                      TEST DATA GENERATOR
// ============================================================================

/// Professional test data generator for SCS CAN messages
///
/// Generates realistic test data for all 17 message types with:
/// - Configurable base values and ranges
/// - Realistic sensor value progression
/// - Proper data format encoding
/// - No panics on edge cases
#[derive(Copy, Clone, Debug, Format)]
pub struct ScsTestGenerator {
    counter: u8,
    base_rpm: u16,
    base_temp: u8,
    base_boost: u16,
}

impl ScsTestGenerator {
    /// Create a new test generator with default values
    pub const fn new() -> Self {
        Self {
            counter: 0,
            base_rpm: 2000,
            base_temp: 20,
            base_boost: 1000,
        }
    }

    /// Create a test generator with custom starting counter
    pub const fn with_counter(counter: u8) -> Self {
        Self {
            counter,
            base_rpm: 2000,
            base_temp: 20,
            base_boost: 1000,
        }
    }

    /// Increment the counter for next generation cycle
    pub fn next_cycle(&mut self) {
        self.counter = self.counter.wrapping_add(1);
    }

    /// Get current counter value
    pub const fn counter(&self) -> u8 {
        self.counter
    }

    /// Generate a CAN frame for the specified message ID
    ///
    /// Returns `(CAN_ID, [8-byte data frame])`
    ///
    /// # Panics
    /// Never panics - returns safe default data for unrecognized IDs
    pub fn generate_frame(&self, msg_id: u32) -> (u32, [u8; 8]) {
        let mut data = [0u8; 8];

        match msg_id {
            0x300 => self.generate_msg300(&mut data),
            0x301 => self.generate_msg301(&mut data),
            0x302 => self.generate_msg302(&mut data),
            0x303 => self.generate_msg303(&mut data),
            0x304 => self.generate_msg304(&mut data),
            0x305 => self.generate_msg305(&mut data),
            0x306 => self.generate_msg306(&mut data),
            0x307 => self.generate_msg307(&mut data),
            0x308 => self.generate_msg308(&mut data),
            0x309 => self.generate_msg309(&mut data),
            0x30A => self.generate_msg30a(&mut data),
            0x30B => self.generate_msg30b(&mut data),
            0x30C => self.generate_msg30c(&mut data),
            0x30D => self.generate_msg30d(&mut data),
            0x30E => self.generate_msg30e(&mut data),
            0x30F => self.generate_msg30f(&mut data),
            0x310 => self.generate_msg310(&mut data),
            _ => {} // Safe default: all zeros
        }

        (msg_id, data)
    }

    // ========================================================================
    //                       MESSAGE GENERATORS
    // ========================================================================

    /// 0x300: Engine Control - RPM, TPS, Fuel Map, MAP, Idle Learn
    /// 
    /// Generates realistic engine control values:
    /// - RPM: 2000-8000 rpm range
    /// - TPS: 0-100% (maps to 0-255 raw value)
    /// - KFuelMAP: 0-400% fuel correction
    /// - MAP: Manifold air pressure in mBar
    /// - IdleLearn: Idle learning value
    fn generate_msg300(&self, data: &mut [u8; 8]) {
        // RPM: Base 2000 + incrementing by 100 rpm per counter cycle, capped at 8000
        let rpm = (self.base_rpm + (self.counter as u16).saturating_mul(RPM_STEP)).min(RPM_MAX);
        data[0..2].copy_from_slice(&rpm.to_be_bytes());
        
        // TPS: Start at 50/255 (~20%) and increment, saturating at 255 (100%)
        data[2] = TPS_BASE.saturating_add(self.counter);
        
        // KFuelMAP: Start at 100/255 (~39%) and increment, saturating at 255 (400%)
        data[3] = KFUEL_MAP_BASE.saturating_add(self.counter);
        
        // MAP: Start at 1000 mBar and increment by 50
        let map = (MAP_BASE as i16 + (self.counter as i16) * MAP_STEP).clamp(MAP_MIN, MAP_MAX);
        data[4..6].copy_from_slice(&map.to_be_bytes());
        
        // IdleLearn: Constant at 100 (base idle learning value)
        data[6..8].copy_from_slice(&IDLE_LEARN_BASE.to_be_bytes());
    }

    /// 0x301: Fuel & Lambda
    fn generate_msg301(&self, data: &mut [u8; 8]) {
        let d_throt = (self.counter as i16) * 10;
        data[0..2].copy_from_slice(&d_throt.to_be_bytes());
        data[2] = 128_u8.saturating_add(self.counter);
        data[3] = 200_u8.saturating_add(self.counter);
        let ae = (self.counter as u16) * 100;
        data[4..6].copy_from_slice(&ae.to_be_bytes());
        let de = (self.counter as u16) * 50;
        data[6..8].copy_from_slice(&de.to_be_bytes());
    }

    /// 0x302: Speed & Idle Control
    fn generate_msg302(&self, data: &mut [u8; 8]) {
        let kmh = ((self.counter as u16) * 5) % 300;
        data[0..2].copy_from_slice(&kmh.to_be_bytes());
        let dc_base = (self.counter as u16) * 100;
        data[2..4].copy_from_slice(&dc_base.to_be_bytes());
        let idle_out = (self.counter as u16) * 80;
        data[4..6].copy_from_slice(&idle_out.to_be_bytes());
        data[6] = (self.counter).wrapping_mul(10);
        data[7] = (self.counter).wrapping_mul(15);
    }

    /// 0x303: Cam Control
    fn generate_msg303(&self, data: &mut [u8; 8]) {
        let ivct = -50i16 + (self.counter as i16) * 5;
        data[0..2].copy_from_slice(&ivct.to_be_bytes());
        let evct = -30i16 + (self.counter as i16) * 3;
        data[2..4].copy_from_slice(&evct.to_be_bytes());
        data[4] = (self.counter).wrapping_mul(2);
        data[5] = (self.counter).wrapping_mul(3);
        let dbw = ((self.counter as u16) * 100) % 1023;
        data[6..8].copy_from_slice(&dbw.to_be_bytes());
    }

    /// 0x304: Injection & Spark
    fn generate_msg304(&self, data: &mut [u8; 8]) {
        let base_inj = (1000 + (self.counter as u16) * 50) % 10000;
        data[0..2].copy_from_slice(&base_inj.to_be_bytes());
        let run_pw = (900 + (self.counter as u16) * 40) % 8000;
        data[2..4].copy_from_slice(&run_pw.to_be_bytes());
        let sa_base = -10i16 + (self.counter as i16) * 2;
        data[4..6].copy_from_slice(&sa_base.to_be_bytes());
        let sa_out = -5i16 + (self.counter as i16);
        data[6..8].copy_from_slice(&sa_out.to_be_bytes());
    }

    /// 0x305: Lambda & Fuel Trim
    fn generate_msg305(&self, data: &mut [u8; 8]) {
        data[0] = 128_u8.saturating_add(self.counter);
        data[1] = 130_u8.saturating_add(self.counter);
        let run_pw2 = (1000 + (self.counter as u16) * 60) % 12000;
        data[2..4].copy_from_slice(&run_pw2.to_be_bytes());
        let clc1 = 100i16 + (self.counter as i16) * 5;
        data[4..6].copy_from_slice(&clc1.to_be_bytes());
        let clc2 = 50i16 + (self.counter as i16) * 3;
        data[6..8].copy_from_slice(&clc2.to_be_bytes());
    }

    /// 0x306: Boost & Pressures
    fn generate_msg306(&self, data: &mut [u8; 8]) {
        data[0] = self.counter % 2;
        data[1] = 50_u8.saturating_add(self.counter);
        let boost_out = (self.counter as u16) * 30;
        data[2..4].copy_from_slice(&boost_out.to_be_bytes());
        let oil_p = (300 + (self.counter as u16) * 20) as u16;
        data[4..6].copy_from_slice(&oil_p.to_be_bytes());
        let fuel_p = (450 + (self.counter as u16) * 10) as u16;
        data[6..8].copy_from_slice(&fuel_p.to_be_bytes());
    }

    /// 0x307: Barometric Pressure & Boost Control
    fn generate_msg307(&self, data: &mut [u8; 8]) {
        let baro = (1013 + (self.counter as u16) * 5) % 1100;
        data[0..2].copy_from_slice(&baro.to_be_bytes());
        let p_boost = (self.counter as u16) * 100;
        data[2..4].copy_from_slice(&p_boost.to_be_bytes());
        let i_boost = (self.counter as u16) * 50;
        data[4..6].copy_from_slice(&i_boost.to_be_bytes());
        let target_boost = self.base_boost + (self.counter as u16) * 20;
        data[6..8].copy_from_slice(&target_boost.to_be_bytes());
    }

    /// 0x308: Battery & Ignition
    fn generate_msg308(&self, data: &mut [u8; 8]) {
        let vbatt = (12000 + (self.counter as u16) * 200) % 18000;
        data[0..2].copy_from_slice(&vbatt.to_be_bytes());
        let djvbatt = (self.counter as u16) * 10;
        data[2..4].copy_from_slice(&djvbatt.to_be_bytes());
        data[4] = (self.counter).wrapping_mul(5);
        data[5] = self.counter / 2;
        let dwell = 3000 + (self.counter as u16) * 50;
        data[6..8].copy_from_slice(&dwell.to_be_bytes());
    }

    /// 0x309: Raw Throttle & Pedal
    fn generate_msg309(&self, data: &mut [u8; 8]) {
        let tps1i = (self.counter as u16) * 50;
        data[0..2].copy_from_slice(&tps1i.to_be_bytes());
        let pps1i = (self.counter as u16) * 45;
        data[2..4].copy_from_slice(&pps1i.to_be_bytes());
        let pps2i = (self.counter as u16) * 42;
        data[4..6].copy_from_slice(&pps2i.to_be_bytes());
        let tps_drv = (self.counter as u16) * 60;
        data[6..8].copy_from_slice(&tps_drv.to_be_bytes());
    }

    /// 0x30A: Scaled Throttle & Pedal
    fn generate_msg30a(&self, data: &mut [u8; 8]) {
        let tps2i = (self.counter as u16) * 55;
        data[0..2].copy_from_slice(&tps2i.to_be_bytes());
        data[2] = self.counter % 3;
        data[3] = 50_u8.saturating_add(self.counter);
        data[4] = 48_u8.saturating_add(self.counter);
        data[5] = 45_u8.saturating_add(self.counter);
        data[6] = 55_u8.saturating_add(self.counter);
        data[7] = 52_u8.saturating_add(self.counter);
    }

    /// 0x30B: Temperatures
    fn generate_msg30b(&self, data: &mut [u8; 8]) {
        data[0] = self.base_temp.saturating_add(self.counter);
        data[1] = (self.base_temp + 5).saturating_add(self.counter);
        data[2] = 100_u8.saturating_add(self.counter % 155);
        data[3] = 20_u8.saturating_add(self.counter);
        let th2o_i = (self.counter as u16) * 40;
        data[4..6].copy_from_slice(&th2o_i.to_be_bytes());
        let toil_i = (self.counter as u16) * 38;
        data[6..8].copy_from_slice(&toil_i.to_be_bytes());
    }

    /// 0x30C: Run Timer & Corrections
    fn generate_msg30c(&self, data: &mut [u8; 8]) {
        let erun = (self.counter as u16) * 100;
        data[0..2].copy_from_slice(&erun.to_be_bytes());
        let tair_i = (self.counter as u16) * 35;
        data[2..4].copy_from_slice(&tair_i.to_be_bytes());
        let lambda1_i = (self.counter as u16) * 42;
        data[4..6].copy_from_slice(&lambda1_i.to_be_bytes());
        data[6] = 120_u8.saturating_add(self.counter % 135);
        data[7] = 100_u8.saturating_add(self.counter % 100);
    }

    /// 0x30D: Crank Counter & Learn Values
    fn generate_msg30d(&self, data: &mut [u8; 8]) {
        let crk_cnt = (self.counter as u16) * 10;
        data[0..2].copy_from_slice(&crk_cnt.to_be_bytes());
        data[2] = 150_u8.saturating_add(self.counter % 105);
        data[3] = 140_u8.saturating_add(self.counter % 115);
        let osat = -20i16 + (self.counter as i16) * 3;
        data[4..6].copy_from_slice(&osat.to_be_bytes());
        let rpm_idle = (750 + (self.counter as u16) * 25) % 2000;
        data[6..8].copy_from_slice(&rpm_idle.to_be_bytes());
    }

    /// 0x30E: Wheel Speeds
    fn generate_msg30e(&self, data: &mut [u8; 8]) {
        let lr = (self.counter as u16) * 5;
        data[0..2].copy_from_slice(&lr.to_be_bytes());
        let rr = (self.counter as u16) * 4;
        data[2..4].copy_from_slice(&rr.to_be_bytes());
        let lf = (self.counter as u16) * 6;
        data[4..6].copy_from_slice(&lf.to_be_bytes());
        let rf = (self.counter as u16) * 5;
        data[6..8].copy_from_slice(&rf.to_be_bytes());
    }

    /// 0x30F: Fuel Learn & Level
    fn generate_msg30f(&self, data: &mut [u8; 8]) {
        let kfuel = 100i16 + (self.counter as i16) * 2;
        data[0..2].copy_from_slice(&kfuel.to_be_bytes());
        data[2] = 75_u8.saturating_add(self.counter % 80);
        data[3] = 150_u8.saturating_add(self.counter);
        let gear = 1000 + (self.counter as u16) * 50;
        data[4..6].copy_from_slice(&gear.to_be_bytes());
        data[6] = (self.counter).wrapping_mul(3);
        data[7] = (self.counter).wrapping_mul(5);
    }

    /// 0x310: Knock Retard (all 8 cylinders)
    fn generate_msg310(&self, data: &mut [u8; 8]) {
        for i in 0..8 {
            data[i] = (self.counter).wrapping_mul((i as u8) + 1);
        }
    }
}

impl Default for ScsTestGenerator {
    fn default() -> Self {
        Self::new()
    }
}
