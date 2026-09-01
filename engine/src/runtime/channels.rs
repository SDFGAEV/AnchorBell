use tokio::sync::mpsc;

use crate::event::EngineEvent;
use crate::execution::OrderIntent;

pub struct RuntimeChannels {
    pub market_rx: mpsc::Receiver<EngineEvent>,
    pub order_tx: mpsc::Sender<OrderIntent>,
}

impl RuntimeChannels {
    pub fn new(
        market_rx: mpsc::Receiver<EngineEvent>,
        order_tx: mpsc::Sender<OrderIntent>,
    ) -> Self {
        Self {
            market_rx,
            order_tx,
        }
    }
}
