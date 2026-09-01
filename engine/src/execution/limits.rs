use super::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderLimits {
    pub min_price_ticks: i64,
    pub max_price_ticks: i64,
    pub price_tick: i64,
    pub min_quantity: i64,
    pub max_quantity: i64,
    pub quantity_step: i64,
    pub min_notional_ticks: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitError {
    NonPositive,
    PriceOutOfRange,
    PriceNotOnTick,
    QuantityOutOfRange,
    QuantityNotOnStep,
    NotionalTooSmall,
    WouldTakeLiquidity,
}

impl OrderLimits {
    pub fn validate(
        &self,
        side: Side,
        price_ticks: i64,
        quantity: i64,
        best_bid_ticks: i64,
        best_ask_ticks: i64,
    ) -> Result<(), LimitError> {
        if price_ticks <= 0 || quantity <= 0 {
            return Err(LimitError::NonPositive);
        }
        if price_ticks < self.min_price_ticks || price_ticks > self.max_price_ticks {
            return Err(LimitError::PriceOutOfRange);
        }
        if self.price_tick <= 0 || price_ticks % self.price_tick != 0 {
            return Err(LimitError::PriceNotOnTick);
        }
        if quantity < self.min_quantity || quantity > self.max_quantity {
            return Err(LimitError::QuantityOutOfRange);
        }
        if self.quantity_step <= 0 || quantity % self.quantity_step != 0 {
            return Err(LimitError::QuantityNotOnStep);
        }
        if price_ticks.saturating_mul(quantity) < self.min_notional_ticks {
            return Err(LimitError::NotionalTooSmall);
        }
        match side {
            Side::Buy if price_ticks >= best_ask_ticks => Err(LimitError::WouldTakeLiquidity),
            Side::Sell if price_ticks <= best_bid_ticks => Err(LimitError::WouldTakeLiquidity),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> OrderLimits {
        OrderLimits {
            min_price_ticks: 1,
            max_price_ticks: 1_000_000,
            price_tick: 5,
            min_quantity: 10,
            max_quantity: 1_000,
            quantity_step: 5,
            min_notional_ticks: 1_000,
        }
    }

    #[test]
    fn accepts_passive_order_on_exchange_filters() {
        assert_eq!(limits().validate(Side::Buy, 995, 10, 990, 1_000), Ok(()));
    }

    #[test]
    fn rejects_taker_price_before_transport() {
        assert_eq!(
            limits().validate(Side::Buy, 1_000, 10, 990, 1_000),
            Err(LimitError::WouldTakeLiquidity)
        );
        assert_eq!(
            limits().validate(Side::Sell, 990, 10, 990, 1_000),
            Err(LimitError::WouldTakeLiquidity)
        );
    }

    #[test]
    fn rejects_invalid_tick_lot_and_notional() {
        assert_eq!(
            limits().validate(Side::Buy, 997, 10, 990, 1_000),
            Err(LimitError::PriceNotOnTick)
        );
        assert_eq!(
            limits().validate(Side::Buy, 995, 11, 990, 1_000),
            Err(LimitError::QuantityNotOnStep)
        );
        assert_eq!(
            OrderLimits {
                min_notional_ticks: 10_000,
                ..limits()
            }
            .validate(Side::Buy, 995, 10, 990, 1_000),
            Err(LimitError::NotionalTooSmall)
        );
    }
}
