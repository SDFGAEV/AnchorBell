use super::{BinanceEnvironment, ExchangeOrder, Side};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOrderRequest {
    pub symbol: String,
    pub side: String,
    pub price: i64,
    pub quantity: i64,
    pub client_order_id: String,
    pub post_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCancelRequest {
    pub symbol: String,
    pub client_order_id: String,
}

pub trait SignedBinanceTransport {
    type Error;

    fn submit_order(
        &mut self,
        environment: BinanceEnvironment,
        request: SignedOrderRequest,
    ) -> Result<(), Self::Error>;

    fn cancel_order(
        &mut self,
        environment: BinanceEnvironment,
        request: SignedCancelRequest,
    ) -> Result<(), Self::Error>;
}

pub struct BinanceOrderClient<T> {
    pub environment: BinanceEnvironment,
    pub transport: T,
}

impl<T> BinanceOrderClient<T> {
    pub fn new(environment: BinanceEnvironment, transport: T) -> Self {
        Self { environment, transport }
    }
}

impl<T: SignedBinanceTransport> BinanceOrderClient<T> {
    pub fn submit(
        &mut self,
        symbol: String,
        order: ExchangeOrder,
        client_order_id: String,
        side: Side,
    ) -> Result<(), T::Error> {
        let side = match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };
        self.transport.submit_order(self.environment, SignedOrderRequest {
            symbol,
            side: side.to_string(),
            price: order.price,
            quantity: order.quantity,
            client_order_id,
            post_only: order.post_only,
        })
    }

    pub fn cancel(&mut self, symbol: String, client_order_id: String) -> Result<(), T::Error> {
        self.transport.cancel_order(self.environment, SignedCancelRequest {
            symbol, client_order_id,
        })
    }
}
