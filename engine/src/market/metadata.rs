use std::time::Duration;

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use thiserror::Error;

#[derive(Debug, Error)]
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
}
