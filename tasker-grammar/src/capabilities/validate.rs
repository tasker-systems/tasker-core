use chrono::NaiveDate;
use serde_json::Value;

use crate::types::{
    CapabilityError, CompositionEnvelope, ExecutionContext, OnFailure, TypedCapabilityExecutor,
};

/// Executor for the `validate` capability — the boundary gate in the action grammar.
///
/// `validate` applies JSON Schema validation to incoming data at trust boundaries
/// where external or untrusted data enters a composition (API responses, file reads,
/// data connectors). It is the *only* capability that performs JSON Schema checks
/// at runtime — all other inter-capability data flow trusts design-time validation.
///
/// ## Config shape
///
/// - **`schema`** (required): JSON Schema to validate against.
/// - **`coerce`** (optional, default `false`): Attempt type coercion before
///   validation (e.g. string `"123"` → number `123`, `"true"` → boolean `true`).
/// - **`filter_extra`** (optional, default `false`): Strip fields not declared in
///   the schema's `properties`.
/// - **`on_failure`** (optional, default `"error"`): Behavior when validation fails.
///   - `"error"` — return `CapabilityError::InputValidation` with field-level details.
///   - `"warn"` — pass data through with `_validation_warnings` metadata attached.
///   - `"skip"` — pass data through unchanged, silently.
///
/// ## Composition context envelope
///
/// The executor receives the composition context envelope and validates `.prev`
/// (the output of the previous capability invocation). If `.prev` is `null` (first
/// invocation), it validates `.context` instead.
///
/// ## Examples
///
/// **Valid input passes through:**
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::types::{CapabilityExecutor, CompositionEnvelope, ExecutionContext};
/// # use tasker_grammar::capabilities::validate::ValidateExecutor;
/// let exec = ValidateExecutor::new();
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"name": "Alice", "age": 30}
/// });
/// let config = json!({
///     "schema": {
///         "type": "object",
///         "required": ["name", "age"],
///         "properties": {
///             "name": {"type": "string"},
///             "age": {"type": "integer"}
///         }
///     }
/// });
/// let envelope = CompositionEnvelope::new(&input);
/// let result = exec.execute(&envelope, &config, &ctx).unwrap();
/// assert_eq!(result, json!({"name": "Alice", "age": 30}));
/// ```
///
/// **Coercion converts string to number:**
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::types::{CapabilityExecutor, CompositionEnvelope, ExecutionContext};
/// # use tasker_grammar::capabilities::validate::ValidateExecutor;
/// let exec = ValidateExecutor::new();
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"amount": "123.45", "count": "7"}
/// });
/// let config = json!({
///     "schema": {
///         "type": "object",
///         "properties": {
///             "amount": {"type": "number"},
///             "count": {"type": "integer"}
///         }
///     },
///     "coerce": true
/// });
/// let envelope = CompositionEnvelope::new(&input);
/// let result = exec.execute(&envelope, &config, &ctx).unwrap();
/// assert_eq!(result["amount"], json!(123.45));
/// assert_eq!(result["count"], json!(7));
/// ```
#[derive(Debug)]
pub struct ValidateExecutor;

impl ValidateExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ValidateExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed configuration for the `validate` capability.
///
/// Deserialized from the `config` JSON at the trait boundary, eliminating
/// runtime field picking throughout the executor.
#[derive(Debug, serde::Deserialize)]
pub struct ValidateConfig {
    /// JSON Schema to validate against.
    pub schema: Value,

    /// Attempt type coercion before validation (default: `false`).
    #[serde(default)]
    pub coerce: bool,

    /// Strip fields not declared in the schema's `properties` (default: `false`).
    #[serde(default)]
    pub filter_extra: bool,

    /// Behavior when validation fails (default: `Error`).
    #[serde(default)]
    pub on_failure: OnFailure,
}

impl TypedCapabilityExecutor for ValidateExecutor {
    type Config = ValidateConfig;

    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &ValidateConfig,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        // Compile schema upfront — errors here are config problems
        let validator = jsonschema::validator_for(&config.schema)
            .map_err(|e| CapabilityError::ConfigValidation(format!("invalid JSON Schema: {e}")))?;

        // Determine what to validate: .prev if present, otherwise .context
        let target = envelope.resolve_target().clone();

        // Apply coercion if requested
        let mut data = if config.coerce {
            apply_coercion(&target, &config.schema)
        } else {
            target
        };

        // Apply extra field filtering if requested
        if config.filter_extra {
            filter_extra_fields(&mut data, &config.schema);
        }

        // Validate against schema
        let errors: Vec<String> = validator
            .iter_errors(&data)
            .map(|e| format_validation_error(&e))
            .collect();

        if errors.is_empty() {
            Ok(data)
        } else {
            match config.on_failure {
                OnFailure::Error => Err(CapabilityError::InputValidation(errors.join("; "))),
                OnFailure::Warn => {
                    // Pass data through with validation warnings attached
                    let mut result = serde_json::Map::new();
                    if let Value::Object(map) = data {
                        result = map;
                    } else {
                        result.insert("_value".to_string(), data);
                    }
                    result.insert(
                        "_validation_warnings".to_string(),
                        Value::Array(errors.into_iter().map(Value::String).collect()),
                    );
                    Ok(Value::Object(result))
                }
                OnFailure::Skip => Ok(data),
            }
        }
    }

    fn capability_name(&self) -> &str {
        "validate"
    }
}

/// Apply type coercion to data based on schema type expectations.
///
/// Coercion targets:
/// - String → Number: `"123"` → `123`, `"1.5"` → `1.5`
/// - String → Integer: `"42"` → `42` (only if no fractional part)
/// - String → Boolean: `"true"` → `true`, `"false"` → `false`
/// - Number → String: `42` → `"42"` (when schema expects string)
/// - Boolean → String: `true` → `"true"` (when schema expects string)
///
/// Coercion is applied recursively for objects and arrays.
fn apply_coercion(value: &Value, schema: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let properties = schema.get("properties").and_then(Value::as_object);

            let mut result = serde_json::Map::new();
            for (key, val) in map {
                let coerced = if let Some(prop_schema) = properties.and_then(|p| p.get(key)) {
                    apply_coercion(val, prop_schema)
                } else {
                    val.clone()
                };
                result.insert(key.clone(), coerced);
            }
            Value::Object(result)
        }

        Value::Array(arr) => {
            let items_schema = schema.get("items");
            Value::Array(
                arr.iter()
                    .map(|item| {
                        if let Some(item_schema) = items_schema {
                            apply_coercion(item, item_schema)
                        } else {
                            item.clone()
                        }
                    })
                    .collect(),
            )
        }

        Value::String(s) => {
            let schema_type = schema.get("type").and_then(Value::as_str);
            let schema_format = schema.get("format").and_then(Value::as_str);
            match schema_type {
                Some("number") => s
                    .parse::<f64>()
                    .map(|n| serde_json::Number::from_f64(n).map_or(value.clone(), Value::Number))
                    .unwrap_or_else(|_| value.clone()),
                Some("integer") => s
                    .parse::<i64>()
                    .map(|n| Value::Number(n.into()))
                    .unwrap_or_else(|_| value.clone()),
                Some("boolean") => match s.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    _ => value.clone(),
                },
                Some("string") => match schema_format {
                    Some("date-time") => coerce_to_rfc3339(s)
                        .map(Value::String)
                        .unwrap_or_else(|| value.clone()),
                    Some("date") => coerce_to_date(s)
                        .map(Value::String)
                        .unwrap_or_else(|| value.clone()),
                    _ => value.clone(),
                },
                _ => value.clone(),
            }
        }

        Value::Number(_) => {
            let schema_type = schema.get("type").and_then(Value::as_str);
            if schema_type == Some("string") {
                Value::String(value.to_string())
            } else {
                value.clone()
            }
        }

        Value::Bool(b) => {
            let schema_type = schema.get("type").and_then(Value::as_str);
            if schema_type == Some("string") {
                Value::String(b.to_string())
            } else {
                value.clone()
            }
        }

        _ => value.clone(),
    }
}

/// Remove fields from `data` that are not declared in the schema's `properties`.
///
/// Only applies to object values where the schema declares `properties`.
/// Recurses into nested objects whose property schemas also declare `properties`.
fn filter_extra_fields(data: &mut Value, schema: &Value) {
    if let Value::Object(map) = data {
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            map.retain(|key, _| properties.contains_key(key));

            // Recurse into nested objects
            for (key, val) in map.iter_mut() {
                if let Some(prop_schema) = properties.get(key) {
                    filter_extra_fields(val, prop_schema);
                }
            }
        }
    }

    // Recurse into array items
    if let Value::Array(arr) = data {
        if let Some(items_schema) = schema.get("items") {
            for item in arr.iter_mut() {
                filter_extra_fields(item, items_schema);
            }
        }
    }
}

/// Format a JSON Schema validation error without leaking instance data.
///
/// Mirrors the safe formatting from the transform executor — only structural
/// information (field path and constraint violated) is emitted, never the
/// actual value that failed validation.
fn format_validation_error(e: &jsonschema::ValidationError<'_>) -> String {
    use jsonschema::error::ValidationErrorKind;

    let path = e.instance_path.to_string();
    let at = if path.is_empty() {
        String::new()
    } else {
        format!("at {path}: ")
    };

    let constraint = match &e.kind {
        ValidationErrorKind::Type { kind } => format!("expected type {kind:?}"),
        ValidationErrorKind::Required { property } => {
            format!("missing required property {property}")
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            format!("unexpected properties: {}", unexpected.join(", "))
        }
        ValidationErrorKind::MinLength { limit } => {
            format!("string length below minimum of {limit}")
        }
        ValidationErrorKind::MaxLength { limit } => {
            format!("string length exceeds maximum of {limit}")
        }
        ValidationErrorKind::Minimum { limit } => {
            format!("value below minimum of {limit}")
        }
        ValidationErrorKind::Maximum { limit } => {
            format!("value exceeds maximum of {limit}")
        }
        ValidationErrorKind::ExclusiveMinimum { limit } => {
            format!("value at or below exclusive minimum of {limit}")
        }
        ValidationErrorKind::ExclusiveMaximum { limit } => {
            format!("value at or above exclusive maximum of {limit}")
        }
        ValidationErrorKind::MinItems { limit } => {
            format!("array has fewer than {limit} items")
        }
        ValidationErrorKind::MaxItems { limit } => {
            format!("array has more than {limit} items")
        }
        ValidationErrorKind::MinProperties { limit } => {
            format!("object has fewer than {limit} properties")
        }
        ValidationErrorKind::MaxProperties { limit } => {
            format!("object has more than {limit} properties")
        }
        ValidationErrorKind::Pattern { pattern } => {
            format!("value does not match pattern {pattern:?}")
        }
        ValidationErrorKind::MultipleOf { multiple_of } => {
            format!("value is not a multiple of {multiple_of}")
        }
        ValidationErrorKind::Enum { options } => {
            format!("value not in enum {options}")
        }
        ValidationErrorKind::Constant { expected_value } => {
            format!("value does not match const {expected_value}")
        }
        ValidationErrorKind::Format { format } => {
            format!("value does not match format {format:?}")
        }
        ValidationErrorKind::UniqueItems => "array contains duplicate items".into(),
        ValidationErrorKind::FalseSchema => "value rejected by false schema".into(),
        ValidationErrorKind::Not { .. } => "value matched a negated schema".into(),
        ValidationErrorKind::AnyOf => "value does not match any 'anyOf' schemas".into(),
        ValidationErrorKind::OneOfNotValid => "value does not match any 'oneOf' schema".into(),
        ValidationErrorKind::OneOfMultipleValid => "value matches multiple 'oneOf' schemas".into(),
        ValidationErrorKind::Contains => "array does not contain a required element".into(),
        // Fallback: use Debug which is less likely to embed raw values than Display
        _ => format!("{:?}", e.kind),
    };

    format!("{at}{constraint}")
}

/// Attempt to coerce a string into RFC 3339 date-time format.
///
/// Handles common date formats that appear at trust boundaries:
/// - Already RFC 3339: `"2026-03-05T14:30:00Z"` (passed through)
/// - ISO 8601 without timezone: `"2026-03-05T14:30:00"` (assumes UTC)
/// - Date only: `"2026-03-05"` (becomes `"2026-03-05T00:00:00Z"`)
/// - US format: `"03/05/2026"` (MM/DD/YYYY → UTC midnight)
/// - European format: `"05.03.2026"` (DD.MM.YYYY → UTC midnight)
/// - Unix epoch seconds as string: `"1709683200"` (→ RFC 3339)
///
/// Returns `None` if the string cannot be recognized as a date, leaving it
/// unchanged so JSON Schema validation will report the format error.
fn coerce_to_rfc3339(s: &str) -> Option<String> {
    use chrono::{DateTime, NaiveDateTime, Utc};

    let trimmed = s.trim();

    // Already valid RFC 3339 — pass through
    if DateTime::parse_from_rfc3339(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    // ISO 8601 without timezone offset → assume UTC
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(
            naive
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        );
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(
            naive
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        );
    }

    // Date-only ISO → midnight UTC
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let dt = date.and_hms_opt(0, 0, 0)?.and_utc();
        return Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    }

    // US format MM/DD/YYYY
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
        let dt = date.and_hms_opt(0, 0, 0)?.and_utc();
        return Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    }

    // European format DD.MM.YYYY
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%d.%m.%Y") {
        let dt = date.and_hms_opt(0, 0, 0)?.and_utc();
        return Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    }

    // Unix epoch seconds as string
    if let Ok(epoch) = trimmed.parse::<i64>() {
        if let Some(dt) = DateTime::<Utc>::from_timestamp(epoch, 0) {
            return Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        }
    }

    // Unix epoch seconds with fractional part
    if let Ok(epoch_f) = trimmed.parse::<f64>() {
        let secs = epoch_f.trunc() as i64;
        let nanos = ((epoch_f.fract()) * 1e9) as u32;
        if let Some(dt) = DateTime::<Utc>::from_timestamp(secs, nanos) {
            return Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        }
    }

    None
}

/// Attempt to coerce a string into ISO 8601 date format (YYYY-MM-DD).
///
/// Handles:
/// - Already correct: `"2026-03-05"` (passed through)
/// - Full date-time: `"2026-03-05T14:30:00Z"` (extracts date portion)
/// - US format: `"03/05/2026"` (MM/DD/YYYY)
/// - European format: `"05.03.2026"` (DD.MM.YYYY)
///
/// Returns `None` if unrecognizable.
fn coerce_to_date(s: &str) -> Option<String> {
    use chrono::DateTime;

    let trimmed = s.trim();

    // Already YYYY-MM-DD
    if NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return Some(trimmed.to_string());
    }

    // Full RFC 3339 date-time → extract date
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.format("%Y-%m-%d").to_string());
    }

    // ISO 8601 without timezone → extract date
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(naive.format("%Y-%m-%d").to_string());
    }

    // US format MM/DD/YYYY
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
        return Some(date.format("%Y-%m-%d").to_string());
    }

    // European format DD.MM.YYYY
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%d.%m.%Y") {
        return Some(date.format("%Y-%m-%d").to_string());
    }

    None
}

#[cfg(test)]
mod tests;
