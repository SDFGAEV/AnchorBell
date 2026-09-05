use crate::{
    execution::{AdapterError, MarketDataAdapter, VenueId},
    market::{
        binance::{parse_market_message, BinanceMarketEvent},
        MarketEventKind, StandardMarketEvent,
    },
    runtime::{DataQuality, EventEnvelope, EventSource},
};

pub struct BinanceMarketDataAdapter {
    venue: VenueId,
    price_scale: u32,
    quantity_scale: u32,
}

impl BinanceMarketDataAdapter {
    pub fn new(price_scale: u32, quantity_scale: u32) -> Result<Self, AdapterError> {
        if price_scale > 18 || quantity_scale > 18 {
            return Err(AdapterError::EmptyIdentity);
        }
        Ok(Self {
            venue: VenueId("binance".into()),
            price_scale,
            quantity_scale,
        })
    }

    pub fn normalize_raw(
        &self,
        run_id: impl Into<String>,
        sequence: u64,
        received_at_ms: u64,
        payload: &[u8],
    ) -> Result<EventEnvelope<StandardMarketEvent>, AdapterError> {
        let event = parse_market_message(payload, self.price_scale, self.quantity_scale)
            .map_err(|_| AdapterError::InvalidEvent)?;
        let (symbol, observed_at_ms, kind, bid, ask, index, mark, quantity) = match event {
            BinanceMarketEvent::BookTicker(value) => (
                value.symbol,
                value.event_time_ms,
                MarketEventKind::Quote,
                Some(value.bid_price.0),
                Some(value.ask_price.0),
                None,
                None,
                None,
            ),
            BinanceMarketEvent::MarkPrice(value) => (
                value.symbol,
                value.event_time_ms,
                MarketEventKind::Mark,
                None,
                None,
                Some(value.index_price.0),
                Some(value.mark_price.0),
                None,
            ),
            BinanceMarketEvent::AggTrade(value) => (
                value.symbol,
                value.event_time_ms,
                MarketEventKind::Trade,
                None,
                None,
                None,
                Some(value.price.0),
                Some(value.quantity.0),
            ),
            BinanceMarketEvent::DepthUpdate(value) => (
                value.symbol,
                value.event_time_ms,
                MarketEventKind::Depth,
                None,
                None,
                None,
                None,
                None,
            ),
        };
        Ok(EventEnvelope {
            event_id: format!("binance-{symbol}-{sequence}").into(),
            run_id: run_id.into().into(),
            causality_id: format!("binance-cause-{symbol}-{sequence}").into(),
            source: EventSource::BinancePublic,
            observed_at_ms,
            received_at_ms,
            sequence,
            state_version: sequence,
            quality: DataQuality::Trusted,
            payload: StandardMarketEvent {
                kind,
                symbol,
                bid,
                ask,
                index_price: index,
                mark_price: mark,
                quantity,
            },
        })
    }
}

impl MarketDataAdapter for BinanceMarketDataAdapter {
    fn venue(&self) -> &VenueId {
        &self.venue
    }

    fn normalize(
        &self,
        event: EventEnvelope<StandardMarketEvent>,
    ) -> Result<EventEnvelope<StandardMarketEvent>, AdapterError> {
        event.validate().map_err(|_| AdapterError::InvalidEvent)?;
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_normalizes_real_binance_book_ticker_wire() {
        let adapter = BinanceMarketDataAdapter::new(8, 8).unwrap();
        let raw = br#"{"e":"bookTicker","E":100,"T":101,"u":7,"s":"BTCUSDT","b":"100.00000000","B":"1.0","a":"101.00000000","A":"2.0"}"#;
        let event = adapter.normalize_raw("run", 7, 102, raw).unwrap();
        assert_eq!(event.payload.symbol, "BTCUSDT");
        assert_eq!(event.payload.bid, Some(10_000_000_000));
        assert_eq!(event.received_at_ms, 102);
    }
}
