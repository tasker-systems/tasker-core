//! In-memory secrets provider for testing.

use std::collections::HashMap;

use crate::secrets::{SecretValue, SecretsError, SecretsProvider};

/// A secrets provider backed by an in-memory HashMap.
#[derive(Debug)]
pub struct InMemorySecretsProvider {
    secrets: HashMap<String, String>,
    name: String,
}

impl InMemorySecretsProvider {
    /// Create a new in-memory provider with the given secrets.
    pub fn new(secrets: HashMap<String, String>) -> Self {
        Self {
            secrets,
            name: "in-memory".to_string(),
        }
    }

    /// Create a provider with a custom name (useful for chaining tests).
    pub fn with_name(name: &str, secrets: HashMap<String, String>) -> Self {
        Self {
            secrets,
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for InMemorySecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        self.secrets
            .get(path)
            .map(|v| SecretValue::from(v.clone()))
            .ok_or_else(|| SecretsError::NotFound {
                path: path.to_string(),
            })
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        Ok(())
    }
}
