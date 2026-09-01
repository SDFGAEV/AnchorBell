//! The deliberately small AnchorBell TradFi execution universe.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquityRegion {
    AShare,
    HongKong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradFiInstrument {
    pub symbol: &'static str,
    pub region: EquityRegion,
}

pub const A_SHARE_INSTRUMENTS: &[TradFiInstrument] = &[
    TradFiInstrument {
        symbol: "CXMTUSDT",
        region: EquityRegion::AShare,
    },
    TradFiInstrument {
        symbol: "UNITREEUSDT",
        region: EquityRegion::AShare,
    },
];

pub const HONG_KONG_INSTRUMENTS: &[TradFiInstrument] = &[
    TradFiInstrument {
        symbol: "CSOPSAMSUNG2LUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "CSOPSKHYNIX2LUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "GIGADEVUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "HK0625USDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "HK0700USDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "HK1810USDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "KUAISHOUUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "MEITUANUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "MINIMAXUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "POPMARTUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "TENCENTUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "ZHIPUUSDT",
        region: EquityRegion::HongKong,
    },
    TradFiInstrument {
        symbol: "ZHONGJIUSDT",
        region: EquityRegion::HongKong,
    },
];

pub fn all_instruments() -> impl Iterator<Item = &'static TradFiInstrument> {
    A_SHARE_INSTRUMENTS
        .iter()
        .chain(HONG_KONG_INSTRUMENTS.iter())
}

pub fn instrument_for(symbol: &str) -> Option<TradFiInstrument> {
    let normalized = symbol.trim().to_ascii_uppercase();
    all_instruments()
        .find(|instrument| instrument.symbol == normalized)
        .copied()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_only_the_confirmed_fifteen_instruments() {
        assert_eq!(A_SHARE_INSTRUMENTS.len(), 2);
        assert_eq!(HONG_KONG_INSTRUMENTS.len(), 13);
        assert_eq!(all_instruments().count(), 15);
    }

    #[test]
    fn maps_the_two_a_share_instruments() {
        assert_eq!(
            instrument_for(" cxmtusdt "),
            Some(TradFiInstrument {
                symbol: "CXMTUSDT",
                region: EquityRegion::AShare,
            })
        );
        assert_eq!(
            instrument_for("UNITREEUSDT").map(|instrument| instrument.region),
            Some(EquityRegion::AShare)
        );
    }

    #[test]
    fn keeps_hong_kong_instruments_in_their_own_region() {
        assert_eq!(
            instrument_for("tencentusdt").map(|instrument| instrument.region),
            Some(EquityRegion::HongKong)
        );
        assert!(A_SHARE_INSTRUMENTS
            .iter()
            .all(|a_share| HONG_KONG_INSTRUMENTS
                .iter()
                .all(|hong_kong| a_share.symbol != hong_kong.symbol)));
    }

    #[test]
    fn rejects_symbols_outside_the_confirmed_universe() {
        assert_eq!(instrument_for("BTCUSDT"), None);
        assert_eq!(instrument_for(""), None);
    }
}
