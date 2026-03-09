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

use serde::Deserialize;
use serde_json::Value;

use crate::types::{Severity, ValidationFinding};

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
    /// Deserialize from a JSON Schema value, falling back to empty defaults
    /// for non-object values or unrecognized structures.
    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    /// Extract the type declarations as a list of strings.
    fn type_names(&self) -> Vec<String> {
        self.schema_type
            .as_ref()
            .map_or_else(Vec::new, SchemaTypeDecl::to_vec)
    }
}

/// JSON Schema `type` field — either `"string"` or `["string", "null"]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SchemaTypeDecl {
    Single(String),
    Multiple(Vec<String>),
}

impl SchemaTypeDecl {
    fn to_vec(&self) -> Vec<String> {
        match self {
            Self::Single(s) => vec![s.clone()],
            Self::Multiple(v) => v.clone(),
        }
    }
}

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

    // If consumer has no schema requirements, everything is compatible
    if consumer.is_null() || consumer.as_object().is_none_or(|o| o.is_empty()) {
        return findings;
    }

    // If producer has no schema but consumer does, that's a problem
    if producer.is_null() || producer.as_object().is_none_or(|o| o.is_empty()) {
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
    }

    let producer_shape = SchemaShape::from_value(producer);
    let consumer_shape = SchemaShape::from_value(consumer);

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
    let producer_shape = SchemaShape::from_value(producer);
    let consumer_shape = SchemaShape::from_value(consumer);

    let producer_types = producer_shape.type_names();
    let consumer_types = consumer_shape.type_names();

    // If either side has no type declaration, skip type checking
    if producer_types.is_empty() || consumer_types.is_empty() {
        return;
    }

    // Check that every consumer type is satisfied by at least one producer type
    for consumer_type in &consumer_types {
        let compatible = producer_types
            .iter()
            .any(|pt| types_compatible(pt, consumer_type));

        if !compatible {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "TYPE_MISMATCH".to_owned(),
                invocation_index,
                message: format!(
                    "{context_label}: field '{field_name}' type mismatch — producer provides {producer_types:?} but consumer expects '{consumer_type}'"
                ),
                field_path: Some(field_name.to_owned()),
            });
        }
    }

    // Recurse into nested objects
    if producer_types.contains(&"object".to_owned())
        && consumer_types.contains(&"object".to_owned())
    {
        let nested =
            check_schema_compatibility(producer, consumer, context_label, invocation_index);
        findings.extend(nested);
    }

    // Check array item compatibility
    if producer_types.contains(&"array".to_owned()) && consumer_types.contains(&"array".to_owned())
    {
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

/// Check if two JSON Schema types are compatible.
///
/// Uses structural subtyping rules:
/// - Same type is always compatible
/// - `integer` is compatible with `number` (integers are a subset of numbers)
/// - `null` in producer is compatible with `null` in consumer (nullable fields)
fn types_compatible(producer_type: &str, consumer_type: &str) -> bool {
    if producer_type == consumer_type {
        return true;
    }

    // integer → number compatibility (integers are a subset of numbers)
    if producer_type == "integer" && consumer_type == "number" {
        return true;
    }

    false
}
