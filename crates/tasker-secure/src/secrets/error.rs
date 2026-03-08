//! Error types for secrets resolution.

/// Errors that can occur when resolving secrets through a provider.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// The requested secret path does not exist in the provider.
    #[error("secret not found: {path}")]
    NotFound { path: String },

    /// The caller does not have permission to access this secret.
    #[error("access denied for secret: {path}")]
    AccessDenied { path: String },

    /// The secrets provider is unavailable (network error, auth failure, etc.).
    #[error("secrets provider unavailable: {message}")]
    ProviderUnavailable { message: String },

    /// The secret path is malformed or invalid for this provider.
    #[error("invalid secret path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },
}
