#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PnlBreakdown {
    pub realized_micros: i64,
    pub unrealized_micros: i64,
    pub funding_micros: i64,
    pub fees_micros: i64,
    pub external_adjustment_micros: i64,
    pub unknown_micros: i64,
}

impl PnlBreakdown {
    pub fn total_micros(self) -> i64 {
        self.realized_micros
            .saturating_add(self.unrealized_micros)
            .saturating_add(self.funding_micros)
            .saturating_sub(self.fees_micros)
            .saturating_add(self.external_adjustment_micros)
            .saturating_add(self.unknown_micros)
    }

    pub fn with_external_adjustment(mut self, delta_micros: i64) -> Self {
        self.external_adjustment_micros =
            self.external_adjustment_micros.saturating_add(delta_micros);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PnlSource {
    Trade,
    Funding,
    Fee,
    PositionMark,
    ExternalAdjustment,
    UnknownGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PnlObservation {
    pub observed_at_ms: u64,
    pub source: PnlSource,
    pub amount_micros: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PnlLedger {
    breakdown: PnlBreakdown,
    last_observed_at_ms: u64,
    authoritative: bool,
}

impl PnlLedger {
    pub fn breakdown(&self) -> PnlBreakdown {
        self.breakdown
    }

    pub fn last_observed_at_ms(&self) -> u64 {
        self.last_observed_at_ms
    }

    pub fn is_authoritative(&self) -> bool {
        self.authoritative
    }
    pub fn apply(&mut self, observation: PnlObservation) {
        if observation.observed_at_ms < self.last_observed_at_ms {
            return;
        }
        self.last_observed_at_ms = observation.observed_at_ms;
        match observation.source {
            PnlSource::Trade => {
                self.breakdown.realized_micros = self
                    .breakdown
                    .realized_micros
                    .saturating_add(observation.amount_micros);
            }
            PnlSource::Funding => {
                self.breakdown.funding_micros = self
                    .breakdown
                    .funding_micros
                    .saturating_add(observation.amount_micros);
            }
            PnlSource::Fee => {
                self.breakdown.fees_micros = self
                    .breakdown
                    .fees_micros
                    .saturating_add(observation.amount_micros);
            }
            PnlSource::PositionMark => {
                self.breakdown.unrealized_micros = observation.amount_micros;
            }
            PnlSource::ExternalAdjustment => {
                self.breakdown.external_adjustment_micros = self
                    .breakdown
                    .external_adjustment_micros
                    .saturating_add(observation.amount_micros);
            }
            PnlSource::UnknownGap => {
                self.breakdown.unknown_micros = self
                    .breakdown
                    .unknown_micros
                    .saturating_add(observation.amount_micros);
                self.authoritative = false;
            }
        }
    }

    pub fn apply_authoritative_snapshot(
        &mut self,
        observed_at_ms: u64,
        realized_micros: i64,
        unrealized_micros: i64,
        funding_micros: i64,
        fees_micros: i64,
    ) {
        self.breakdown.realized_micros = realized_micros;
        self.breakdown.unrealized_micros = unrealized_micros;
        self.breakdown.funding_micros = funding_micros;
        self.breakdown.fees_micros = fees_micros;
        self.breakdown.unknown_micros = 0;
        self.last_observed_at_ms = observed_at_ms;
        self.authoritative = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_is_explainable_and_fees_are_costs() {
        let mut ledger = PnlLedger::default();
        ledger.apply_authoritative_snapshot(10, 100, 50, 20, 5);
        assert_eq!(ledger.breakdown().total_micros(), 165);
        assert!(ledger.is_authoritative());
    }
    #[test]
    fn unknown_gap_blocks_authoritative_claim_until_snapshot() {
        let mut ledger = PnlLedger::default();
        ledger.apply(PnlObservation {
            observed_at_ms: 10,
            source: PnlSource::UnknownGap,
            amount_micros: 0,
        });
        assert!(!ledger.is_authoritative());
        ledger.apply_authoritative_snapshot(20, 1, 2, 3, 4);
        assert!(ledger.is_authoritative());
        assert_eq!(ledger.breakdown().unknown_micros, 0);
    }

    #[test]
    fn stale_observations_do_not_rewind_pnl() {
        let mut ledger = PnlLedger::default();
        ledger.apply(PnlObservation {
            observed_at_ms: 20,
            source: PnlSource::Trade,
            amount_micros: 10,
        });
        ledger.apply(PnlObservation {
            observed_at_ms: 10,
            source: PnlSource::Trade,
            amount_micros: 100,
        });
        assert_eq!(ledger.breakdown().realized_micros, 10);
    }
}
