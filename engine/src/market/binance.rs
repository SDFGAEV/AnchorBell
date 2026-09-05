use serde::Deserialize;

use crate::strategy::{PriceTicks, Quantity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookTicker {
    pub symbol: String,
    pub event_time_ms: u64,
    pub transaction_time_ms: u64,
    pub update_id: u64,
    pub bid_price: PriceTicks,
    pub bid_quantity: Quantity,
    pub ask_price: PriceTicks,
    pub ask_quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkPrice {
    pub symbol: String,
    pub event_time_ms: u64,
    pub mark_price: PriceTicks,
    pub index_price: PriceTicks,
    pub next_funding_time_ms: u64,
    /// Latest displayed funding rate, scaled by 1e8; absent on incomplete feeds.
    pub latest_funding_rate_e8: Option<i64>,
}

/// An aggregate public trade. `buyer_is_maker` is Binance's `m` field.
/// A passive BUY can only fill against a sell aggressor (`m=true`), while a
/// passive SELL can only fill against a buy aggressor (`m=false`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggTrade {
    pub symbol: String,
    pub event_time_ms: u64,
    pub trade_time_ms: u64,
    pub aggregate_trade_id: u64,
    pub price: PriceTicks,
    pub quantity: Quantity,
    pub buyer_is_maker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthLevel {
    pub price: PriceTicks,
    pub quantity: Quantity,
}

/// Binance diff-depth update. `U/u/pu` are preserved so consumers can enforce
/// the exchange's snapshot-plus-diff continuity contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthUpdate {
    pub symbol: String,
    pub event_time_ms: u64,
    pub transaction_time_ms: u64,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: Option<u64>,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinanceMarketEvent {
    BookTicker(BookTicker),
    MarkPrice(MarkPrice),
    AggTrade(AggTrade),
    DepthUpdate(DepthUpdate),
}

#[derive(Debug, Deserialize)]
struct BookTickerWire<'a> {
    #[serde(rename = "e", borrow)]
    event_type: &'a str,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "T")]
    transaction_time_ms: u64,
    #[serde(rename = "u")]
    update_id: u64,
    #[serde(rename = "s", borrow)]
    symbol: &'a str,
    #[serde(rename = "b", borrow)]
    bid_price: &'a str,
    #[serde(rename = "B", borrow)]
    bid_quantity: &'a str,
    #[serde(rename = "a", borrow)]
    ask_price: &'a str,
    #[serde(rename = "A", borrow)]
    ask_quantity: &'a str,
}

#[derive(Debug, Deserialize)]
struct MarkPriceWire<'a> {
    #[serde(rename = "e", borrow)]
    event_type: &'a str,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "s", borrow)]
    symbol: &'a str,
    #[serde(rename = "p", borrow)]
    mark_price: &'a str,
    #[serde(rename = "i", borrow)]
    index_price: &'a str,
    #[serde(rename = "T")]
    next_funding_time_ms: u64,
    #[serde(rename = "r", borrow)]
    latest_funding_rate: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct AggTradeWire<'a> {
    #[serde(rename = "e", borrow)]
    event_type: &'a str,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "s", borrow)]
    symbol: &'a str,
    #[serde(rename = "a")]
    aggregate_trade_id: u64,
    #[serde(rename = "p", borrow)]
    price: &'a str,
    #[serde(rename = "q", borrow)]
    quantity: &'a str,
    #[serde(rename = "T")]
    trade_time_ms: u64,
    #[serde(rename = "m")]
    buyer_is_maker: bool,
}

#[derive(Debug, Deserialize)]
struct DepthUpdateWire<'a> {
    #[serde(rename = "e", borrow)]
    event_type: &'a str,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "T")]
    transaction_time_ms: u64,
    #[serde(rename = "s", borrow)]
    symbol: &'a str,
    #[serde(rename = "U")]
    first_update_id: u64,
    #[serde(rename = "u")]
    final_update_id: u64,
    #[serde(rename = "pu")]
    previous_final_update_id: Option<u64>,
    #[serde(rename = "b", borrow)]
    bids: Vec<[&'a str; 2]>,
    #[serde(rename = "a", borrow)]
    asks: Vec<[&'a str; 2]>,
}

#[derive(Debug, Deserialize)]
struct UnknownWire<'a> {
    #[serde(rename = "e", borrow)]
    event_type: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged, bound(deserialize = "'de: 'a"))]
enum MarketMessage<'a> {
    CombinedBook { data: BookTickerWire<'a> },
    CombinedMark { data: MarkPriceWire<'a> },
    CombinedAgg { data: AggTradeWire<'a> },
    CombinedDepth { data: DepthUpdateWire<'a> },
    Book(BookTickerWire<'a>),
    Mark(MarkPriceWire<'a>),
    Agg(AggTradeWire<'a>),
    Depth(DepthUpdateWire<'a>),
    CombinedUnknown { data: UnknownWire<'a> },
    Unknown(UnknownWire<'a>),
    CombinedNull { data: () },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidJson,
    UnsupportedEvent,
    InvalidDecimal,
    DecimalOverflow,
    MissingCombinedData,
}

pub fn parse_market_message(
    payload: &[u8],
    price_scale: u32,
    quantity_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    let message: MarketMessage<'_> =
        serde_json::from_slice(payload).map_err(|_| ParseError::InvalidJson)?;
    match message {
        MarketMessage::CombinedBook { data } | MarketMessage::Book(data) => {
            parse_book_ticker(data, price_scale, quantity_scale)
        }
        MarketMessage::CombinedMark { data } | MarketMessage::Mark(data) => {
            parse_mark_price(data, price_scale)
        }
        MarketMessage::CombinedAgg { data } | MarketMessage::Agg(data) => {
            parse_agg_trade(data, price_scale, quantity_scale)
        }
        MarketMessage::CombinedDepth { data } | MarketMessage::Depth(data) => {
            parse_depth_update(data, price_scale, quantity_scale)
        }
        MarketMessage::CombinedUnknown { data } => {
            let _ = data.event_type;
            Err(ParseError::UnsupportedEvent)
        }
        MarketMessage::Unknown(data) => {
            let _ = data.event_type;
            Err(ParseError::UnsupportedEvent)
        }
        MarketMessage::CombinedNull { data: () } => Err(ParseError::MissingCombinedData),
    }
}

/// Parses a Binance decimal price into the engine's integer price ticks.
pub fn parse_price_ticks(value: &str, scale: u32) -> Result<PriceTicks, ParseError> {
    Ok(PriceTicks(parse_decimal(value, scale)?))
}

/// Parses a Binance decimal quantity into the engine's integer quantity units.
pub fn parse_quantity(value: &str, scale: u32) -> Result<Quantity, ParseError> {
    Ok(Quantity(parse_decimal(value, scale)?))
}

fn parse_book_ticker(
    wire: BookTickerWire<'_>,
    price_scale: u32,
    quantity_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    if wire.event_type != "bookTicker" || wire.symbol.is_empty() {
        return Err(ParseError::UnsupportedEvent);
    }
    Ok(BinanceMarketEvent::BookTicker(BookTicker {
        symbol: wire.symbol.to_owned(),
        event_time_ms: wire.event_time_ms,
        transaction_time_ms: wire.transaction_time_ms,
        update_id: wire.update_id,
        bid_price: PriceTicks(parse_decimal(wire.bid_price, price_scale)?),
        bid_quantity: Quantity(parse_decimal(wire.bid_quantity, quantity_scale)?),
        ask_price: PriceTicks(parse_decimal(wire.ask_price, price_scale)?),
        ask_quantity: Quantity(parse_decimal(wire.ask_quantity, quantity_scale)?),
    }))
}

fn parse_mark_price(
    wire: MarkPriceWire<'_>,
    price_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    if wire.event_type != "markPriceUpdate" || wire.symbol.is_empty() {
        return Err(ParseError::UnsupportedEvent);
    }
    Ok(BinanceMarketEvent::MarkPrice(MarkPrice {
        symbol: wire.symbol.to_owned(),
        event_time_ms: wire.event_time_ms,
        mark_price: PriceTicks(parse_decimal(wire.mark_price, price_scale)?),
        index_price: PriceTicks(parse_decimal(wire.index_price, price_scale)?),
        next_funding_time_ms: wire.next_funding_time_ms,
        latest_funding_rate_e8: wire
            .latest_funding_rate
            .map(|value| parse_signed_decimal(value, 8))
            .transpose()?,
    }))
}

fn parse_agg_trade(
    wire: AggTradeWire<'_>,
    price_scale: u32,
    quantity_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    if wire.event_type != "aggTrade" || wire.symbol.is_empty() {
        return Err(ParseError::UnsupportedEvent);
    }
    Ok(BinanceMarketEvent::AggTrade(AggTrade {
        symbol: wire.symbol.to_owned(),
        event_time_ms: wire.event_time_ms,
        trade_time_ms: wire.trade_time_ms,
        aggregate_trade_id: wire.aggregate_trade_id,
        price: PriceTicks(parse_decimal(wire.price, price_scale)?),
        quantity: Quantity(parse_decimal(wire.quantity, quantity_scale)?),
        buyer_is_maker: wire.buyer_is_maker,
    }))
}

fn parse_depth_update(
    wire: DepthUpdateWire<'_>,
    price_scale: u32,
    quantity_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    if wire.event_type != "depthUpdate"
        || wire.symbol.is_empty()
        || wire.first_update_id > wire.final_update_id
    {
        return Err(ParseError::UnsupportedEvent);
    }
    let parse_levels = |levels: Vec<[&str; 2]>| {
        levels
            .into_iter()
            .map(|[price, quantity]| {
                Ok(DepthLevel {
                    price: parse_price_ticks(price, price_scale)?,
                    quantity: Quantity(parse_decimal(quantity, quantity_scale)?),
                })
            })
            .collect::<Result<Vec<_>, ParseError>>()
    };
    Ok(BinanceMarketEvent::DepthUpdate(DepthUpdate {
        symbol: wire.symbol.to_owned(),
        event_time_ms: wire.event_time_ms,
        transaction_time_ms: wire.transaction_time_ms,
        first_update_id: wire.first_update_id,
        final_update_id: wire.final_update_id,
        previous_final_update_id: wire.previous_final_update_id,
        bids: parse_levels(wire.bids)?,
        asks: parse_levels(wire.asks)?,
    }))
}

fn parse_signed_decimal(value: &str, scale: u32) -> Result<i64, ParseError> {
    let (negative, unsigned) = match value.strip_prefix('-') {
        Some(unsigned) => (true, unsigned),
        None => (false, value),
    };
    let parsed = parse_decimal(unsigned, scale)?;
    if negative {
        parsed.checked_neg().ok_or(ParseError::DecimalOverflow)
    } else {
        Ok(parsed)
    }
}

fn parse_decimal(value: &str, scale: u32) -> Result<i64, ParseError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ParseError::InvalidDecimal);
    }
    if fraction.len() > scale as usize && fraction[scale as usize..].bytes().any(|b| b != b'0') {
        return Err(ParseError::InvalidDecimal);
    }

    let multiplier = 10_i64
        .checked_pow(scale)
        .ok_or(ParseError::DecimalOverflow)?;
    let whole_value = whole
        .parse::<i64>()
        .map_err(|_| ParseError::DecimalOverflow)?;
    let mut result = whole_value
        .checked_mul(multiplier)
        .ok_or(ParseError::DecimalOverflow)?;
    let mut fraction_text = fraction.to_owned();
    fraction_text.truncate(scale as usize);
    while fraction_text.len() < scale as usize {
        fraction_text.push('0');
    }
    if !fraction_text.is_empty() {
        result = result
            .checked_add(
                fraction_text
                    .parse::<i64>()
                    .map_err(|_| ParseError::DecimalOverflow)?,
            )
            .ok_or(ParseError::DecimalOverflow)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_ticker_without_float_rounding() {
        let payload = br#"{"e":"bookTicker","u":7,"E":1000,"T":999,"s":"ABCUSDT","b":"12.3400","B":"2.50","a":"12.3500","A":"3.00"}"#;
        let event = parse_market_message(payload, 4, 2).unwrap();
        assert_eq!(
            event,
            BinanceMarketEvent::BookTicker(BookTicker {
                symbol: "ABCUSDT".into(),
                event_time_ms: 1000,
                transaction_time_ms: 999,
                update_id: 7,
                bid_price: PriceTicks(123400),
                bid_quantity: Quantity(250),
                ask_price: PriceTicks(123500),
                ask_quantity: Quantity(300),
            })
        );
    }

    #[test]
    fn parses_combined_mark_price_stream() {
        let payload = br#"{"stream":"abcusdt@markPrice@1s","data":{"e":"markPriceUpdate","E":1000,"s":"ABCUSDT","p":"12.3456","i":"12.3000","r":"-0.00010000","T":2000}}"#;
        let event = parse_market_message(payload, 4, 2).unwrap();
        assert_eq!(
            event,
            BinanceMarketEvent::MarkPrice(MarkPrice {
                symbol: "ABCUSDT".into(),
                event_time_ms: 1000,
                mark_price: PriceTicks(123456),
                index_price: PriceTicks(123000),
                next_funding_time_ms: 2000,
                latest_funding_rate_e8: Some(-10_000),
            })
        );
    }

    #[test]
    fn parses_aggregate_trade_with_maker_side() {
        let payload = br#"{"e":"aggTrade","E":1000,"s":"ABCUSDT","a":42,"p":"12.3450","q":"2.50","T":999,"m":true}"#;
        let event = parse_market_message(payload, 4, 2).unwrap();
        assert_eq!(
            event,
            BinanceMarketEvent::AggTrade(AggTrade {
                symbol: "ABCUSDT".into(),
                event_time_ms: 1000,
                trade_time_ms: 999,
                aggregate_trade_id: 42,
                price: PriceTicks(123450),
                quantity: Quantity(250),
                buyer_is_maker: true,
            })
        );
    }

    #[test]
    fn parses_combined_diff_depth_with_sequence_fields() {
        let payload = br#"{"stream":"abcusdt@depth@100ms","data":{"e":"depthUpdate","E":1000,"T":999,"s":"ABCUSDT","U":11,"u":12,"pu":10,"b":[["12.3400","2.50"]],"a":[["12.3500","3.00"]]}}"#;
        let event = parse_market_message(payload, 4, 2).unwrap();
        assert_eq!(
            event,
            BinanceMarketEvent::DepthUpdate(DepthUpdate {
                symbol: "ABCUSDT".into(),
                event_time_ms: 1000,
                transaction_time_ms: 999,
                first_update_id: 11,
                final_update_id: 12,
                previous_final_update_id: Some(10),
                bids: vec![DepthLevel {
                    price: PriceTicks(123400),
                    quantity: Quantity(250)
                }],
                asks: vec![DepthLevel {
                    price: PriceTicks(123500),
                    quantity: Quantity(300)
                }],
            })
        );
    }

    #[test]
    fn rejects_nonzero_digits_beyond_scale() {
        let payload = br#"{"e":"bookTicker","u":1,"E":1,"T":1,"s":"ABCUSDT","b":"1.001","B":"1","a":"1.002","A":"1"}"#;
        assert_eq!(
            parse_market_message(payload, 2, 0),
            Err(ParseError::InvalidDecimal)
        );
    }

    #[test]
    fn rejects_unknown_event() {
        let payload = br#"{"e":"aggTrade","E":1,"s":"ABCUSDT"}"#;
        assert_eq!(
            parse_market_message(payload, 2, 0),
            Err(ParseError::UnsupportedEvent)
        );
    }

    #[test]
    fn preserves_missing_combined_data_error() {
        let payload = br#"{"stream":"abcusdt@bookTicker","data":null}"#;
        assert_eq!(
            parse_market_message(payload, 2, 0),
            Err(ParseError::MissingCombinedData)
        );
    }
}
