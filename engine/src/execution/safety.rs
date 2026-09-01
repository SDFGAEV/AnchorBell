use super::BinanceEnvironment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeploymentPolicy {
    pub environment: BinanceEnvironment,
    pub allow_live_orders: bool,
    pub allow_production: bool,
    pub credentials_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyError {
    MissingCredentials,
    ProductionNotExplicitlyEnabled,
    OrderSubmissionNotExplicitlyEnabled,
    EnvironmentMismatch,
}

impl DeploymentPolicy {
    pub fn validate(self) -> Result<(), SafetyError> {
        self.validate_for(self.environment)
    }

    pub fn validate_for(
        self,
        requested_environment: BinanceEnvironment,
    ) -> Result<(), SafetyError> {
        if self.environment != requested_environment {
            return Err(SafetyError::EnvironmentMismatch);
        }
        if !self.credentials_loaded {
            return Err(SafetyError::MissingCredentials);
        }
        if self.environment == BinanceEnvironment::Production && !self.allow_production {
            return Err(SafetyError::ProductionNotExplicitlyEnabled);
        }
        Ok(())
    }

    pub fn validate_for_order(
        self,
        requested_environment: BinanceEnvironment,
    ) -> Result<(), SafetyError> {
        self.validate_for(requested_environment)?;
        if !self.allow_live_orders {
            return Err(SafetyError::OrderSubmissionNotExplicitlyEnabled);
        }
        Ok(())
    }

    pub fn testnet() -> Self {
        Self {
            environment: BinanceEnvironment::Testnet,
            allow_live_orders: false,
            allow_production: false,
            credentials_loaded: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_testnet_policy_fails_closed_without_credentials() {
        assert_eq!(
            DeploymentPolicy::testnet().validate(),
            Err(SafetyError::MissingCredentials)
        );
    }

    #[test]
    fn production_requires_explicit_enablement() {
        let policy = DeploymentPolicy {
            environment: BinanceEnvironment::Production,
            allow_live_orders: false,
            allow_production: false,
            credentials_loaded: true,
        };
        assert_eq!(
            policy.validate(),
            Err(SafetyError::ProductionNotExplicitlyEnabled)
        );
    }

    #[test]
    fn requested_environment_must_match_policy() {
        let policy = DeploymentPolicy {
            environment: BinanceEnvironment::Testnet,
            allow_live_orders: false,
            allow_production: false,
            credentials_loaded: true,
        };
        assert_eq!(
            policy.validate_for(BinanceEnvironment::Production),
            Err(SafetyError::EnvironmentMismatch)
        );
    }

    #[test]
    fn read_only_production_requires_only_production_enablement() {
        let policy = DeploymentPolicy {
            environment: BinanceEnvironment::Production,
            allow_live_orders: false,
            allow_production: true,
            credentials_loaded: true,
        };
        assert_eq!(policy.validate(), Ok(()));
        assert_eq!(
            policy.validate_for_order(BinanceEnvironment::Production),
            Err(SafetyError::OrderSubmissionNotExplicitlyEnabled)
        );
    }
}
