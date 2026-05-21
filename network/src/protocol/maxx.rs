//! MaxxECU default CAN protocol
//!
//! Implements the documented 11-bit CAN output map from the MaxxECU default CAN output PDF.
//! The parser converts known frames into JSON messages for UDP logging and falls back to raw
//! frame forwarding for anything else so the receiver remains robust under wrong initialization.

use core::fmt::Write;

use heapless::String;

use crate::protocol::ecu::{raw_frame_to_message, EcuParser, ParseResult};
use crate::protocol::messages::{EcuJsonData, Message};

pub const MAXX_MESSAGE_IDS: &[u32] = &[
    0x520, 0x521, 0x522, 0x523, 0x524, 0x526, 0x527, 0x528,
    0x530, 0x531, 0x532, 0x533, 0x534, 0x535, 0x536, 0x537, 0x538, 0x539,
    0x540, 0x541, 0x542,
];

fn le_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn le_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

fn scale_i16(raw: i16, scale: f32) -> f32 {
    (raw as f32) * scale
}

fn json_message(body: String<256>) -> Message {
    Message::EcuJson(EcuJsonData::new(body))
}

fn build_json<F>(builder: F) -> ParseResult
where
    F: FnOnce(&mut String<256>) -> core::fmt::Result,
{
    let mut json = String::<256>::new();
    builder(&mut json).map_err(|_| "MaxxECU JSON formatting failed")?;
    Ok(json_message(json))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MaxxParser;

impl MaxxParser {
    pub const fn new() -> Self {
        Self
    }
}

pub static MAXX_PARSER: MaxxParser = MaxxParser::new();

impl EcuParser for MaxxParser {
    fn matches_id(&self, id: u32) -> bool {
        (0x520..=0x542).contains(&id)
    }

    fn parse(&self, id: u32, data: &[u8]) -> ParseResult {
        if data.len() < 8 {
            return Ok(raw_frame_to_message(id, data));
        }

        match id {
            0x520 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x520,\"rpm\":{},\"throttle_position_pedal\":{:.1},\"map_kpa\":{:.1},\"lambda\":{:.3}}}",
                    le_i16(data, 0),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.001)
                )
            }),
            0x521 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x521,\"lambda_a\":{:.3},\"lambda_b\":{:.3},\"ignition_angle_btdc\":{:.1},\"ignition_cut\":{:.0}}}",
                    scale_i16(le_i16(data, 0), 0.001),
                    scale_i16(le_i16(data, 2), 0.001),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 1.0)
                )
            }),
            0x522 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x522,\"fuel_pulsewidth_primary_ms\":{:.2},\"fuel_duty_primary\":{:.1},\"fuel_cut\":{:.0},\"vehicle_speed_kmh\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.01),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 1.0),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x523 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x523,\"undriven_wheels_avg_spd_kmh\":{:.1},\"driven_wheels_avg_spd_kmh\":{:.1},\"wheel_slip\":{:.1},\"target_slip\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x524 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x524,\"traction_ctrl_power_limit\":{:.1},\"lambda_corr_a\":{:.1},\"lambda_corr_b\":{:.1},\"firmware_version\":{:.3}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.001)
                )
            }),
            0x526 => build_json(|json| {
                let flags0 = data[0];
                let flags1 = data[1];
                write!(
                    json,
                    "{{\"id\":0x526,\"throttle_blip_active\":{},\"ac_idle_up_active\":{},\"knock_detected\":{},\"brake_pedal_active\":{},\"clutch_pedal_active\":{},\"speed_limit_active\":{},\"gp_limiter_active\":{},\"user_cut_active\":{},\"ecu_is_logging\":{},\"nitrous_active\":{},\"spare_1_7\":{},\"spare_2\":{},\"rev_limit_rpm\":{},\"spare_3\":{}}}",
                    (flags0 & (1 << 5)) != 0,
                    (flags0 & (1 << 6)) != 0,
                    (flags0 & (1 << 7)) != 0,
                    (flags1 & (1 << 0)) != 0,
                    (flags1 & (1 << 1)) != 0,
                    (flags1 & (1 << 2)) != 0,
                    (flags1 & (1 << 3)) != 0,
                    (flags1 & (1 << 4)) != 0,
                    (flags1 & (1 << 5)) != 0,
                    (flags1 & (1 << 6)) != 0,
                    (flags1 & (1 << 7)) != 0,
                    le_i16(data, 2),
                    le_i16(data, 4),
                    le_i16(data, 6)
                )
            }),
            0x527 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x527,\"acceleration_forward_g\":{:.2},\"acceleration_right_g\":{:.2},\"acceleration_up_g\":{:.2},\"lambda_target\":{:.3}}}",
                    scale_i16(le_i16(data, 0), 0.01),
                    scale_i16(le_i16(data, 2), 0.01),
                    scale_i16(le_i16(data, 4), 0.01),
                    scale_i16(le_i16(data, 6), 0.001)
                )
            }),
            0x528 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x528,\"knocklevel_all_peak\":{},\"knock_correction_deg\":{:.1},\"knock_count\":{},\"last_knock_cylinder\":{}}}",
                    le_i16(data, 0),
                    scale_i16(le_i16(data, 2), 0.1),
                    le_i16(data, 4),
                    le_i16(data, 6)
                )
            }),
            0x530 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x530,\"battery_voltage_v\":{:.2},\"baro_pressure_kpa\":{:.1},\"intake_air_temp_c\":{:.1},\"coolant_temp_c\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.01),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x531 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x531,\"total_fuel_trim_pct\":{:.1},\"ethanol_concentration_pct\":{:.1},\"total_ignition_comp_deg\":{:.1},\"egt1_c\":{}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    le_i16(data, 6)
                )
            }),
            0x532 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x532,\"egt2_c\":{},\"egt3_c\":{},\"egt4_c\":{},\"egt5_c\":{}}}",
                    le_i16(data, 0),
                    le_i16(data, 2),
                    le_i16(data, 4),
                    le_i16(data, 6)
                )
            }),
            0x533 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x533,\"egt6_c\":{},\"egt7_c\":{},\"egt8_c\":{},\"egt_highest_c\":{}}}",
                    le_i16(data, 0),
                    le_i16(data, 2),
                    le_i16(data, 4),
                    le_i16(data, 6)
                )
            }),
            0x534 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x534,\"egt_difference_c\":{},\"cpu_temp_c\":{},\"error_code_count\":{},\"lost_sync_count\":{}}}",
                    le_i16(data, 0),
                    le_i16(data, 2),
                    le_i16(data, 4),
                    le_i16(data, 6)
                )
            }),
            0x535 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x535,\"user_analog_input_1\":{:.1},\"user_analog_input_2\":{:.1},\"user_analog_input_3\":{:.1},\"user_analog_input_4\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x536 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x536,\"gear\":{},\"boost_solenoid_duty_pct\":{:.1},\"oil_pressure_kpa\":{:.1},\"oil_temp_c\":{:.1}}}",
                    le_i16(data, 0),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x537 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x537,\"fuel_pressure_1_kpa\":{:.1},\"wastegate_pressure_kpa\":{:.1},\"coolant_pressure_kpa\":{:.1},\"boost_target_kpa\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x538 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x538,\"user_channel_1\":{:.1},\"user_channel_2\":{:.1},\"user_channel_3\":{:.1},\"user_channel_4\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x539 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x539,\"user_channel_5\":{:.1},\"user_channel_6\":{:.1},\"user_channel_7\":{:.1},\"user_channel_8\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x540 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x540,\"active_boost_table\":{},\"active_tune_selector\":{},\"virtual_fuel_tank_l\":{:.1},\"transmission_temp_c\":{:.1},\"differential_temp_c\":{:.1}}}",
                    data[0],
                    data[1],
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x541 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x541,\"vvt_intake_cam1_deg\":{:.1},\"vvt_exhaust_cam1_deg\":{:.1},\"vvt_intake_cam2_deg\":{:.1},\"vvt_exhaust_cam2_deg\":{:.1}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    scale_i16(le_i16(data, 4), 0.1),
                    scale_i16(le_i16(data, 6), 0.1)
                )
            }),
            0x542 => build_json(|json| {
                write!(
                    json,
                    "{{\"id\":0x542,\"vvt_intake_cam_target_deg\":{:.1},\"vvt_exhaust_cam_target_deg\":{:.1},\"ecu_errors_code\":{},\"spare\":{}}}",
                    scale_i16(le_i16(data, 0), 0.1),
                    scale_i16(le_i16(data, 2), 0.1),
                    le_u16(data, 4),
                    le_i16(data, 6)
                )
            }),
            _ => Ok(raw_frame_to_message(id, data)),
        }
    }
}

/// Generator for MaxxECU default CAN frames.
/// Produces the documented 11-bit frame layout with correct little-endian payloads.
#[derive(Clone, Copy, Debug)]
pub struct MaxxTestGenerator {
    counter: u8,
}

impl MaxxTestGenerator {
    pub const fn new() -> Self {
        Self { counter: 0 }
    }

    pub const fn with_counter(counter: u8) -> Self {
        Self { counter }
    }

    pub fn counter(&self) -> u8 {
        self.counter
    }

    pub fn next_cycle(&mut self) {
        self.counter = self.counter.wrapping_add(1);
    }

    pub fn generate_frame(&self, msg_id: u32) -> (u32, [u8; 8]) {
        let mut data = [0u8; 8];
        match msg_id {
            0x520 => self.write_520(&mut data),
            0x521 => self.write_521(&mut data),
            0x522 => self.write_522(&mut data),
            0x523 => self.write_523(&mut data),
            0x524 => self.write_524(&mut data),
            0x526 => self.write_526(&mut data),
            0x527 => self.write_527(&mut data),
            0x528 => self.write_528(&mut data),
            0x530 => self.write_530(&mut data),
            0x531 => self.write_531(&mut data),
            0x532 => self.write_532(&mut data),
            0x533 => self.write_533(&mut data),
            0x534 => self.write_534(&mut data),
            0x535 => self.write_535(&mut data),
            0x536 => self.write_536(&mut data),
            0x537 => self.write_537(&mut data),
            0x538 => self.write_538(&mut data),
            0x539 => self.write_539(&mut data),
            0x540 => self.write_540(&mut data),
            0x541 => self.write_541(&mut data),
            0x542 => self.write_542(&mut data),
            _ => {}
        }
        (msg_id, data)
    }

    fn write_u16(data: &mut [u8; 8], offset: usize, value: u16) {
        data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_i16(data: &mut [u8; 8], offset: usize, value: i16) {
        data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_520(&self, data: &mut [u8; 8]) {
        let rpm = 1500i16 + (self.counter as i16) * 80;
        let tps = 220i16 + (self.counter as i16) * 5;
        let map = 1000i16 + (self.counter as i16) * 12;
        let lambda = 1000i16 + (self.counter as i16) * 3;
        Self::write_i16(data, 0, rpm);
        Self::write_i16(data, 2, tps);
        Self::write_i16(data, 4, map);
        Self::write_i16(data, 6, lambda);
    }

    fn write_521(&self, data: &mut [u8; 8]) {
        let lambda_a = 1000i16 + (self.counter as i16) * 2;
        let lambda_b = 1005i16 + (self.counter as i16) * 2;
        let ignition_angle = 150i16 - (self.counter as i16) * 2;
        let ignition_cut = (self.counter % 10) as i16;
        Self::write_i16(data, 0, lambda_a);
        Self::write_i16(data, 2, lambda_b);
        Self::write_i16(data, 4, ignition_angle);
        Self::write_i16(data, 6, ignition_cut);
    }

    fn write_522(&self, data: &mut [u8; 8]) {
        let fuel_pw = 250i16 + (self.counter as i16) * 4;
        let duty = 300i16 + (self.counter as i16) * 6;
        let fuel_cut = (self.counter % 5) as i16;
        let speed = 300i16 + (self.counter as i16) * 8;
        Self::write_i16(data, 0, fuel_pw);
        Self::write_i16(data, 2, duty);
        Self::write_i16(data, 4, fuel_cut);
        Self::write_i16(data, 6, speed);
    }

    fn write_523(&self, data: &mut [u8; 8]) {
        let undriven = 200i16 + (self.counter as i16) * 5;
        let driven = 195i16 + (self.counter as i16) * 5;
        let slip = 10i16 + (self.counter as i16);
        let target = 12i16 + (self.counter as i16);
        Self::write_i16(data, 0, undriven);
        Self::write_i16(data, 2, driven);
        Self::write_i16(data, 4, slip);
        Self::write_i16(data, 6, target);
    }

    fn write_524(&self, data: &mut [u8; 8]) {
        let power_limit = 1000i16 - (self.counter as i16) * 3;
        let lambda_corr_a = 50i16 + (self.counter as i16);
        let lambda_corr_b = 45i16 + (self.counter as i16);
        let firmware = 1135i16;
        Self::write_i16(data, 0, power_limit);
        Self::write_i16(data, 2, lambda_corr_a);
        Self::write_i16(data, 4, lambda_corr_b);
        Self::write_i16(data, 6, firmware);
    }

    fn write_526(&self, data: &mut [u8; 8]) {
        data[0] = (1 << 5) | (self.counter & 0x03);
        data[1] = (1 << 0) | ((self.counter & 0x01) << 1);
        Self::write_i16(data, 2, 0);
        Self::write_i16(data, 4, 7000i16 - (self.counter as i16) * 50);
        Self::write_i16(data, 6, 0);
    }

    fn write_527(&self, data: &mut [u8; 8]) {
        let forward = 10i16 + (self.counter as i16);
        let right = 2i16 + (self.counter as i16);
        let up = 0i16 + (self.counter as i16);
        let lambda_target = 1000i16 + (self.counter as i16) * 2;
        Self::write_i16(data, 0, forward);
        Self::write_i16(data, 2, right);
        Self::write_i16(data, 4, up);
        Self::write_i16(data, 6, lambda_target);
    }

    fn write_528(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 2 + (self.counter as i16));
        Self::write_i16(data, 2, 5 + (self.counter as i16));
        Self::write_i16(data, 4, (self.counter % 8) as i16);
        Self::write_i16(data, 6, 1 + (self.counter % 8) as i16);
    }

    fn write_530(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 1200 + (self.counter as i16) * 5);
        Self::write_i16(data, 2, 1013 + (self.counter as i16));
        Self::write_i16(data, 4, 200 + (self.counter as i16));
        Self::write_i16(data, 6, 90 + (self.counter as i16));
    }

    fn write_531(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 100 + (self.counter as i16));
        Self::write_i16(data, 2, 850 + (self.counter as i16));
        Self::write_i16(data, 4, 10 + (self.counter as i16));
        Self::write_i16(data, 6, 700 + (self.counter as i16));
    }

    fn write_532(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 705 + (self.counter as i16));
        Self::write_i16(data, 2, 710 + (self.counter as i16));
        Self::write_i16(data, 4, 715 + (self.counter as i16));
        Self::write_i16(data, 6, 720 + (self.counter as i16));
    }

    fn write_533(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 725 + (self.counter as i16));
        Self::write_i16(data, 2, 730 + (self.counter as i16));
        Self::write_i16(data, 4, 735 + (self.counter as i16));
        Self::write_i16(data, 6, 740 + (self.counter as i16));
    }

    fn write_534(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 5 + (self.counter as i16));
        Self::write_i16(data, 2, 80 + (self.counter as i16));
        Self::write_i16(data, 4, (self.counter % 3) as i16);
        Self::write_i16(data, 6, (self.counter % 2) as i16);
    }

    fn write_535(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 100 + (self.counter as i16));
        Self::write_i16(data, 2, 200 + (self.counter as i16));
        Self::write_i16(data, 4, 300 + (self.counter as i16));
        Self::write_i16(data, 6, 400 + (self.counter as i16));
    }

    fn write_536(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, (self.counter % 6) as i16);
        Self::write_i16(data, 2, 35 + (self.counter as i16));
        Self::write_i16(data, 4, 400 + (self.counter as i16) * 2);
        Self::write_i16(data, 6, 85 + (self.counter as i16));
    }

    fn write_537(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 300 + (self.counter as i16));
        Self::write_i16(data, 2, 250 + (self.counter as i16));
        Self::write_i16(data, 4, 120 + (self.counter as i16));
        Self::write_i16(data, 6, 200 + (self.counter as i16));
    }

    fn write_538(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 10 + (self.counter as i16));
        Self::write_i16(data, 2, 20 + (self.counter as i16));
        Self::write_i16(data, 4, 30 + (self.counter as i16));
        Self::write_i16(data, 6, 40 + (self.counter as i16));
    }

    fn write_539(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 50 + (self.counter as i16));
        Self::write_i16(data, 2, 60 + (self.counter as i16));
        Self::write_i16(data, 4, 70 + (self.counter as i16));
        Self::write_i16(data, 6, 80 + (self.counter as i16));
    }

    fn write_540(&self, data: &mut [u8; 8]) {
        data[0] = self.counter % 4;
        data[1] = (self.counter + 1) % 3;
        Self::write_i16(data, 2, 45i16 + (self.counter as i16));
        Self::write_i16(data, 4, 80i16 + (self.counter as i16));
        Self::write_i16(data, 6, 60i16 + (self.counter as i16));
    }

    fn write_541(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 100i16 + (self.counter as i16));
        Self::write_i16(data, 2, 90i16 + (self.counter as i16));
        Self::write_i16(data, 4, 95i16 + (self.counter as i16));
        Self::write_i16(data, 6, 85i16 + (self.counter as i16));
    }

    fn write_542(&self, data: &mut [u8; 8]) {
        Self::write_i16(data, 0, 110i16 + (self.counter as i16));
        Self::write_i16(data, 2, 105i16 + (self.counter as i16));
        Self::write_u16(data, 4, 0x0000);
        Self::write_i16(data, 6, 0);
    }
}

impl Default for MaxxTestGenerator {
    fn default() -> Self {
        Self::new()
    }
}
