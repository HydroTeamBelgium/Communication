//! ECU parser abstraction
//!
//! Defines the `EcuParser` trait and adapters for protocol parsers.

use crate::protocol::messages::Message;
use crate::protocol::messages::CanFrameData;

/// Parser result type alias. On success returns a `Message` ready for channel dispatch.
pub type ParseResult = Result<Message, &'static str>;

/// Trait implemented by ECU protocol parsers (SCS, MaxxECU, ...)
pub trait EcuParser {
    /// Fast check whether this parser is responsible for the given CAN ID
    fn matches_id(&self, id: u32) -> bool;

    /// Parse the CAN frame (id + data slice) into a `Message`.
    /// Return `Err(&'static str)` for unrecoverable parse errors.
    fn parse(&self, id: u32, data: &[u8]) -> ParseResult;
}

// Small helper to wrap raw frames when parser does not recognise the payload
pub fn raw_frame_to_message(id: u32, data: &[u8]) -> Message {
    let mut d = [0u8; 8];
    let copy_len = core::cmp::min(data.len(), 8);
    d[0..copy_len].copy_from_slice(&data[0..copy_len]);
    Message::CanFrame(CanFrameData::new(id, d, copy_len as u8))
}
