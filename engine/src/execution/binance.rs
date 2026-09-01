use super::{BinanceEndpoints, BinanceEnvironment, ExchangeOrder, ExecutionGateway, GatewayResult};

#[derive(Debug, Clone, Copy)]
pub struct BinanceGateway {
    pub environment: BinanceEnvironment,
}

impl BinanceGateway {
    pub fn new(testnet: bool) -> Self {
        let environment = if testnet {
            BinanceEnvironment::Testnet
        } else {
            BinanceEnvironment::Production
        };
        Self { environment }
    }

    pub fn endpoints(&self) -> BinanceEndpoints {
        self.environment.endpoints()
    }
}

impl ExecutionGateway for BinanceGateway {
    fn submit(&self, order: ExchangeOrder) -> GatewayResult {
        // Network signing and acknowledgement stay outside this synchronous boundary.
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
