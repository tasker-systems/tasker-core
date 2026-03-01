//! Schema comparison for producer/consumer compatibility checking.
//!
//! Compares JSON Schema `result_schema` definitions between steps to detect
//! breaking changes and compatibility issues in the data contract.

use serde::Serialize;
use serde_json::Value;

/// Compatibility level between producer and consumer schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Compatible,
    CompatibleWithWarnings,
    Incompatible,
}

/// A single comparison finding.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonFinding {
    /// Machine-readable code.
    pub code: String,
    /// Whether this is a breaking incompatibility.
    pub breaking: bool,
    /// Dotted field path (e.g., `metadata.source`).
    pub field_path: String,
    /// Human-readable message.
    pub message: String,
}

/// Complete comparison report between two step schemas.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    /// Name of the producing step.
    pub producer_step: String,
    /// Name of the consuming step.
    pub consumer_step: String,
    /// Overall compatibility verdict.
    pub compatibility: Compatibility,
    /// All findings.
    pub findings: Vec<ComparisonFinding>,
}

/// Compare a producer step's output schema against a consumer step's expected input.
pub fn compare_schemas(
    producer_step: &str,
    producer_schema: &Value,
    consumer_step: &str,
    consumer_schema: &Value,
) -> ComparisonReport {
    let mut findings = Vec::new();

    compare_properties(producer_schema, consumer_schema, "", &mut findings);

    let compatibility = if findings.iter().any(|f| f.breaking) {
        Compatibility::Incompatible
    } else if findings.is_empty() {
        Compatibility::Compatible
    } else {
        Compatibility::CompatibleWithWarnings
    };

    ComparisonReport {
        producer_step: producer_step.to_string(),
        consumer_step: consumer_step.to_string(),
        compatibility,
        findings,
    }
}

fn compare_properties(
    producer: &Value,
    consumer: &Value,
    prefix: &str,
    findings: &mut Vec<ComparisonFinding>,
) {
    let producer_props = producer.get("properties").and_then(Value::as_object);
    let consumer_props = consumer.get("properties").and_then(Value::as_object);

    let (Some(consumer_props), producer_props) = (consumer_props, producer_props) else {
        return;
    };

    let consumer_required: std::collections::HashSet<&str> = consumer
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    // Check consumer fields against producer
    for (field_name, consumer_field_schema) in consumer_props {
        let field_path = if prefix.is_empty() {
            field_name.clone()
        } else {
            format!("{prefix}.{field_name}")
        };

        let producer_field_schema = producer_props.and_then(|p| p.get(field_name));

        match producer_field_schema {
            None => {
                let is_required = consumer_required.contains(field_name.as_str());
                if is_required {
                    findings.push(ComparisonFinding {
                        code: "MISSING_REQUIRED_FIELD".into(),
                        breaking: true,
                        field_path,
                        message: format!(
                            "Consumer requires field '{}' not present in producer output",
                            field_name
                        ),
                    });
                } else {
                    findings.push(ComparisonFinding {
                        code: "MISSING_OPTIONAL_FIELD".into(),
                        breaking: false,
                        field_path,
                        message: format!(
                            "Consumer optional field '{}' not present in producer output",
                            field_name
                        ),
                    });
                }
            }
            Some(producer_field) => {
                // Check type compatibility
                let producer_type = producer_field.get("type").and_then(Value::as_str);
                let consumer_type = consumer_field_schema.get("type").and_then(Value::as_str);

                if let (Some(pt), Some(ct)) = (producer_type, consumer_type) {
                    if pt != ct {
                        findings.push(ComparisonFinding {
                            code: "TYPE_MISMATCH".into(),
                            breaking: true,
                            field_path: field_path.clone(),
                            message: format!(
                                "Field '{}' type mismatch: producer has '{}', consumer expects '{}'",
                                field_name, pt, ct
                            ),
                        });
                    } else if ct == "object" {
                        // Recurse into nested objects
                        compare_properties(
                            producer_field,
                            consumer_field_schema,
                            &field_path,
                            findings,
                        );
                    }
                }
            }
        }
    }

    // Check for extra producer fields not referenced by consumer
    if let Some(producer_props) = producer_props {
        let consumer_field_names: std::collections::HashSet<&str> =
            consumer_props.keys().map(|k| k.as_str()).collect();

        for field_name in producer_props.keys() {
            if !consumer_field_names.contains(field_name.as_str()) {
                let field_path = if prefix.is_empty() {
                    field_name.clone()
                } else {
                    format!("{prefix}.{field_name}")
                };

                findings.push(ComparisonFinding {
                    code: "EXTRA_PRODUCER_FIELD".into(),
                    breaking: false,
                    field_path,
                    message: format!(
                        "Producer field '{}' is not referenced by consumer",
                        field_name
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compatible_schemas() {
        let producer = json!({
            "type": "object",
            "required": ["id", "name"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" }
            }
        });
        let consumer = json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" }
            }
        });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "EXTRA_PRODUCER_FIELD"));
        assert!(!report.findings.iter().any(|f| f.breaking));
    }

    #[test]
    fn test_exact_match() {
        let schema = json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" }
            }
        });

        let report = compare_schemas("step_a", &schema, "step_b", &schema);
        assert_eq!(report.compatibility, Compatibility::Compatible);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn test_missing_required_field() {
        let producer = json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            }
        });
        let consumer = json!({
            "type": "object",
            "required": ["id", "email"],
            "properties": {
                "id": { "type": "string" },
                "email": { "type": "string" }
            }
        });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "MISSING_REQUIRED_FIELD" && f.field_path == "email"));
    }

    #[test]
    fn test_type_mismatch() {
        let producer = json!({
            "type": "object",
            "properties": {
                "count": { "type": "string" }
            }
        });
        let consumer = json!({
            "type": "object",
            "required": ["count"],
            "properties": {
                "count": { "type": "integer" }
            }
        });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report.findings.iter().any(|f| f.code == "TYPE_MISMATCH"));
    }

    #[test]
    fn test_extra_producer_fields() {
        let producer = json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "internal_id": { "type": "integer" },
                "debug_info": { "type": "string" }
            }
        });
        let consumer = json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" }
            }
        });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        let extra: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "EXTRA_PRODUCER_FIELD")
            .collect();
        assert_eq!(extra.len(), 2);
    }

    #[test]
    fn test_nested_comparison() {
        let producer = json!({
            "type": "object",
            "properties": {
                "metadata": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string" },
                        "confidence": { "type": "number" }
                    }
                }
            }
        });
        let consumer = json!({
            "type": "object",
            "required": ["metadata"],
            "properties": {
                "metadata": {
                    "type": "object",
                    "required": ["source", "version"],
                    "properties": {
                        "source": { "type": "string" },
                        "version": { "type": "integer" }
                    }
                }
            }
        });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "MISSING_REQUIRED_FIELD" && f.field_path == "metadata.version"));
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "EXTRA_PRODUCER_FIELD" && f.field_path == "metadata.confidence"));
    }

    #[test]
    fn test_both_empty() {
        let producer = json!({ "type": "object" });
        let consumer = json!({ "type": "object" });

        let report = compare_schemas("step_a", &producer, "step_b", &consumer);
        assert_eq!(report.compatibility, Compatibility::Compatible);
        assert!(report.findings.is_empty());
    }
}
