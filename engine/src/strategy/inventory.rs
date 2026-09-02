#[derive(Debug, Clone, Copy, Default)]
pub struct InventoryState {
    pub position: i64,
    pub max_position: i64,
}

impl InventoryState {
    pub fn new(max_position: i64) -> Self {
        Self {
            position: 0,
            max_position,
        }
    }

    pub fn can_buy(&self, quantity: i64) -> bool {
        quantity >= 0
            && i128::from(self.position) + i128::from(quantity) <= i128::from(self.max_position)
    }

    pub fn can_sell(&self, quantity: i64) -> bool {
        quantity >= 0
            && i128::from(self.position) - i128::from(quantity) >= -i128::from(self.max_position)
    }

    pub fn update(&mut self, delta: i64) -> bool {
        match self.position.checked_add(delta) {
            Some(position) => {
                self.position = position;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InventoryState;

    #[test]
    fn checks_position_without_integer_overflow() {
        let state = InventoryState {
            position: i64::MAX,
            max_position: i64::MAX,
        };
        assert!(!state.can_buy(1));
        assert!(state.can_sell(1));
        assert!(!state.can_buy(-1));
        assert!(!state.can_sell(-1));
    }

    #[test]
    fn update_is_checked_and_preserves_state_on_overflow() {
        let mut state = InventoryState {
            position: i64::MAX,
            max_position: i64::MAX,
        };
        assert!(!state.update(1));
        assert_eq!(state.position, i64::MAX);
        assert!(state.update(-1));
        assert_eq!(state.position, i64::MAX - 1);
    }
}
