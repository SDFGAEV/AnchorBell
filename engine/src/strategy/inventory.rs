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
        self.position + quantity <= self.max_position
    }

    pub fn can_sell(&self, quantity: i64) -> bool {
        self.position - quantity >= -self.max_position
    }

    pub fn update(&mut self, delta: i64) {
        self.position += delta;
    }
}
