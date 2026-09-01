use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteOrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteOrderSnapshot {
    pub client_order_id: String,
    pub symbol: String,
    pub status: RemoteOrderStatus,
    pub executed_quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOrderSnapshot {
    pub client_order_id: String,
    pub symbol: String,
    pub terminal: bool,
    pub executed_quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationInput {
    pub local_orders: Vec<LocalOrderSnapshot>,
    pub remote_orders: Vec<RemoteOrderSnapshot>,
    pub local_position: i64,
    pub remote_position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationAction {
    AdoptRemoteOrder(String),
    CancelRemoteOrder(String),
    ApplyRemoteFill { client_order_id: String, quantity: i64 },
    FlattenPosition { quantity: i64 },
    Halt { reason: &'static str },
    Continue,
}

pub fn reconcile(input: ReconciliationInput) -> Vec<ReconciliationAction> {
    if input.local_position != input.remote_position {
        return vec![ReconciliationAction::Halt {
            reason: "position mismatch requires operator-reviewed recovery",
        }];
    }

    let local_ids = input
        .local_orders
        .iter()
        .map(|order| order.client_order_id.as_str())
        .collect::<BTreeSet<_>>();
    let remote_ids = input
        .remote_orders
        .iter()
        .map(|order| order.client_order_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut actions = Vec::new();

    for id in remote_ids.difference(&local_ids) {
        actions.push(ReconciliationAction::CancelRemoteOrder((*id).to_string()));
    }
    for id in local_ids.difference(&remote_ids) {
        if !input
            .local_orders
            .iter()
            .any(|order| order.client_order_id == *id && order.terminal)
        {
            actions.push(ReconciliationAction::Halt {
                reason: "local non-terminal order is absent remotely",
            });
        }
    }
    for remote in &input.remote_orders {
        if let Some(local) = input
            .local_orders
            .iter()
            .find(|order| order.client_order_id == remote.client_order_id)
        {
            if remote.executed_quantity > local.executed_quantity {
                actions.push(ReconciliationAction::ApplyRemoteFill {
                    client_order_id: remote.client_order_id.clone(),
                    quantity: remote.executed_quantity - local.executed_quantity,
                });
            }
        }
    }
    if actions.is_empty() {
        actions.push(ReconciliationAction::Continue);
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ReconciliationInput {
        ReconciliationInput {
            local_orders: vec![LocalOrderSnapshot {
                client_order_id: "a".into(),
                symbol: "ABCUSDT".into(),
                terminal: false,
                executed_quantity: 2,
            }],
            remote_orders: vec![RemoteOrderSnapshot {
                client_order_id: "a".into(),
                symbol: "ABCUSDT".into(),
                status: RemoteOrderStatus::PartiallyFilled,
                executed_quantity: 5,
            }],
            local_position: 3,
            remote_position: 3,
        }
    }

    #[test]
    fn applies_remote_fill_delta_once() {
        assert_eq!(
            reconcile(input()),
            vec![ReconciliationAction::ApplyRemoteFill {
                client_order_id: "a".into(),
                quantity: 3,
            }]
        );
    }

    #[test]
    fn cancels_untracked_remote_orders() {
        let mut value = input();
        value.remote_orders.push(RemoteOrderSnapshot {
            client_order_id: "orphan".into(),
            symbol: "ABCUSDT".into(),
            status: RemoteOrderStatus::New,
            executed_quantity: 0,
        });
        assert!(reconcile(value)
            .contains(&ReconciliationAction::CancelRemoteOrder("orphan".into())));
    }

    #[test]
    fn position_mismatch_halts_before_order_actions() {
        let mut value = input();
        value.remote_position = 4;
        assert_eq!(
            reconcile(value),
            vec![ReconciliationAction::Halt {
                reason: "position mismatch requires operator-reviewed recovery",
            }]
        );
    }
}
