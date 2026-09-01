//! The AnchorBell TradFi catalog and hard ADR eligibility boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquityRegion {
    AShare,
    HongKong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdrStatus {
    /// The issuer has an active or historical ADR/ADS program.
    ConfirmedPresent,
    /// The instrument/issuer was checked and no ADR/ADS was found.
    ConfirmedAbsent,
    /// Evidence is incomplete; this status is never tradable.
    Unknown,
}

impl AdrStatus {
    pub const fn is_tradable(self) -> bool {
        matches!(self, Self::ConfirmedAbsent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradFiInstrument {
    pub symbol: &'static str,
    pub region: EquityRegion,
    pub adr_status: AdrStatus,
}

pub const A_SHARE_INSTRUMENTS: &[TradFiInstrument] = &[
    TradFiInstrument {
        symbol: "CXMTUSDT",
        region: EquityRegion::AShare,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "UNITREEUSDT",
        region: EquityRegion::AShare,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
];

pub const HONG_KONG_INSTRUMENTS: &[TradFiInstrument] = &[
    // HK-listed leveraged products are not ADR/ADS instruments themselves.
    TradFiInstrument {
        symbol: "CSOPSAMSUNG2LUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "CSOPSKHYNIX2LUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "GIGADEVUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "HK0625USDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "HK0700USDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "HK1810USDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "KUAISHOUUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "MEITUANUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "MINIMAXUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "POPMARTUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "TENCENTUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedPresent,
    },
    TradFiInstrument {
        symbol: "ZHIPUUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
    TradFiInstrument {
        symbol: "ZHONGJIUSDT",
        region: EquityRegion::HongKong,
        adr_status: AdrStatus::ConfirmedAbsent,
    },
];

/// Returns the complete reviewed catalog, including hard-excluded instruments.
pub fn catalog_instruments() -> impl Iterator<Item = &'static TradFiInstrument> {
    A_SHARE_INSTRUMENTS
        .iter()
        .chain(HONG_KONG_INSTRUMENTS.iter())
}

/// Returns only instruments that satisfy the current hard execution policy.
pub fn all_instruments() -> impl Iterator<Item = &'static TradFiInstrument> {
    catalog_instruments().filter(|instrument| instrument.adr_status.is_tradable())
}

pub fn catalog_instrument_for(symbol: &str) -> Option<TradFiInstrument> {
    let normalized = symbol.trim().to_ascii_uppercase();
    catalog_instruments()
        .find(|instrument| instrument.symbol == normalized)
        .copied()
}

pub fn instrument_for(symbol: &str) -> Option<TradFiInstrument> {
    catalog_instrument_for(symbol).filter(|instrument| instrument.adr_status.is_tradable())
}

pub fn adr_excluded_instruments() -> impl Iterator<Item = &'static TradFiInstrument> {
    catalog_instruments()
        .filter(|instrument| matches!(instrument.adr_status, AdrStatus::ConfirmedPresent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_reviewed_catalog_from_execution_universe() {
        assert_eq!(catalog_instruments().count(), 15);
        assert_eq!(all_instruments().count(), 9);
        assert_eq!(adr_excluded_instruments().count(), 6);
    }

    #[test]
    fn maps_a_share_instruments_without_adr_exclusion() {
        assert_eq!(
            instrument_for(" cxmtusdt "),
            Some(TradFiInstrument {
                symbol: "CXMTUSDT",
                region: EquityRegion::AShare,
                adr_status: AdrStatus::ConfirmedAbsent,
            })
        );
        assert_eq!(
            instrument_for("UNITREEUSDT").map(|instrument| instrument.region),
            Some(EquityRegion::AShare)
        );
    }

    #[test]
    fn rejects_all_hong_kong_issuers_with_adr_or_ads() {
        for symbol in [
            "HK0700USDT",
            "HK1810USDT",
            "KUAISHOUUSDT",
            "MEITUANUSDT",
            "POPMARTUSDT",
            "TENCENTUSDT",
        ] {
            assert_eq!(instrument_for(symbol), None, "{symbol} must be rejected");
            assert_eq!(
                catalog_instrument_for(symbol).map(|instrument| instrument.adr_status),
                Some(AdrStatus::ConfirmedPresent)
            );
        }
    }

    #[test]
    fn unknown_and_external_symbols_fail_closed() {
        assert_eq!(instrument_for("BTCUSDT"), None);
        assert_eq!(instrument_for(""), None);
        assert!(!AdrStatus::Unknown.is_tradable());
    }

    #[test]
    fn execution_universe_contains_no_adr_instrument() {
        assert!(all_instruments().all(|instrument| instrument.adr_status.is_tradable()));
        assert!(A_SHARE_INSTRUMENTS
            .iter()
            .all(|a_share| HONG_KONG_INSTRUMENTS
                .iter()
                .all(|hong_kong| a_share.symbol != hong_kong.symbol)));
    }
}
