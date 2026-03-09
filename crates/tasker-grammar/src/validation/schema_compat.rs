//! JSON Schema structural compatibility checking.
//!
//! Implements the "structural subtyping" model for contract chaining:
//! the producer must provide a superset of what the consumer requires.
//!
//! - Every **required** field in the consumer must exist in the producer
//! - Shared fields must have **compatible types**
//! - Extra fields in the producer are permitted
//! - Optional fields in the consumer may be absent from the producer

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::types::{Severity, ValidationFinding};

// ---------------------------------------------------------------------------
// JSON Schema type model
// ---------------------------------------------------------------------------

/// Well-known JSON Schema primitive types.
///
/// Compatibility is expressed through [`JsonType::satisfies`] rather than
/// string comparisons, making the subtyping rules explicit and exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JsonType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

impl JsonType {
    /// Parse from the JSON Schema type name. Returns `None` for unrecognized types.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "string" => Some(Self::String),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "object" => Some(Self::Object),
            "array" => Some(Self::Array),
            "null" => Some(Self::Null),
            _ => None,
        }
    }

    /// Whether this producer type satisfies a consumer expecting `consumer_type`.
    ///
    /// Structural subtyping rules:
    /// - Same type always satisfies
    /// - `Integer` satisfies `Number` (integers are a subset of numbers)
    fn satisfies(self, consumer_type: Self) -> bool {
        self == consumer_type || (self == Self::Integer && consumer_type == Self::Number)
    }
}

impl fmt::Display for JsonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Integer => write!(f, "integer"),
            Self::Boolean => write!(f, "boolean"),
            Self::Object => write!(f, "object"),
            Self::Array => write!(f, "array"),
            Self::Null => write!(f, "null"),
        }
    }
}

// ---------------------------------------------------------------------------
// Schema shape model
// ---------------------------------------------------------------------------

/// JSON Schema `type` field — either `"string"` or `["string", "null"]`.
///
/// Deserialized as raw strings, then converted to [`JsonType`] via
/// [`SchemaTypeDecl::to_json_types`]. Unrecognized type strings are silently
/// filtered (the schema spec is finite; extensions are ignored).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SchemaTypeDecl {
    Single(String),
    Multiple(Vec<String>),
}

impl SchemaTypeDecl {
    fn to_json_types(&self) -> Vec<JsonType> {
        match self {
            Self::Single(s) => JsonType::parse(s).into_iter().collect(),
            Self::Multiple(v) => v.iter().filter_map(|s| JsonType::parse(s)).collect(),
        }
    }
}

/// Typed representation of the JSON Schema subset used in compatibility checking.
///
/// Rather than imperatively picking fields from raw `Value` via `.get()` chains,
/// we deserialize the known JSON Schema fields into this struct, letting serde
/// handle missing-field defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SchemaShape {
    /// Fields required by this schema.
    required: Vec<String>,

    /// Property definitions keyed by field name. Values are sub-schemas
    /// (kept as `Value` for recursive compatibility checking).
    properties: HashMap<String, Value>,

    /// The `type` declaration — single string or array of strings.
    #[serde(rename = "type")]
    schema_type: Option<SchemaTypeDecl>,

    /// Schema for array items (kept as `Value` for recursive checking).
    items: Option<Box<Value>>,
}

impl SchemaShape {
    /// Attempt to parse a meaningful schema from a JSON value.
    ///
    /// Returns `None` for null values and empty objects (no schema requirements),
    /// `Some` for non-trivial schemas.
    fn from_value(value: &Value) -> Option<Self> {
        if value.is_null() {
            return None;
        }
        if value.as_object().is_some_and(|o| o.is_empty()) {
            return None;
        }
        serde_json::from_value(value.clone()).ok()
    }

    /// The declared JSON types, empty if `type` is absent or unrecognized.
    fn types(&self) -> Vec<JsonType> {
        self.schema_type
            .as_ref()
            .map_or_else(Vec::new, SchemaTypeDecl::to_json_types)
    }
}

// ---------------------------------------------------------------------------
// Compatibility checking
// ---------------------------------------------------------------------------

/// Check that a producer schema is structurally compatible with a consumer schema.
///
/// Returns validation findings for any incompatibilities found. An empty
/// result means the schemas are compatible.
pub(crate) fn check_schema_compatibility(
    producer: &Value,
    consumer: &Value,
    context_label: &str,
    invocation_index: Option<usize>,
) -> Vec<ValidationFinding> {
    let mut findings = Vec::new();

    let Some(consumer_shape) = SchemaShape::from_value(consumer) else {
        // Consumer has no schema requirements — everything is compatible
        return findings;
    };

    let Some(producer_shape) = SchemaShape::from_value(producer) else {
        // Producer has no schema but consumer does — that's a problem
        findings.push(ValidationFinding {
            severity: Severity::Error,
            code: "MISSING_PRODUCER_SCHEMA".to_owned(),
            invocation_index,
            message: format!(
                "{context_label}: consumer expects a schema but producer declares none"
            ),
            field_path: None,
        });
        return findings;
    };

    // Check required fields
    for field_name in &consumer_shape.required {
        if !producer_shape.properties.contains_key(field_name) {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "MISSING_REQUIRED_FIELD".to_owned(),
                invocation_index,
                message: format!(
                    "{context_label}: consumer requires field '{field_name}' but producer output does not contain it"
                ),
                field_path: Some(field_name.clone()),
            });
            continue;
        }

        // Check type compatibility for shared fields
        if let (Some(producer_prop), Some(consumer_prop)) = (
            producer_shape.properties.get(field_name),
            consumer_shape.properties.get(field_name),
        ) {
            check_type_compatibility(
                producer_prop,
                consumer_prop,
                field_name,
                context_label,
                invocation_index,
                &mut findings,
            );
        }
    }

    // Check type compatibility for optional shared fields (warnings only)
    for (field_name, consumer_prop) in &consumer_shape.properties {
        if consumer_shape.required.iter().any(|r| r == field_name) {
            continue; // Already checked above
        }
        if let Some(producer_prop) = producer_shape.properties.get(field_name) {
            let mut type_findings = Vec::new();
            check_type_compatibility(
                producer_prop,
                consumer_prop,
                field_name,
                context_label,
                invocation_index,
                &mut type_findings,
            );
            // Downgrade to warnings for optional fields
            for mut finding in type_findings {
                finding.severity = Severity::Warning;
                findings.push(finding);
            }
        }
    }

    findings
}

/// Check that a producer property type is compatible with a consumer property type.
fn check_type_compatibility(
    producer: &Value,
    consumer: &Value,
    field_name: &str,
    context_label: &str,
    invocation_index: Option<usize>,
    findings: &mut Vec<ValidationFinding>,
) {
    // Parse just the type-relevant fields from each property sub-schema.
    // from_value returns None for null/empty; unwrap_or_default gives us
    // empty types() which the early-return below handles.
    let producer_shape = SchemaShape::from_value(producer).unwrap_or_default();
    let consumer_shape = SchemaShape::from_value(consumer).unwrap_or_default();

    let producer_types = producer_shape.types();
    let consumer_types = consumer_shape.types();

    // If either side has no type declaration, skip type checking
    if producer_types.is_empty() || consumer_types.is_empty() {
        return;
    }

    // Check that every consumer type is satisfied by at least one producer type
    for consumer_type in &consumer_types {
        let compatible = producer_types
            .iter()
            .any(|pt| pt.satisfies(*consumer_type));

        if !compatible {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "TYPE_MISMATCH".to_owned(),
                invocation_index,
                message: format!(
                    "{context_label}: field '{field_name}' type mismatch — producer provides [{}] but consumer expects '{consumer_type}'",
                    format_type_list(&producer_types),
                ),
                field_path: Some(field_name.to_owned()),
            });
        }
    }

    // Recurse into nested objects
    if producer_types.contains(&JsonType::Object) && consumer_types.contains(&JsonType::Object) {
        let nested =
            check_schema_compatibility(producer, consumer, context_label, invocation_index);
        findings.extend(nested);
    }

    // Check array item compatibility
    if producer_types.contains(&JsonType::Array) && consumer_types.contains(&JsonType::Array) {
        if let (Some(producer_items), Some(consumer_items)) = (
            producer_shape.items.as_deref(),
            consumer_shape.items.as_deref(),
        ) {
            let item_context = format!("{context_label}[].{field_name}");
            let nested = check_schema_compatibility(
                producer_items,
                consumer_items,
                &item_context,
                invocation_index,
            );
            findings.extend(nested);
        }
    }
}

fn format_type_list(types: &[JsonType]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
