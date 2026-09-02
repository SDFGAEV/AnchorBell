//! Shared paper-trading and replay execution engine.
//!
//! The paper path consumes the same Binance bookTicker, markPrice, and
//! aggregate-trade events as the live adapter.  A passive order is filled only
//! when a public aggregate trade is at the order price and its aggressor side
//! is compatible with the order.  No bar-only shortcut is used here.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use thiserror::Error;
use tokio::{io::AsyncWriteExt, sync::mpsc};

use crate::{
    execution::BinanceEnvironment,
    execution::{OrderIntent, Side},
    market::{
        binance::{AggTrade, BinanceMarketEvent, BookTicker, MarkPrice},
        BinanceC2cFxClient, BinanceMarketConfig, BinanceMarketStream, BinanceSubscription,
        PublicMarketMetadataClient, ReconnectPolicy,
    },
    strategy::{profile_for, universe::instrument_for, AnchorCurrency, AnchorMakerStrategy},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PaperAnchor {
    pub close_price_ticks: i64,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}

impl PaperAnchor {
    pub fn valid_at(self, now_ms: u64, max_age_ms: u64) -> bool {
        self.close_price_ticks > 0
            && (self.observed_at_ms == 0 || now_ms >= self.observed_at_ms)
            && (self.valid_until_ms == 0 || now_ms < self.valid_until_ms)
            && (self.observed_at_ms == 0
                || max_age_ms == 0
                || now_ms.saturating_sub(self.observed_at_ms) <= max_age_ms)
    }
}

#[derive(Debug, Error)]
pub enum PaperError {
    #[error("invalid paper configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("invalid anchor row {row}: {reason}")]
    InvalidAnchorRow { row: usize, reason: &'static str },
    #[error("duplicate anchor symbol: {0}")]
    DuplicateAnchor(String),
    #[error("no anchors were loaded")]
    NoAnchors,
    #[error("I/O error: {0}")]
    Io(String),
    #[error("market stream error: {0}")]
    Market(String),
    #[error("JSON error: {0}")]
    Json(String),
    #[error("replay parse failed at line {line}: {error:?}")]
    ReplayParse {
        line: usize,
        error: crate::market::binance::ParseError,
    },
    #[error("replay timestamp moved backwards from {previous_ms} to {current_ms}")]
    ReplayOutOfOrder { previous_ms: u64, current_ms: u64 },
    #[error("replay event symbol is not configured: {0}")]
    ReplaySymbolNotConfigured(String),
}

impl From<std::io::Error> for PaperError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for PaperError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

pub fn load_anchors(path: &Path) -> Result<BTreeMap<String, PaperAnchor>, PaperError> {
    let reader = BufReader::new(File::open(path)?);
    let mut anchors = BTreeMap::new();
    for (index, line) in reader.lines().enumerate() {
        let row = index + 1;
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
        if fields
            .first()
            .is_some_and(|field| field.eq_ignore_ascii_case("symbol"))
        {
            continue;
        }
        if fields.len() != 4 {
            return Err(PaperError::InvalidAnchorRow {
                row,
                reason: "expected symbol,close_price_ticks,observed_at_ms,valid_until_ms",
            });
        }
        let symbol = normalize_symbol(fields[0]).ok_or(PaperError::InvalidAnchorRow {
            row,
            reason: "symbol must be non-empty ASCII alphanumeric text",
        })?;
        let close_price_ticks =
            fields[1]
                .parse::<i64>()
                .map_err(|_| PaperError::InvalidAnchorRow {
                    row,
                    reason: "close_price_ticks must be an integer",
                })?;
        let observed_at_ms =
            fields[2]
                .parse::<u64>()
                .map_err(|_| PaperError::InvalidAnchorRow {
                    row,
                    reason: "observed_at_ms must be an unsigned integer",
                })?;
        let valid_until_ms =
            fields[3]
                .parse::<u64>()
                .map_err(|_| PaperError::InvalidAnchorRow {
                    row,
                    reason: "valid_until_ms must be an unsigned integer",
                })?;
        if close_price_ticks <= 0
            || (valid_until_ms != 0 && observed_at_ms != 0 && valid_until_ms <= observed_at_ms)
        {
            return Err(PaperError::InvalidAnchorRow {
                row,
                reason: "anchor price must be positive and validity must be ordered",
            });
        }
        if anchors
            .insert(
                symbol.clone(),
                PaperAnchor {
                    close_price_ticks,
                    observed_at_ms,
                    valid_until_ms,
                },
            )
            .is_some()
        {
            return Err(PaperError::DuplicateAnchor(symbol));
        }
    }
    if anchors.is_empty() {
        return Err(PaperError::NoAnchors);
    }
    Ok(anchors)
}

/// Fetches the official Binance TradFi index price for every selected symbol
/// and materializes a run-local static anchor. No credentials or order API are
/// involved. The caller controls the run lifetime; the anchor itself has no
/// file-backed expiry and cannot silently survive a process restart.
///
/// A Binance index anchor stays in USDT for strategy and execution math; the
/// local equivalent is recorded separately after multiplying by the live
/// local-currency-per-USDT FX midpoint.
#[derive(Debug, Clone, Serialize)]
pub struct IndexAnchorConversion {
    pub index_price_usdt_ticks: i64,
    pub local_currency: String,
    pub index_price_local_ticks: i64,
    pub local_per_usdt_ppm: i64,
    pub fx_buy_local_per_usdt_ppm: i64,
    pub fx_sell_local_per_usdt_ppm: i64,
    pub fx_observed_at_ms: u64,
    pub fx_source: String,
    pub index_observed_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct BinanceIndexAnchorSet {
    pub anchors: BTreeMap<String, PaperAnchor>,
    pub conversions: BTreeMap<String, IndexAnchorConversion>,
}

pub async fn load_binance_index_anchor_set(
    environment: BinanceEnvironment,
    symbols: &[String],
    price_scale: u32,
    http_proxy: Option<&str>,
) -> Result<BinanceIndexAnchorSet, PaperError> {
    if symbols.is_empty() {
        return Err(PaperError::InvalidConfig(
            "index anchors require at least one symbol",
        ));
    }
    let mut seen = BTreeMap::new();
    let mut selected_metadata = Vec::with_capacity(symbols.len());
    let client = PublicMarketMetadataClient::new(environment.endpoints().rest_base, http_proxy)
        .map_err(|error| PaperError::Market(format!("index anchor client: {error}")))?;
    let exchange_info = client
        .exchange_info()
        .await
        .map_err(|error| PaperError::Market(format!("index anchor exchangeInfo: {error}")))?;

    for symbol in symbols {
        let normalized = normalize_symbol(symbol).ok_or(PaperError::InvalidConfig(
            "index anchor symbol must be non-empty ASCII alphanumeric text",
        ))?;
        if instrument_for(&normalized).is_none() {
            return Err(PaperError::InvalidConfig(
                "index anchor symbols must be selected TradFi instruments",
            ));
        }
        if seen.insert(normalized.clone(), ()).is_some() {
            return Err(PaperError::DuplicateAnchor(normalized));
        }
        let metadata = exchange_info
            .iter()
            .find(|metadata| metadata.symbol == normalized)
            .cloned()
            .ok_or_else(|| {
                PaperError::Market(format!(
                    "Binance exchangeInfo has no selected symbol {normalized}"
                ))
            })?;
        if !metadata.is_trading_tradifi_perpetual() {
            return Err(PaperError::Market(format!(
                "selected symbol {normalized} is not a trading TradFi perpetual"
            )));
        }
        selected_metadata.push(metadata);
    }

    let snapshots = client.symbol_snapshots(selected_metadata.clone(), 4).await;
    let observed_now_ms = now_ms();
    let mut fx_quotes = BTreeMap::new();
    let fx_client = BinanceC2cFxClient::new(http_proxy)
        .map_err(|error| PaperError::Market(format!("index anchor FX client: {error}")))?;
    let needs_cny = selected_metadata.iter().any(|metadata| {
        profile_for(&metadata.symbol)
            .is_some_and(|profile| profile.anchor_currency == AnchorCurrency::Cny)
    });
    let needs_hkd = selected_metadata.iter().any(|metadata| {
        profile_for(&metadata.symbol)
            .is_some_and(|profile| profile.anchor_currency == AnchorCurrency::Hkd)
    });
    if needs_cny {
        let quote = fx_client
            .midpoint(AnchorCurrency::Cny)
            .await
            .map_err(|error| PaperError::Market(format!("index anchor CNY/USDT FX: {error}")))?;
        fx_quotes.insert(AnchorCurrency::Cny.as_str().to_owned(), quote);
    }
    if needs_hkd {
        let quote = fx_client
            .midpoint(AnchorCurrency::Hkd)
            .await
            .map_err(|error| PaperError::Market(format!("index anchor HKD/USDT FX: {error}")))?;
        fx_quotes.insert(AnchorCurrency::Hkd.as_str().to_owned(), quote);
    }

    let mut anchors = BTreeMap::new();
    let mut conversions = BTreeMap::new();
    for snapshot in snapshots {
        let snapshot = snapshot
            .map_err(|error| PaperError::Market(format!("index anchor snapshot: {error}")))?;
        snapshot
            .validate_for_runtime(observed_now_ms)
            .map_err(|error| PaperError::Market(format!("index anchor validation: {error}")))?;
        let symbol = snapshot.metadata.symbol.clone();
        let profile = profile_for(&symbol).ok_or_else(|| {
            PaperError::Market(format!("no anchor currency profile for {symbol}"))
        })?;
        let fx_quote = fx_quotes
            .get(profile.anchor_currency.as_str())
            .ok_or_else(|| {
                PaperError::Market(format!(
                    "missing {}/USDT FX quote for {symbol}",
                    profile.anchor_currency.as_str()
                ))
            })?;
        let index_price = crate::market::binance::parse_price_ticks(
            &snapshot.premium_index.index_price,
            price_scale,
        )
        .map_err(|error| {
            PaperError::Market(format!(
                "index anchor price for {symbol} is invalid: {error:?}"
            ))
        })?;
        if index_price.0 <= 0 {
            return Err(PaperError::Market(format!(
                "index anchor price for {symbol} is not positive"
            )));
        }
        let local_price = fx_quote
            .convert_usdt_ticks_to_local(index_price.0)
            .ok_or_else(|| {
                PaperError::Market(format!(
                    "local FX conversion overflow for {symbol} at {}",
                    profile.anchor_currency.as_str()
                ))
            })?;
        anchors.insert(
            symbol.clone(),
            PaperAnchor {
                close_price_ticks: index_price.0,
                observed_at_ms: snapshot.observed_at_ms,
                valid_until_ms: 0,
            },
        );
        conversions.insert(
            symbol,
            IndexAnchorConversion {
                index_price_usdt_ticks: index_price.0,
                local_currency: profile.anchor_currency.as_str().to_owned(),
                index_price_local_ticks: local_price,
                local_per_usdt_ppm: fx_quote.midpoint_local_per_usdt_ppm,
                fx_buy_local_per_usdt_ppm: fx_quote.buy_local_per_usdt_ppm,
                fx_sell_local_per_usdt_ppm: fx_quote.sell_local_per_usdt_ppm,
                fx_observed_at_ms: fx_quote.observed_at_ms,
                fx_source: fx_quote.source.to_owned(),
                index_observed_at_ms: snapshot.observed_at_ms,
            },
        );
    }
    if anchors.len() != seen.len() || conversions.len() != seen.len() {
        return Err(PaperError::Market(
            "Binance returned an incomplete index-anchor set".to_owned(),
        ));
    }
    Ok(BinanceIndexAnchorSet {
        anchors,
        conversions,
    })
}

pub async fn load_binance_index_anchors(
    environment: BinanceEnvironment,
    symbols: &[String],
    price_scale: u32,
    http_proxy: Option<&str>,
) -> Result<BTreeMap<String, PaperAnchor>, PaperError> {
    Ok(
        load_binance_index_anchor_set(environment, symbols, price_scale, http_proxy)
            .await?
            .anchors,
    )
}

fn normalize_symbol(value: &str) -> Option<String> {
    let symbol = value.trim().to_ascii_uppercase();
    (!symbol.is_empty() && symbol.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .then_some(symbol)
}

#[derive(Debug, Clone, Copy)]
struct BookState {
    bid_price_ticks: i64,
    bid_quantity: i64,
    ask_price_ticks: i64,
    ask_quantity: i64,
}

#[derive(Debug, Clone, Copy)]
struct WorkingOrder {
    client_id: u64,
    side: Side,
    price_ticks: i64,
    remaining_quantity: i64,
}

#[derive(Debug, Clone)]
struct PaperSymbolState {
    symbol_id: u32,
    anchor: PaperAnchor,
    book: Option<BookState>,
    mark_price_ticks: Option<i64>,
    index_price_ticks: Option<i64>,
    next_funding_time_ms: u64,
    last_mark_time_ms: u64,
    last_trade_id: Option<u64>,
    working: Option<WorkingOrder>,
    position: i64,
    average_entry_ticks: i64,
    realized_pnl_ticks: i64,
    fees_ticks: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaperRecord {
    pub timestamp_ms: u64,
    pub kind: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_ticks: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i64>,
    pub position: i64,
    pub realized_pnl_ticks: i64,
    pub fees_ticks: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

struct RecordFields<'a> {
    kind: &'a str,
    client_id: Option<u64>,
    side: Option<Side>,
    price_ticks: Option<i64>,
    quantity: Option<i64>,
    detail: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaperSummary {
    pub event_count: u64,
    pub order_count: u64,
    pub fill_count: u64,
    pub filled_quantity: i64,
    pub rejected_entries: u64,
    pub realized_pnl_ticks: i64,
    pub unrealized_pnl_ticks: i64,
    pub fees_ticks: i64,
    pub net_pnl_ticks: i64,
    pub unrealized_valuation_complete: bool,
    pub current_absolute_position: i64,
    pub peak_absolute_position: i64,
    pub working_orders: u64,
    pub flat_at_end: bool,
}

#[derive(Debug, Clone)]
pub struct PaperEngine {
    strategy: AnchorMakerStrategy,
    max_position: i64,
    requested_quantity: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    fee_ppm: i64,
    quantity_scale: u32,
    states: BTreeMap<String, PaperSymbolState>,
    next_client_id: u64,
    event_count: u64,
    order_count: u64,
    fill_count: u64,
    filled_quantity: i64,
    rejected_entries: u64,
    peak_absolute_position: i64,
}

impl PaperEngine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        anchors: BTreeMap<String, PaperAnchor>,
        entry_threshold_bps: i64,
        max_position: i64,
        requested_quantity: i64,
        max_mark_index_gap_bps: i64,
        max_anchor_age_ms: u64,
        fee_ppm: i64,
        quantity_scale: u32,
    ) -> Result<Self, PaperError> {
        if anchors.is_empty()
            || entry_threshold_bps < 0
            || max_position <= 0
            || requested_quantity <= 0
            || max_mark_index_gap_bps < 0
            || fee_ppm < 0
            || quantity_scale > 18
        {
            return Err(PaperError::InvalidConfig(
                "anchors, position, quantity, thresholds, and fee must be valid",
            ));
        }
        let states = anchors
            .into_iter()
            .map(|(symbol, anchor)| {
                (
                    symbol.clone(),
                    PaperSymbolState {
                        symbol_id: stable_symbol_id(&symbol),
                        anchor,
                        book: None,
                        mark_price_ticks: None,
                        index_price_ticks: None,
                        next_funding_time_ms: 0,
                        last_mark_time_ms: 0,
                        last_trade_id: None,
                        working: None,
                        position: 0,
                        average_entry_ticks: 0,
                        realized_pnl_ticks: 0,
                        fees_ticks: 0,
                    },
                )
            })
            .collect();
        Ok(Self {
            strategy: AnchorMakerStrategy::new(entry_threshold_bps, 0),
            max_position,
            requested_quantity,
            max_mark_index_gap_bps,
            max_anchor_age_ms,
            fee_ppm,
            quantity_scale,
            states,
            next_client_id: 1,
            event_count: 0,
            order_count: 0,
            fill_count: 0,
            filled_quantity: 0,
            rejected_entries: 0,
            peak_absolute_position: 0,
        })
    }

    pub fn on_event(&mut self, event: BinanceMarketEvent) -> Vec<PaperRecord> {
        self.event_count = self.event_count.saturating_add(1);
        match event {
            BinanceMarketEvent::BookTicker(ticker) => self.on_book_ticker(ticker),
            BinanceMarketEvent::MarkPrice(mark) => self.on_mark_price(mark),
            BinanceMarketEvent::AggTrade(trade) => self.on_agg_trade(trade),
        }
    }

    pub fn cancel_all(&mut self, timestamp_ms: u64, detail: &str) -> Vec<PaperRecord> {
        let symbols = self.states.keys().cloned().collect::<Vec<_>>();
        symbols
            .into_iter()
            .flat_map(|symbol| self.cancel_symbol(&symbol, timestamp_ms, detail))
            .collect()
    }

    pub fn summary(&self) -> PaperSummary {
        let mut current_absolute_position = 0_i64;
        let mut realized_pnl_ticks = 0_i64;
        let mut unrealized_pnl_ticks = 0_i64;
        let mut fees_ticks = 0_i64;
        let mut working_orders = 0_u64;
        let mut unrealized_valuation_complete = true;
        for state in self.states.values() {
            current_absolute_position = current_absolute_position
                .saturating_add(state.position.checked_abs().unwrap_or(i64::MAX));
            realized_pnl_ticks = realized_pnl_ticks.saturating_add(state.realized_pnl_ticks);
            fees_ticks = fees_ticks.saturating_add(state.fees_ticks);
            working_orders += u64::from(state.working.is_some());
            if state.position != 0 {
                match unrealized_pnl(state, self.quantity_scale) {
                    Some(pnl) => unrealized_pnl_ticks = unrealized_pnl_ticks.saturating_add(pnl),
                    None => unrealized_valuation_complete = false,
                }
            }
        }
        let flat_at_end = current_absolute_position == 0 && working_orders == 0;
        PaperSummary {
            event_count: self.event_count,
            order_count: self.order_count,
            fill_count: self.fill_count,
            filled_quantity: self.filled_quantity,
            rejected_entries: self.rejected_entries,
            realized_pnl_ticks,
            unrealized_pnl_ticks,
            fees_ticks,
            net_pnl_ticks: realized_pnl_ticks
                .saturating_add(unrealized_pnl_ticks)
                .saturating_sub(fees_ticks),
            unrealized_valuation_complete,
            current_absolute_position,
            peak_absolute_position: self.peak_absolute_position,
            working_orders,
            flat_at_end,
        }
    }

    fn on_book_ticker(&mut self, ticker: BookTicker) -> Vec<PaperRecord> {
        let symbol = ticker.symbol.to_ascii_uppercase();
        if let Some(state) = self.states.get_mut(&symbol) {
            state.book = Some(BookState {
                bid_price_ticks: ticker.bid_price.0,
                bid_quantity: ticker.bid_quantity.0,
                ask_price_ticks: ticker.ask_price.0,
                ask_quantity: ticker.ask_quantity.0,
            });
        } else {
            return Vec::new();
        }
        self.rebalance_symbol(&symbol, ticker.event_time_ms)
    }

    fn on_mark_price(&mut self, mark: MarkPrice) -> Vec<PaperRecord> {
        let symbol = mark.symbol.to_ascii_uppercase();
        if let Some(state) = self.states.get_mut(&symbol) {
            state.mark_price_ticks = Some(mark.mark_price.0);
            state.index_price_ticks = Some(mark.index_price.0);
            state.next_funding_time_ms = mark.next_funding_time_ms;
            state.last_mark_time_ms = mark.event_time_ms;
        } else {
            return Vec::new();
        }
        self.rebalance_symbol(&symbol, mark.event_time_ms)
    }

    fn on_agg_trade(&mut self, trade: AggTrade) -> Vec<PaperRecord> {
        let symbol = trade.symbol.to_ascii_uppercase();
        let fee_ppm = self.fee_ppm;
        let quantity_scale = self.quantity_scale;
        let (quantity, order) = {
            let Some(state) = self.states.get_mut(&symbol) else {
                return Vec::new();
            };
            if state
                .last_trade_id
                .is_some_and(|last| trade.aggregate_trade_id <= last)
            {
                return Vec::new();
            }
            state.last_trade_id = Some(trade.aggregate_trade_id);
            let Some(order) = state.working else {
                return Vec::new();
            };
            let compatible = match order.side {
                Side::Buy => trade.buyer_is_maker && trade.price.0 == order.price_ticks,
                Side::Sell => !trade.buyer_is_maker && trade.price.0 == order.price_ticks,
            };
            if !compatible {
                return Vec::new();
            }
            let quantity = trade.quantity.0.min(order.remaining_quantity).max(0);
            if quantity <= 0 {
                return Vec::new();
            }
            let mut updated_order = order;
            updated_order.remaining_quantity -= quantity;
            state.working = (updated_order.remaining_quantity > 0).then_some(updated_order);
            apply_position_fill(
                state,
                order.side,
                order.price_ticks,
                quantity,
                fee_ppm,
                quantity_scale,
            );
            (quantity, order)
        };
        self.fill_count = self.fill_count.saturating_add(1);
        self.filled_quantity = self.filled_quantity.saturating_add(quantity);
        let state = self.states.get(&symbol).expect("symbol state exists");
        self.peak_absolute_position = self
            .peak_absolute_position
            .max(state.position.checked_abs().unwrap_or(i64::MAX));
        vec![self.record(
            &symbol,
            state,
            trade.trade_time_ms.max(trade.event_time_ms),
            RecordFields {
                kind: "fill",
                client_id: Some(order.client_id),
                side: Some(order.side),
                price_ticks: Some(order.price_ticks),
                quantity: Some(quantity),
                detail: Some(if state.working.is_some() {
                    "partial maker fill"
                } else {
                    "complete maker fill"
                }),
            },
        )]
    }

    fn rebalance_symbol(&mut self, symbol: &str, timestamp_ms: u64) -> Vec<PaperRecord> {
        let max_position = self.max_position;
        let (desired, has_working) = {
            let state = self.states.get(symbol).expect("symbol state exists");
            let Some(book) = state.book else {
                return Vec::new();
            };
            let mark_index_ok = match (state.mark_price_ticks, state.index_price_ticks) {
                (Some(mark), Some(index)) => {
                    let gap = (i128::from(mark) - i128::from(index)).abs() * 10_000;
                    gap <= i128::from(self.max_mark_index_gap_bps) * i128::from(index.max(1))
                }
                _ => false,
            };
            if book.bid_price_ticks <= 0
                || book.ask_price_ticks < book.bid_price_ticks
                || book.bid_quantity <= 0
                || book.ask_quantity <= 0
                || !mark_index_ok
                || !state.anchor.valid_at(timestamp_ms, self.max_anchor_age_ms)
            {
                (None, state.working.is_some())
            } else {
                (
                    self.strategy
                        .generate_intent(
                            state.symbol_id,
                            book.bid_price_ticks,
                            book.ask_price_ticks,
                            state.anchor.close_price_ticks,
                            self.requested_quantity,
                        )
                        .and_then(|mut intent| {
                            intent.quantity = intent.quantity.min(max_order_quantity(
                                state.position,
                                intent.side,
                                max_position,
                            ));
                            (intent.quantity > 0).then_some(intent)
                        }),
                    state.working.is_some(),
                )
            }
        };
        if desired.is_none() {
            if has_working {
                return self.cancel_symbol(symbol, timestamp_ms, "signal or data gate blocked");
            }
            if let Some(state) = self.states.get_mut(symbol) {
                self.rejected_entries = self.rejected_entries.saturating_add(1);
                state.last_mark_time_ms = timestamp_ms;
            }
            return Vec::new();
        }
        let desired = desired.expect("desired intent exists");
        let same_order = self.states[symbol].working.is_some_and(|order| {
            order.side == desired.side
                && order.price_ticks == desired.price
                && order.remaining_quantity >= desired.quantity
        });
        if same_order {
            return Vec::new();
        }
        let mut records = if has_working {
            self.cancel_symbol(symbol, timestamp_ms, "quote replacement")
        } else {
            Vec::new()
        };
        records.extend(self.place_symbol(symbol, desired, timestamp_ms));
        records
    }

    fn place_symbol(
        &mut self,
        symbol: &str,
        intent: OrderIntent,
        timestamp_ms: u64,
    ) -> Vec<PaperRecord> {
        if !intent.post_only || intent.price <= 0 || intent.quantity <= 0 {
            return Vec::new();
        }
        let client_id = self.next_client_id;
        self.next_client_id = self.next_client_id.saturating_add(1);
        let state = self.states.get_mut(symbol).expect("symbol state exists");
        let Some(book) = state.book else {
            return Vec::new();
        };
        let maker_valid = match intent.side {
            Side::Buy => intent.price <= book.bid_price_ticks,
            Side::Sell => intent.price >= book.ask_price_ticks,
        };
        if !maker_valid {
            self.rejected_entries = self.rejected_entries.saturating_add(1);
            return Vec::new();
        }
        state.working = Some(WorkingOrder {
            client_id,
            side: intent.side,
            price_ticks: intent.price,
            remaining_quantity: intent.quantity,
        });
        self.order_count = self.order_count.saturating_add(1);
        let state = self.states.get(symbol).expect("symbol state exists");
        vec![self.record(
            symbol,
            state,
            timestamp_ms,
            RecordFields {
                kind: "order_placed",
                client_id: Some(client_id),
                side: Some(intent.side),
                price_ticks: Some(intent.price),
                quantity: Some(intent.quantity),
                detail: Some("maker-only paper order"),
            },
        )]
    }

    fn cancel_symbol(&mut self, symbol: &str, timestamp_ms: u64, detail: &str) -> Vec<PaperRecord> {
        let canceled = self
            .states
            .get_mut(symbol)
            .and_then(|state| state.working.take());
        let Some(order) = canceled else {
            return Vec::new();
        };
        let state = self.states.get(symbol).expect("symbol state exists");
        vec![self.record(
            symbol,
            state,
            timestamp_ms,
            RecordFields {
                kind: "order_canceled",
                client_id: Some(order.client_id),
                side: Some(order.side),
                price_ticks: Some(order.price_ticks),
                quantity: Some(order.remaining_quantity),
                detail: Some(detail),
            },
        )]
    }

    fn record(
        &self,
        symbol: &str,
        state: &PaperSymbolState,
        timestamp_ms: u64,
        fields: RecordFields<'_>,
    ) -> PaperRecord {
        PaperRecord {
            timestamp_ms,
            kind: fields.kind.to_owned(),
            symbol: symbol.to_owned(),
            client_id: fields.client_id,
            side: fields.side.map(side_name),
            price_ticks: fields.price_ticks,
            quantity: fields.quantity,
            position: state.position,
            realized_pnl_ticks: state.realized_pnl_ticks,
            fees_ticks: state.fees_ticks,
            detail: fields.detail.map(str::to_owned),
        }
    }
}

fn max_order_quantity(position: i64, side: Side, max_position: i64) -> i64 {
    let position = i128::from(position);
    let max_position = i128::from(max_position);
    let allowed = match side {
        Side::Buy => max_position - position,
        Side::Sell => max_position + position,
    };
    clamp_i128(allowed.max(0))
}

fn apply_position_fill(
    state: &mut PaperSymbolState,
    side: Side,
    price_ticks: i64,
    quantity: i64,
    fee_ppm: i64,
    quantity_scale: u32,
) {
    let delta = match side {
        Side::Buy => quantity,
        Side::Sell => -quantity,
    };
    let old_position = state.position;
    let same_direction =
        old_position == 0 || (old_position > 0 && delta > 0) || (old_position < 0 && delta < 0);
    if same_direction {
        let old_abs = i128::from(old_position).abs();
        let delta_abs = i128::from(delta).abs();
        let total = old_abs + delta_abs;
        let weighted = (old_abs * i128::from(state.average_entry_ticks)
            + delta_abs * i128::from(price_ticks))
            / total.max(1);
        state.average_entry_ticks = clamp_i128(weighted);
    } else {
        let close_quantity = i128::from(old_position).abs().min(i128::from(delta).abs());
        let pnl_per_unit = if old_position > 0 {
            i128::from(price_ticks) - i128::from(state.average_entry_ticks)
        } else {
            i128::from(state.average_entry_ticks) - i128::from(price_ticks)
        };
        state.realized_pnl_ticks = state.realized_pnl_ticks.saturating_add(clamp_i128(
            pnl_per_unit * close_quantity / quantity_scale_multiplier(quantity_scale),
        ));
        if i128::from(delta).abs() > close_quantity {
            state.average_entry_ticks = price_ticks;
        } else if i128::from(old_position) + i128::from(delta) == 0 {
            state.average_entry_ticks = 0;
        }
    }
    state.position = clamp_i128(i128::from(old_position) + i128::from(delta));
    let notional = i128::from(price_ticks).abs() * i128::from(quantity).abs();
    state.fees_ticks = state.fees_ticks.saturating_add(clamp_i128(
        notional * i128::from(fee_ppm) / 1_000_000 / quantity_scale_multiplier(quantity_scale),
    ));
}

fn quantity_scale_multiplier(quantity_scale: u32) -> i128 {
    10_i128.pow(quantity_scale)
}

fn unrealized_pnl(state: &PaperSymbolState, quantity_scale: u32) -> Option<i64> {
    if state.position == 0 {
        return Some(0);
    }
    let mark_price_ticks = i128::from(state.mark_price_ticks?);
    let entry_price_ticks = i128::from(state.average_entry_ticks);
    let position = i128::from(state.position);
    let pnl_per_unit = if position > 0 {
        mark_price_ticks - entry_price_ticks
    } else {
        entry_price_ticks - mark_price_ticks
    };
    Some(clamp_i128(
        pnl_per_unit * position.abs() / quantity_scale_multiplier(quantity_scale),
    ))
}

fn clamp_i128(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn side_name(side: Side) -> String {
    match side {
        Side::Buy => "BUY".to_owned(),
        Side::Sell => "SELL".to_owned(),
    }
}

/// Serializes a parsed event back to Binance-compatible JSONL.  The optional
/// receipt timestamp lets replay order events from multiple live shards while
/// retaining each exchange event timestamp for strategy decisions.
pub fn market_event_to_json(
    event: &BinanceMarketEvent,
    price_scale: u32,
    quantity_scale: u32,
    received_at_ms: Option<u64>,
) -> serde_json::Value {
    let mut value = match event {
        BinanceMarketEvent::BookTicker(book) => serde_json::json!({
            "e": "bookTicker",
            "E": book.event_time_ms,
            "T": book.transaction_time_ms,
            "u": book.update_id,
            "s": book.symbol,
            "b": crate::execution::binance_wire::format_ticks(book.bid_price.0, price_scale),
            "B": crate::execution::binance_wire::format_ticks(book.bid_quantity.0, quantity_scale),
            "a": crate::execution::binance_wire::format_ticks(book.ask_price.0, price_scale),
            "A": crate::execution::binance_wire::format_ticks(book.ask_quantity.0, quantity_scale),
        }),
        BinanceMarketEvent::MarkPrice(mark) => serde_json::json!({
            "e": "markPriceUpdate",
            "E": mark.event_time_ms,
            "s": mark.symbol,
            "p": crate::execution::binance_wire::format_ticks(mark.mark_price.0, price_scale),
            "i": crate::execution::binance_wire::format_ticks(mark.index_price.0, price_scale),
            "T": mark.next_funding_time_ms,
            "r": mark.latest_funding_rate_e8
                .map(|value| crate::execution::binance_wire::format_ticks(value, 8))
                .unwrap_or_else(|| "0".to_owned()),
        }),
        BinanceMarketEvent::AggTrade(trade) => serde_json::json!({
            "e": "aggTrade",
            "E": trade.event_time_ms,
            "s": trade.symbol,
            "a": trade.aggregate_trade_id,
            "p": crate::execution::binance_wire::format_ticks(trade.price.0, price_scale),
            "q": crate::execution::binance_wire::format_ticks(trade.quantity.0, quantity_scale),
            "T": trade.trade_time_ms,
            "m": trade.buyer_is_maker,
        }),
    };
    if let Some(received_at_ms) = received_at_ms {
        value["_anchorbell_received_at_ms"] = serde_json::json!(received_at_ms);
    }
    value
}

fn stable_symbol_id(symbol: &str) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for byte in symbol.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}

#[derive(Debug, Clone)]
pub struct PaperRunConfig {
    pub environment: BinanceEnvironment,
    pub symbols: Vec<String>,
    pub price_scale: u32,
    pub quantity_scale: u32,
    pub max_subscriptions_per_shard: usize,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub duration_secs: u64,
    pub http_proxy: Option<String>,
    pub market_output_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PaperRunResult {
    pub summary: PaperSummary,
    pub records_written: u64,
    pub records_dropped: u64,
    pub market_records_written: u64,
    pub market_records_dropped: u64,
    pub stopped_by_duration: bool,
}

// The explicit arguments keep the paper run's strategy assumptions visible at the call site.
#[allow(clippy::too_many_arguments)]
pub async fn run_live(
    config: PaperRunConfig,
    anchors: BTreeMap<String, PaperAnchor>,
    entry_threshold_bps: i64,
    max_position: i64,
    requested_quantity: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    fee_ppm: i64,
    output_path: Option<PathBuf>,
) -> Result<PaperRunResult, PaperError> {
    if config.duration_secs == 0 || config.symbols.is_empty() {
        return Err(PaperError::InvalidConfig(
            "duration and symbols are required",
        ));
    }
    if config
        .symbols
        .iter()
        .any(|symbol| instrument_for(symbol).is_none())
    {
        return Err(PaperError::InvalidConfig(
            "symbols must be selected execution-universe TradFi instruments",
        ));
    }
    let public_subscriptions = config
        .symbols
        .iter()
        .map(|symbol| {
            BinanceSubscription::new(symbol)
                .map(|subscription| subscription.book_ticker_only())
                .map_err(|error| PaperError::InvalidConfig(subscription_error_name(error)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let market_subscriptions = config
        .symbols
        .iter()
        .map(|symbol| {
            BinanceSubscription::new(symbol)
                .map(|subscription| subscription.market_reference_and_trades())
                .map_err(|error| PaperError::InvalidConfig(subscription_error_name(error)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let endpoints = config.environment.endpoints();
    let public_config = BinanceMarketConfig {
        market_ws_base: endpoints.public_market_ws_base.into(),
        subscriptions: public_subscriptions,
        price_scale: config.price_scale,
        quantity_scale: config.quantity_scale,
        max_frame_bytes: 1_048_576,
        connect_timeout_ms: config.connect_timeout_ms,
        read_timeout_ms: config.read_timeout_ms,
        http_proxy: config.http_proxy.clone(),
        reconnect: ReconnectPolicy::default(),
    };
    let market_config = BinanceMarketConfig {
        market_ws_base: endpoints.market_ws_base.into(),
        subscriptions: market_subscriptions,
        price_scale: config.price_scale,
        quantity_scale: config.quantity_scale,
        max_frame_bytes: 1_048_576,
        connect_timeout_ms: config.connect_timeout_ms,
        read_timeout_ms: config.read_timeout_ms,
        http_proxy: config.http_proxy.clone(),
        reconnect: ReconnectPolicy::default(),
    };
    let mut shard_configs = public_config
        .into_shards(config.max_subscriptions_per_shard)
        .map_err(|error| PaperError::Market(error.to_string()))?;
    shard_configs.extend(
        market_config
            .into_shards(config.max_subscriptions_per_shard)
            .map_err(|error| PaperError::Market(error.to_string()))?,
    );
    let mut engine = PaperEngine::new(
        anchors,
        entry_threshold_bps,
        max_position,
        requested_quantity,
        max_mark_index_gap_bps,
        max_anchor_age_ms,
        fee_ppm,
        config.quantity_scale,
    )?;
    let (record_tx, record_writer, written, dropped) = spawn_record_writer(output_path).await?;
    let (market_tx, market_writer, market_written, market_dropped) =
        spawn_record_writer(config.market_output_path.clone()).await?;
    let (event_tx, mut event_rx) = mpsc::channel::<BinanceMarketEvent>(4096);
    let event_dropped = Arc::new(AtomicU64::new(0));
    let mut shard_tasks = tokio::task::JoinSet::new();
    for shard_config in shard_configs {
        let event_tx = event_tx.clone();
        let event_dropped = Arc::clone(&event_dropped);
        shard_tasks.spawn(async move {
            let mut stream = BinanceMarketStream::new(shard_config);
            stream
                .run_until_error(|event| {
                    if event_tx.try_send(event).is_err() {
                        event_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .await
        });
    }
    drop(event_tx);

    let price_scale = config.price_scale;
    let quantity_scale = config.quantity_scale;
    let run_result = tokio::time::timeout(Duration::from_secs(config.duration_secs), async {
        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        return Err(PaperError::Market(
                            "all market shards stopped".to_owned(),
                        ));
                    };
                    let market_line = serde_json::to_string(&market_event_to_json(
                        &event,
                        price_scale,
                        quantity_scale,
                        Some(now_ms()),
                    ))
                    .unwrap_or_else(|_| "{}".to_owned());
                    if market_tx.try_send(market_line).is_err() {
                        market_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    for record in engine.on_event(event) {
                        let line = serde_json::to_string(&record)
                            .unwrap_or_else(|_| "{}".to_owned());
                        if record_tx.try_send(line).is_err() {
                            dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                joined = shard_tasks.join_next() => {
                    match joined {
                        Some(Ok(Ok(()))) => {
                            return Err(PaperError::Market(
                                "market shard stopped".to_owned(),
                            ));
                        }
                        Some(Ok(Err(error))) => {
                            return Err(PaperError::Market(error.to_string()));
                        }
                        Some(Err(error)) => {
                            return Err(PaperError::Market(format!(
                                "market shard task failed: {error}"
                            )));
                        }
                        None => {
                            return Err(PaperError::Market(
                                "all market shards stopped".to_owned(),
                            ));
                        }
                    }
                }
            }
        }
    })
    .await;

    shard_tasks.abort_all();
    while shard_tasks.join_next().await.is_some() {}

    for record in engine.cancel_all(now_ms(), "paper run stopped") {
        let line = serde_json::to_string(&record)?;
        if record_tx.try_send(line).is_err() {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    drop(record_tx);
    drop(market_tx);
    let records_written = record_writer
        .await
        .map_err(|error| PaperError::Io(error.to_string()))??;
    let market_records_written = market_writer
        .await
        .map_err(|error| PaperError::Io(error.to_string()))??;
    let stopped_by_duration = match run_result {
        Err(_) => true,
        Ok(Ok(())) => false,
        Ok(Err(error)) => return Err(error),
    };
    let event_dropped = event_dropped.load(Ordering::Relaxed);
    if event_dropped != 0 {
        return Err(PaperError::Market(format!(
            "market event queue dropped {event_dropped} events"
        )));
    }
    Ok(PaperRunResult {
        summary: engine.summary(),
        records_written: records_written.max(written.load(Ordering::Relaxed)),
        records_dropped: dropped.load(Ordering::Relaxed),
        market_records_written: market_records_written.max(market_written.load(Ordering::Relaxed)),
        market_records_dropped: market_dropped.load(Ordering::Relaxed),
        stopped_by_duration,
    })
}

async fn spawn_record_writer(
    output_path: Option<PathBuf>,
) -> Result<
    (
        mpsc::Sender<String>,
        tokio::task::JoinHandle<Result<u64, PaperError>>,
        Arc<AtomicU64>,
        Arc<AtomicU64>,
    ),
    PaperError,
> {
    let (tx, mut rx) = mpsc::channel::<String>(4096);
    let written = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let writer_count = Arc::clone(&written);
    let task = tokio::spawn(async move {
        let Some(path) = output_path else {
            while let Some(_line) = rx.recv().await {
                writer_count.fetch_add(1, Ordering::Relaxed);
            }
            return Ok(writer_count.load(Ordering::Relaxed));
        };
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::File::create(path).await?;
        let mut file = tokio::io::BufWriter::new(file);
        while let Some(line) = rx.recv().await {
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
            writer_count.fetch_add(1, Ordering::Relaxed);
        }
        file.flush().await?;
        Ok(writer_count.load(Ordering::Relaxed))
    });
    Ok((tx, task, written, dropped))
}

fn event_symbol(event: &BinanceMarketEvent) -> &str {
    match event {
        BinanceMarketEvent::BookTicker(value) => &value.symbol,
        BinanceMarketEvent::MarkPrice(value) => &value.symbol,
        BinanceMarketEvent::AggTrade(value) => &value.symbol,
    }
}

// The explicit arguments keep replay assumptions visible and deterministic.
#[allow(clippy::too_many_arguments)]
pub fn replay_jsonl(
    input_path: &Path,
    output_path: Option<&Path>,
    anchors: BTreeMap<String, PaperAnchor>,
    price_scale: u32,
    quantity_scale: u32,
    entry_threshold_bps: i64,
    max_position: i64,
    requested_quantity: i64,
    max_mark_index_gap_bps: i64,
    max_anchor_age_ms: u64,
    fee_ppm: i64,
) -> Result<PaperSummary, PaperError> {
    let mut engine = PaperEngine::new(
        anchors,
        entry_threshold_bps,
        max_position,
        requested_quantity,
        max_mark_index_gap_bps,
        max_anchor_age_ms,
        fee_ppm,
        quantity_scale,
    )?;
    let reader = BufReader::new(File::open(input_path)?);
    if let Some(parent) = output_path
        .and_then(Path::parent)
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = output_path
        .map(|path| File::create(path).map(BufWriter::new))
        .transpose()?;
    let mut previous_ms = None;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let envelope = serde_json::from_str::<serde_json::Value>(&line).ok();
        let payload = envelope
            .as_ref()
            .and_then(|value| value.get("payload"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(line.as_str());
        if payload.contains("\"id\"") && payload.contains("\"result\"") {
            continue;
        }
        let received_at_ms = envelope
            .as_ref()
            .and_then(|value| value.get("received_at_ms"))
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
            .or_else(|| {
                envelope
                    .as_ref()
                    .and_then(|value| value.get("_anchorbell_received_at_ms"))
                    .and_then(serde_json::Value::as_u64)
            });
        let event = crate::market::binance::parse_market_message(
            payload.as_bytes(),
            price_scale,
            quantity_scale,
        )
        .map_err(|error| PaperError::ReplayParse {
            line: line_number,
            error,
        })?;
        let symbol = event_symbol(&event).to_ascii_uppercase();
        if !engine.states.contains_key(&symbol) {
            return Err(PaperError::ReplaySymbolNotConfigured(symbol));
        }
        let event_timestamp_ms = match &event {
            BinanceMarketEvent::BookTicker(value) => value.event_time_ms,
            BinanceMarketEvent::MarkPrice(value) => value.event_time_ms,
            BinanceMarketEvent::AggTrade(value) => value.event_time_ms,
        };
        let timestamp_ms = received_at_ms.unwrap_or(event_timestamp_ms);
        if previous_ms.is_some_and(|previous| timestamp_ms < previous) {
            return Err(PaperError::ReplayOutOfOrder {
                previous_ms: previous_ms.unwrap(),
                current_ms: timestamp_ms,
            });
        }
        previous_ms = Some(timestamp_ms);
        for record in engine.on_event(event) {
            if let Some(output) = output.as_mut() {
                serde_json::to_writer(&mut *output, &record)?;
                output.write_all(b"\n")?;
            }
        }
    }
    // A replay window cannot assume an order remains live after its last event.
    // Cancel working quotes at EOF, but never synthesize a position flatten fill.
    let final_timestamp_ms = previous_ms.unwrap_or(0);
    for record in engine.cancel_all(final_timestamp_ms, "replay window ended") {
        if let Some(output) = output.as_mut() {
            serde_json::to_writer(&mut *output, &record)?;
            output.write_all(b"\n")?;
        }
    }
    if let Some(output) = output.as_mut() {
        output.flush()?;
    }
    Ok(engine.summary())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn subscription_error_name(error: crate::market::SubscriptionError) -> &'static str {
    match error {
        crate::market::SubscriptionError::EmptySymbol => "empty symbol",
        crate::market::SubscriptionError::InvalidSymbol => "invalid symbol",
        crate::market::SubscriptionError::NoStreams => "no streams",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::binance::parse_market_message;

    fn anchors() -> BTreeMap<String, PaperAnchor> {
        [(
            "ABCUSDT".to_owned(),
            PaperAnchor {
                close_price_ticks: 100,
                observed_at_ms: 0,
                valid_until_ms: 0,
            },
        )]
        .into_iter()
        .collect()
    }

    fn engine() -> PaperEngine {
        PaperEngine::new(anchors(), 100, 100, 10, 20, 0, 0, 0).unwrap()
    }

    fn feed(engine: &mut PaperEngine, raw: &[u8]) -> Vec<PaperRecord> {
        let event = parse_market_message(raw, 0, 0).unwrap();
        engine.on_event(event)
    }

    #[test]
    fn paper_order_fills_only_on_compatible_aggressor_at_exact_price() {
        let mut engine = engine();
        feed(
            &mut engine,
            br#"{"e":"markPriceUpdate","E":1,"s":"ABCUSDT","p":"100","i":"100","T":1000,"r":"0"}"#,
        );
        let placed = feed(
            &mut engine,
            br#"{"e":"bookTicker","u":1,"E":2,"T":2,"s":"ABCUSDT","b":"98","B":"10","a":"99","A":"10"}"#,
        );
        assert_eq!(placed.len(), 1);
        let wrong_side = feed(
            &mut engine,
            br#"{"e":"aggTrade","E":3,"s":"ABCUSDT","a":1,"p":"98","q":"3","T":3,"m":false}"#,
        );
        assert!(wrong_side.is_empty());
        let filled = feed(
            &mut engine,
            br#"{"e":"aggTrade","E":4,"s":"ABCUSDT","a":2,"p":"98","q":"3","T":4,"m":true}"#,
        );
        assert_eq!(filled.len(), 1);
        assert_eq!(filled[0].quantity, Some(3));
        assert_eq!(engine.summary().current_absolute_position, 3);
    }

    #[test]
    fn summary_includes_mark_to_market_for_open_position() {
        let mut engine = engine();
        feed(
            &mut engine,
            br#"{"e":"markPriceUpdate","E":1,"s":"ABCUSDT","p":"100","i":"100","T":1000,"r":"0"}"#,
        );
        feed(
            &mut engine,
            br#"{"e":"bookTicker","u":1,"E":2,"T":2,"s":"ABCUSDT","b":"98","B":"10","a":"99","A":"10"}"#,
        );
        feed(
            &mut engine,
            br#"{"e":"aggTrade","E":3,"s":"ABCUSDT","a":1,"p":"98","q":"3","T":3,"m":true}"#,
        );
        let summary = engine.summary();
        assert_eq!(summary.current_absolute_position, 3);
        assert_eq!(summary.unrealized_pnl_ticks, 6);
        assert_eq!(summary.net_pnl_ticks, 6);
        assert!(summary.unrealized_valuation_complete);
        assert!(!summary.flat_at_end);
    }

    #[test]
    fn quantity_precision_is_applied_to_mark_to_market_pnl() {
        let mut engine = PaperEngine::new(anchors(), 100, 1_000, 300, 20, 0, 0, 2).unwrap();
        let feed_scaled = |engine: &mut PaperEngine, raw: &[u8]| {
            let event = parse_market_message(raw, 0, 2).unwrap();
            engine.on_event(event)
        };
        feed_scaled(
            &mut engine,
            br#"{"e":"markPriceUpdate","E":1,"s":"ABCUSDT","p":"100","i":"100","T":1000,"r":"0"}"#,
        );
        feed_scaled(
            &mut engine,
            br#"{"e":"bookTicker","u":1,"E":2,"T":2,"s":"ABCUSDT","b":"98","B":"10","a":"99","A":"10"}"#,
        );
        feed_scaled(
            &mut engine,
            br#"{"e":"aggTrade","E":3,"s":"ABCUSDT","a":1,"p":"98","q":"3","T":3,"m":true}"#,
        );
        let summary = engine.summary();
        assert_eq!(summary.current_absolute_position, 300);
        assert_eq!(summary.unrealized_pnl_ticks, 6);
    }

    #[test]
    fn replay_cancels_working_quotes_at_window_end_without_faking_a_fill() {
        let path = std::env::temp_dir().join("anchorbell-paper-replay-eof.jsonl");
        std::fs::write(
            &path,
            "{\"e\":\"markPriceUpdate\",\"E\":1,\"s\":\"ABCUSDT\",\"p\":\"100\",\"i\":\"100\",\"T\":1000,\"r\":\"0\"}\n{\"e\":\"bookTicker\",\"u\":1,\"E\":2,\"T\":2,\"s\":\"ABCUSDT\",\"b\":\"98\",\"B\":\"10\",\"a\":\"99\",\"A\":\"10\"}\n",
        )
        .unwrap();
        let result = replay_jsonl(&path, None, anchors(), 0, 0, 100, 100, 10, 20, 0, 0).unwrap();
        assert_eq!(result.order_count, 1);
        assert_eq!(result.fill_count, 0);
        assert_eq!(result.working_orders, 0);
        assert!(result.flat_at_end);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn paper_replay_rejects_symbols_without_configured_state() {
        let path = std::env::temp_dir().join("anchorbell-paper-replay-symbol.jsonl");
        std::fs::write(
            &path,
            "{\"e\":\"markPriceUpdate\",\"E\":1,\"s\":\"XYZUSDT\",\"p\":\"100\",\"i\":\"100\",\"T\":1000,\"r\":\"0\"}\n",
        )
        .unwrap();
        let result = replay_jsonl(&path, None, anchors(), 0, 0, 100, 100, 10, 20, 0, 0);
        assert!(matches!(
            result,
            Err(PaperError::ReplaySymbolNotConfigured(symbol)) if symbol == "XYZUSDT"
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn paper_replay_rejects_out_of_order_events() {
        let path = std::env::temp_dir().join("anchorbell-paper-replay-order.jsonl");
        std::fs::write(
            &path,
            "{\"e\":\"markPriceUpdate\",\"E\":2,\"s\":\"ABCUSDT\",\"p\":\"100\",\"i\":\"100\",\"T\":2,\"r\":\"0\"}\n{\"e\":\"markPriceUpdate\",\"E\":1,\"s\":\"ABCUSDT\",\"p\":\"100\",\"i\":\"100\",\"T\":1,\"r\":\"0\"}\n",
        )
        .unwrap();
        let result = replay_jsonl(&path, None, anchors(), 0, 0, 100, 100, 10, 20, 0, 0);
        assert!(matches!(result, Err(PaperError::ReplayOutOfOrder { .. })));
        let _ = std::fs::remove_file(path);
    }
}
