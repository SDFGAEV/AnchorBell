use std::env;

use super::{BinanceEnvironment, DeploymentPolicy};

pub const ENVIRONMENT_VAR: &str = "ANCHORBELL_BINANCE_ENV";
pub const ENABLE_PRODUCTION_VAR: &str = "ANCHORBELL_ENABLE_PRODUCTION";
pub const ENABLE_ORDER_SUBMISSION_VAR: &str = "ANCHORBELL_ENABLE_ORDER_SUBMISSION";
pub const LIVE_TRADING_CONFIRMATION_VAR: &str = "ANCHORBELL_LIVE_TRADING_CONFIRMATION";
pub const LIVE_TRADING_CONFIRMATION: &str = "I_UNDERSTAND_REAL_FUNDS_RISK";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeploymentConfig {
    pub environment: BinanceEnvironment,
    pub allow_production: bool,
    pub allow_live_orders: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentConfigError {
    InvalidEnvironment,
    ProductionNotExplicitlyEnabled,
    LiveOrdersNotExplicitlyEnabled,
}

impl DeploymentConfig {
    pub fn from_process_environment() -> Result<Self, DeploymentConfigError> {
        let environment = match env::var(ENVIRONMENT_VAR) {
            Ok(value) => value
                .parse()
                .map_err(|_| DeploymentConfigError::InvalidEnvironment)?,
            Err(_) => BinanceEnvironment::Testnet,
        };
        let allow_production = is_enabled(ENABLE_PRODUCTION_VAR);
        let allow_order_submission = is_enabled(ENABLE_ORDER_SUBMISSION_VAR);
        let confirmation = env::var(LIVE_TRADING_CONFIRMATION_VAR).ok();

        Self::from_values(
            environment,
            allow_production,
            allow_order_submission,
            confirmation.as_deref(),
        )
    }

    pub fn from_values(
        environment: BinanceEnvironment,
        allow_production: bool,
        allow_order_submission: bool,
        confirmation: Option<&str>,
    ) -> Result<Self, DeploymentConfigError> {
        if environment == BinanceEnvironment::Production && !allow_production {
            return Err(DeploymentConfigError::ProductionNotExplicitlyEnabled);
        }

        let live_order_confirmation = confirmation == Some(LIVE_TRADING_CONFIRMATION);
        if environment == BinanceEnvironment::Production
            && allow_order_submission
            && !live_order_confirmation
        {
            return Err(DeploymentConfigError::LiveOrdersNotExplicitlyEnabled);
        }

        Ok(Self {
            environment,
            allow_production,
            allow_live_orders: allow_order_submission
                && (environment == BinanceEnvironment::Testnet || live_order_confirmation),
        })
    }

    pub fn policy(self, credentials_loaded: bool) -> DeploymentPolicy {
        DeploymentPolicy {
            environment: self.environment,
            allow_live_orders: self.allow_live_orders,
            allow_production: self.allow_production,
            credentials_loaded,
        }
    }
}

fn is_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_testnet_without_order_submission() {
        let config =
            DeploymentConfig::from_values(BinanceEnvironment::Testnet, false, false, None).unwrap();
        assert_eq!(config.environment, BinanceEnvironment::Testnet);
        assert!(!config.allow_live_orders);
    }

    #[test]
    fn production_read_only_requires_production_switch_only() {
        let config =
            DeploymentConfig::from_values(BinanceEnvironment::Production, true, false, None)
                .unwrap();
        assert_eq!(config.environment, BinanceEnvironment::Production);
        assert!(!config.allow_live_orders);
        assert!(config.policy(true).validate().is_ok());
    }

    #[test]
    fn production_orders_require_confirmation() {
        assert_eq!(
            DeploymentConfig::from_values(BinanceEnvironment::Production, true, true, None),
            Err(DeploymentConfigError::LiveOrdersNotExplicitlyEnabled)
        );
        let config = DeploymentConfig::from_values(
            BinanceEnvironment::Production,
            true,
            true,
            Some(LIVE_TRADING_CONFIRMATION),
        )
        .unwrap();
        assert!(config
            .policy(true)
            .validate_for_order(BinanceEnvironment::Production)
            .is_ok());
    }

    #[test]
    fn production_cannot_be_selected_by_accident() {
        assert_eq!(
            DeploymentConfig::from_values(BinanceEnvironment::Production, false, false, None),
            Err(DeploymentConfigError::ProductionNotExplicitlyEnabled)
        );
    }
}
