pub mod binance;
#[path = "binance_wire.rs"]
pub mod binance_wire;
pub mod credentials;
pub mod environment;
pub mod funding_risk;
pub mod gateway;
pub mod intent;
pub mod lifecycle;
#[path = "limits.rs"]
pub mod limits;
pub mod order_api;
pub mod order_manager;
#[path = "order_ws.rs"]
pub mod order_ws;
#[path = "reconciliation.rs"]
pub mod reconciliation;
#[path = "recovery.rs"]
pub mod recovery;
pub mod risk;
pub mod safety;
pub mod signing;
#[path = "spot.rs"]
pub mod spot;

pub use order_ws::{BinanceOrderWebSocket, OrderTransportError};

pub use limits::{LimitError, OrderLimits};

pub use reconciliation::{reconcile, ReconciliationAction, ReconciliationInput};
pub use recovery::{RecoveryEvent, RecoveryMachine, RecoveryState};

pub use signing::{canonical_query, sign_query, signed_params, SigningError};

pub use binance::BinanceGateway;
pub use binance_wire::{
    BinanceAccountStatusResponse, BinanceAccountStatusResult, BinanceAccountStatusWire,
    BinanceOrderStatusResponse, BinanceOrderStatusResult, BinanceOrderStatusWire,
    BinancePositionSnapshot, BinancePositionStatusResponse, BinancePositionStatusWire,
};
pub use credentials::{BinanceCredentials, CredentialsError};
pub use environment::{BinanceEndpoints, BinanceEnvironment};
pub use funding_risk::{FundingAwareRiskGate, FundingRiskAction, FundingRiskInput};
pub use gateway::{ExchangeOrder, ExecutionGateway, GatewayResult, PaperGateway};
pub use intent::{OrderIntent, Side};
pub use lifecycle::{LifecycleError, LifecycleEvent, MakerOrder, OrderStatus};
pub use order_api::{
    BinanceOrderClient, SignedBinanceTransport, SignedCancelRequest, SignedOrderRequest,
};
pub use order_manager::{OrderManager, OrderState};
pub use risk::{RiskAction, RiskInput, SessionRiskGate};
pub use safety::{DeploymentPolicy, SafetyError};
pub use spot::{SpotDemoEndpoints, SpotOrderWire};
