//! M8 Funding-Aware Robust Anchor Control.
//! Pure strategy math: no exchange, simulator, or persistence dependency.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FundingAction {
    NoAction,
    Collect,
    Tolerate,
    Avoid,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FundingRateStatus {
    Observed,
    Missing,
    Stale,
    Special,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct M8Input {
    pub now_ms: u64,
    pub anchor_ticks: i64,
    pub mid_ticks: i64,
    pub mark_ticks: i64,
    pub index_ticks: i64,
    pub position: i64,
    pub max_position: i64,
    pub funding_rate_e8: Option<i64>,
    pub next_funding_ms: Option<u64>,
    pub funding_rate_status: FundingRateStatus,
    pub fee_ppm: i64,
    pub volatility_bps: i64,
    pub spread_bps: i64,
    pub model_uncertainty_bps: i64,
    pub liquidation_buffer_bps: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct M8Decision {
    pub action: FundingAction,
    pub allow_entry: bool,
    pub reduce_only: bool,
    pub side: i8,
    pub anchor_edge_bps: i64,
    pub funding_carry_bps: i64,
    pub total_cost_bps: i64,
    pub net_edge_bps: i64,
    pub safety_margin_bps: i64,
    pub reason: &'static str,
}

fn bps(num: i64, den: i64) -> i64 {
    if num <= 0 || den <= 0 {
        return 0;
    }
    ((i128::from(num) * 10_000) / i128::from(den)).clamp(0, i128::from(i64::MAX)) as i64
}

fn signed_edge(anchor: i64, mid: i64) -> (i8, i64) {
    if mid < anchor {
        (1, bps(anchor - mid, mid))
    } else if mid > anchor {
        (-1, bps(mid - anchor, mid))
    } else {
        (0, 0)
    }
}

fn funding_carry(side: i8, rate_e8: Option<i64>) -> i64 {
    rate_e8
        .map(|rate| -(i64::from(side)) * rate / 10_000)
        .unwrap_or(0)
}

pub fn decide(input: M8Input) -> M8Decision {
    let zero = M8Decision {
        action: FundingAction::NoAction,
        allow_entry: false,
        reduce_only: false,
        side: 0,
        anchor_edge_bps: 0,
        funding_carry_bps: 0,
        total_cost_bps: i64::MAX,
        net_edge_bps: i64::MIN,
        safety_margin_bps: 0,
        reason: "invalid_or_unknown_state",
    };
    if input.anchor_ticks <= 0
        || input.mid_ticks <= 0
        || input.mark_ticks <= 0
        || input.index_ticks <= 0
        || input.max_position <= 0
        || input.fee_ppm < 0
        || input.volatility_bps < 0
        || input.spread_bps < 0
        || input.model_uncertainty_bps < 0
        || input.liquidation_buffer_bps < 0
    {
        return zero;
    }
    if input.funding_rate_status == FundingRateStatus::Unknown
        || input.funding_rate_status == FundingRateStatus::Missing
        || input.next_funding_ms.is_none()
    {
        return zero;
    }
    let (signal_side, anchor_edge) = signed_edge(input.anchor_ticks, input.mid_ticks);
    let held_side = input.position.signum() as i8;
    let side = if held_side != 0 {
        held_side
    } else {
        signal_side
    };
    let carry = funding_carry(side, input.funding_rate_e8);
    let costs = ((input.fee_ppm.saturating_mul(2) + 99) / 100)
        .saturating_add(input.volatility_bps)
        .saturating_add(input.spread_bps / 2)
        .saturating_add(input.model_uncertainty_bps)
        .saturating_add(input.liquidation_buffer_bps);
    let net = anchor_edge.saturating_add(carry).saturating_sub(costs);
    let remaining = input
        .next_funding_ms
        .unwrap_or(0)
        .saturating_sub(input.now_ms);
    let near_funding = remaining <= 5 * 60 * 1_000;
    let safety = costs.saturating_add(5);
    if input.position != 0 && (net <= 0 || input.funding_rate_status == FundingRateStatus::Special)
    {
        return M8Decision {
            action: FundingAction::Exit,
            allow_entry: false,
            reduce_only: true,
            side,
            anchor_edge_bps: anchor_edge,
            funding_carry_bps: carry,
            total_cost_bps: costs,
            net_edge_bps: net,
            safety_margin_bps: safety,
            reason: "held_edge_not_compensating_funding_or_special",
        };
    }
    if signal_side == 0 || anchor_edge <= 0 {
        return M8Decision {
            action: FundingAction::NoAction,
            allow_entry: false,
            reduce_only: false,
            side,
            anchor_edge_bps: anchor_edge,
            funding_carry_bps: carry,
            total_cost_bps: costs,
            net_edge_bps: net,
            safety_margin_bps: safety,
            reason: "anchor_edge_not_observable",
        };
    }
    if near_funding && carry > 0 && net > safety {
        return M8Decision {
            action: FundingAction::Collect,
            allow_entry: true,
            reduce_only: false,
            side,
            anchor_edge_bps: anchor_edge,
            funding_carry_bps: carry,
            total_cost_bps: costs,
            net_edge_bps: net,
            safety_margin_bps: safety,
            reason: "carry_and_anchor_edge_cover_tail_cost",
        };
    }
    if net > safety {
        return M8Decision {
            action: FundingAction::Tolerate,
            allow_entry: true,
            reduce_only: false,
            side,
            anchor_edge_bps: anchor_edge,
            funding_carry_bps: carry,
            total_cost_bps: costs,
            net_edge_bps: net,
            safety_margin_bps: safety,
            reason: "anchor_edge_covers_funding_and_cost",
        };
    }
    M8Decision {
        action: FundingAction::Avoid,
        allow_entry: false,
        reduce_only: input.position != 0,
        side,
        anchor_edge_bps: anchor_edge,
        funding_carry_bps: carry,
        total_cost_bps: costs,
        net_edge_bps: net,
        safety_margin_bps: safety,
        reason: "funding_or_tail_cost_exceeds_conservative_edge",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input(rate: Option<i64>) -> M8Input {
        M8Input {
            now_ms: 0,
            anchor_ticks: 100_000,
            mid_ticks: 99_000,
            mark_ticks: 99_000,
            index_ticks: 99_000,
            position: 0,
            max_position: 1_000,
            funding_rate_e8: rate,
            next_funding_ms: Some(60_000),
            funding_rate_status: FundingRateStatus::Observed,
            fee_ppm: 4,
            volatility_bps: 1,
            spread_bps: 1,
            model_uncertainty_bps: 1,
            liquidation_buffer_bps: 1,
        }
    }
    #[test]
    fn positive_short_funding_can_be_collected_with_edge() {
        let d = decide(input(Some(-20_000)));
        assert!(d.allow_entry);
        assert_eq!(d.action, FundingAction::Collect);
    }
    #[test]
    fn unknown_rate_fails_closed() {
        let mut x = input(None);
        x.funding_rate_status = FundingRateStatus::Missing;
        assert_eq!(decide(x).action, FundingAction::NoAction);
    }
    #[test]
    fn zero_funding_does_not_create_a_deadline_exit_when_edge_remains() {
        let d = decide(input(Some(0)));
        assert_eq!(d.action, FundingAction::Tolerate);
        assert!(d.allow_entry);
        assert_ne!(d.reason, "held_edge_not_compensating_funding_or_special");
    }
    #[test]
    fn expensive_carry_is_avoided() {
        let mut x = input(Some(200_000));
        x.mid_ticks = 99_950;
        assert_eq!(decide(x).action, FundingAction::Avoid);
    }
}
