use std::time::Duration;

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
    #[error("metadata snapshot has no complete two-sided quote")]
    IncompleteQuote,
    #[error("metadata snapshot contains a non-positive market value")]
    NonPositiveMarketValue,
    #[error("metadata snapshot contains an invalid funding rate")]
    InvalidFundingRate,
    #[error("metadata snapshot contains an expired funding time")]
    ExpiredFundingTime,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceSymbolSnapshot {
    pub metadata: BinanceSymbolMetadata,
    pub book_ticker: BinanceBookTickerSnapshot,
    pub premium_index: BinancePremiumIndexSnapshot,
}
impl BinanceSymbolMetadata {
    pub fn is_trading_tradifi_perpetual(&self) -> bool {
        self.status == "TRADING" && self.contract_type == "TRADIFI_PERPETUAL"
    }
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
        if self.metadata.symbol != self.book_ticker.symbol
            || self.metadata.symbol != self.premium_index.symbol
        {
            return Err(PublicMetadataError::SymbolMismatch);
        }
        if !self.metadata.is_trading_tradifi_perpetual() {
            return Err(PublicMetadataError::NotTradingTradFiPerpetual);
        }
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
        let mut builder = Client::builder()
            .no_proxy()
            .user_agent("AnchorBell/0.1 public-metadata")
            .timeout(Duration::from_secs(10));
        if let Some(proxy_url) = http_proxy {
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
