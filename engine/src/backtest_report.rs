#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BacktestReport {
    pub event_count: u64,
    pub fill_count: u64,
    pub filled_quantity: i64,
    pub fees_ticks: i64,
    pub realized_pnl_ticks: i64,
    pub peak_absolute_position: i64,
    pub rejected_entries: u64,
}

impl BacktestReport {
    pub fn record_event(&mut self) {
        self.event_count = self.event_count.saturating_add(1);
    }

    pub fn record_fill(&mut self, quantity: i64, fee_ticks: i64, pnl_ticks: i64) {
        self.fill_count = self.fill_count.saturating_add(1);
        self.filled_quantity = self.filled_quantity.saturating_add(quantity);
        self.fees_ticks = self.fees_ticks.saturating_add(fee_ticks);
        self.realized_pnl_ticks = self.realized_pnl_ticks.saturating_add(pnl_ticks);
    }

    pub fn record_position(&mut self, position: i64) {
        let absolute = position.checked_abs().unwrap_or(i64::MAX);
        self.peak_absolute_position = self.peak_absolute_position.max(absolute);
    }

    pub fn record_rejected_entry(&mut self) {
        self.rejected_entries = self.rejected_entries.saturating_add(1);
    }

    pub fn net_pnl_ticks(&self) -> i64 {
        self.realized_pnl_ticks.saturating_sub(self.fees_ticks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_aggregates_fills_and_fees() {
        let mut report = BacktestReport::default();
        report.record_event();
        report.record_fill(3, 2, 10);
        report.record_position(-8);
        report.record_rejected_entry();
        assert_eq!(report.event_count, 1);
        assert_eq!(report.fill_count, 1);
        assert_eq!(report.net_pnl_ticks(), 8);
        assert_eq!(report.peak_absolute_position, 8);
        assert_eq!(report.rejected_entries, 1);
    }
}
