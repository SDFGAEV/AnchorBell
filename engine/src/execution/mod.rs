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
pub mod funding_risk;
pub mod signing;
#[path = "binance_wire.rs"]
pub mod binance_wire;
#[path = "reconciliation.rs"]
pub mod reconciliation;
#[path = "recovery.rs"]
pub mod recovery;
#[path = "limits.rs"]
pub mod limits;
#[path = "order_ws.rs"]
pub mod order_ws;
#[path = "spot.rs"]
pub mod spot;

pub use order_ws::{BinanceOrderWebSocket, OrderTransportError};

pub use limits::{LimitError, OrderLimits};

pub use reconciliation::{reconcile, ReconciliationAction, ReconciliationInput};
pub use recovery::{RecoveryEvent, RecoveryMachine, RecoveryState};

pub use signing::{canonical_query, sign_query, signed_params, SigningError};

pub use intent::{OrderIntent, Side};
pub use order_manager::{OrderManager, OrderState};
pub use lifecycle::{LifecycleError, LifecycleEvent, MakerOrder, OrderStatus};
pub use risk::{RiskAction, RiskInput, SessionRiskGate};
pub use funding_risk::{FundingAwareRiskGate, FundingRiskAction, FundingRiskInput};
pub use gateway::{ExchangeOrder, ExecutionGateway, GatewayResult, PaperGateway};
pub use binance::BinanceGateway;
pub use environment::{BinanceEndpoints, BinanceEnvironment};
pub use order_api::{BinanceOrderClient, SignedBinanceTransport, SignedCancelRequest, SignedOrderRequest};
pub use safety::{DeploymentPolicy, SafetyError};
pub use credentials::{BinanceCredentials, CredentialsError};
pub use spot::{SpotDemoEndpoints, SpotOrderWire};
