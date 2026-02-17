//! FFI logging utilities for TypeScript worker (napi-rs).
//!
//! TAS-290: Replaces C FFI logging with napi-rs native functions.
//! No more unsafe C string handling — strings cross as native JS strings.

use std::collections::HashMap;

/// Log an error message with optional structured fields.
#[napi]
pub fn log_error(message: String, fields: Option<HashMap<String, serde_json::Value>>) {
    if let Some(fields) = fields {
        tracing::error!(fields = ?fields, "{}", message);
    } else {
        tracing::error!("{}", message);
    }
}

/// Log a warning message with optional structured fields.
#[napi]
pub fn log_warn(message: String, fields: Option<HashMap<String, serde_json::Value>>) {
    if let Some(fields) = fields {
        tracing::warn!(fields = ?fields, "{}", message);
    } else {
        tracing::warn!("{}", message);
    }
}

/// Log an info message with optional structured fields.
#[napi]
pub fn log_info(message: String, fields: Option<HashMap<String, serde_json::Value>>) {
    if let Some(fields) = fields {
        tracing::info!(fields = ?fields, "{}", message);
    } else {
        tracing::info!("{}", message);
    }
}

/// Log a debug message with optional structured fields.
#[napi]
pub fn log_debug(message: String, fields: Option<HashMap<String, serde_json::Value>>) {
    if let Some(fields) = fields {
        tracing::debug!(fields = ?fields, "{}", message);
    } else {
        tracing::debug!("{}", message);
    }
}

/// Log a trace message with optional structured fields.
#[napi]
pub fn log_trace(message: String, fields: Option<HashMap<String, serde_json::Value>>) {
    if let Some(fields) = fields {
        tracing::trace!(fields = ?fields, "{}", message);
    } else {
        tracing::trace!("{}", message);
    }
}
