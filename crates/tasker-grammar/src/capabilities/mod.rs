//! Capability executor implementations.
//!
//! Each capability is a pure function: `(input: Value, config: Value) → Result<Value>`.
//! No database, no messaging, no worker context.
//!
//! ## Data capabilities (pure, no side effects)
//!
//! - `transform` — jaq filter execution with JSON Schema output validation (TAS-325/326/327)
//! - `validate` — JSON Schema validation with coercion modes (TAS-324)
//! - `assert` — jaq boolean filter evaluation; gates execution (TAS-328)
//!
//! ## Action capabilities (side-effecting, tested with stubs in Phase 1)
//!
//! - `persist` — resource abstraction layer with jaq data filter (TAS-330)
//! - `acquire` — resource abstraction layer with jaq result filter (TAS-331)
//! - `emit` — domain event construction with jaq payload filter (TAS-332)
//!
//! **Tickets**: TAS-324 through TAS-332

pub mod acquire;
pub mod assert;
pub mod emit;
pub mod persist;
pub mod transform;
pub mod validate;

use crate::types::CapabilityError;

/// Maximum length for resource reference and entity strings.
const MAX_RESOURCE_REF_LEN: usize = 128;

/// Validate a resource reference or entity string.
///
/// Resource references are untrusted user input that flows to
/// [`OperationProvider`] implementations. This function enforces:
/// - Non-empty
/// - Maximum length of [`MAX_RESOURCE_REF_LEN`]
/// - Characters limited to alphanumeric, underscore, hyphen, and period
///
/// `OperationProvider` implementations **must** also treat these values
/// as untrusted input and apply their own validation appropriate to their
/// storage backend (SQL parameterization, path sanitization, etc.).
pub(crate) fn validate_resource_ref(
    value: &str,
    field_name: &str,
) -> Result<(), CapabilityError> {
    if value.is_empty() {
        return Err(CapabilityError::ConfigValidation(format!(
            "{field_name} must not be empty"
        )));
    }
    if value.len() > MAX_RESOURCE_REF_LEN {
        return Err(CapabilityError::ConfigValidation(format!(
            "{field_name} length {} exceeds maximum of {MAX_RESOURCE_REF_LEN}",
            value.len()
        )));
    }
    if !value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(CapabilityError::ConfigValidation(format!(
            "{field_name} contains invalid characters (allowed: alphanumeric, '_', '-', '.')"
        )));
    }
    Ok(())
}
