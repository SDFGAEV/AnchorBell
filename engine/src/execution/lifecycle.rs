//! Deterministic maker-order lifecycle.

use super::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    Submitted,
    Acknowledged,
    PartiallyFilled,
    Filled,
    CancelRequested,
    Canceled,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MakerOrder {
    pub client_id: u64,
    pub symbol: u32,
    pub side: Side,
    pub price: i64,
    pub quantity: i64,
    pub filled_quantity: i64,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    Submitted,
    Acknowledged,
    PartialFill { quantity: i64 },
    Filled { quantity: i64 },
    CancelRequested,
    Canceled,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    NotMaker,
    InvalidQuantity,
    InvalidTransition,
    FillExceedsOrder,
    IncompleteFinalFill,
    DuplicateTerminalEvent,
}

impl MakerOrder {
    pub fn new(
        client_id: u64,
        symbol: u32,
        side: Side,
        price: i64,
        quantity: i64,
        post_only: bool,
    ) -> Result<Self, LifecycleError> {
        if !post_only {
            return Err(LifecycleError::NotMaker);
        }
        if price <= 0 || quantity <= 0 {
            return Err(LifecycleError::InvalidQuantity);
        }
        Ok(Self {
            client_id,
            symbol,
            side,
            price,
            quantity,
            filled_quantity: 0,
            status: OrderStatus::New,
        })
    }

    pub fn apply(&mut self, event: LifecycleEvent) -> Result<(), LifecycleError> {
        match (self.status, event) {
            (OrderStatus::New, LifecycleEvent::Submitted) => {
                self.status = OrderStatus::Submitted;
                Ok(())
            }
            (OrderStatus::Submitted, LifecycleEvent::Acknowledged) => {
                self.status = OrderStatus::Acknowledged;
                Ok(())
            }
            (OrderStatus::Acknowledged, LifecycleEvent::PartialFill { quantity })
            | (OrderStatus::PartiallyFilled, LifecycleEvent::PartialFill { quantity }) => {
                self.record_fill(quantity, false)
            }
            (OrderStatus::Acknowledged, LifecycleEvent::Filled { quantity })
            | (OrderStatus::PartiallyFilled, LifecycleEvent::Filled { quantity }) => {
                self.record_fill(quantity, true)
            }
            (OrderStatus::Acknowledged, LifecycleEvent::CancelRequested)
            | (OrderStatus::PartiallyFilled, LifecycleEvent::CancelRequested) => {
                self.status = OrderStatus::CancelRequested;
                Ok(())
            }
            (OrderStatus::CancelRequested, LifecycleEvent::Canceled) => {
                self.status = OrderStatus::Canceled;
                Ok(())
            }
            (OrderStatus::Acknowledged, LifecycleEvent::Expired)
            | (OrderStatus::PartiallyFilled, LifecycleEvent::Expired)
            | (OrderStatus::CancelRequested, LifecycleEvent::Expired) => {
                self.status = OrderStatus::Expired;
                Ok(())
            }
            (OrderStatus::Submitted, LifecycleEvent::Rejected)
            | (OrderStatus::Acknowledged, LifecycleEvent::Rejected) => {
                self.status = OrderStatus::Rejected;
                Ok(())
            }
            (OrderStatus::Filled, _)
            | (OrderStatus::Canceled, _)
            | (OrderStatus::Expired, _)
            | (OrderStatus::Rejected, _) => Err(LifecycleError::DuplicateTerminalEvent),
            _ => Err(LifecycleError::InvalidTransition),
        }
    }

    fn record_fill(&mut self, quantity: i64, final_fill: bool) -> Result<(), LifecycleError> {
        if quantity <= 0 {
            return Err(LifecycleError::InvalidQuantity);
        }
        let filled = self
            .filled_quantity
            .checked_add(quantity)
            .ok_or(LifecycleError::FillExceedsOrder)?;
        if filled > self.quantity {
            return Err(LifecycleError::FillExceedsOrder);
        }
        if final_fill && filled != self.quantity {
            return Err(LifecycleError::IncompleteFinalFill);
        }
        self.filled_quantity = filled;
        if final_fill {
            if filled != self.quantity {
                return Err(LifecycleError::IncompleteFinalFill);
            }
            self.status = OrderStatus::Filled;
        } else if filled == self.quantity {
            self.status = OrderStatus::Filled;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> MakerOrder {
        MakerOrder::new(1, 7, Side::Buy, 100_000, 100, true).unwrap()
    }

    #[test]
    fn rejects_non_maker_orders_at_boundary() {
        assert_eq!(
            MakerOrder::new(1, 7, Side::Buy, 100, 1, false),
            Err(LifecycleError::NotMaker)
        );
    }

    #[test]
    fn follows_ack_fill_cancel_lifecycle() {
        let mut order = order();
        order.apply(LifecycleEvent::Submitted).unwrap();
        order.apply(LifecycleEvent::Acknowledged).unwrap();
        order
            .apply(LifecycleEvent::PartialFill { quantity: 25 })
            .unwrap();
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        order.apply(LifecycleEvent::CancelRequested).unwrap();
        order.apply(LifecycleEvent::Canceled).unwrap();
        assert_eq!(order.status, OrderStatus::Canceled);
        assert_eq!(order.filled_quantity, 25);
    }

    #[test]
    fn rejects_out_of_order_and_excess_fills() {
        let mut order = order();
        assert_eq!(
            order.apply(LifecycleEvent::Acknowledged),
            Err(LifecycleError::InvalidTransition)
        );
        order.apply(LifecycleEvent::Submitted).unwrap();
        order.apply(LifecycleEvent::Acknowledged).unwrap();
        assert_eq!(
            order.apply(LifecycleEvent::Filled { quantity: 101 }),
            Err(LifecycleError::FillExceedsOrder)
        );
    }

    #[test]
    fn incomplete_final_fill_is_rejected_without_mutation() {
        let mut order = order();
        order.apply(LifecycleEvent::Submitted).unwrap();
        order.apply(LifecycleEvent::Acknowledged).unwrap();
        assert_eq!(
            order.apply(LifecycleEvent::Filled { quantity: 25 }),
            Err(LifecycleError::IncompleteFinalFill)
        );
        assert_eq!(order.filled_quantity, 0);
        assert_eq!(order.status, OrderStatus::Acknowledged);
    }

    #[test]
    fn exact_fill_is_terminal_and_duplicate_events_fail() {
        let mut order = order();
        order.apply(LifecycleEvent::Submitted).unwrap();
        order.apply(LifecycleEvent::Acknowledged).unwrap();
        order
            .apply(LifecycleEvent::Filled { quantity: 100 })
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(
            order.apply(LifecycleEvent::Canceled),
            Err(LifecycleError::DuplicateTerminalEvent)
        );
    }
}
