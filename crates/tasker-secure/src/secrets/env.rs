//! Environment variable secrets provider.
//!
//! Maps secret paths to environment variable names. Optional prefix
//! stripping and path normalization (uppercase, `/` and `-` → `_`).

use super::{SecretValue, SecretsError, SecretsProvider};

/// Reads secrets from environment variables.
///
/// With prefix `"TASKER_SECRET_"`:
///   path `"orders-db/password"` → env var `"TASKER_SECRET_ORDERS_DB_PASSWORD"`
#[derive(Debug)]
pub struct EnvSecretsProvider {
    prefix: Option<String>,
}

impl EnvSecretsProvider {
    /// Create a provider that reads env vars directly (no prefix, no normalization).
    pub fn new(prefix: Option<String>) -> Self {
        Self { prefix }
    }

    /// Create a provider with a prefix. Paths are normalized before prefix is prepended.
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: Some(prefix.to_string()),
        }
    }

    /// Normalize a secret path to an environment variable name.
    /// Rules: Replace `/`, `-`, `.` with `_`, uppercase, prepend prefix.
    fn normalize_path(&self, path: &str) -> String {
        match &self.prefix {
            Some(prefix) => {
                let normalized = path.replace(['/', '-', '.'], "_").to_uppercase();
                format!("{prefix}{normalized}")
            }
            None => path.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for EnvSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        let var_name = self.normalize_path(path);
        match std::env::var(&var_name) {
            Ok(value) => Ok(SecretValue::from(value)),
            Err(std::env::VarError::NotPresent) => Err(SecretsError::NotFound {
                path: path.to_string(),
            }),
            Err(std::env::VarError::NotUnicode(_)) => Err(SecretsError::InvalidPath {
                path: path.to_string(),
                reason: format!("environment variable '{var_name}' contains invalid UTF-8"),
            }),
        }
    }

    fn provider_name(&self) -> &str {
        "env"
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        Ok(())
    }
}
