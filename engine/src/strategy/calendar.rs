//! Exchange-local session calendars for the two supported equity regions.

use super::universe::EquityRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWindow {
    pub open_minute: u16,
    pub close_minute: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VenueSessionState {
    Weekend,
    Holiday,
    Closed,
    PreOpenFlatten,
    PreOpenAuction,
    Open,
    MiddayBreak,
    ClosingAuction,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquitySessionCalendar {
    pub region: EquityRegion,
    pub windows: &'static [SessionWindow],
}

const A_SHARE_WINDOWS: &[SessionWindow] = &[
    SessionWindow {
        open_minute: 570,
        close_minute: 690,
    },
    SessionWindow {
        open_minute: 780,
        close_minute: 900,
    },
];

const HONG_KONG_WINDOWS: &[SessionWindow] = &[
    SessionWindow {
        open_minute: 570,
        close_minute: 720,
    },
    SessionWindow {
        open_minute: 780,
        close_minute: 960,
    },
];

pub const A_SHARE_CALENDAR: EquitySessionCalendar = EquitySessionCalendar {
    region: EquityRegion::AShare,
    windows: A_SHARE_WINDOWS,
};

pub const HONG_KONG_CALENDAR: EquitySessionCalendar = EquitySessionCalendar {
    region: EquityRegion::HongKong,
    windows: HONG_KONG_WINDOWS,
};

pub fn calendar_for(region: EquityRegion) -> EquitySessionCalendar {
    match region {
        EquityRegion::AShare => A_SHARE_CALENDAR,
        EquityRegion::HongKong => HONG_KONG_CALENDAR,
    }
}
impl EquitySessionCalendar {
    /// Classifies a minute in China/Hong Kong local time.
    ///
    /// weekday is ISO weekday 1..=7 and local_minute is 0..=1439.
    /// Holidays are intentionally supplied by a separate calendar provider.
    pub fn state_at(
        &self,
        weekday: u8,
        local_minute: u16,
        flatten_lead_minutes: u16,
    ) -> VenueSessionState {
        if weekday == 0 || weekday > 7 || local_minute >= 1_440 {
            return VenueSessionState::Unknown;
        }
        if weekday > 5 {
            return VenueSessionState::Weekend;
        }

        for window in self.windows {
            if local_minute >= window.open_minute && local_minute < window.close_minute {
                return VenueSessionState::Open;
            }
            let flatten_start = window.open_minute.saturating_sub(flatten_lead_minutes);
            if local_minute >= flatten_start && local_minute < window.open_minute {
                return VenueSessionState::PreOpenFlatten;
            }
        }
        VenueSessionState::Closed
    }

    /// Classifies exchange events that are hidden by the compact legacy API.
    /// holiday is supplied by an authoritative exchange calendar provider.
    pub fn detailed_state_at(
        &self,
        weekday: u8,
        local_minute: u16,
        holiday: bool,
        flatten_lead_minutes: u16,
        closing_auction: bool,
    ) -> VenueSessionState {
        if weekday == 0 || weekday > 7 || local_minute >= 1_440 {
            return VenueSessionState::Unknown;
        }
        if weekday > 5 {
            return VenueSessionState::Weekend;
        }
        if holiday {
            return VenueSessionState::Holiday;
        }

        let (pre_open, auction_start, auction_end, midday_start, midday_end): (
            u16,
            u16,
            u16,
            u16,
            u16,
        ) = match self.region {
            EquityRegion::AShare => (555, 897, 900, 690, 780),
            EquityRegion::HongKong => (540, 960, 970, 720, 780),
        };
        if local_minute >= pre_open.saturating_sub(flatten_lead_minutes) && local_minute < pre_open
        {
            return VenueSessionState::PreOpenFlatten;
        }
        if local_minute >= pre_open && local_minute < self.windows[0].open_minute {
            return VenueSessionState::PreOpenAuction;
        }
        if local_minute >= midday_start && local_minute < midday_end {
            return VenueSessionState::MiddayBreak;
        }
        if closing_auction && local_minute >= auction_start && local_minute < auction_end {
            return VenueSessionState::ClosingAuction;
        }
        if self
            .windows
            .iter()
            .any(|window| local_minute >= window.open_minute && local_minute < window.close_minute)
        {
            return VenueSessionState::Open;
        }
        VenueSessionState::Closed
    }

    pub fn entry_allowed_detailed(
        &self,
        weekday: u8,
        local_minute: u16,
        holiday: bool,
        flatten_lead_minutes: u16,
        allow_midday_break: bool,
        closing_auction: bool,
    ) -> bool {
        match self.detailed_state_at(
            weekday,
            local_minute,
            holiday,
            flatten_lead_minutes,
            closing_auction,
        ) {
            VenueSessionState::Closed => true,
            VenueSessionState::MiddayBreak => allow_midday_break,
            _ => false,
        }
    }

    pub fn entry_allowed(&self, weekday: u8, local_minute: u16, flatten_lead_minutes: u16) -> bool {
        self.state_at(weekday, local_minute, flatten_lead_minutes) == VenueSessionState::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_calendar_has_distinct_morning_and_afternoon_windows() {
        assert_eq!(
            A_SHARE_CALENDAR.state_at(1, 600, 30),
            VenueSessionState::Open
        );
        assert_eq!(
            A_SHARE_CALENDAR.state_at(1, 750, 30),
            VenueSessionState::PreOpenFlatten
        );
        assert_eq!(
            A_SHARE_CALENDAR.state_at(1, 720, 30),
            VenueSessionState::Closed
        );
    }

    #[test]
    fn hong_kong_calendar_has_a_longer_morning_session() {
        assert_eq!(
            HONG_KONG_CALENDAR.state_at(1, 700, 30),
            VenueSessionState::Open
        );
        assert_eq!(
            HONG_KONG_CALENDAR.state_at(1, 750, 30),
            VenueSessionState::PreOpenFlatten
        );
        assert_eq!(
            HONG_KONG_CALENDAR.state_at(1, 735, 30),
            VenueSessionState::Closed
        );
    }

    #[test]
    fn weekend_and_invalid_clock_values_fail_closed() {
        assert_eq!(
            A_SHARE_CALENDAR.state_at(6, 600, 30),
            VenueSessionState::Weekend
        );
        assert_eq!(
            A_SHARE_CALENDAR.state_at(0, 600, 30),
            VenueSessionState::Unknown
        );
        assert_eq!(
            A_SHARE_CALENDAR.state_at(1, 1_440, 30),
            VenueSessionState::Unknown
        );
        assert!(!A_SHARE_CALENDAR.entry_allowed(1, 570, 30));
    }

    #[test]
    fn detailed_rules_expose_auction_and_midday_breaks() {
        assert_eq!(
            A_SHARE_CALENDAR.detailed_state_at(1, 555, false, 30, true),
            VenueSessionState::PreOpenAuction
        );
        assert_eq!(
            A_SHARE_CALENDAR.detailed_state_at(1, 700, false, 30, true),
            VenueSessionState::MiddayBreak
        );
        assert_eq!(
            HONG_KONG_CALENDAR.detailed_state_at(1, 965, false, 30, true),
            VenueSessionState::ClosingAuction
        );
        assert!(!A_SHARE_CALENDAR.entry_allowed_detailed(1, 700, false, 30, false, true));
        assert!(A_SHARE_CALENDAR.entry_allowed_detailed(1, 700, false, 30, true, true));
        assert!(!A_SHARE_CALENDAR.entry_allowed_detailed(1, 600, true, 30, true, true));
    }
}
