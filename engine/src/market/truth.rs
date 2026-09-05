use crate::runtime::{DataQuality, EventEnvelope, EventSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketEventKind {
    Quote,
    Trade,
    Mark,
    Funding,
    Depth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandardMarketEvent {
    pub kind: MarketEventKind,
    pub symbol: String,
    pub bid: Option<i64>,
    pub ask: Option<i64>,
    pub index_price: Option<i64>,
    pub mark_price: Option<i64>,
    pub quantity: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTruthSnapshot {
    pub symbol: String,
    pub bid: i64,
    pub ask: i64,
    pub index_price: i64,
    pub mark_price: i64,
    pub watermark_ms: u64,
    pub sequence: u64,
    pub quality: DataQuality,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MarketTruthError {
    #[error("market symbol is empty")]
    EmptySymbol,
    #[error("market sequence regressed")]
    SequenceRegression,
    #[error("market event is stale")]
    Stale,
    #[error("market quote or price is invalid")]
    InvalidMarket,
}

#[derive(Debug, Default)]
pub struct MarketTruthState {
    snapshot: Option<MarketTruthSnapshot>,
}

impl MarketTruthState {
    pub fn apply(
        &mut self,
        event: &EventEnvelope<StandardMarketEvent>,
        now_ms: u64,
        max_age_ms: u64,
    ) -> Result<&MarketTruthSnapshot, MarketTruthError> {
        event
            .validate()
            .map_err(|_| MarketTruthError::InvalidMarket)?;
        let payload = &event.payload;
        if payload.symbol.trim().is_empty() {
            return Err(MarketTruthError::EmptySymbol);
        }
        if event.observed_at_ms > now_ms || now_ms.saturating_sub(event.observed_at_ms) > max_age_ms
        {
            return Err(MarketTruthError::Stale);
        }
        if let Some(previous) = &self.snapshot {
            if event.sequence < previous.sequence {
                return Err(MarketTruthError::SequenceRegression);
            }
        }
        let bid = payload.bid.ok_or(MarketTruthError::InvalidMarket)?;
        let ask = payload.ask.ok_or(MarketTruthError::InvalidMarket)?;
        let index_price = payload.index_price.ok_or(MarketTruthError::InvalidMarket)?;
        let mark_price = payload.mark_price.ok_or(MarketTruthError::InvalidMarket)?;
        if bid <= 0 || ask <= bid || index_price <= 0 || mark_price <= 0 {
            return Err(MarketTruthError::InvalidMarket);
        }
        self.snapshot = Some(MarketTruthSnapshot {
            symbol: payload.symbol.clone(),
            bid,
            ask,
            index_price,
            mark_price,
            watermark_ms: event.observed_at_ms,
            sequence: event.sequence,
            quality: event.quality.clone(),
        });
        Ok(self.snapshot.as_ref().expect("snapshot assigned"))
    }

    pub fn snapshot(&self) -> Option<&MarketTruthSnapshot> {
        self.snapshot.as_ref()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn quote_event(
    run_id: impl Into<String>,
    symbol: impl Into<String>,
    sequence: u64,
    observed_at_ms: u64,
    bid: i64,
    ask: i64,
    index_price: i64,
    mark_price: i64,
) -> EventEnvelope<StandardMarketEvent> {
    EventEnvelope {
        event_id: format!("quote-{sequence}").into(),
        run_id: run_id.into().into(),
        causality_id: format!("quote-cause-{sequence}").into(),
        source: EventSource::BinancePublic,
        observed_at_ms,
        received_at_ms: observed_at_ms,
        sequence,
        state_version: sequence,
        quality: DataQuality::Trusted,
        payload: StandardMarketEvent {
            kind: MarketEventKind::Quote,
            symbol: symbol.into(),
            bid: Some(bid),
            ask: Some(ask),
            index_price: Some(index_price),
            mark_price: Some(mark_price),
            quantity: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truth_state_rejects_regression_and_crossed_quotes() {
        let mut state = MarketTruthState::default();
        let first = quote_event("run", "BTCUSDT", 2, 100, 99, 101, 100, 100);
        state.apply(&first, 101, 5).unwrap();
        let regression = quote_event("run", "BTCUSDT", 1, 101, 99, 101, 100, 100);
        assert_eq!(
            state.apply(&regression, 101, 5),
            Err(MarketTruthError::SequenceRegression)
        );
        let crossed = quote_event("run", "BTCUSDT", 3, 102, 101, 100, 100, 100);
        assert_eq!(
            state.apply(&crossed, 102, 5),
            Err(MarketTruthError::InvalidMarket)
        );
    }
}
