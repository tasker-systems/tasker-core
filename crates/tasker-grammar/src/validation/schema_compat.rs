//! JSON Schema structural compatibility checking.
//!
//! Implements the "structural subtyping" model for contract chaining:
//! the producer must provide a superset of what the consumer requires.
//!
//! - Every **required** field in the consumer must exist in the producer
//! - Shared fields must have **compatible types**
//! - Extra fields in the producer are permitted
//! - Optional fields in the consumer may be absent from the producer

use serde_json::Value;

use crate::types::{Severity, ValidationFinding};

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

    // Check required fields
    let consumer_required = consumer
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let producer_properties = producer
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let consumer_properties = consumer
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for req_field in &consumer_required {
        let Some(field_name) = req_field.as_str() else {
            continue;
        };

        if !producer_properties.contains_key(field_name) {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "MISSING_REQUIRED_FIELD".to_owned(),
                invocation_index,
                message: format!(
                    "{context_label}: consumer requires field '{field_name}' but producer output does not contain it"
                ),
                field_path: Some(field_name.to_owned()),
            });
            continue;
        }

        // Check type compatibility for shared fields
        if let (Some(producer_prop), Some(consumer_prop)) = (
            producer_properties.get(field_name),
            consumer_properties.get(field_name),
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
    for (field_name, consumer_prop) in &consumer_properties {
        if consumer_required
            .iter()
            .any(|r| r.as_str() == Some(field_name))
        {
            continue; // Already checked above
        }
        if let Some(producer_prop) = producer_properties.get(field_name) {
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
    let producer_types = extract_types(producer);
    let consumer_types = extract_types(consumer);

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
        if let (Some(producer_items), Some(consumer_items)) =
            (producer.get("items"), consumer.get("items"))
        {
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

/// Extract the type(s) declared by a JSON Schema property.
///
/// Handles both `"type": "string"` and `"type": ["string", "null"]` forms.
fn extract_types(schema: &Value) -> Vec<String> {
    match schema.get("type") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        _ => Vec::new(),
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
