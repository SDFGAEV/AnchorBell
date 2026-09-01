use tokio::sync::{mpsc, watch};

use crate::event::EngineEvent;
use crate::execution::OrderIntent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapacities {
    pub market_events: usize,
    pub order_intents: usize,
}

impl Default for RuntimeCapacities {
    fn default() -> Self {
        Self {
            market_events: 4096,
            order_intents: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSignal {
    Running,
    Draining,
    Halted,
}

pub struct RuntimeBus {
    pub market_tx: mpsc::Sender<EngineEvent>,
    pub order_rx: mpsc::Receiver<OrderIntent>,
    signal_rx: watch::Receiver<RuntimeSignal>,
}

pub struct RuntimeHandles {
    pub market_rx: mpsc::Receiver<EngineEvent>,
    pub order_tx: mpsc::Sender<OrderIntent>,
    signal_tx: watch::Sender<RuntimeSignal>,
}

impl RuntimeBus {
    pub fn bounded(capacities: RuntimeCapacities) -> (Self, RuntimeHandles) {
        let (market_tx, market_rx) = mpsc::channel(capacities.market_events);
        let (order_tx, order_rx) = mpsc::channel(capacities.order_intents);
        let (signal_tx, signal_rx) = watch::channel(RuntimeSignal::Running);
        (
            Self {
                market_tx,
                order_rx,
                signal_rx,
            },
            RuntimeHandles {
                market_rx,
                order_tx,
                signal_tx,
            },
        )
    }

    pub fn signal(&self) -> RuntimeSignal {
        *self.signal_rx.borrow()
    }

    pub async fn wait_for_signal_change(&mut self) -> Result<RuntimeSignal, watch::error::RecvError> {
        self.signal_rx.changed().await?;
        Ok(*self.signal_rx.borrow())
    }
}

impl RuntimeHandles {
    pub fn drain(&self) -> Result<(), watch::error::SendError<RuntimeSignal>> {
        self.signal_tx.send(RuntimeSignal::Draining)
    }

    pub fn halt(&self) -> Result<(), watch::error::SendError<RuntimeSignal>> {
        self.signal_tx.send(RuntimeSignal::Halted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bounded_bus_exposes_explicit_capacities_and_signals() {
        let (mut bus, handles) = RuntimeBus::bounded(RuntimeCapacities {
            market_events: 2,
            order_intents: 1,
        });
        assert_eq!(bus.signal(), RuntimeSignal::Running);
        handles.drain().unwrap();
        assert_eq!(bus.wait_for_signal_change().await.unwrap(), RuntimeSignal::Draining);
        handles.halt().unwrap();
        assert_eq!(bus.wait_for_signal_change().await.unwrap(), RuntimeSignal::Halted);
    }
}
