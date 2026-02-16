//! Error types for napi-rs FFI boundary.
//!
//! Unlike the koffi approach which wraps errors in JSON envelopes,
//! napi-rs converts Rust errors directly into JavaScript exceptions.
//! This matches how pyo3 and magnus handle errors.

/// FFI-specific errors that map to JavaScript exceptions.
#[derive(Debug, thiserror::Error)]
pub enum NapiFfiError {
    #[error("Worker not initialized — call bootstrapWorker() first")]
    WorkerNotInitialized,

    #[error("Failed to acquire lock on worker system")]
    LockError,

    #[error("Bootstrap failed: {0}")]
    BootstrapFailed(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<NapiFfiError> for napi::Error {
    fn from(err: NapiFfiError) -> Self {
        napi::Error::from_reason(err.to_string())
    }
}
