//! Constraint and result types for grammar resource operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The type of persist operation to perform.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistMode {
    /// INSERT — create new record(s). Fail on conflict.
    #[default]
    Insert,
    /// UPDATE ... WHERE — modify existing record(s) by identity.
    Update,
    /// INSERT ... ON CONFLICT DO UPDATE — create or update.
    Upsert,
    /// DELETE ... WHERE — remove record(s) by identity.
    Delete,
}

/// Constraints for persist operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistConstraints {
    /// The type of write operation (insert, update, upsert, delete).
    #[serde(default)]
    pub mode: PersistMode,
    /// Keys that identify the target record(s) for update/upsert/delete.
    pub identity_keys: Option<Vec<String>>,
    /// Keys for upsert conflict resolution (e.g., ["id"], ["order_id", "line_number"])
    pub upsert_key: Option<Vec<String>>,
    /// Conflict resolution strategy
    pub on_conflict: Option<ConflictStrategy>,
    /// Idempotency key for at-most-once semantics
    pub idempotency_key: Option<String>,
}

/// Conflict resolution strategy for persist upsert operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Error on conflict (default)
    Reject,
    /// Update existing record
    Update,
    /// Skip the conflicting row
    Skip,
}

/// Result of a persist operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistResult {
    /// The raw result data (affected rows, returned record, API response)
    pub data: serde_json::Value,
    /// Number of rows/records affected
    pub affected_count: Option<u64>,
}

/// Constraints for acquire operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AcquireConstraints {
    /// Maximum number of records to return
    pub limit: Option<u64>,
    /// Pagination offset
    pub offset: Option<u64>,
    /// Request timeout override
    pub timeout_ms: Option<u64>,
}

/// Result of an acquire operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquireResult {
    /// The acquired data
    pub data: serde_json::Value,
    /// Total count if available (for pagination)
    pub total_count: Option<u64>,
}

/// Metadata for emit operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmitMetadata {
    /// Correlation ID for event tracing
    pub correlation_id: Option<String>,
    /// Idempotency key for at-most-once delivery
    pub idempotency_key: Option<String>,
    /// Additional headers/attributes for the event
    pub attributes: Option<HashMap<String, String>>,
}

/// Result of an emit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmitResult {
    /// The publish confirmation data (message ID, timestamp, etc.)
    pub data: serde_json::Value,
    /// Whether delivery was confirmed by the target
    pub confirmed: bool,
}
