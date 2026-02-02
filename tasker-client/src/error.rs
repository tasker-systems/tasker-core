//! # Client Error Types
//!
//! Unified error handling for tasker-client library and CLI operations.

use anyhow::Result;
use tasker_shared::errors::TaskerError;
use thiserror::Error;

/// Client operation result type
pub type ClientResult<T> = Result<T, ClientError>;

/// Comprehensive error types for client operations
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("Worker not found: {worker_id}")]
    WorkerNotFound { worker_id: String },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Service unavailable: {service} - {reason}")]
    ServiceUnavailable { service: String, reason: String },

    #[error("Timeout waiting for operation: {operation}")]
    Timeout { operation: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UUID parsing error: {0}")]
    UuidError(#[from] uuid::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid response: {field} - {reason}")]
    InvalidResponse { field: String, reason: String },

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Tasker system error: {0}")]
    TaskerError(#[from] TaskerError),
}

impl ClientError {
    /// Create an API error from HTTP response
    pub fn api_error(status: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            status,
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError(message.into())
    }

    /// Create a service unavailable error
    pub fn service_unavailable(service: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            service: service.into(),
            reason: reason.into(),
        }
    }

    /// Create an invalid response error for protocol violations
    ///
    /// Use this when a gRPC response is missing required fields or contains
    /// malformed data. This indicates a protocol violation that should not
    /// be silently defaulted.
    pub fn invalid_response(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidResponse {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Check if error is recoverable (worth retrying)
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        match self {
            ClientError::HttpError(e) => e.is_timeout() || e.is_connect(),
            ClientError::ServiceUnavailable { .. } => true,
            ClientError::Timeout { .. } => true,
            ClientError::ApiError { status, .. } => *status >= 500,
            // Protocol violations are not recoverable - the server is broken
            ClientError::InvalidResponse { .. } => false,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Constructor tests ----

    #[test]
    fn test_api_error_constructor() {
        let err = ClientError::api_error(404, "not found");
        match err {
            ClientError::ApiError { status, message } => {
                assert_eq!(status, 404);
                assert_eq!(message, "not found");
            }
            _ => panic!("Expected ApiError variant"),
        }
    }

    #[test]
    fn test_config_error_constructor() {
        let err = ClientError::config_error("bad config");
        match err {
            ClientError::ConfigError(msg) => assert_eq!(msg, "bad config"),
            _ => panic!("Expected ConfigError variant"),
        }
    }

    #[test]
    fn test_service_unavailable_constructor() {
        let err = ClientError::service_unavailable("grpc", "connection refused");
        match err {
            ClientError::ServiceUnavailable { service, reason } => {
                assert_eq!(service, "grpc");
                assert_eq!(reason, "connection refused");
            }
            _ => panic!("Expected ServiceUnavailable variant"),
        }
    }

    #[test]
    fn test_invalid_response_constructor() {
        let err = ClientError::invalid_response("task.id", "missing field");
        match err {
            ClientError::InvalidResponse { field, reason } => {
                assert_eq!(field, "task.id");
                assert_eq!(reason, "missing field");
            }
            _ => panic!("Expected InvalidResponse variant"),
        }
    }

    // ---- is_recoverable tests ----

    #[test]
    fn test_service_unavailable_is_recoverable() {
        let err = ClientError::ServiceUnavailable {
            service: "api".to_string(),
            reason: "down".to_string(),
        };
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_timeout_is_recoverable() {
        let err = ClientError::Timeout {
            operation: "create_task".to_string(),
        };
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_api_error_500_is_recoverable() {
        let err = ClientError::api_error(500, "internal server error");
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_api_error_502_is_recoverable() {
        let err = ClientError::api_error(502, "bad gateway");
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_api_error_400_not_recoverable() {
        let err = ClientError::api_error(400, "bad request");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_api_error_404_not_recoverable() {
        let err = ClientError::api_error(404, "not found");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_invalid_response_not_recoverable() {
        let err = ClientError::invalid_response("field", "broken");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_config_error_not_recoverable() {
        let err = ClientError::ConfigError("bad".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_auth_error_not_recoverable() {
        let err = ClientError::AuthError("invalid token".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_task_not_found_not_recoverable() {
        let err = ClientError::TaskNotFound {
            task_id: "abc-123".to_string(),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_worker_not_found_not_recoverable() {
        let err = ClientError::WorkerNotFound {
            worker_id: "w-1".to_string(),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_invalid_input_not_recoverable() {
        let err = ClientError::InvalidInput("bad input".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_internal_error_not_recoverable() {
        let err = ClientError::Internal("oops".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_not_implemented_not_recoverable() {
        let err = ClientError::NotImplemented("feature X".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_serialization_error_not_recoverable() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ClientError::SerializationError(json_err);
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_uuid_error_not_recoverable() {
        let uuid_err = uuid::Uuid::parse_str("not-a-uuid").unwrap_err();
        let err = ClientError::UuidError(uuid_err);
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_io_error_not_recoverable() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
        let err = ClientError::IoError(io_err);
        assert!(!err.is_recoverable());
    }

    // ---- Display tests ----

    #[test]
    fn test_display_api_error() {
        let err = ClientError::api_error(503, "service down");
        assert_eq!(format!("{err}"), "API error: 503 - service down");
    }

    #[test]
    fn test_display_config_error() {
        let err = ClientError::config_error("missing field");
        assert_eq!(format!("{err}"), "Configuration error: missing field");
    }

    #[test]
    fn test_display_task_not_found() {
        let err = ClientError::TaskNotFound {
            task_id: "abc".to_string(),
        };
        assert_eq!(format!("{err}"), "Task not found: abc");
    }

    #[test]
    fn test_display_worker_not_found() {
        let err = ClientError::WorkerNotFound {
            worker_id: "w-1".to_string(),
        };
        assert_eq!(format!("{err}"), "Worker not found: w-1");
    }

    #[test]
    fn test_display_service_unavailable() {
        let err = ClientError::service_unavailable("api", "timeout");
        assert_eq!(format!("{err}"), "Service unavailable: api - timeout");
    }

    #[test]
    fn test_display_timeout() {
        let err = ClientError::Timeout {
            operation: "poll".to_string(),
        };
        assert_eq!(format!("{err}"), "Timeout waiting for operation: poll");
    }

    #[test]
    fn test_display_invalid_response() {
        let err = ClientError::invalid_response("checks", "missing");
        assert_eq!(format!("{err}"), "Invalid response: checks - missing");
    }

    #[test]
    fn test_display_auth_error() {
        let err = ClientError::AuthError("expired".to_string());
        assert_eq!(format!("{err}"), "Authentication failed: expired");
    }

    #[test]
    fn test_display_invalid_input() {
        let err = ClientError::InvalidInput("empty name".to_string());
        assert_eq!(format!("{err}"), "Invalid input: empty name");
    }

    #[test]
    fn test_display_internal() {
        let err = ClientError::Internal("panic".to_string());
        assert_eq!(format!("{err}"), "Internal error: panic");
    }

    #[test]
    fn test_display_not_implemented() {
        let err = ClientError::NotImplemented("batch cancel".to_string());
        assert_eq!(format!("{err}"), "Not implemented: batch cancel");
    }

    // ---- From impls ----

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{{bad}}").unwrap_err();
        let err: ClientError = json_err.into();
        assert!(matches!(err, ClientError::SerializationError(_)));
    }

    #[test]
    fn test_from_uuid_error() {
        let uuid_err = uuid::Uuid::parse_str("not-valid").unwrap_err();
        let err: ClientError = uuid_err.into();
        assert!(matches!(err, ClientError::UuidError(_)));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let err: ClientError = io_err.into();
        assert!(matches!(err, ClientError::IoError(_)));
    }

    #[test]
    fn test_debug_impl() {
        let err = ClientError::api_error(500, "boom");
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("ApiError"));
    }
}
