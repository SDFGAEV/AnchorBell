#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceEnvironment {
    Testnet,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinanceEndpoints {
    pub rest_base: &'static str,
    pub market_ws_base: &'static str,
    pub order_ws_base: &'static str,
}

impl BinanceEnvironment {
    pub const fn endpoints(self) -> BinanceEndpoints {
        match self {
            Self::Testnet => BinanceEndpoints {
                rest_base: "https://demo-fapi.binance.com",
                market_ws_base: "wss://demo-fstream.binance.com/public",
                order_ws_base: "wss://testnet.binancefuture.com/ws-fapi/v1",
            },
            Self::Production => BinanceEndpoints {
                rest_base: "https://fapi.binance.com",
                market_ws_base: "wss://fstream.binance.com",
                order_ws_base: "wss://ws-fapi.binance.com/ws-fapi/v1",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testnet_is_explicitly_separate_from_production() {
        let testnet = BinanceEnvironment::Testnet.endpoints();
        let production = BinanceEnvironment::Production.endpoints();
        assert_eq!(testnet.rest_base, "https://demo-fapi.binance.com");
        assert_eq!(
            testnet.market_ws_base,
            "wss://demo-fstream.binance.com/public"
        );
        assert_ne!(testnet.rest_base, production.rest_base);
        assert_ne!(testnet.market_ws_base, production.market_ws_base);
    }
}
