//! US equity session boundaries in exchange-local Eastern Time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsSessionState {
    Weekend,
    Holiday,
    Closed,
    PreOpenFlatten,
    PreMarket,
    RegularOpenAuction,
    RegularOpen,
    RegularCloseAuction,
    AfterHours,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsEquityCalendar {
    pub premarket_start_minute: u16,
    pub regular_open_minute: u16,
    pub regular_close_minute: u16,
    pub after_hours_end_minute: u16,
}

pub const US_EQUITY_CALENDAR: UsEquityCalendar = UsEquityCalendar {
    premarket_start_minute: 240,
    regular_open_minute: 570,
    regular_close_minute: 960,
    after_hours_end_minute: 1_200,
};
impl UsEquityCalendar {
    /// Uses Eastern local time; DST conversion belongs to the clock provider.
    pub fn state_at(
        &self,
        weekday: u8,
        local_minute: u16,
        holiday: bool,
        flatten_lead_minutes: u16,
    ) -> UsSessionState {
        if weekday == 0 || weekday > 7 || local_minute >= 1_440 {
            return UsSessionState::Unknown;
        }
        if weekday > 5 {
            return UsSessionState::Weekend;
        }
        if holiday {
            return UsSessionState::Holiday;
        }
        let flatten_start = self
            .premarket_start_minute
            .saturating_sub(flatten_lead_minutes);
        if local_minute >= flatten_start && local_minute < self.premarket_start_minute {
            return UsSessionState::PreOpenFlatten;
        }
        let opening_auction_start = self.regular_open_minute.saturating_sub(5);
        if local_minute >= self.premarket_start_minute && local_minute < opening_auction_start {
            return UsSessionState::PreMarket;
        }
        if local_minute >= opening_auction_start && local_minute < self.regular_open_minute {
            return UsSessionState::RegularOpenAuction;
        }
        if local_minute >= self.regular_open_minute && local_minute < self.regular_close_minute {
            return UsSessionState::RegularOpen;
        }
        if local_minute >= self.regular_close_minute && local_minute < self.after_hours_end_minute {
            return UsSessionState::AfterHours;
        }
        UsSessionState::Closed
    }

    pub fn static_anchor_entry_allowed(
        &self,
        weekday: u8,
        local_minute: u16,
        holiday: bool,
        flatten_lead_minutes: u16,
    ) -> bool {
        matches!(
            self.state_at(weekday, local_minute, holiday, flatten_lead_minutes),
            UsSessionState::Closed
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_premarket_and_after_hours_outside_static_anchor_window() {
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(1, 300, false, 30),
            UsSessionState::PreMarket
        );
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(1, 1_000, false, 30),
            UsSessionState::AfterHours
        );
        assert!(!US_EQUITY_CALENDAR.static_anchor_entry_allowed(1, 300, false, 30));
        assert!(US_EQUITY_CALENDAR.static_anchor_entry_allowed(1, 1_230, false, 30));
    }

    #[test]
    fn opens_are_fail_closed_for_auction_and_holidays() {
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(1, 230, false, 30),
            UsSessionState::PreOpenFlatten
        );
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(1, 568, false, 30),
            UsSessionState::RegularOpenAuction
        );
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(1, 600, true, 30),
            UsSessionState::Holiday
        );
        assert_eq!(
            US_EQUITY_CALENDAR.state_at(6, 600, false, 30),
            UsSessionState::Weekend
        );
    }
}
