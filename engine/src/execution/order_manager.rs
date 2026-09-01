#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderState {
    New,
    Submitted,
    Acknowledged,
    PartialFill,
    Filled,
    Cancelled,
}

#[derive(Debug)]
pub struct OrderManager {
    state: OrderState,
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            state: OrderState::New,
        }
    }

    pub fn state(&self) -> OrderState {
        self.state
    }

    pub fn submit_post_only(&mut self) {
        if self.state == OrderState::New {
            self.state = OrderState::Submitted;
        }
    }

    pub fn acknowledge(&mut self) {
        if self.state == OrderState::Submitted {
            self.state = OrderState::Acknowledged;
        }
    }
}
