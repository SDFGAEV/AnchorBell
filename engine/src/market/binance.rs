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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinanceMarketEvent {
    BookTicker(BookTicker),
    MarkPrice(MarkPrice),
}

#[derive(Debug, Deserialize)]
struct BookTickerWire {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "T")]
    transaction_time_ms: u64,
    #[serde(rename = "u")]
    update_id: u64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    bid_price: String,
    #[serde(rename = "B")]
    bid_quantity: String,
    #[serde(rename = "a")]
    ask_price: String,
    #[serde(rename = "A")]
    ask_quantity: String,
}

#[derive(Debug, Deserialize)]
struct MarkPriceWire {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time_ms: u64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "p")]
    mark_price: String,
    #[serde(rename = "i")]
    index_price: String,
    #[serde(rename = "T")]
    next_funding_time_ms: u64,
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
    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|_| ParseError::InvalidJson)?;
    let data = value.get("data").cloned().unwrap_or(value);
    if data.is_null() {
        return Err(ParseError::MissingCombinedData);
    }

    let event_type = data.get("e")
        .and_then(serde_json::Value::as_str)
        .ok_or(ParseError::UnsupportedEvent)?;

    match event_type {
        "bookTicker" => parse_book_ticker(data, price_scale, quantity_scale),
        "markPriceUpdate" => parse_mark_price(data, price_scale),
        _ => Err(ParseError::UnsupportedEvent),
    }
}

fn parse_book_ticker(
    value: serde_json::Value,
    price_scale: u32,
    quantity_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    let wire: BookTickerWire =
        serde_json::from_value(value).map_err(|_| ParseError::InvalidJson)?;
    if wire.event_type != "bookTicker" || wire.symbol.is_empty() {
        return Err(ParseError::UnsupportedEvent);
    }
    Ok(BinanceMarketEvent::BookTicker(BookTicker {
        symbol: wire.symbol,
        event_time_ms: wire.event_time_ms,
        transaction_time_ms: wire.transaction_time_ms,
        update_id: wire.update_id,
        bid_price: PriceTicks(parse_decimal(&wire.bid_price, price_scale)?),
        bid_quantity: Quantity(parse_decimal(&wire.bid_quantity, quantity_scale)?),
        ask_price: PriceTicks(parse_decimal(&wire.ask_price, price_scale)?),
        ask_quantity: Quantity(parse_decimal(&wire.ask_quantity, quantity_scale)?),
    }))
}

fn parse_mark_price(
    value: serde_json::Value,
    price_scale: u32,
) -> Result<BinanceMarketEvent, ParseError> {
    let wire: MarkPriceWire =
        serde_json::from_value(value).map_err(|_| ParseError::InvalidJson)?;
    if wire.event_type != "markPriceUpdate" || wire.symbol.is_empty() {
        return Err(ParseError::UnsupportedEvent);
    }
    Ok(BinanceMarketEvent::MarkPrice(MarkPrice {
        symbol: wire.symbol,
        event_time_ms: wire.event_time_ms,
        mark_price: PriceTicks(parse_decimal(&wire.mark_price, price_scale)?),
        index_price: PriceTicks(parse_decimal(&wire.index_price, price_scale)?),
        next_funding_time_ms: wire.next_funding_time_ms,
    }))
}
fn parse_decimal(value: &str, scale: u32) -> Result<i64, ParseError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ParseError::InvalidDecimal);
    }
    if fraction.len() > scale as usize
        && fraction[scale as usize..].bytes().any(|b| b != b'0')
    {
        return Err(ParseError::InvalidDecimal);
    }

    let multiplier = 10_i64.checked_pow(scale).ok_or(ParseError::DecimalOverflow)?;
    let whole_value = whole.parse::<i64>().map_err(|_| ParseError::DecimalOverflow)?;
    let mut result = whole_value
        .checked_mul(multiplier)
        .ok_or(ParseError::DecimalOverflow)?;
    let mut fraction_text = fraction.to_owned();
    fraction_text.truncate(scale as usize);
    while fraction_text.len() < scale as usize {
        fraction_text.push('0');
    }
    if !fraction_text.is_empty() {
        result = result.checked_add(
            fraction_text.parse::<i64>().map_err(|_| ParseError::DecimalOverflow)?
        ).ok_or(ParseError::DecimalOverflow)?;
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
        assert_eq!(event, BinanceMarketEvent::BookTicker(BookTicker {
            symbol: "ABCUSDT".into(),
            event_time_ms: 1000,
            transaction_time_ms: 999,
            update_id: 7,
            bid_price: PriceTicks(123400),
            bid_quantity: Quantity(250),
            ask_price: PriceTicks(123500),
            ask_quantity: Quantity(300),
        }));
    }

    #[test]
    fn parses_combined_mark_price_stream() {
        let payload = br#"{"stream":"abcusdt@markPrice@1s","data":{"e":"markPriceUpdate","E":1000,"s":"ABCUSDT","p":"12.3456","i":"12.3000","T":2000}}"#;
        let event = parse_market_message(payload, 4, 2).unwrap();
        assert_eq!(event, BinanceMarketEvent::MarkPrice(MarkPrice {
            symbol: "ABCUSDT".into(),
            event_time_ms: 1000,
            mark_price: PriceTicks(123456),
            index_price: PriceTicks(123000),
            next_funding_time_ms: 2000,
        }));
    }

    #[test]
    fn rejects_nonzero_digits_beyond_scale() {
        let payload = br#"{"e":"bookTicker","u":1,"E":1,"T":1,"s":"ABCUSDT","b":"1.001","B":"1","a":"1.002","A":"1"}"#;
        assert_eq!(parse_market_message(payload, 2, 0), Err(ParseError::InvalidDecimal));
    }

    #[test]
    fn rejects_unknown_event() {
        let payload = br#"{"e":"aggTrade","E":1,"s":"ABCUSDT"}"#;
        assert_eq!(parse_market_message(payload, 2, 0), Err(ParseError::UnsupportedEvent));
    }
}
