//! Temporal schema diff for detecting field-level changes between template versions.
//!
//! Unlike [`schema_comparator`](crate::schema_comparator) which compares two *different* steps'
//! schemas within a single template, this module compares two *versions* of the same template
//! to detect field additions, removals, type changes, and required/optional status changes.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;
use tasker_shared::models::core::task_template::TaskTemplate;

use crate::schema_comparator::Compatibility;

/// A single diff finding for a field or step.
#[derive(Debug, Clone, Serialize)]
pub struct DiffFinding {
    /// Machine-readable change code.
    pub code: String,
    /// Whether this change is a breaking change.
    pub breaking: bool,
    /// Dotted field path (empty for step-level findings).
    pub field_path: String,
    /// Type in the before version (null for additions).
    pub before_type: Option<String>,
    /// Type in the after version (null for removals).
    pub after_type: Option<String>,
    /// Human-readable description.
    pub message: String,
}

/// Diff results for a single step.
#[derive(Debug, Clone, Serialize)]
pub struct StepDiff {
    /// Step name.
    pub step_name: String,
    /// Overall status: added, removed, modified, unchanged.
    pub status: String,
    /// Individual field-level findings.
    pub findings: Vec<DiffFinding>,
}

/// Complete diff report between two template versions.
#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    /// Template name from the before version.
    pub before_template: String,
    /// Template name from the after version.
    pub after_template: String,
    /// Overall compatibility verdict.
    pub compatibility: Compatibility,
    /// Per-step diffs (steps with no changes are omitted unless selected by step_filter).
    pub step_diffs: Vec<StepDiff>,
}

/// Compare two versions of a task template and report field-level changes.
///
/// When `step_filter` is `Some`, only the named step is included in the report.
/// Steps with no changes are omitted unless explicitly selected by `step_filter`.
pub fn diff_templates(
    before: &TaskTemplate,
    after: &TaskTemplate,
    step_filter: Option<&str>,
) -> DiffReport {
    let before_steps: HashMap<&str, _> =
        before.steps.iter().map(|s| (s.name.as_str(), s)).collect();
    let after_steps: HashMap<&str, _> = after.steps.iter().map(|s| (s.name.as_str(), s)).collect();

    let all_step_names: HashSet<&str> = before_steps
        .keys()
        .chain(after_steps.keys())
        .copied()
        .collect();

    let mut step_diffs = Vec::new();

    for &step_name in &all_step_names {
        if let Some(filter) = step_filter {
            if step_name != filter {
                continue;
            }
        }

        let before_step = before_steps.get(step_name);
        let after_step = after_steps.get(step_name);

        match (before_step, after_step) {
            (None, Some(_)) => {
                step_diffs.push(StepDiff {
                    step_name: step_name.to_string(),
                    status: "added".to_string(),
                    findings: vec![DiffFinding {
                        code: "STEP_ADDED".to_string(),
                        breaking: false,
                        field_path: String::new(),
                        before_type: None,
                        after_type: None,
                        message: format!("Step '{step_name}' was added"),
                    }],
                });
            }
            (Some(_), None) => {
                step_diffs.push(StepDiff {
                    step_name: step_name.to_string(),
                    status: "removed".to_string(),
                    findings: vec![DiffFinding {
                        code: "STEP_REMOVED".to_string(),
                        breaking: true,
                        field_path: String::new(),
                        before_type: None,
                        after_type: None,
                        message: format!("Step '{step_name}' was removed"),
                    }],
                });
            }
            (Some(before_s), Some(after_s)) => {
                let findings = diff_result_schemas(
                    before_s.result_schema.as_ref(),
                    after_s.result_schema.as_ref(),
                );

                if findings.is_empty() {
                    // Only include unchanged steps if explicitly filtered
                    if step_filter.is_some() {
                        step_diffs.push(StepDiff {
                            step_name: step_name.to_string(),
                            status: "unchanged".to_string(),
                            findings: vec![],
                        });
                    }
                } else {
                    step_diffs.push(StepDiff {
                        step_name: step_name.to_string(),
                        status: "modified".to_string(),
                        findings,
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }

    // Sort for deterministic output
    step_diffs.sort_by(|a, b| a.step_name.cmp(&b.step_name));

    let compatibility = if step_diffs
        .iter()
        .flat_map(|d| &d.findings)
        .any(|f| f.breaking)
    {
        Compatibility::Incompatible
    } else if step_diffs.iter().all(|d| d.findings.is_empty()) {
        Compatibility::Compatible
    } else {
        Compatibility::CompatibleWithWarnings
    };

    DiffReport {
        before_template: before.name.clone(),
        after_template: after.name.clone(),
        compatibility,
        step_diffs,
    }
}

/// Compare two optional result_schema values, producing field-level findings.
fn diff_result_schemas(before: Option<&Value>, after: Option<&Value>) -> Vec<DiffFinding> {
    match (before, after) {
        (None, None) => vec![],
        (None, Some(_)) => vec![DiffFinding {
            code: "SCHEMA_ADDED".to_string(),
            breaking: false,
            field_path: String::new(),
            before_type: None,
            after_type: None,
            message: "Step gained a result_schema".to_string(),
        }],
        (Some(_), None) => vec![DiffFinding {
            code: "SCHEMA_REMOVED".to_string(),
            breaking: true,
            field_path: String::new(),
            before_type: None,
            after_type: None,
            message: "Step lost its result_schema".to_string(),
        }],
        (Some(before_schema), Some(after_schema)) => {
            let mut findings = Vec::new();
            diff_properties(before_schema, after_schema, "", &mut findings);
            findings
        }
    }
}

/// Recursively compare properties between two JSON Schema objects.
fn diff_properties(before: &Value, after: &Value, prefix: &str, findings: &mut Vec<DiffFinding>) {
    let before_props = before.get("properties").and_then(Value::as_object);
    let after_props = after.get("properties").and_then(Value::as_object);

    let before_required: HashSet<&str> = before
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let after_required: HashSet<&str> = after
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    // Collect all field names from both schemas
    let mut all_fields: HashSet<&str> = HashSet::new();
    if let Some(props) = before_props {
        all_fields.extend(props.keys().map(|k| k.as_str()));
    }
    if let Some(props) = after_props {
        all_fields.extend(props.keys().map(|k| k.as_str()));
    }

    // Sort for deterministic output
    let mut all_fields: Vec<&str> = all_fields.into_iter().collect();
    all_fields.sort();

    for field_name in all_fields {
        let field_path = if prefix.is_empty() {
            field_name.to_string()
        } else {
            format!("{prefix}.{field_name}")
        };

        let before_field = before_props.and_then(|p| p.get(field_name));
        let after_field = after_props.and_then(|p| p.get(field_name));

        match (before_field, after_field) {
            (None, Some(af)) => {
                let after_type = af.get("type").and_then(Value::as_str);
                findings.push(DiffFinding {
                    code: "FIELD_ADDED".to_string(),
                    breaking: false,
                    field_path,
                    before_type: None,
                    after_type: after_type.map(String::from),
                    message: format!("Field '{field_name}' was added"),
                });
            }
            (Some(bf), None) => {
                let before_type = bf.get("type").and_then(Value::as_str);
                let was_required = before_required.contains(field_name);
                findings.push(DiffFinding {
                    code: "FIELD_REMOVED".to_string(),
                    breaking: was_required,
                    field_path,
                    before_type: before_type.map(String::from),
                    after_type: None,
                    message: if was_required {
                        format!("Required field '{field_name}' was removed")
                    } else {
                        format!("Optional field '{field_name}' was removed")
                    },
                });
            }
            (Some(bf), Some(af)) => {
                let before_type = bf.get("type").and_then(Value::as_str);
                let after_type = af.get("type").and_then(Value::as_str);

                // Check type change
                if let (Some(bt), Some(at)) = (before_type, after_type) {
                    if bt != at {
                        findings.push(DiffFinding {
                            code: "TYPE_CHANGED".to_string(),
                            breaking: true,
                            field_path: field_path.clone(),
                            before_type: Some(bt.to_string()),
                            after_type: Some(at.to_string()),
                            message: format!(
                                "Field '{field_name}' type changed from '{bt}' to '{at}'"
                            ),
                        });
                    } else if bt == "object" {
                        // Recurse into nested objects
                        diff_properties(bf, af, &field_path, findings);
                        // Don't check required/optional for objects — fall through
                        // to the required check below for the object field itself
                    }
                }

                // Check required/optional status change
                let was_required = before_required.contains(field_name);
                let is_required = after_required.contains(field_name);

                if was_required && !is_required {
                    findings.push(DiffFinding {
                        code: "REQUIRED_TO_OPTIONAL".to_string(),
                        breaking: false,
                        field_path: field_path.clone(),
                        before_type: before_type.map(String::from),
                        after_type: after_type.map(String::from),
                        message: format!("Field '{field_name}' changed from required to optional"),
                    });
                } else if !was_required && is_required {
                    findings.push(DiffFinding {
                        code: "OPTIONAL_TO_REQUIRED".to_string(),
                        breaking: true,
                        field_path,
                        before_type: before_type.map(String::from),
                        after_type: after_type.map(String::from),
                        message: format!("Field '{field_name}' changed from optional to required"),
                    });
                }
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    fn parse(yaml: &str) -> TaskTemplate {
        parse_template_str(yaml).unwrap()
    }

    fn base_yaml(result_schema_yaml: &str) -> String {
        format!(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
{result_schema_yaml}"#
        )
    }

    fn schema_block(schema: &str) -> String {
        let indented: String = schema
            .lines()
            .map(|line| format!("      {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("    result_schema:\n{indented}")
    }

    #[test]
    fn test_no_changes() {
        let yaml = base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        ));
        let before = parse(&yaml);
        let after = parse(&yaml);

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Compatible);
        assert!(report.step_diffs.is_empty());
    }

    #[test]
    fn test_field_added() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string
  name:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        assert_eq!(report.step_diffs.len(), 1);
        assert_eq!(report.step_diffs[0].status, "modified");
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "FIELD_ADDED" && f.field_path == "name" && !f.breaking));
    }

    #[test]
    fn test_required_field_removed() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id, name]
properties:
  id:
    type: string
  name:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        let finding = report.step_diffs[0]
            .findings
            .iter()
            .find(|f| f.code == "FIELD_REMOVED")
            .unwrap();
        assert!(finding.breaking);
        assert_eq!(finding.before_type.as_deref(), Some("string"));
        assert!(finding.after_type.is_none());
    }

    #[test]
    fn test_optional_field_removed() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string
  debug:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        let finding = report.step_diffs[0]
            .findings
            .iter()
            .find(|f| f.code == "FIELD_REMOVED")
            .unwrap();
        assert!(!finding.breaking);
    }

    #[test]
    fn test_type_changed() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  count:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  count:
    type: integer"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        let finding = report.step_diffs[0]
            .findings
            .iter()
            .find(|f| f.code == "TYPE_CHANGED")
            .unwrap();
        assert!(finding.breaking);
        assert_eq!(finding.before_type.as_deref(), Some("string"));
        assert_eq!(finding.after_type.as_deref(), Some("integer"));
    }

    #[test]
    fn test_required_to_optional() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  id:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        let finding = report.step_diffs[0]
            .findings
            .iter()
            .find(|f| f.code == "REQUIRED_TO_OPTIONAL")
            .unwrap();
        assert!(!finding.breaking);
    }

    #[test]
    fn test_optional_to_required() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  id:
    type: string"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
required: [id]
properties:
  id:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "OPTIONAL_TO_REQUIRED" && f.breaking));
    }

    #[test]
    fn test_step_added() {
        let before = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#,
        );
        let after = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
  - name: step_b
    handler:
      callable: test.step_b
"#,
        );

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::CompatibleWithWarnings);
        assert!(report
            .step_diffs
            .iter()
            .any(|d| d.step_name == "step_b" && d.status == "added"));
    }

    #[test]
    fn test_step_removed() {
        let before = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
  - name: step_b
    handler:
      callable: test.step_b
"#,
        );
        let after = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#,
        );

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report
            .step_diffs
            .iter()
            .any(|d| d.step_name == "step_b" && d.status == "removed"));
    }

    #[test]
    fn test_schema_added() {
        let before = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#,
        );
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  id:
    type: string"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "SCHEMA_ADDED" && !f.breaking));
    }

    #[test]
    fn test_schema_removed() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  id:
    type: string"#,
        )));
        let after = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#,
        );

        let report = diff_templates(&before, &after, None);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "SCHEMA_REMOVED" && f.breaking));
    }

    #[test]
    fn test_nested_field_changes() {
        let before = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  metadata:
    type: object
    required: [source]
    properties:
      source:
        type: string
      confidence:
        type: number"#,
        )));
        let after = parse(&base_yaml(&schema_block(
            r#"type: object
properties:
  metadata:
    type: object
    required: [source]
    properties:
      source:
        type: string
      version:
        type: integer"#,
        )));

        let report = diff_templates(&before, &after, None);
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "FIELD_REMOVED" && f.field_path == "metadata.confidence"));
        assert!(report.step_diffs[0]
            .findings
            .iter()
            .any(|f| f.code == "FIELD_ADDED" && f.field_path == "metadata.version"));
    }

    #[test]
    fn test_step_filter() {
        let before = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      properties:
        id:
          type: string
  - name: step_b
    handler:
      callable: test.step_b
    result_schema:
      type: object
      properties:
        id:
          type: string
"#,
        );
        let after = parse(
            r#"
name: test_template
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      properties:
        id:
          type: string
  - name: step_c
    handler:
      callable: test.step_c
"#,
        );

        // Filter to step "step_a" — should show unchanged
        let report = diff_templates(&before, &after, Some("step_a"));
        assert_eq!(report.step_diffs.len(), 1);
        assert_eq!(report.step_diffs[0].step_name, "step_a");
        assert_eq!(report.step_diffs[0].status, "unchanged");

        // Filter to step "step_b" — should show removed
        let report = diff_templates(&before, &after, Some("step_b"));
        assert_eq!(report.step_diffs.len(), 1);
        assert_eq!(report.step_diffs[0].status, "removed");
    }
}
