//! Binance Public Data adapters used by historical replay and backtest tooling.
//! The official archive is ZIP-wrapped CSV; this module consumes the extracted
//! CSV stream so ingestion stays independent from a particular archive library.

use std::io::{self, BufRead};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicDataKind {
    Klines,
    Trades,
    AggTrades,
}

impl PublicDataKind {
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Klines => "klines",
            Self::Trades => "trades",
            Self::AggTrades => "aggTrades",
        }
    }
}

pub fn monthly_download_url(
    kind: PublicDataKind,
    symbol: &str,
    interval: Option<&str>,
    year: u32,
    month: u32,
) -> Result<String, DataError> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(DataError::InvalidSymbol);
    }
    if !(1..=12).contains(&month) || year < 2017 {
        return Err(DataError::InvalidDate);
    }
    let (path, file) = match kind {
        PublicDataKind::Klines => {
            let interval = interval.ok_or(DataError::MissingInterval)?;
            if interval.is_empty() || interval.contains('/') {
                return Err(DataError::InvalidInterval);
            }
            let file = format!("{symbol}-{interval}-{year}-{month:02}.zip");
            (format!("klines/{symbol}/{interval}"), file)
        }
        PublicDataKind::Trades | PublicDataKind::AggTrades => {
            let file = format!("{symbol}-{}-{year}-{month:02}.zip", kind.directory());
            (format!("{}/{symbol}", kind.directory()), file)
        }
    };
    Ok(format!(
        "https://data.binance.vision/data/futures/um/monthly/{path}/{file}"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRow {
    pub open_time_ms: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub close_time_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeRow {
    pub trade_id: i64,
    pub price: String,
    pub quantity: String,
    pub quote_quantity: String,
    pub time_ms: i64,
    pub buyer_maker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggTradeRow {
    pub aggregate_trade_id: i64,
    pub price: String,
    pub quantity: String,
    pub first_trade_id: i64,
    pub last_trade_id: i64,
    pub time_ms: i64,
    pub buyer_maker: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoricalFileSummary {
    pub row_count: u64,
    pub first_timestamp_ms: Option<i64>,
    pub last_timestamp_ms: Option<i64>,
}

impl HistoricalFileSummary {
    fn record(&mut self, timestamp_ms: i64) {
        self.row_count = self.row_count.saturating_add(1);
        self.first_timestamp_ms.get_or_insert(timestamp_ms);
        self.last_timestamp_ms = Some(timestamp_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    InvalidSymbol,
    InvalidDate,
    MissingInterval,
    InvalidInterval,
    Io,
    InvalidRow {
        expected_at_least: usize,
        actual: usize,
    },
    InvalidInteger,
    InvalidBoolean,
}

impl From<io::Error> for DataError {
    fn from(_: io::Error) -> Self {
        Self::Io
    }
}

fn fields(line: &str) -> Vec<&str> {
    line.trim_end_matches(['\r', '\n']).split(',').collect()
}

fn is_header(fields: &[&str], first: &str) -> bool {
    fields.first().copied() == Some(first)
}

fn integer(value: &str) -> Result<i64, DataError> {
    value.trim().parse().map_err(|_| DataError::InvalidInteger)
}

fn boolean(value: &str) -> Result<bool, DataError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(DataError::InvalidBoolean),
    }
}
pub fn parse_kline_row(line: &str) -> Result<Option<KlineRow>, DataError> {
    let values = fields(line);
    if is_header(&values, "open_time") {
        return Ok(None);
    }
    if values.len() < 7 {
        return Err(DataError::InvalidRow {
            expected_at_least: 7,
            actual: values.len(),
        });
    }
    Ok(Some(KlineRow {
        open_time_ms: integer(values[0])?,
        open: values[1].to_owned(),
        high: values[2].to_owned(),
        low: values[3].to_owned(),
        close: values[4].to_owned(),
        volume: values[5].to_owned(),
        close_time_ms: integer(values[6])?,
    }))
}

pub fn parse_trade_row(line: &str) -> Result<Option<TradeRow>, DataError> {
    let values = fields(line);
    if is_header(&values, "id") || is_header(&values, "trade_id") {
        return Ok(None);
    }
    if values.len() < 6 {
        return Err(DataError::InvalidRow {
            expected_at_least: 6,
            actual: values.len(),
        });
    }
    Ok(Some(TradeRow {
        trade_id: integer(values[0])?,
        price: values[1].to_owned(),
        quantity: values[2].to_owned(),
        quote_quantity: values[3].to_owned(),
        time_ms: integer(values[4])?,
        buyer_maker: boolean(values[5])?,
    }))
}

pub fn parse_agg_trade_row(line: &str) -> Result<Option<AggTradeRow>, DataError> {
    let values = fields(line);
    if is_header(&values, "agg_trade_id") || is_header(&values, "aggregate_trade_id") {
        return Ok(None);
    }
    if values.len() < 7 {
        return Err(DataError::InvalidRow {
            expected_at_least: 7,
            actual: values.len(),
        });
    }
    Ok(Some(AggTradeRow {
        aggregate_trade_id: integer(values[0])?,
        price: values[1].to_owned(),
        quantity: values[2].to_owned(),
        first_trade_id: integer(values[3])?,
        last_trade_id: integer(values[4])?,
        time_ms: integer(values[5])?,
        buyer_maker: boolean(values[6])?,
    }))
}

pub fn summarize_klines<R: BufRead>(reader: R) -> Result<HistoricalFileSummary, DataError> {
    let mut summary = HistoricalFileSummary::default();
    for line in reader.lines() {
        if let Some(row) = parse_kline_row(&line?)? {
            summary.record(row.open_time_ms);
        }
    }
    Ok(summary)
}

pub fn summarize_trades<R: BufRead>(reader: R) -> Result<HistoricalFileSummary, DataError> {
    let mut summary = HistoricalFileSummary::default();
    for line in reader.lines() {
        if let Some(row) = parse_trade_row(&line?)? {
            summary.record(row.time_ms);
        }
    }
    Ok(summary)
}

pub fn summarize_agg_trades<R: BufRead>(reader: R) -> Result<HistoricalFileSummary, DataError> {
    let mut summary = HistoricalFileSummary::default();
    for line in reader.lines() {
        if let Some(row) = parse_agg_trade_row(&line?)? {
            summary.record(row.time_ms);
        }
    }
    Ok(summary)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn builds_official_um_monthly_kline_url() {
        assert_eq!(
            monthly_download_url(PublicDataKind::Klines, "cxmtusdt", Some("1m"), 2026, 9)
                .unwrap(),
            "https://data.binance.vision/data/futures/um/monthly/klines/CXMTUSDT/1m/CXMTUSDT-1m-2026-09.zip"
        );
    }

    #[test]
    fn parses_and_summarizes_kline_csv() {
        let csv = "open_time,open,high,low,close,volume,close_time\n1000,1,2,0.5,1.5,10,1999\n2000,1.5,3,1,2,20,2999\n";
        let summary = summarize_klines(Cursor::new(csv)).unwrap();
        assert_eq!(summary.row_count, 2);
        assert_eq!(summary.first_timestamp_ms, Some(1000));
        assert_eq!(summary.last_timestamp_ms, Some(2000));
    }

    #[test]
    fn parses_trade_and_agg_trade_rows() {
        let trade = parse_trade_row("7,12.3,4,49.2,1000,true").unwrap().unwrap();
        assert_eq!(trade.trade_id, 7);
        assert!(trade.buyer_maker);
        let agg = parse_agg_trade_row("8,12.3,4,7,7,1000,false")
            .unwrap()
            .unwrap();
        assert_eq!(agg.aggregate_trade_id, 8);
        assert!(!agg.buyer_maker);
    }

    #[test]
    fn rejects_malformed_rows_without_partial_acceptance() {
        assert!(matches!(
            parse_kline_row("1,2,3"),
            Err(DataError::InvalidRow { .. })
        ));
        assert_eq!(
            parse_trade_row("1,2,3,4,5,maybe"),
            Err(DataError::InvalidBoolean)
        );
    }

    #[test]
    fn rejects_invalid_download_parameters() {
        assert_eq!(
            monthly_download_url(PublicDataKind::Klines, "CX-MT", Some("1m"), 2026, 1),
            Err(DataError::InvalidSymbol)
        );
        assert_eq!(
            monthly_download_url(PublicDataKind::Klines, "CXMTUSDT", None, 2026, 1),
            Err(DataError::MissingInterval)
        );
    }
}
