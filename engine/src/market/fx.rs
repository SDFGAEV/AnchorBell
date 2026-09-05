use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::{Deserialize, Serialize};
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

/// Configuration for the live public C2C FX poller.
///
/// The default runtime uses a thirty-second refresh and a two-minute stale
/// window. This feed is deliberately separate from the strategy's USDT
/// price path, so a delayed local-currency quote cannot silently change
/// exchange-order math.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxPollerConfig {
    pub refresh_interval_ms: u64,
    pub max_stale_ms: u64,
    pub max_backoff_ms: u64,
}

impl FxPollerConfig {
    pub const fn high_frequency() -> Self {
        Self {
            refresh_interval_ms: 30_000,
            max_stale_ms: 120_000,
            max_backoff_ms: 300_000,
        }
    }

    pub fn validate(self) -> Result<(), FxError> {
        if self.refresh_interval_ms == 0 || self.max_backoff_ms < self.refresh_interval_ms {
            return Err(FxError::InvalidRefreshInterval);
        }
        if self.max_stale_ms < self.refresh_interval_ms {
            return Err(FxError::InvalidStaleWindow);
        }
        Ok(())
    }
}

/// One timestamped observation emitted by the high-frequency FX feed.
#[derive(Debug, Clone, Serialize)]
pub struct FxUpdate {
    pub sequence: u64,
    pub observed_at_ms: u64,
    pub currency: String,
    pub buy_local_per_usdt_ppm: i64,
    pub sell_local_per_usdt_ppm: i64,
    pub midpoint_local_per_usdt_ppm: i64,
    pub source: String,
}

impl FxUpdate {
    fn from_quote(sequence: u64, quote: FxQuote) -> Self {
        Self {
            sequence,
            observed_at_ms: quote.observed_at_ms,
            currency: quote.currency.as_str().to_owned(),
            buy_local_per_usdt_ppm: quote.buy_local_per_usdt_ppm,
            sell_local_per_usdt_ppm: quote.sell_local_per_usdt_ppm,
            midpoint_local_per_usdt_ppm: quote.midpoint_local_per_usdt_ppm,
            source: quote.source.to_owned(),
        }
    }

    pub fn is_fresh_at(&self, now_ms: u64, max_stale_ms: u64) -> bool {
        now_ms >= self.observed_at_ms && now_ms.saturating_sub(self.observed_at_ms) <= max_stale_ms
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
    #[error("FX poll interval must be positive")]
    InvalidRefreshInterval,
    #[error("FX stale window must be at least the refresh interval")]
    InvalidStaleWindow,
    #[error("FX poller needs at least one supported currency")]
    NoCurrencies,
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
        let http_proxy = crate::network::resolve_http_proxy(http_proxy);
        let mut builder = Client::builder()
            .no_proxy()
            .user_agent("AnchorBell/0.1 public-fx")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10));
        if let Some(proxy_url) = http_proxy.as_deref() {
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
        let (buy, sell) = tokio::try_join!(self.quote(fiat, "BUY"), self.quote(fiat, "SELL"),)?;
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

    /// Fetches all required fiat quotes concurrently. The C2C endpoint
    /// exposes buy and sell prices separately, so each midpoint is made
    /// from a fresh two-sided observation rather than a hardcoded rate.
    pub async fn midpoints(&self, currencies: &[AnchorCurrency]) -> Result<Vec<FxQuote>, FxError> {
        let needs_cny = currencies.contains(&AnchorCurrency::Cny);
        let needs_hkd = currencies.contains(&AnchorCurrency::Hkd);
        if currencies.contains(&AnchorCurrency::Usd) {
            return Err(FxError::UnsupportedCurrency("USD"));
        }
        if !needs_cny && !needs_hkd {
            return Err(FxError::NoCurrencies);
        }
        match (needs_cny, needs_hkd) {
            (true, true) => {
                let (cny, hkd) = tokio::try_join!(
                    self.midpoint(AnchorCurrency::Cny),
                    self.midpoint(AnchorCurrency::Hkd)
                )?;
                Ok(vec![cny, hkd])
            }
            (true, false) => Ok(vec![self.midpoint(AnchorCurrency::Cny).await?]),
            (false, true) => Ok(vec![self.midpoint(AnchorCurrency::Hkd).await?]),
            (false, false) => Err(FxError::NoCurrencies),
        }
    }

    async fn quote(&self, fiat: &str, trade_type: &str) -> Result<i64, FxError> {
        super::metadata::pace_public_rest_request().await;
        let response = tokio::time::timeout(
            Duration::from_secs(12),
            self.client
                .get(format!(
                    "{}/bapi/c2c/v1/public/c2c/agent/quote-price",
                    self.base_url
                ))
                .query(&[("fiat", fiat), ("asset", "USDT"), ("tradeType", trade_type)])
                .send(),
        )
        .await
        .map_err(|_| FxError::Transport)?
        .map_err(|_| FxError::Transport)?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            super::metadata::note_public_rest_response(status, response.headers()).await;
            return Err(FxError::HttpStatus(status));
        }
        let envelope =
            tokio::time::timeout(Duration::from_secs(12), response.json::<QuoteEnvelope>())
                .await
                .map_err(|_| FxError::Transport)?
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

/// High-frequency public FX feed for the currencies used by the selected
/// TradFi instruments. It polls the public Binance C2C quote endpoint,
/// refreshes both sides concurrently, and backs off after transport or
/// response failures without blocking the market-data event loop.
pub struct BinanceC2cFxPoller {
    client: BinanceC2cFxClient,
    currencies: Vec<AnchorCurrency>,
    config: FxPollerConfig,
}

impl BinanceC2cFxPoller {
    pub fn new(
        client: BinanceC2cFxClient,
        requested_currencies: &[AnchorCurrency],
        config: FxPollerConfig,
    ) -> Result<Self, FxError> {
        config.validate()?;
        if requested_currencies.is_empty() {
            return Err(FxError::NoCurrencies);
        }
        let mut currencies = Vec::new();
        for currency in requested_currencies {
            if *currency == AnchorCurrency::Usd {
                return Err(FxError::UnsupportedCurrency("USD"));
            }
            if !currencies.contains(currency) {
                currencies.push(*currency);
            }
        }
        if currencies.is_empty() {
            return Err(FxError::NoCurrencies);
        }
        Ok(Self {
            client,
            currencies,
            config,
        })
    }

    pub async fn run(self, sender: tokio::sync::mpsc::Sender<FxUpdate>) -> Result<(), FxError> {
        let mut sequence = 0_u64;
        let mut delay_ms = self.config.refresh_interval_ms;
        loop {
            match self.client.midpoints(&self.currencies).await {
                Ok(quotes) => {
                    for quote in quotes {
                        sequence = sequence.saturating_add(1);
                        if sender
                            .send(FxUpdate::from_quote(sequence, quote))
                            .await
                            .is_err()
                        {
                            return Ok(());
                        }
                    }
                    delay_ms = self.config.refresh_interval_ms;
                }
                Err(error) => {
                    eprintln!("FX poll failed; retrying with backoff: {error}");
                    delay_ms = delay_ms.saturating_mul(2).min(self.config.max_backoff_ms);
                }
            }
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
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

    #[test]
    fn high_frequency_config_uses_safe_c2c_refresh_and_stale_window() {
        let config = FxPollerConfig::high_frequency();
        assert_eq!(config.refresh_interval_ms, 30_000);
        assert_eq!(config.max_stale_ms, 120_000);
        assert_eq!(config.max_backoff_ms, 300_000);
        assert!(config.validate().is_ok());
        assert!(FxPollerConfig {
            refresh_interval_ms: 0,
            ..config
        }
        .validate()
        .is_err());
        assert!(FxPollerConfig {
            max_stale_ms: 999,
            ..config
        }
        .validate()
        .is_err());
    }

    #[test]
    fn fx_update_freshness_is_explicit_and_inclusive_at_the_boundary() {
        let update = FxUpdate {
            sequence: 1,
            observed_at_ms: 10_000,
            currency: "CNY".to_owned(),
            buy_local_per_usdt_ppm: 6_660_000,
            sell_local_per_usdt_ppm: 6_670_000,
            midpoint_local_per_usdt_ppm: 6_665_000,
            source: "test".to_owned(),
        };
        assert!(update.is_fresh_at(15_000, 5_000));
        assert!(!update.is_fresh_at(15_001, 5_000));
        assert!(!update.is_fresh_at(9_999, 5_000));
    }
}
