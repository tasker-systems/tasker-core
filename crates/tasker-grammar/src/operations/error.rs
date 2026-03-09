//! Errors from resource operations.
//!
//! Distinct from tasker-secure's `ResourceError` (which covers initialization,
//! health check, and credential refresh). These errors describe operation-level
//! failures — the things that go wrong when you try to use a resource, not
//! when you try to connect to it.

/// Errors from resource operations.
#[derive(Debug, thiserror::Error)]
pub enum ResourceOperationError {
    #[error("Entity not found: {entity}")]
    EntityNotFound { entity: String },

    #[error("Conflict on persist to {entity}: {reason}")]
    Conflict { entity: String, reason: String },

    #[error("Authorization failed for {operation} on {entity}")]
    AuthorizationFailed { operation: String, entity: String },

    #[error("Resource unavailable: {message}")]
    Unavailable { message: String },

    #[error("Operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Resource operation error: {message}")]
    Other {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
