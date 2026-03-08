//! Secrets provider trait and implementations.
//!
//! The `SecretsProvider` trait defines how Tasker resolves secret references
//! to their values. Implementations talk to specific backends (environment
//! variables, SOPS files, Vault, AWS SSM, etc.).

mod chained;
mod env;
mod error;
#[cfg(feature = "sops")]
pub mod sops;
mod value;

use std::collections::HashMap;
use std::fmt;

pub use chained::ChainedSecretsProvider;
pub use env::EnvSecretsProvider;
pub use error::SecretsError;
#[cfg(feature = "sops")]
pub use sops::SopsSecretsProvider;
pub use value::SecretValue;

/// A secrets provider resolves named secret references to their values.
///
/// Implementations talk to specific backends (Vault, AWS SSM, SOPS files,
/// environment variables, etc.). Tasker does not store secrets — it resolves
/// references through provider implementations.
///
/// Secrets are resolved at startup or pool initialization time, not during
/// step execution. Capability executors never receive secret values — they
/// receive already-initialized resource handles.
#[async_trait::async_trait]
pub trait SecretsProvider: Send + Sync + fmt::Debug {
    /// Resolve a single secret by its path or identifier.
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError>;

    /// Resolve multiple secrets in a single call.
    /// Default implementation calls `get_secret` for each path.
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

    /// Provider identity — used in diagnostic messages only.
    fn provider_name(&self) -> &str;

    /// Verify the provider is reachable and the configuration is valid.
    async fn health_check(&self) -> Result<(), SecretsError>;
}
