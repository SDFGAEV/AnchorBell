//! Typed per-instrument market metadata.
use super::universe::{instrument_for, EquityRegion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentKind {
    OrdinaryEquity,
    LeveragedEtf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorCurrency {
    Cny,
    Hkd,
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentProfile {
    pub symbol: &'static str,
    pub region: EquityRegion,
    pub kind: InstrumentKind,
    pub anchor_currency: AnchorCurrency,
    pub timezone: &'static str,
    pub pre_open_minute: u16,
    pub regular_open_minute: u16,
    pub regular_close_minute: u16,
    pub final_close_minute: u16,
    pub has_midday_break: bool,
    pub require_final_close: bool,
    pub minimum_threshold_bps: u16,
}

const A_SHARE_PROFILES: &[InstrumentProfile] = &[
    InstrumentProfile {
        symbol: "CXMTUSDT",
        region: EquityRegion::AShare,
        kind: InstrumentKind::OrdinaryEquity,
        anchor_currency: AnchorCurrency::Cny,
        timezone: "Asia/Shanghai",
        pre_open_minute: 555,
        regular_open_minute: 570,
        regular_close_minute: 900,
        final_close_minute: 900,
        has_midday_break: true,
        require_final_close: true,
        minimum_threshold_bps: 50,
    },
    InstrumentProfile {
        symbol: "UNITREEUSDT",
        region: EquityRegion::AShare,
        kind: InstrumentKind::OrdinaryEquity,
        anchor_currency: AnchorCurrency::Cny,
        timezone: "Asia/Shanghai",
        pre_open_minute: 555,
        regular_open_minute: 570,
        regular_close_minute: 900,
        final_close_minute: 900,
        has_midday_break: true,
        require_final_close: true,
        minimum_threshold_bps: 50,
    },
];

const HONG_KONG_PROFILES: &[InstrumentProfile] = &[
    InstrumentProfile {
        symbol: "CSOPSAMSUNG2LUSDT",
        region: EquityRegion::HongKong,
        kind: InstrumentKind::LeveragedEtf,
        anchor_currency: AnchorCurrency::Hkd,
        timezone: "Asia/Hong_Kong",
        pre_open_minute: 540,
        regular_open_minute: 570,
        regular_close_minute: 960,
        final_close_minute: 970,
        has_midday_break: true,
        require_final_close: true,
        minimum_threshold_bps: 100,
    },
    InstrumentProfile {
        symbol: "CSOPSKHYNIX2LUSDT",
        region: EquityRegion::HongKong,
        kind: InstrumentKind::LeveragedEtf,
        anchor_currency: AnchorCurrency::Hkd,
        timezone: "Asia/Hong_Kong",
        pre_open_minute: 540,
        regular_open_minute: 570,
        regular_close_minute: 960,
        final_close_minute: 970,
        has_midday_break: true,
        require_final_close: true,
        minimum_threshold_bps: 100,
    },
];

pub fn profile_for(symbol: &str) -> Option<InstrumentProfile> {
    let instrument = instrument_for(symbol)?;
    if let Some(profile) = A_SHARE_PROFILES
        .iter()
        .chain(HONG_KONG_PROFILES.iter())
        .find(|profile| profile.symbol == instrument.symbol)
    {
        return Some(*profile);
    }
    Some(InstrumentProfile {
        symbol: instrument.symbol,
        region: instrument.region,
        kind: InstrumentKind::OrdinaryEquity,
        anchor_currency: AnchorCurrency::Hkd,
        timezone: "Asia/Hong_Kong",
        pre_open_minute: 540,
        regular_open_minute: 570,
        regular_close_minute: 960,
        final_close_minute: 970,
        has_midday_break: true,
        require_final_close: true,
        minimum_threshold_bps: 75,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_keep_a_share_and_hong_kong_metadata_distinct() {
        let a_share = profile_for("CXMTUSDT").unwrap();
        let hong_kong = profile_for("TENCENTUSDT").unwrap();
        assert_eq!(a_share.anchor_currency, AnchorCurrency::Cny);
        assert_eq!(a_share.pre_open_minute, 555);
        assert_eq!(hong_kong.anchor_currency, AnchorCurrency::Hkd);
        assert_eq!(hong_kong.pre_open_minute, 540);
    }

    #[test]
    fn leveraged_etfs_do_not_share_ordinary_equity_metadata() {
        assert_eq!(
            profile_for("CSOPSAMSUNG2LUSDT").unwrap().kind,
            InstrumentKind::LeveragedEtf
        );
        assert_eq!(
            profile_for("TENCENTUSDT").unwrap().kind,
            InstrumentKind::OrdinaryEquity
        );
    }

    #[test]
    fn unknown_symbols_fail_closed() {
        assert!(profile_for("BTCUSDT").is_none());
    }
}
