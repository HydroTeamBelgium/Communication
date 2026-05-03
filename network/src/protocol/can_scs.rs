//! SCS Delta CAN Protocol - Professional Implementation
//!
//! Handles all CAN IDs (0x300-0x310) with proper parsing and JSON serialization.
//! Each CAN message is decoded into typed structs with scaling applied.

use defmt::Format;
use heapless::String;

// ============================================================================
//                         HELPER MACROS & TYPES
// ============================================================================

/// Linear scaling: raw_value * scale + offset
fn scale_u8(raw: u8, scale: f32, offset: f32) -> f32 {
    (raw as f32) * scale + offset
}

fn scale_u16(raw: u16, scale: f32, offset: f32) -> f32 {
    (raw as f32) * scale + offset
}

fn scale_i16(raw: i16, scale: f32, offset: f32) -> f32 {
    (raw as f32) * scale + offset
}

// ============================================================================
//                         MESSAGE STRUCTURES
// ============================================================================

/// CAN ID 0x300 - Engine Control
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg300 {
    pub rpm: u16,           // Byte 0-1, 1 rpm/bit
    pub tps: f32,           // Byte 2, 0-100%
    pub kfuel_map: f32,     // Byte 3, 0-400%
    pub map: i16,           // Byte 4-5, 1 mBar/bit
    pub idle_learn: f32,    // Byte 6-7, 0.00038696%/bit
}

impl Msg300 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            rpm: u16::from_be_bytes([data[0], data[1]]),
            tps: scale_u8(data[2], 100.0 / 255.0, 0.0),
            kfuel_map: scale_u8(data[3], 400.0 / 255.0, 0.0),
            map: i16::from_be_bytes([data[4], data[5]]),
            idle_learn: scale_u16(u16::from_be_bytes([data[6], data[7]]), 0.00038696, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x300,"rpm":{},"tps":{:.2},"kfuel_map":{:.2},"map":{},"idle_learn":{:.4}}}"#,
            self.rpm, self.tps, self.kfuel_map, self.map, self.idle_learn);
        s
    }
}

/// CAN ID 0x301 - Fuel & Lambda
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg301 {
    pub d_throt: f32,       // Byte 0-1, 0.1%/s/bit
    pub lambda2: f32,       // Byte 2, 0-2.0
    pub inj_h_perc: f32,    // Byte 3, 0-100%
    pub ae: u16,            // Byte 4-5, 1 usec/bit
    pub de: u16,            // Byte 6-7, 1 usec/bit
}

impl Msg301 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            d_throt: scale_i16(i16::from_be_bytes([data[0], data[1]]), 0.1, 0.0),
            lambda2: scale_u8(data[2], 2.0 / 255.0, 0.0),
            inj_h_perc: scale_u8(data[3], 100.0 / 255.0, 0.0),
            ae: u16::from_be_bytes([data[4], data[5]]),
            de: u16::from_be_bytes([data[6], data[7]]),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x301,"d_throt":{:.1},"lambda2":{:.2},"inj_h_perc":{:.2},"ae":{},"de":{}}}"#,
            self.d_throt, self.lambda2, self.inj_h_perc, self.ae, self.de);
        s
    }
}

/// CAN ID 0x302 - Speed & Idle Control
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg302 {
    pub kmh: f32,           // Byte 0-1, 0.1 kmh/bit
    pub dc_base_idle: f32,  // Byte 2-3, 0-100%
    pub idle_out: f32,      // Byte 4-5, 0-100%
    pub perc_slip: f32,     // Byte 6, 0-100%
    pub target_slip: f32,   // Byte 7, 0-100%
}

impl Msg302 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            kmh: scale_u16(u16::from_be_bytes([data[0], data[1]]), 0.1, 0.0),
            dc_base_idle: scale_u16(u16::from_be_bytes([data[2], data[3]]), 100.0 / 255.0, 0.0),
            idle_out: scale_u16(u16::from_be_bytes([data[4], data[5]]), 100.0 / 255.0, 0.0),
            perc_slip: scale_u8(data[6], 100.0 / 255.0, 0.0),
            target_slip: scale_u8(data[7], 100.0 / 255.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x302,"kmh":{:.1},"dc_base_idle":{:.2},"idle_out":{:.2},"perc_slip":{:.2},"target_slip":{:.2}}}"#,
            self.kmh, self.dc_base_idle, self.idle_out, self.perc_slip, self.target_slip);
        s
    }
}

/// CAN ID 0x303 - Cam Control
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg303 {
    pub ivct_angle: f32,        // Byte 0-1, 0.25 deg/bit
    pub evct_angle: f32,        // Byte 2-3, 0.25 deg/bit
    pub ivct_angle_target: f32, // Byte 4, 0.25 deg/bit
    pub evct_angle_target: f32, // Byte 5, 0.25 deg/bit
    pub dbw_tps1: f32,          // Byte 6-7, 0-100% (0-1023 range)
}

impl Msg303 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            ivct_angle: scale_i16(i16::from_be_bytes([data[0], data[1]]), 0.25, 0.0),
            evct_angle: scale_i16(i16::from_be_bytes([data[2], data[3]]), 0.25, 0.0),
            ivct_angle_target: scale_u8(data[4], 0.25, 0.0),
            evct_angle_target: scale_u8(data[5], 0.25, 0.0),
            dbw_tps1: scale_u16(u16::from_be_bytes([data[6], data[7]]), 100.0 / 1023.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x303,"ivct_angle":{:.2},"evct_angle":{:.2},"ivct_angle_target":{:.2},"evct_angle_target":{:.2},"dbw_tps1":{:.2}}}"#,
            self.ivct_angle, self.evct_angle, self.ivct_angle_target, self.evct_angle_target, self.dbw_tps1);
        s
    }
}

/// CAN ID 0x304 - Injection & Spark
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg304 {
    pub base_inj_pw: u16,   // Byte 0-1, 1 usec/bit
    pub run_pw1: u16,       // Byte 2-3, 1 usec/bit
    pub sa_base: f32,       // Byte 4-5, 0.25 deg/bit
    pub sa_out: f32,        // Byte 6-7, 0.25 deg/bit
}

impl Msg304 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            base_inj_pw: u16::from_be_bytes([data[0], data[1]]),
            run_pw1: u16::from_be_bytes([data[2], data[3]]),
            sa_base: scale_i16(i16::from_be_bytes([data[4], data[5]]), 0.25, 0.0),
            sa_out: scale_i16(i16::from_be_bytes([data[6], data[7]]), 0.25, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x304,"base_inj_pw":{},"run_pw1":{},"sa_base":{:.2},"sa_out":{:.2}}}"#,
            self.base_inj_pw, self.run_pw1, self.sa_base, self.sa_out);
        s
    }
}

/// CAN ID 0x305 - Lambda & Fuel Trim
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg305 {
    pub lambda1: f32,       // Byte 0, 0-2.0
    pub target_lambda: f32, // Byte 1, 0-2.55
    pub run_pw2: u16,       // Byte 2-3, 1 usec/bit
    pub clc1: f32,          // Byte 4-5, 0.05%/bit
    pub clc2: f32,          // Byte 6-7, 0.05%/bit
}

impl Msg305 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            lambda1: scale_u8(data[0], 2.0 / 255.0, 0.0),
            target_lambda: scale_u8(data[1], 2.55 / 255.0, 0.0),
            run_pw2: u16::from_be_bytes([data[2], data[3]]),
            clc1: scale_i16(i16::from_be_bytes([data[4], data[5]]), 0.05, 0.0),
            clc2: scale_i16(i16::from_be_bytes([data[6], data[7]]), 0.05, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x305,"lambda1":{:.2},"target_lambda":{:.2},"run_pw2":{},"clc1":{:.2},"clc2":{:.2}}}"#,
            self.lambda1, self.target_lambda, self.run_pw2, self.clc1, self.clc2);
        s
    }
}

/// CAN ID 0x306 - Boost & Pressures
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg306 {
    pub gear: u8,           // Byte 0, 0=N, 255=R
    pub base_boost_dc: f32, // Byte 1, 0-100%
    pub boost_out: f32,     // Byte 2-3, 0-100%
    pub oil_p: f32,         // Byte 4-5, 0.1 kPa/bit
    pub fuel_p: f32,        // Byte 6-7, 0.01 kPa/bit
}

impl Msg306 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            gear: data[0],
            base_boost_dc: scale_u8(data[1], 100.0 / 255.0, 0.0),
            boost_out: scale_u16(u16::from_be_bytes([data[2], data[3]]), 100.0 / 255.0, 0.0),
            oil_p: scale_u16(u16::from_be_bytes([data[4], data[5]]), 0.1, 0.0),
            fuel_p: scale_u16(u16::from_be_bytes([data[6], data[7]]), 0.01, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x306,"gear":{},"base_boost_dc":{:.2},"boost_out":{:.2},"oil_p":{:.1},"fuel_p":{:.2}}}"#,
            self.gear, self.base_boost_dc, self.boost_out, self.oil_p, self.fuel_p);
        s
    }
}

/// CAN ID 0x307 - Barometric Pressure & Boost Control
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg307 {
    pub baro: u16,          // Byte 0-1, 1 mBar/bit
    pub p_boost: f32,       // Byte 2-3, 0.39%/bit
    pub i_boost: f32,       // Byte 4-5, 0.39%/bit
    pub target_boost: u16,  // Byte 6-7, 1 mBar/bit
}

impl Msg307 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            baro: u16::from_be_bytes([data[0], data[1]]),
            p_boost: scale_u16(u16::from_be_bytes([data[2], data[3]]), 0.39, 0.0),
            i_boost: scale_u16(u16::from_be_bytes([data[4], data[5]]), 0.39, 0.0),
            target_boost: u16::from_be_bytes([data[6], data[7]]),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x307,"baro":{},"p_boost":{:.2},"i_boost":{:.2},"target_boost":{}}}"#,
            self.baro, self.p_boost, self.i_boost, self.target_boost);
        s
    }
}

/// CAN ID 0x308 - Battery & Ignition
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg308 {
    pub vbatt: f32,         // Byte 0-1, 0-18V (0-1023 range)
    pub djvbatt: u16,       // Byte 2-3, 1 usec/bit
    pub phase: f32,         // Byte 4, 2.8235 deg/bit
    pub cam_count: u8,      // Byte 5, 1 rev/bit
    pub dwell: u16,         // Byte 6-7, 1 us/bit
}

impl Msg308 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            vbatt: scale_u16(u16::from_be_bytes([data[0], data[1]]), 18.0 / 1023.0, 0.0),
            djvbatt: u16::from_be_bytes([data[2], data[3]]),
            phase: scale_u8(data[4], 2.8235, 0.0),
            cam_count: data[5],
            dwell: u16::from_be_bytes([data[6], data[7]]),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x308,"vbatt":{:.2},"djvbatt":{},"phase":{:.2},"cam_count":{},"dwell":{}}}"#,
            self.vbatt, self.djvbatt, self.phase, self.cam_count, self.dwell);
        s
    }
}

/// CAN ID 0x309 - Raw Throttle & Pedal
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg309 {
    pub tps1i: f32,         // Byte 0-1, 0-5V (0-1023 range)
    pub pps1i: f32,         // Byte 2-3, 0-5V
    pub pps2i: f32,         // Byte 4-5, 0-5V
    pub tps_drv_req: f32,   // Byte 6-7, 0-100%
}

impl Msg309 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            tps1i: scale_u16(u16::from_be_bytes([data[0], data[1]]), 5.0 / 1023.0, 0.0),
            pps1i: scale_u16(u16::from_be_bytes([data[2], data[3]]), 5.0 / 1023.0, 0.0),
            pps2i: scale_u16(u16::from_be_bytes([data[4], data[5]]), 5.0 / 1023.0, 0.0),
            tps_drv_req: scale_u16(u16::from_be_bytes([data[6], data[7]]), 100.0 / 1023.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x309,"tps1i":{:.2},"pps1i":{:.2},"pps2i":{:.2},"tps_drv_req":{:.2}}}"#,
            self.tps1i, self.pps1i, self.pps2i, self.tps_drv_req);
        s
    }
}

/// CAN ID 0x30A - Scaled Throttle & Pedal
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30A {
    pub tps2i: f32,         // Byte 0-1, 0-5V
    pub tps_pps_fault: u8,  // Byte 2
    pub pps: f32,           // Byte 3, 0-100%
    pub pps1: f32,          // Byte 4, 0-100%
    pub pps2: f32,          // Byte 5, 0-100%
    pub tps1: f32,          // Byte 6, 0-100%
    pub tps2: f32,          // Byte 7, 0-100%
}

impl Msg30A {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            tps2i: scale_u16(u16::from_be_bytes([data[0], data[1]]), 5.0 / 1023.0, 0.0),
            tps_pps_fault: data[2],
            pps: scale_u8(data[3], 100.0 / 255.0, 0.0),
            pps1: scale_u8(data[4], 100.0 / 255.0, 0.0),
            pps2: scale_u8(data[5], 100.0 / 255.0, 0.0),
            tps1: scale_u8(data[6], 100.0 / 255.0, 0.0),
            tps2: scale_u8(data[7], 100.0 / 255.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30A,"tps2i":{:.2},"tps_pps_fault":{},"pps":{:.2},"pps1":{:.2},"pps2":{:.2},"tps1":{:.2},"tps2":{:.2}}}"#,
            self.tps2i, self.tps_pps_fault, self.pps, self.pps1, self.pps2, self.tps1, self.tps2);
        s
    }
}

/// CAN ID 0x30B - Temperatures
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30B {
    pub th2o: f32,          // Byte 0, -10 to 150°C
    pub toil: f32,          // Byte 1, -10 to 150°C
    pub kfuel_crk: f32,     // Byte 2, 0-800%
    pub tair: f32,          // Byte 3, -10 to 150°C
    pub th2o_i: f32,        // Byte 4-5, 0-5V
    pub toil_i: f32,        // Byte 6-7, 0-5V
}

impl Msg30B {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            th2o: scale_u8(data[0], 160.0 / 255.0, -10.0),
            toil: scale_u8(data[1], 160.0 / 255.0, -10.0),
            kfuel_crk: scale_u8(data[2], 800.0 / 255.0, 0.0),
            tair: scale_u8(data[3], 160.0 / 255.0, -10.0),
            th2o_i: scale_u16(u16::from_be_bytes([data[4], data[5]]), 5.0 / 1023.0, 0.0),
            toil_i: scale_u16(u16::from_be_bytes([data[6], data[7]]), 5.0 / 1023.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30B,"th2o":{:.2},"toil":{:.2},"kfuel_crk":{:.2},"tair":{:.2},"th2o_i":{:.2},"toil_i":{:.2}}}"#,
            self.th2o, self.toil, self.kfuel_crk, self.tair, self.th2o_i, self.toil_i);
        s
    }
}

/// CAN ID 0x30C - Run Timer & Corrections
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30C {
    pub erun_timer: f32,    // Byte 0-1, 0.05s/bit
    pub tair_i: f32,        // Byte 2-3, 0-5V
    pub lambda1_i: f32,     // Byte 4-5, 0-5V
    pub kfuel_th2o: f32,    // Byte 6, 0-400%
    pub kfuel_tair: f32,    // Byte 7, 0-200%
}

impl Msg30C {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            erun_timer: scale_u16(u16::from_be_bytes([data[0], data[1]]), 0.05, 0.0),
            tair_i: scale_u16(u16::from_be_bytes([data[2], data[3]]), 5.0 / 1023.0, 0.0),
            lambda1_i: scale_u16(u16::from_be_bytes([data[4], data[5]]), 5.0 / 1023.0, 0.0),
            kfuel_th2o: scale_u8(data[6], 400.0 / 255.0, 0.0),
            kfuel_tair: scale_u8(data[7], 200.0 / 255.0, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30C,"erun_timer":{:.2},"tair_i":{:.2},"lambda1_i":{:.2},"kfuel_th2o":{:.2},"kfuel_tair":{:.2}}}"#,
            self.erun_timer, self.tair_i, self.lambda1_i, self.kfuel_th2o, self.kfuel_tair);
        s
    }
}

/// CAN ID 0x30D - Crank Counter & Learn Values
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30D {
    pub crk_cnt: u16,           // Byte 0-1, 1 rev/bit
    pub kfuel_baro: f32,        // Byte 2, 0-400%
    pub kfuel_p: f32,           // Byte 3, 0-400%
    pub osat_air: f32,          // Byte 4-5, 0.25 deg/bit
    pub rpm_target_idle: u16,   // Byte 6-7, 1 rpm/bit
}

impl Msg30D {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            crk_cnt: u16::from_be_bytes([data[0], data[1]]),
            kfuel_baro: scale_u8(data[2], 400.0 / 255.0, 0.0),
            kfuel_p: scale_u8(data[3], 400.0 / 255.0, 0.0),
            osat_air: scale_i16(i16::from_be_bytes([data[4], data[5]]), 0.25, 0.0),
            rpm_target_idle: u16::from_be_bytes([data[6], data[7]]),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30D,"crk_cnt":{},"kfuel_baro":{:.2},"kfuel_p":{:.2},"osat_air":{:.2},"rpm_target_idle":{}}}"#,
            self.crk_cnt, self.kfuel_baro, self.kfuel_p, self.osat_air, self.rpm_target_idle);
        s
    }
}

/// CAN ID 0x30E - Wheel Speeds
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30E {
    pub kmh16_lr: f32,      // Byte 0-1, 0.01 kmh/bit
    pub kmh16_rr: f32,      // Byte 2-3, 0.01 kmh/bit
    pub kmh16_lf: f32,      // Byte 4-5, 0.01 kmh/bit
    pub kmh16_rf: f32,      // Byte 6-7, 0.01 kmh/bit
}

impl Msg30E {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            kmh16_lr: scale_u16(u16::from_be_bytes([data[0], data[1]]), 0.01, 0.0),
            kmh16_rr: scale_u16(u16::from_be_bytes([data[2], data[3]]), 0.01, 0.0),
            kmh16_lf: scale_u16(u16::from_be_bytes([data[4], data[5]]), 0.01, 0.0),
            kmh16_rf: scale_u16(u16::from_be_bytes([data[6], data[7]]), 0.01, 0.0),
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30E,"kmh16_lr":{:.2},"kmh16_rr":{:.2},"kmh16_lf":{:.2},"kmh16_rf":{:.2}}}"#,
            self.kmh16_lr, self.kmh16_rr, self.kmh16_lf, self.kmh16_rf);
        s
    }
}

/// CAN ID 0x30F - Fuel Learn & Level
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg30F {
    pub kfuel_learn: f32,   // Byte 0-1, 0.39%/bit
    pub fuel_level: f32,    // Byte 2, 0-100%
    pub fuel_level_i: f32,  // Byte 3, 0-5V
    pub gear_ratio: u16,    // Byte 4-5, 1 rpm/kmh/bit
    pub aux_stat1: u8,      // Byte 6
    pub aux_stat2: u8,      // Byte 7
}

impl Msg30F {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        Some(Self {
            kfuel_learn: scale_i16(i16::from_be_bytes([data[0], data[1]]), 0.39, 0.0),
            fuel_level: scale_u8(data[2], 0.39, 0.0),
            fuel_level_i: scale_u8(data[3], 5.0 / 255.0, 0.0),
            gear_ratio: u16::from_be_bytes([data[4], data[5]]),
            aux_stat1: data[6],
            aux_stat2: data[7],
        })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x30F,"kfuel_learn":{:.2},"fuel_level":{:.2},"fuel_level_i":{:.2},"gear_ratio":{},"aux_stat1":{},"aux_stat2":{}}}"#,
            self.kfuel_learn, self.fuel_level, self.fuel_level_i, self.gear_ratio, self.aux_stat1, self.aux_stat2);
        s
    }
}

/// CAN ID 0x310 - Knock Retard (Cylinders 1-8)
#[derive(Copy, Clone, Debug, Format)]
pub struct Msg310 {
    pub sa_retard: [f32; 8],  // Bytes 0-7, 0.25 deg/bit each
}

impl Msg310 {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        let mut sa_retard = [0.0f32; 8];
        for i in 0..8 {
            sa_retard[i] = scale_u8(data[i], 0.25, 0.0);
        }
        Some(Self { sa_retard })
    }

    pub fn to_json_string(&self) -> String<256> {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(&mut s, r#"{{"id":0x310,"cyl":[{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}]}}"#,
            self.sa_retard[0], self.sa_retard[1], self.sa_retard[2], self.sa_retard[3],
            self.sa_retard[4], self.sa_retard[5], self.sa_retard[6], self.sa_retard[7]);
        s
    }
}

// ============================================================================
//                         ENUM MESSAGE TYPE
// ============================================================================

/// Union of all possible CAN messages for pattern matching
#[derive(Copy, Clone, Debug, Format)]
pub enum CanMessage {
    Msg300(Msg300),
    Msg301(Msg301),
    Msg302(Msg302),
    Msg303(Msg303),
    Msg304(Msg304),
    Msg305(Msg305),
    Msg306(Msg306),
    Msg307(Msg307),
    Msg308(Msg308),
    Msg309(Msg309),
    Msg30A(Msg30A),
    Msg30B(Msg30B),
    Msg30C(Msg30C),
    Msg30D(Msg30D),
    Msg30E(Msg30E),
    Msg30F(Msg30F),
    Msg310(Msg310),
}

impl CanMessage {
    /// Parse a CAN frame into a typed message
    pub fn from_frame(id: u32, data: &[u8]) -> Option<Self> {
        match id {
            0x300 => Msg300::from_bytes(data).map(CanMessage::Msg300),
            0x301 => Msg301::from_bytes(data).map(CanMessage::Msg301),
            0x302 => Msg302::from_bytes(data).map(CanMessage::Msg302),
            0x303 => Msg303::from_bytes(data).map(CanMessage::Msg303),
            0x304 => Msg304::from_bytes(data).map(CanMessage::Msg304),
            0x305 => Msg305::from_bytes(data).map(CanMessage::Msg305),
            0x306 => Msg306::from_bytes(data).map(CanMessage::Msg306),
            0x307 => Msg307::from_bytes(data).map(CanMessage::Msg307),
            0x308 => Msg308::from_bytes(data).map(CanMessage::Msg308),
            0x309 => Msg309::from_bytes(data).map(CanMessage::Msg309),
            0x30A => Msg30A::from_bytes(data).map(CanMessage::Msg30A),
            0x30B => Msg30B::from_bytes(data).map(CanMessage::Msg30B),
            0x30C => Msg30C::from_bytes(data).map(CanMessage::Msg30C),
            0x30D => Msg30D::from_bytes(data).map(CanMessage::Msg30D),
            0x30E => Msg30E::from_bytes(data).map(CanMessage::Msg30E),
            0x30F => Msg30F::from_bytes(data).map(CanMessage::Msg30F),
            0x310 => Msg310::from_bytes(data).map(CanMessage::Msg310),
            _ => None,
        }
    }

    /// Serialize to JSON bytes
    pub fn to_json_bytes(&self) -> heapless::Vec<u8, 512> {
        let json_str = match self {
            CanMessage::Msg300(msg) => msg.to_json_string(),
            CanMessage::Msg301(msg) => msg.to_json_string(),
            CanMessage::Msg302(msg) => msg.to_json_string(),
            CanMessage::Msg303(msg) => msg.to_json_string(),
            CanMessage::Msg304(msg) => msg.to_json_string(),
            CanMessage::Msg305(msg) => msg.to_json_string(),
            CanMessage::Msg306(msg) => msg.to_json_string(),
            CanMessage::Msg307(msg) => msg.to_json_string(),
            CanMessage::Msg308(msg) => msg.to_json_string(),
            CanMessage::Msg309(msg) => msg.to_json_string(),
            CanMessage::Msg30A(msg) => msg.to_json_string(),
            CanMessage::Msg30B(msg) => msg.to_json_string(),
            CanMessage::Msg30C(msg) => msg.to_json_string(),
            CanMessage::Msg30D(msg) => msg.to_json_string(),
            CanMessage::Msg30E(msg) => msg.to_json_string(),
            CanMessage::Msg30F(msg) => msg.to_json_string(),
            CanMessage::Msg310(msg) => msg.to_json_string(),
        };
        heapless::Vec::from_slice(json_str.as_bytes()).unwrap_or_default()
    }
}
