use super::{ExchangeOrder, ExecutionGateway, GatewayResult};

#[derive(Debug, Clone)]
pub struct BinanceGateway {
    pub testnet: bool,
}

impl BinanceGateway {
    pub fn new(testnet: bool) -> Self {
        Self { testnet }
    }
}

impl ExecutionGateway for BinanceGateway {
    fn submit(&self, order: ExchangeOrder) -> GatewayResult {
        // Real REST/WebSocket integration will be attached here.
        // The boundary is intentionally isolated from OrderManager.
        if order.post_only {
            GatewayResult::Accepted
        } else {
            GatewayResult::Rejected
        }
    }

    fn cancel(&self, _client_id: u64) -> GatewayResult {
        GatewayResult::Accepted
    }
}
