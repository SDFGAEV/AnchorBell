use crate::execution::{OrderIntent, Side};

use super::{decide_adaptive_signal, AdaptiveThreshold, SignalDecision, SignalInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    BuyMaker,
    SellMaker,
    NoAction,
}

#[derive(Debug, Clone, Copy)]
pub struct AnchorMakerStrategy {
    pub entry_threshold_bps: i64,
    pub exit_threshold_bps: i64,
}

impl AnchorMakerStrategy {
    pub fn new(entry_threshold_bps: i64, exit_threshold_bps: i64) -> Self {
        Self {
            entry_threshold_bps,
            exit_threshold_bps,
        }
    }

    pub fn generate_intent(
        &self,
        symbol: u32,
        bid: i64,
        ask: i64,
        index_price: i64,
        quantity: i64,
    ) -> Option<OrderIntent> {
        if index_price == 0 {
            return None;
        }

        let mid = (bid + ask) / 2;
        let deviation_bps = (mid - index_price) * 10000 / index_price;

        if deviation_bps <= -self.entry_threshold_bps {
            Some(OrderIntent {
                symbol,
                side: Side::Buy,
                price: bid,
                quantity,
                post_only: true,
            })
        } else if deviation_bps >= self.entry_threshold_bps {
            Some(OrderIntent {
                symbol,
                side: Side::Sell,
                price: ask,
                quantity,
                post_only: true,
            })
        } else {
            None
        }
    }

    /// Routes live, replay, paper, and backtest callers through the
    /// cost-aware admission contract.
    pub fn generate_adaptive_intent(input: SignalInput) -> Option<OrderIntent> {
        match decide_adaptive_signal(input) {
            SignalDecision::BuyMaker { price, quantity } => Some(OrderIntent {
                symbol: input.symbol,
                side: Side::Buy,
                price: price.0,
                quantity,
                post_only: true,
            }),
            SignalDecision::SellMaker { price, quantity } => Some(OrderIntent {
                symbol: input.symbol,
                side: Side::Sell,
                price: price.0,
                quantity,
                post_only: true,
            }),
            SignalDecision::Blocked(_) => None,
        }
    }

    pub fn default_adaptive_threshold(&self, floor_bps: i64) -> Option<AdaptiveThreshold> {
        if floor_bps < 0 {
            return None;
        }
        Some(AdaptiveThreshold {
            floor_bps,
            residual_volatility_bps: 0,
            cost_bps: 0,
            uncertainty_bps: 0,
            deadline_risk_bps: 0,
            safety_margin_bps: 0,
        })
    }
}
