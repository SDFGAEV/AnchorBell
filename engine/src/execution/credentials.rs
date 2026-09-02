use std::env;

use zeroize::Zeroize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceCredentials {
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialsError {
    MissingApiKey,
    MissingApiSecret,
    EmptyApiKey,
    EmptyApiSecret,
}

impl Drop for BinanceCredentials {
    fn drop(&mut self) {
        self.api_key.zeroize();
        self.api_secret.zeroize();
    }
}

impl BinanceCredentials {
    pub fn from_environment() -> Result<Self, CredentialsError> {
        Self::from_environment_for(super::BinanceEnvironment::Testnet)
    }

    pub fn from_environment_for(
        environment: super::BinanceEnvironment,
    ) -> Result<Self, CredentialsError> {
        let (api_key_name, api_secret_name) = environment.credential_env_names();
        let api_key = env::var(api_key_name).map_err(|_| CredentialsError::MissingApiKey)?;
        let api_secret =
            env::var(api_secret_name).map_err(|_| CredentialsError::MissingApiSecret)?;
        Self::from_values(api_key, api_secret)
    }

    pub fn from_values(api_key: String, api_secret: String) -> Result<Self, CredentialsError> {
        if api_key.is_empty() {
            return Err(CredentialsError::EmptyApiKey);
        }
        if api_secret.is_empty() {
            return Err(CredentialsError::EmptyApiSecret);
        }
        Ok(Self {
            api_key,
            api_secret,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_values_are_rejected_without_exposing_secret_contents() {
        assert_eq!(
            BinanceCredentials::from_values(String::new(), "secret".into()),
            Err(CredentialsError::EmptyApiKey)
        );
        assert_eq!(
            BinanceCredentials::from_values("key".into(), String::new()),
            Err(CredentialsError::EmptyApiSecret)
        );
    }
}
