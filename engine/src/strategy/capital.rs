//! Strategy-layer dynamic capital allocation.
//!
//! This module only converts current risk observations into target capital
//! weights. It has no order-book, fill, latency, or exchange dependencies.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalRiskInput {
    pub risk_bps: i64,
    pub eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalWeight {
    pub weight_bps: i64,
    pub risk_bps: i64,
}

const TOTAL_WEIGHT_BPS: i64 = 10_000;

/// Allocate capital inversely to conservative risk scores, with explicit
/// floors/caps. Ineligible symbols keep the floor so a transient data issue
/// cannot cause a large re-entry when the feed recovers.
pub fn dynamic_weights(
    inputs: &BTreeMap<String, CapitalRiskInput>,
    min_weight_bps: i64,
    max_weight_bps: i64,
) -> Result<BTreeMap<String, CapitalWeight>, &'static str> {
    if inputs.is_empty()
        || min_weight_bps < 0
        || max_weight_bps < min_weight_bps
        || max_weight_bps > TOTAL_WEIGHT_BPS
        || i128::from(min_weight_bps) * inputs.len() as i128 > i128::from(TOTAL_WEIGHT_BPS)
        || i128::from(max_weight_bps) * (inputs.len() as i128) < i128::from(TOTAL_WEIGHT_BPS)
    {
        return Err("dynamic capital weight bounds are invalid");
    }
    if !inputs.values().any(|input| input.eligible) {
        return Err("dynamic capital has no eligible symbols");
    }

    let mut weights = inputs
        .keys()
        .map(|symbol| (symbol.clone(), min_weight_bps))
        .collect::<BTreeMap<_, _>>();
    let mut active = inputs
        .iter()
        .filter(|(_, input)| input.eligible)
        .map(|(symbol, _)| symbol.clone())
        .collect::<Vec<_>>();
    let score = |symbol: &String| -> i128 {
        let input = inputs.get(symbol).expect("active symbol exists");
        1_000_000_i128 / i128::from(input.risk_bps.max(1))
    };
    let mut remaining = TOTAL_WEIGHT_BPS - weights.values().copied().sum::<i64>();

    while remaining > 0 && !active.is_empty() {
        let total_score = active.iter().map(&score).sum::<i128>().max(1);
        let mut assigned = 0_i64;
        let mut capped = Vec::new();
        for symbol in &active {
            let capacity = max_weight_bps - weights[symbol];
            if capacity <= 0 {
                capped.push(symbol.clone());
                continue;
            }
            let share = if active.len() == 1 {
                remaining
            } else {
                (i128::from(remaining) * score(symbol) / total_score) as i64
            };
            let add = share.max(0).min(capacity);
            if add > 0 {
                *weights.get_mut(symbol).expect("weight exists") += add;
                assigned += add;
            }
            if weights[symbol] >= max_weight_bps {
                capped.push(symbol.clone());
            }
        }
        if assigned == 0 {
            for symbol in &active {
                if remaining == 0 {
                    break;
                }
                if weights[symbol] < max_weight_bps {
                    *weights.get_mut(symbol).expect("weight exists") += 1;
                    remaining -= 1;
                }
            }
        } else {
            remaining -= assigned;
        }
        active.retain(|symbol| !capped.contains(symbol) && weights[symbol] < max_weight_bps);
    }
    if remaining != 0 {
        return Err("dynamic capital could not satisfy weight bounds");
    }

    Ok(inputs
        .iter()
        .map(|(symbol, input)| {
            (
                symbol.clone(),
                CapitalWeight {
                    weight_bps: weights[symbol],
                    risk_bps: input.risk_bps.max(1),
                },
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_risk_receives_more_weight_without_breaking_caps() {
        let inputs = [
            (
                "A".to_owned(),
                CapitalRiskInput {
                    risk_bps: 10,
                    eligible: true,
                },
            ),
            (
                "B".to_owned(),
                CapitalRiskInput {
                    risk_bps: 20,
                    eligible: true,
                },
            ),
            (
                "C".to_owned(),
                CapitalRiskInput {
                    risk_bps: 100,
                    eligible: true,
                },
            ),
        ]
        .into_iter()
        .collect();
        let result = dynamic_weights(&inputs, 500, 6_000).unwrap();
        assert_eq!(result.values().map(|v| v.weight_bps).sum::<i64>(), 10_000);
        assert!(result["A"].weight_bps > result["B"].weight_bps);
        assert!(result["B"].weight_bps > result["C"].weight_bps);
        assert!(result.values().all(|v| v.weight_bps <= 6_000));
    }

    #[test]
    fn ineligible_symbol_is_kept_at_floor() {
        let inputs = [
            (
                "A".to_owned(),
                CapitalRiskInput {
                    risk_bps: 10,
                    eligible: true,
                },
            ),
            (
                "B".to_owned(),
                CapitalRiskInput {
                    risk_bps: 10,
                    eligible: false,
                },
            ),
        ]
        .into_iter()
        .collect();
        let result = dynamic_weights(&inputs, 500, 9_500).unwrap();
        assert_eq!(result["B"].weight_bps, 500);
        assert_eq!(result.values().map(|v| v.weight_bps).sum::<i64>(), 10_000);
    }
}
