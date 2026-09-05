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
    ApplyRemoteFill {
        client_order_id: String,
        quantity: i64,
    },
    ApplyRemoteStatus {
        client_order_id: String,
        status: RemoteOrderStatus,
    },
    FlattenPosition {
        quantity: i64,
    },
    AdoptRemotePosition {
        from: i64,
        to: i64,
    },
    RecordExternalAdjustment {
        delta: i64,
    },
    Halt {
        reason: &'static str,
    },
    Continue,
}

pub fn reconcile(input: ReconciliationInput) -> Vec<ReconciliationAction> {
    if has_duplicate_local_ids(&input.local_orders)
        || has_duplicate_remote_ids(&input.remote_orders)
        || input
            .local_orders
            .iter()
            .any(|order| order.executed_quantity < 0 || order.symbol.is_empty())
        || input
            .remote_orders
            .iter()
            .any(|order| order.executed_quantity < 0 || order.symbol.is_empty())
    {
        return vec![ReconciliationAction::Halt {
            reason: "order reconciliation snapshot is structurally invalid",
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
            if local.symbol != remote.symbol {
                actions.push(ReconciliationAction::Halt {
                    reason: "order identity has a symbol mismatch",
                });
                continue;
            }
            if local.executed_quantity < 0
                || remote.executed_quantity < 0
                || remote.executed_quantity < local.executed_quantity
            {
                actions.push(ReconciliationAction::Halt {
                    reason: "remote executed quantity regressed or is invalid",
                });
                continue;
            }
            if remote.executed_quantity > local.executed_quantity {
                actions.push(ReconciliationAction::ApplyRemoteFill {
                    client_order_id: remote.client_order_id.clone(),
                    quantity: remote.executed_quantity - local.executed_quantity,
                });
            }
            match remote.status {
                RemoteOrderStatus::Canceled
                | RemoteOrderStatus::Expired
                | RemoteOrderStatus::Rejected
                    if !local.terminal =>
                {
                    actions.push(ReconciliationAction::ApplyRemoteStatus {
                        client_order_id: remote.client_order_id.clone(),
                        status: remote.status,
                    });
                }
                RemoteOrderStatus::Filled if local.terminal => {
                    actions.push(ReconciliationAction::Halt {
                        reason: "remote filled order conflicts with a terminal local order",
                    });
                }
                RemoteOrderStatus::New | RemoteOrderStatus::PartiallyFilled if local.terminal => {
                    actions.push(ReconciliationAction::Halt {
                        reason: "remote active order conflicts with a terminal local order",
                    });
                }
                _ => {}
            }
        }
    }
    if input.local_position != input.remote_position {
        actions.push(ReconciliationAction::AdoptRemotePosition {
            from: input.local_position,
            to: input.remote_position,
        });
        actions.push(ReconciliationAction::RecordExternalAdjustment {
            delta: input.remote_position.saturating_sub(input.local_position),
        });
    }
    if actions.is_empty() {
        actions.push(ReconciliationAction::Continue);
    }
    actions
}

fn has_duplicate_local_ids(orders: &[LocalOrderSnapshot]) -> bool {
    orders
        .iter()
        .map(|order| order.client_order_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != orders.len()
}

fn has_duplicate_remote_ids(orders: &[RemoteOrderSnapshot]) -> bool {
    orders
        .iter()
        .map(|order| order.client_order_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != orders.len()
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
        assert!(
            reconcile(value).contains(&ReconciliationAction::CancelRemoteOrder("orphan".into()))
        );
    }

    #[test]
    fn position_mismatch_is_adopted_from_authoritative_snapshot() {
        let mut value = input();
        value.remote_position = 4;
        assert_eq!(
            reconcile(value),
            vec![
                ReconciliationAction::ApplyRemoteFill {
                    client_order_id: "a".into(),
                    quantity: 3,
                },
                ReconciliationAction::AdoptRemotePosition { from: 3, to: 4 },
                ReconciliationAction::RecordExternalAdjustment { delta: 1 },
            ]
        );
    }

    #[test]
    fn remote_terminal_status_is_explicitly_applied_to_local_order() {
        let mut value = input();
        value.remote_orders[0].status = RemoteOrderStatus::Canceled;
        assert_eq!(
            reconcile(value),
            vec![
                ReconciliationAction::ApplyRemoteFill {
                    client_order_id: "a".into(),
                    quantity: 3,
                },
                ReconciliationAction::ApplyRemoteStatus {
                    client_order_id: "a".into(),
                    status: RemoteOrderStatus::Canceled,
                }
            ]
        );
    }

    #[test]
    fn symbol_mismatch_halts_before_fill_or_continue() {
        let mut value = input();
        value.remote_orders[0].symbol = "OTHERUSDT".into();
        assert_eq!(
            reconcile(value),
            vec![ReconciliationAction::Halt {
                reason: "order identity has a symbol mismatch",
            }]
        );
    }

    #[test]
    fn executed_quantity_regression_halts() {
        let mut value = input();
        value.remote_orders[0].executed_quantity = 1;
        assert_eq!(
            reconcile(value),
            vec![ReconciliationAction::Halt {
                reason: "remote executed quantity regressed or is invalid",
            }]
        );
    }

    #[test]
    fn duplicate_client_ids_halt_instead_of_being_deduplicated() {
        let mut value = input();
        value.local_orders.push(value.local_orders[0].clone());
        assert_eq!(
            reconcile(value),
            vec![ReconciliationAction::Halt {
                reason: "order reconciliation snapshot is structurally invalid",
            }]
        );
    }

    #[test]
    fn invalid_orphan_snapshot_halts() {
        let mut value = input();
        value.remote_orders.push(RemoteOrderSnapshot {
            client_order_id: "orphan".into(),
            symbol: "ABCUSDT".into(),
            status: RemoteOrderStatus::New,
            executed_quantity: -1,
        });
        assert_eq!(
            reconcile(value),
            vec![ReconciliationAction::Halt {
                reason: "order reconciliation snapshot is structurally invalid",
            }]
        );
    }
}
