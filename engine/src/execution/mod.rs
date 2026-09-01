pub mod intent;
pub mod order_manager;
pub mod lifecycle;
pub mod risk;
pub mod gateway;
pub mod binance;
pub mod environment;
pub mod order_api;
pub mod safety;
pub mod credentials;
pub mod signing;

pub use signing::{canonical_query, sign_query, signed_params, SigningError};

pub use intent::{OrderIntent, Side};
pub use order_manager::{OrderManager, OrderState};
pub use lifecycle::{LifecycleError, LifecycleEvent, MakerOrder, OrderStatus};
pub use risk::{RiskAction, RiskInput, SessionRiskGate};
pub use gateway::{ExchangeOrder, ExecutionGateway, GatewayResult, PaperGateway};
pub use binance::BinanceGateway;
pub use environment::{BinanceEndpoints, BinanceEnvironment};
pub use order_api::{BinanceOrderClient, SignedBinanceTransport, SignedCancelRequest, SignedOrderRequest};
pub use safety::{DeploymentPolicy, SafetyError};
pub use credentials::{BinanceCredentials, CredentialsError};
