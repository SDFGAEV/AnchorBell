use crate::event::EngineEvent;

pub struct BinanceEventParser;

impl BinanceEventParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_depth_message(&self, _payload: &[u8]) -> Option<EngineEvent> {
        // High performance parser boundary.
        // JSON decoding will be implemented with zero-copy serde/msgspec equivalent.
        None
    }

    pub fn parse_mark_price_message(&self, _payload: &[u8]) -> Option<EngineEvent> {
        // indexPrice and markPrice stream boundary.
        None
    }
}
