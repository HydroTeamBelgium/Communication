//! Simple temperature and humidity CAN test data.

use core::fmt::Write;
use defmt::Format;

use crate::protocol::ecu::{raw_frame_to_message, EcuParser, ParseResult};
use crate::protocol::messages::{EcuJsonData, Message};

pub const TEST_DATA_CAN_ID: u32 = 0x100;

#[derive(Copy, Clone, Debug, Format)]
pub struct TestDataGenerator {
    counter: u8,
}

impl TestDataGenerator {
    pub const fn new() -> Self {
        Self { counter: 0 }
    }

    pub const fn counter(&self) -> u8 {
        self.counter
    }

    pub fn next_cycle(&mut self) {
        self.counter = self.counter.wrapping_add(1);
    }

    pub fn generate_frame(&self) -> (u32, [u8; 8]) {
        let mut data = [0; 8];
        data[0] = 20 + self.counter % 16;
        data[1] = 40 + self.counter % 51;
        (TEST_DATA_CAN_ID, data)
    }
}

impl Default for TestDataGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct TestDataParser;

pub static TEST_DATA_PARSER: TestDataParser = TestDataParser;

impl EcuParser for TestDataParser {
    fn matches_id(&self, id: u32) -> bool {
        id == TEST_DATA_CAN_ID
    }

    fn parse(&self, id: u32, data: &[u8]) -> ParseResult {
        if id != TEST_DATA_CAN_ID || data.len() < 2 {
            return Ok(raw_frame_to_message(id, data));
        }

        let mut json = heapless::String::<256>::new();
        write!(
            &mut json,
            "{{\"id\":{},\"temperature_c\":{},\"humidity_percent\":{}}}",
            id, data[0], data[1]
        )
        .map_err(|_| "Test data JSON formatting failed")?;

        Ok(Message::EcuJson(EcuJsonData::new(json)))
    }
}
