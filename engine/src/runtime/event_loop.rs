use crate::event::EngineEvent;

pub struct TradingRuntime {
    running: bool,
}

impl TradingRuntime {
    pub fn new() -> Self {
        Self { running: false }
    }

    pub async fn run(&mut self) {
        self.running = true;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn handle_event(&self, _event: EngineEvent) {
        // event dispatch boundary
    }
}
