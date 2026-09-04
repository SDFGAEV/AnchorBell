use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicMetadataError {
    #[error("invalid HTTP proxy configuration")]
    InvalidProxy,
    #[error("public metadata client construction failed")]
    ClientBuild,
    #[error("public metadata transport failed")]
    Transport,
    #[error("public metadata endpoint returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("public metadata response could not be decoded")]
    Decode,
    #[error("symbol metadata is not present in exchangeInfo: {0}")]
    SymbolNotFound(String),
    #[error("metadata snapshot has inconsistent symbols")]
    SymbolMismatch,
    #[error("symbol is not a trading TradFi perpetual")]
    NotTradingTradFiPerpetual,
    #[error("required exchange filter is missing: {0}")]
    MissingExchangeFilter(&'static str),
    #[error("exchange filter is invalid: {filter}.{field}")]
    InvalidExchangeFilter {
        filter: &'static str,
        field: &'static str,
    },
    #[error("metadata snapshot has no complete two-sided quote")]
    IncompleteQuote,
    #[error("metadata snapshot contains a non-positive market value")]
    NonPositiveMarketValue,
    #[error("metadata snapshot contains an invalid funding rate")]
    InvalidFundingRate,
    #[error("metadata snapshot contains an expired funding time")]
    ExpiredFundingTime,
    #[error("metadata snapshot is stale or has a future observation timestamp")]
    StaleSnapshot,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceSymbolFilter {
    #[serde(rename = "filterType")]
    pub filter_type: String,
    #[serde(rename = "minPrice")]
    pub min_price: Option<String>,
    #[serde(rename = "maxPrice")]
    pub max_price: Option<String>,
    #[serde(rename = "tickSize")]
    pub tick_size: Option<String>,
    #[serde(rename = "minQty")]
    pub min_quantity: Option<String>,
    #[serde(rename = "maxQty")]
    pub max_quantity: Option<String>,
    #[serde(rename = "stepSize")]
    pub step_size: Option<String>,
    pub notional: Option<String>,
    #[serde(rename = "multiplierUp")]
    pub multiplier_up: Option<String>,
    #[serde(rename = "multiplierDown")]
    pub multiplier_down: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceExecutionFilters {
    pub min_price: String,
    pub max_price: String,
    pub price_tick: String,
    pub min_quantity: String,
    pub max_quantity: String,
    pub quantity_step: String,
    pub min_notional: String,
    pub multiplier_up: String,
    pub multiplier_down: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceSymbolMetadata {
    pub symbol: String,
    pub status: String,
    #[serde(rename = "contractType")]
    pub contract_type: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
    #[serde(rename = "marginAsset")]
    pub margin_asset: String,
    #[serde(rename = "pricePrecision")]
    pub price_precision: u32,
    #[serde(rename = "quantityPrecision")]
    pub quantity_precision: u32,
    #[serde(rename = "onboardDate")]
    pub onboard_date_ms: u64,
    #[serde(rename = "deliveryDate")]
    pub delivery_date_ms: u64,
    pub filters: Vec<BinanceSymbolFilter>,
}
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct ExchangeInfoWire {
    symbols: Vec<BinanceSymbolMetadata>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceBookTickerSnapshot {
    pub symbol: String,
    #[serde(rename = "bidPrice")]
    pub bid_price: String,
    #[serde(rename = "bidQty")]
    pub bid_quantity: String,
    #[serde(rename = "askPrice")]
    pub ask_price: String,
    #[serde(rename = "askQty")]
    pub ask_quantity: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinanceDepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinancePremiumIndexSnapshot {
    pub symbol: String,
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "lastFundingRate")]
    pub last_funding_rate: String,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time_ms: u64,
}

pub const PUBLIC_SNAPSHOT_MAX_AGE_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceSymbolSnapshot {
    pub metadata: BinanceSymbolMetadata,
    pub book_ticker: BinanceBookTickerSnapshot,
    pub premium_index: BinancePremiumIndexSnapshot,
    pub observed_at_ms: u64,
}
impl BinanceSymbolMetadata {
    pub fn is_trading_tradifi_perpetual(&self) -> bool {
        self.status == "TRADING" && self.contract_type == "TRADIFI_PERPETUAL"
    }

    /// Extracts the filters required for a passive limit order.
    ///
    /// The exchange's precision fields are display hints; these filters are
    /// the authoritative order-admission contract.
    pub fn execution_filters(&self) -> Result<BinanceExecutionFilters, PublicMetadataError> {
        let price = self.required_filter("PRICE_FILTER")?;
        let lot = self.required_filter("LOT_SIZE")?;
        let notional = self.required_filter("MIN_NOTIONAL")?;
        let percent = self.required_filter("PERCENT_PRICE")?;
        let values = BinanceExecutionFilters {
            min_price: required_field(price, "PRICE_FILTER", "minPrice")?,
            max_price: required_field(price, "PRICE_FILTER", "maxPrice")?,
            price_tick: required_field(price, "PRICE_FILTER", "tickSize")?,
            min_quantity: required_field(lot, "LOT_SIZE", "minQty")?,
            max_quantity: required_field(lot, "LOT_SIZE", "maxQty")?,
            quantity_step: required_field(lot, "LOT_SIZE", "stepSize")?,
            min_notional: required_field(notional, "MIN_NOTIONAL", "notional")?,
            multiplier_up: required_field(percent, "PERCENT_PRICE", "multiplierUp")?,
            multiplier_down: required_field(percent, "PERCENT_PRICE", "multiplierDown")?,
        };
        for (filter, field, value) in [
            ("PRICE_FILTER", "minPrice", &values.min_price),
            ("PRICE_FILTER", "maxPrice", &values.max_price),
            ("PRICE_FILTER", "tickSize", &values.price_tick),
            ("LOT_SIZE", "minQty", &values.min_quantity),
            ("LOT_SIZE", "maxQty", &values.max_quantity),
            ("LOT_SIZE", "stepSize", &values.quantity_step),
            ("MIN_NOTIONAL", "notional", &values.min_notional),
            ("PERCENT_PRICE", "multiplierUp", &values.multiplier_up),
            ("PERCENT_PRICE", "multiplierDown", &values.multiplier_down),
        ] {
            if !is_positive_decimal(value) {
                return Err(PublicMetadataError::InvalidExchangeFilter { filter, field });
            }
        }
        Ok(values)
    }

    fn required_filter(
        &self,
        filter_type: &'static str,
    ) -> Result<&BinanceSymbolFilter, PublicMetadataError> {
        self.filters
            .iter()
            .find(|filter| filter.filter_type == filter_type)
            .ok_or(PublicMetadataError::MissingExchangeFilter(filter_type))
    }
}

fn required_field(
    filter: &BinanceSymbolFilter,
    filter_name: &'static str,
    field: &'static str,
) -> Result<String, PublicMetadataError> {
    let value = match field {
        "minPrice" => filter.min_price.as_ref(),
        "maxPrice" => filter.max_price.as_ref(),
        "tickSize" => filter.tick_size.as_ref(),
        "minQty" => filter.min_quantity.as_ref(),
        "maxQty" => filter.max_quantity.as_ref(),
        "stepSize" => filter.step_size.as_ref(),
        "notional" => filter.notional.as_ref(),
        "multiplierUp" => filter.multiplier_up.as_ref(),
        "multiplierDown" => filter.multiplier_down.as_ref(),
        _ => None,
    };
    value
        .cloned()
        .ok_or(PublicMetadataError::InvalidExchangeFilter {
            filter: filter_name,
            field,
        })
}

impl BinanceBookTickerSnapshot {
    pub fn has_two_sided_quote(&self) -> bool {
        !self.bid_price.is_empty()
            && !self.ask_price.is_empty()
            && !self.bid_quantity.is_empty()
            && !self.ask_quantity.is_empty()
    }
}

impl BinanceSymbolSnapshot {
    /// Validates the public snapshot before it can enter a live decision path.
    /// This is a data-quality gate, not a profitability signal.
    pub fn validate_for_runtime(&self, now_ms: u64) -> Result<(), PublicMetadataError> {
        if self.observed_at_ms > now_ms
            || now_ms.saturating_sub(self.observed_at_ms) > PUBLIC_SNAPSHOT_MAX_AGE_MS
        {
            return Err(PublicMetadataError::StaleSnapshot);
        }
        if self.metadata.symbol != self.book_ticker.symbol
            || self.metadata.symbol != self.premium_index.symbol
        {
            return Err(PublicMetadataError::SymbolMismatch);
        }
        if !self.metadata.is_trading_tradifi_perpetual() {
            return Err(PublicMetadataError::NotTradingTradFiPerpetual);
        }
        self.metadata.execution_filters()?;
        if !self.book_ticker.has_two_sided_quote()
            || !is_positive_decimal(&self.book_ticker.bid_price)
            || !is_positive_decimal(&self.book_ticker.ask_price)
            || !is_positive_decimal(&self.book_ticker.bid_quantity)
            || !is_positive_decimal(&self.book_ticker.ask_quantity)
            || !is_positive_decimal(&self.premium_index.mark_price)
            || !is_positive_decimal(&self.premium_index.index_price)
        {
            return Err(if self.book_ticker.has_two_sided_quote() {
                PublicMetadataError::NonPositiveMarketValue
            } else {
                PublicMetadataError::IncompleteQuote
            });
        }
        if !is_signed_decimal(&self.premium_index.last_funding_rate) {
            return Err(PublicMetadataError::InvalidFundingRate);
        }
        if self.premium_index.next_funding_time_ms != 0
            && self.premium_index.next_funding_time_ms <= now_ms
        {
            return Err(PublicMetadataError::ExpiredFundingTime);
        }
        Ok(())
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_millis() as u64
}

fn is_positive_decimal(value: &str) -> bool {
    !value.starts_with('-')
        && is_signed_decimal(value)
        && value
            .bytes()
            .any(|byte| byte.is_ascii_digit() && byte != b'0')
}

fn is_signed_decimal(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

pub struct PublicMarketMetadataClient {
    rest_base: String,
    client: Client,
}

impl PublicMarketMetadataClient {
    pub fn new(rest_base: &str, http_proxy: Option<&str>) -> Result<Self, PublicMetadataError> {
        let http_proxy = crate::network::resolve_http_proxy(http_proxy);
        let mut builder = Client::builder()
            .no_proxy()
            .user_agent("AnchorBell/0.1 public-metadata")
            .timeout(Duration::from_secs(10));
        if let Some(proxy_url) = http_proxy.as_deref() {
            let proxy =
                reqwest::Proxy::all(proxy_url).map_err(|_| PublicMetadataError::InvalidProxy)?;
            builder = builder.proxy(proxy);
        }
        let client = builder
            .build()
            .map_err(|_| PublicMetadataError::ClientBuild)?;
        Ok(Self {
            rest_base: rest_base.trim_end_matches('/').to_owned(),
            client,
        })
    }
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, PublicMetadataError> {
        let response = self
            .client
            .get(format!("{}{}", self.rest_base, path))
            .send()
            .await
            .map_err(|_| PublicMetadataError::Transport)?;
        if !response.status().is_success() {
            return Err(PublicMetadataError::HttpStatus {
                status: response.status().as_u16(),
            });
        }
        response
            .json::<T>()
            .await
            .map_err(|_| PublicMetadataError::Decode)
    }

    pub async fn exchange_info(&self) -> Result<Vec<BinanceSymbolMetadata>, PublicMetadataError> {
        Ok(self
            .get_json::<ExchangeInfoWire>("/fapi/v1/exchangeInfo")
            .await?
            .symbols)
    }

    /// Fetches a REST depth snapshot used to seed a sequence-validated local
    /// order book before consuming the websocket diff stream.
    pub async fn depth_snapshot(
        &self,
        symbol: &str,
        limit: usize,
    ) -> Result<BinanceDepthSnapshot, PublicMetadataError> {
        let symbol = symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() || !symbol.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(PublicMetadataError::SymbolNotFound(symbol));
        }
        let limit = limit.clamp(5, 1_000);
        self.get_json::<BinanceDepthSnapshot>(&format!(
            "/fapi/v1/depth?symbol={symbol}&limit={limit}"
        ))
        .await
    }

    /// Fetches several symbol snapshots with bounded concurrency while
    /// preserving the input order. The bound protects the public REST rate
    /// limit when the execution universe grows.
    pub async fn symbol_snapshots(
        &self,
        metadata: Vec<BinanceSymbolMetadata>,
        max_concurrency: usize,
    ) -> Vec<Result<BinanceSymbolSnapshot, PublicMetadataError>> {
        let concurrency = max_concurrency.max(1);
        stream::iter(metadata.into_iter().map(|metadata| {
            let symbol = metadata.symbol.clone();
            async move { self.symbol_snapshot(&symbol, metadata).await }
        }))
        .buffered(concurrency)
        .collect()
        .await
    }

    pub async fn symbol_snapshot(
        &self,
        symbol: &str,
        metadata: BinanceSymbolMetadata,
    ) -> Result<BinanceSymbolSnapshot, PublicMetadataError> {
        let symbol = symbol.trim().to_ascii_uppercase();
        if metadata.symbol != symbol {
            return Err(PublicMetadataError::SymbolNotFound(symbol));
        }
        let book_ticker = self
            .get_json::<BinanceBookTickerSnapshot>(&format!(
                "/fapi/v1/ticker/bookTicker?symbol={symbol}"
            ))
            .await?;
        let premium_index = self
            .get_json::<BinancePremiumIndexSnapshot>(&format!(
                "/fapi/v1/premiumIndex?symbol={symbol}"
            ))
            .await?;
        Ok(BinanceSymbolSnapshot {
            metadata,
            book_ticker,
            premium_index,
            observed_at_ms: current_time_ms(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> BinanceSymbolMetadata {
        BinanceSymbolMetadata {
            symbol: "CXMTUSDT".into(),
            status: "TRADING".into(),
            contract_type: "TRADIFI_PERPETUAL".into(),
            base_asset: "CXMT".into(),
            quote_asset: "USDT".into(),
            margin_asset: "USDT".into(),
            price_precision: 5,
            quantity_precision: 2,
            onboard_date_ms: 1,
            delivery_date_ms: 4_102_444_800_000,
            filters: vec![
                BinanceSymbolFilter {
                    filter_type: "PRICE_FILTER".into(),
                    min_price: Some("0.001".into()),
                    max_price: Some("20000".into()),
                    tick_size: Some("0.001".into()),
                    min_quantity: None,
                    max_quantity: None,
                    step_size: None,
                    notional: None,
                    multiplier_up: None,
                    multiplier_down: None,
                },
                BinanceSymbolFilter {
                    filter_type: "LOT_SIZE".into(),
                    min_price: None,
                    max_price: None,
                    tick_size: None,
                    min_quantity: Some("0.01".into()),
                    max_quantity: Some("400000".into()),
                    step_size: Some("0.01".into()),
                    notional: None,
                    multiplier_up: None,
                    multiplier_down: None,
                },
                BinanceSymbolFilter {
                    filter_type: "MIN_NOTIONAL".into(),
                    min_price: None,
                    max_price: None,
                    tick_size: None,
                    min_quantity: None,
                    max_quantity: None,
                    step_size: None,
                    notional: Some("5".into()),
                    multiplier_up: None,
                    multiplier_down: None,
                },
                BinanceSymbolFilter {
                    filter_type: "PERCENT_PRICE".into(),
                    min_price: None,
                    max_price: None,
                    tick_size: None,
                    min_quantity: None,
                    max_quantity: None,
                    step_size: None,
                    notional: None,
                    multiplier_up: Some("1.03".into()),
                    multiplier_down: Some("0.97".into()),
                },
            ],
        }
    }

    #[test]
    fn only_trading_tradifi_perpetual_is_runtime_eligible() {
        assert!(metadata().is_trading_tradifi_perpetual());
        let mut inactive = metadata();
        inactive.status = "BREAK".into();
        assert!(!inactive.is_trading_tradifi_perpetual());
    }

    #[test]
    fn two_sided_quote_requires_all_fields() {
        let snapshot = BinanceBookTickerSnapshot {
            symbol: "CXMTUSDT".into(),
            bid_price: "8.27800".into(),
            bid_quantity: "22.46".into(),
            ask_price: "8.28100".into(),
            ask_quantity: "14.18".into(),
        };
        assert!(snapshot.has_two_sided_quote());
    }

    fn snapshot() -> BinanceSymbolSnapshot {
        BinanceSymbolSnapshot {
            metadata: metadata(),
            book_ticker: BinanceBookTickerSnapshot {
                symbol: "CXMTUSDT".into(),
                bid_price: "8.27800".into(),
                bid_quantity: "22.46".into(),
                ask_price: "8.28100".into(),
                ask_quantity: "14.18".into(),
            },
            observed_at_ms: 900,
            premium_index: BinancePremiumIndexSnapshot {
                symbol: "CXMTUSDT".into(),
                mark_price: "8.27900".into(),
                index_price: "8.27850".into(),
                last_funding_rate: "-0.00010000".into(),
                next_funding_time_ms: 2_000,
            },
        }
    }

    #[test]
    fn execution_filters_extract_authoritative_order_contract() {
        let filters = metadata()
            .execution_filters()
            .expect("fixture filters are valid");
        assert_eq!(filters.price_tick, "0.001");
        assert_eq!(filters.quantity_step, "0.01");
        assert_eq!(filters.min_notional, "5");
        assert_eq!(filters.multiplier_up, "1.03");
    }

    #[test]
    fn execution_filters_fail_closed_when_required_filter_is_missing() {
        let mut value = metadata();
        value
            .filters
            .retain(|filter| filter.filter_type != "MIN_NOTIONAL");
        assert_eq!(
            value.execution_filters(),
            Err(PublicMetadataError::MissingExchangeFilter("MIN_NOTIONAL"))
        );
    }

    #[test]
    fn execution_filters_fail_closed_when_required_value_is_invalid() {
        let mut value = metadata();
        value.filters[0].tick_size = Some("0".into());
        assert_eq!(
            value.execution_filters(),
            Err(PublicMetadataError::InvalidExchangeFilter {
                filter: "PRICE_FILTER",
                field: "tickSize"
            })
        );
    }

    #[test]
    fn runtime_validation_accepts_complete_fresh_snapshot() {
        assert!(snapshot().validate_for_runtime(1_000).is_ok());
    }

    #[test]
    fn runtime_validation_rejects_zero_market_values() {
        let mut invalid = snapshot();
        invalid.book_ticker.bid_quantity = "0".into();
        assert_eq!(
            invalid.validate_for_runtime(1_000),
            Err(PublicMetadataError::NonPositiveMarketValue)
        );
    }

    #[test]
    fn runtime_validation_rejects_stale_snapshot() {
        let mut invalid = snapshot();
        invalid.observed_at_ms = 1;
        assert_eq!(
            invalid.validate_for_runtime(PUBLIC_SNAPSHOT_MAX_AGE_MS + 2),
            Err(PublicMetadataError::StaleSnapshot)
        );
    }

    #[test]
    fn runtime_validation_rejects_future_snapshot_timestamp() {
        let mut invalid = snapshot();
        invalid.observed_at_ms = 1_001;
        assert_eq!(
            invalid.validate_for_runtime(1_000),
            Err(PublicMetadataError::StaleSnapshot)
        );
    }

    #[test]
    fn runtime_validation_rejects_expired_funding_time() {
        let mut invalid = snapshot();
        invalid.premium_index.next_funding_time_ms = 1_000;
        assert_eq!(
            invalid.validate_for_runtime(1_000),
            Err(PublicMetadataError::ExpiredFundingTime)
        );
    }

    #[test]
    fn runtime_validation_rejects_symbol_mismatch() {
        let mut invalid = snapshot();
        invalid.premium_index.symbol = "OTHERUSDT".into();
        assert_eq!(
            invalid.validate_for_runtime(1_000),
            Err(PublicMetadataError::SymbolMismatch)
        );
    }
}
