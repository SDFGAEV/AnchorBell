use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

use super::binance::parse_price_ticks;
use crate::strategy::AnchorCurrency;

const C2C_BASE_URL: &str = "https://www.binance.com";
const FX_SCALE: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxQuote {
    pub currency: AnchorCurrency,
    pub buy_local_per_usdt_ppm: i64,
    pub sell_local_per_usdt_ppm: i64,
    pub midpoint_local_per_usdt_ppm: i64,
    pub observed_at_ms: u64,
    pub source: &'static str,
}

impl FxQuote {
    pub fn convert_usdt_ticks_to_local(self, usdt_price_ticks: i64) -> Option<i64> {
        if usdt_price_ticks <= 0 || self.midpoint_local_per_usdt_ppm <= 0 {
            return None;
        }
        let converted = i128::from(usdt_price_ticks)
            .checked_mul(i128::from(self.midpoint_local_per_usdt_ppm))?
            .checked_add(500_000)?
            .checked_div(1_000_000)?;
        (converted > 0 && converted <= i128::from(i64::MAX)).then_some(converted as i64)
    }
}

#[derive(Debug, Error)]
pub enum FxError {
    #[error("unsupported fiat currency for Binance C2C: {0}")]
    UnsupportedCurrency(&'static str),
    #[error("invalid Binance C2C client: {0}")]
    Client(String),
    #[error("Binance C2C transport failed")]
    Transport,
    #[error("Binance C2C returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("Binance C2C response was invalid")]
    InvalidResponse,
    #[error("Binance C2C quote was invalid: {0}")]
    InvalidQuote(String),
}

#[derive(Debug, Deserialize)]
struct QuoteEnvelope {
    code: String,
    success: bool,
    data: Option<QuoteData>,
}

#[derive(Debug, Deserialize)]
struct QuoteData {
    asset: String,
    fiat: String,
    price: serde_json::Value,
}

pub struct BinanceC2cFxClient {
    client: Client,
    base_url: String,
}

impl BinanceC2cFxClient {
    pub fn new(http_proxy: Option<&str>) -> Result<Self, FxError> {
        let mut builder = Client::builder()
            .no_proxy()
            .user_agent("AnchorBell/0.1 public-fx")
            .timeout(Duration::from_secs(10));
        if let Some(proxy_url) = http_proxy {
            let proxy = reqwest::Proxy::all(proxy_url)
                .map_err(|error| FxError::Client(error.to_string()))?;
            builder = builder.proxy(proxy);
        }
        let client = builder
            .build()
            .map_err(|error| FxError::Client(error.to_string()))?;
        Ok(Self {
            client,
            base_url: C2C_BASE_URL.to_owned(),
        })
    }

    pub async fn midpoint(&self, currency: AnchorCurrency) -> Result<FxQuote, FxError> {
        let fiat = fiat_code(currency)?;
        let buy = self.quote(fiat, "BUY").await?;
        let sell = self.quote(fiat, "SELL").await?;
        let midpoint = i128::from(buy)
            .checked_add(i128::from(sell))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| FxError::InvalidQuote("midpoint overflow".to_owned()))?;
        let midpoint = i64::try_from(midpoint)
            .map_err(|_| FxError::InvalidQuote("midpoint does not fit i64".to_owned()))?;
        Ok(FxQuote {
            currency,
            buy_local_per_usdt_ppm: buy,
            sell_local_per_usdt_ppm: sell,
            midpoint_local_per_usdt_ppm: midpoint,
            observed_at_ms: now_ms(),
            source: "binance_c2c_midpoint",
        })
    }

    async fn quote(&self, fiat: &str, trade_type: &str) -> Result<i64, FxError> {
        let response = self
            .client
            .get(format!(
                "{}/bapi/c2c/v1/public/c2c/agent/quote-price",
                self.base_url
            ))
            .query(&[("fiat", fiat), ("asset", "USDT"), ("tradeType", trade_type)])
            .send()
            .await
            .map_err(|_| FxError::Transport)?;
        if !response.status().is_success() {
            return Err(FxError::HttpStatus(response.status().as_u16()));
        }
        let envelope = response
            .json::<QuoteEnvelope>()
            .await
            .map_err(|_| FxError::InvalidResponse)?;
        if !envelope.success || envelope.code != "000000" {
            return Err(FxError::InvalidResponse);
        }
        let data = envelope.data.ok_or(FxError::InvalidResponse)?;
        if data.asset != "USDT" || data.fiat != fiat {
            return Err(FxError::InvalidResponse);
        }
        let price = match data.price {
            serde_json::Value::String(value) => value,
            value => value.to_string(),
        };
        let ticks = parse_price_ticks(&price, FX_SCALE)
            .map_err(|error| FxError::InvalidQuote(format!("{error:?}")))?;
        if ticks.0 <= 0 {
            return Err(FxError::InvalidQuote("price must be positive".to_owned()));
        }
        Ok(ticks.0)
    }
}

fn fiat_code(currency: AnchorCurrency) -> Result<&'static str, FxError> {
    match currency {
        AnchorCurrency::Cny => Ok("CNY"),
        AnchorCurrency::Hkd => Ok("HKD"),
        AnchorCurrency::Usd => Err(FxError::UnsupportedCurrency("USD")),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_millis() as u64
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_usdt_index_price_to_local_ticks_by_multiplication() {
        let quote = FxQuote {
            currency: AnchorCurrency::Cny,
            buy_local_per_usdt_ppm: 6_650_000,
            sell_local_per_usdt_ppm: 6_670_000,
            midpoint_local_per_usdt_ppm: 6_660_000,
            observed_at_ms: 1,
            source: "test",
        };
        assert_eq!(
            quote.convert_usdt_ticks_to_local(808_921_162),
            Some(5_387_414_939)
        );
    }

    #[test]
    fn rejects_non_positive_or_overflowing_conversion() {
        let quote = FxQuote {
            currency: AnchorCurrency::Hkd,
            buy_local_per_usdt_ppm: 7_940_000,
            sell_local_per_usdt_ppm: 7_950_000,
            midpoint_local_per_usdt_ppm: 7_945_000,
            observed_at_ms: 1,
            source: "test",
        };
        assert_eq!(quote.convert_usdt_ticks_to_local(0), None);
        assert_eq!(quote.convert_usdt_ticks_to_local(-1), None);
        assert_eq!(quote.convert_usdt_ticks_to_local(i64::MAX), None);
    }
}
