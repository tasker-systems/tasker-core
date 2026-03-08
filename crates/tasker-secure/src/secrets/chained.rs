//! Chained secrets provider — tries providers in priority order.
//!
//! First success wins; all must fail for an error to be returned.

use std::collections::HashMap;
use std::sync::Arc;

use super::{SecretValue, SecretsError, SecretsProvider};

/// Tries providers in priority order. First success wins.
#[derive(Debug)]
pub struct ChainedSecretsProvider {
    providers: Vec<Arc<dyn SecretsProvider>>,
    name: String,
}

impl ChainedSecretsProvider {
    /// Create a chained provider from a list of providers in priority order.
    pub fn new(providers: Vec<Arc<dyn SecretsProvider>>) -> Self {
        let name = format!(
            "chain[{}]",
            providers
                .iter()
                .map(|p| p.provider_name().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Self { providers, name }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for ChainedSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        if self.providers.is_empty() {
            return Err(SecretsError::ProviderUnavailable {
                message: "no providers configured in chain".to_string(),
            });
        }

        let mut last_error = None;
        for provider in &self.providers {
            match provider.get_secret(path).await {
                Ok(value) => return Ok(value),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.expect("at least one provider exists"))
    }

    async fn get_secrets(
        &self,
        paths: &[&str],
    ) -> Result<HashMap<String, SecretValue>, SecretsError> {
        let mut results = HashMap::with_capacity(paths.len());
        for &path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }
        Ok(results)
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        if self.providers.is_empty() {
            return Err(SecretsError::ProviderUnavailable {
                message: "no providers configured in chain".to_string(),
            });
        }

        let mut last_error = None;
        for provider in &self.providers {
            match provider.health_check().await {
                Ok(()) => return Ok(()),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.expect("at least one provider exists"))
    }
}
