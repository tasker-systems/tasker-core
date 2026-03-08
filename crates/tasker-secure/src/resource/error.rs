//! Error types for resource lifecycle management.

use crate::secrets::SecretsError;

/// Errors that can occur during resource lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    /// A resource failed to initialize (e.g., connection pool creation failed).
    #[error("resource '{name}' initialization failed: {message}")]
    InitializationFailed { name: String, message: String },

    /// A resource health check failed.
    #[error("resource '{name}' health check failed: {message}")]
    HealthCheckFailed { name: String, message: String },

    /// Credential refresh failed for a resource.
    #[error("resource '{name}' credential refresh failed: {message}")]
    CredentialRefreshFailed { name: String, message: String },

    /// The requested resource was not found in the registry.
    #[error("resource not found: '{name}'")]
    ResourceNotFound { name: String },

    /// A resource was found but has the wrong type.
    #[error("resource '{name}' type mismatch: expected {expected}, got {actual}")]
    WrongResourceType {
        name: String,
        expected: String,
        actual: String,
    },

    /// A required configuration key is missing.
    #[error("resource '{resource}' missing required config key: '{key}'")]
    MissingConfigKey { resource: String, key: String },

    /// A secret reference could not be resolved.
    #[error("resource '{resource}' secret resolution failed: {source}")]
    SecretResolution {
        resource: String,
        source: SecretsError,
    },
}
