//! Task template validation: structural checks, cycle detection, and DAG analysis.
//!
//! Validates a [`TaskTemplate`] for correctness without requiring any external
//! services. All checks are pure functions over the template structure.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Severity level for a validation finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single validation finding.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationFinding {
    /// Machine-readable code (e.g., `DUPLICATE_STEP_NAME`).
    pub code: String,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Step name involved, if applicable.
    pub step: Option<String>,
}

/// Complete validation report for a task template.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// Whether the template is valid (no errors).
    pub valid: bool,
    /// All findings (errors, warnings, info).
    pub findings: Vec<ValidationFinding>,
    /// Number of steps in the template.
    pub step_count: usize,
    /// Whether a dependency cycle was detected.
    pub has_cycles: bool,
}

/// Validate a task template and return a detailed report.
pub fn validate(template: &TaskTemplate) -> ValidationReport {
    let mut findings = Vec::new();
    let mut has_cycles = false;

    check_duplicate_step_names(template, &mut findings);
    check_dependencies(template, &mut findings);
    check_handlers(template, &mut findings);
    check_namespace_length(template, &mut findings);
    check_schemas(template, &mut findings);
    check_orphan_steps(template, &mut findings);

    if let Some(cycle_findings) = check_cycles(template) {
        has_cycles = true;
        findings.extend(cycle_findings);
    }

    let valid = !findings.iter().any(|f| f.severity == Severity::Error);

    ValidationReport {
        valid,
        findings,
        step_count: template.steps.len(),
        has_cycles,
    }
}

fn check_duplicate_step_names(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    let mut seen = HashSet::new();
    for step in &template.steps {
        if !seen.insert(&step.name) {
            findings.push(ValidationFinding {
                code: "DUPLICATE_STEP_NAME".into(),
                severity: Severity::Error,
                message: format!("Duplicate step name: '{}'", step.name),
                step: Some(step.name.clone()),
            });
        }
    }
}

fn check_dependencies(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    let step_names: HashSet<&str> = template.steps.iter().map(|s| s.name.as_str()).collect();

    for step in &template.steps {
        for dep in &step.dependencies {
            if dep == &step.name {
                findings.push(ValidationFinding {
                    code: "SELF_DEPENDENCY".into(),
                    severity: Severity::Error,
                    message: format!("Step '{}' depends on itself", step.name),
                    step: Some(step.name.clone()),
                });
            } else if !step_names.contains(dep.as_str()) {
                findings.push(ValidationFinding {
                    code: "MISSING_DEP_REF".into(),
                    severity: Severity::Error,
                    message: format!(
                        "Step '{}' depends on '{}' which does not exist",
                        step.name, dep
                    ),
                    step: Some(step.name.clone()),
                });
            }
        }
    }
}

fn check_handlers(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    for step in &template.steps {
        if step.handler.callable.is_empty() {
            findings.push(ValidationFinding {
                code: "EMPTY_CALLABLE".into(),
                severity: Severity::Error,
                message: format!("Step '{}' has an empty handler callable", step.name),
                step: Some(step.name.clone()),
            });
        }
    }
}

fn check_namespace_length(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    if template.namespace_name.len() > 29 {
        findings.push(ValidationFinding {
            code: "NAMESPACE_TOO_LONG".into(),
            severity: Severity::Warning,
            message: format!(
                "Namespace '{}' is {} chars (max 29 for PGMQ queue names)",
                template.namespace_name,
                template.namespace_name.len()
            ),
            step: None,
        });
    }
}

fn check_schemas(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    for step in &template.steps {
        match &step.result_schema {
            Some(schema) => {
                if let Some(type_val) = schema.get("type") {
                    if type_val.as_str() != Some("object") {
                        findings.push(ValidationFinding {
                            code: "SCHEMA_NOT_OBJECT".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "Step '{}' result_schema type is '{}', expected 'object'",
                                step.name,
                                type_val.as_str().unwrap_or("unknown")
                            ),
                            step: Some(step.name.clone()),
                        });
                    }
                }
            }
            None => {
                findings.push(ValidationFinding {
                    code: "NO_RESULT_SCHEMA".into(),
                    severity: Severity::Info,
                    message: format!("Step '{}' has no result_schema defined", step.name),
                    step: Some(step.name.clone()),
                });
            }
        }
    }
}

fn check_orphan_steps(template: &TaskTemplate, findings: &mut Vec<ValidationFinding>) {
    if template.steps.len() <= 1 {
        return;
    }

    let depended_on: HashSet<&str> = template
        .steps
        .iter()
        .flat_map(|s| s.dependencies.iter().map(|d| d.as_str()))
        .collect();

    for step in &template.steps {
        let has_deps = !step.dependencies.is_empty();
        let is_depended_on = depended_on.contains(step.name.as_str());

        if !has_deps && !is_depended_on {
            findings.push(ValidationFinding {
                code: "ORPHAN_STEP".into(),
                severity: Severity::Warning,
                message: format!(
                    "Step '{}' has no dependencies and nothing depends on it",
                    step.name
                ),
                step: Some(step.name.clone()),
            });
        }
    }
}

/// Detect cycles using DFS with white/gray/black coloring.
/// Returns `Some(findings)` if cycles found, `None` otherwise.
fn check_cycles(template: &TaskTemplate) -> Option<Vec<ValidationFinding>> {
    let step_names: Vec<&str> = template.steps.iter().map(|s| s.name.as_str()).collect();
    let adj: HashMap<&str, Vec<&str>> = template
        .steps
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.dependencies.iter().map(|d| d.as_str()).collect(),
            )
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut colors: HashMap<&str, Color> = step_names.iter().map(|&n| (n, Color::White)).collect();
    let mut findings = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&str, Vec<&'a str>>,
        colors: &mut HashMap<&'a str, Color>,
        path: &mut Vec<&'a str>,
        findings: &mut Vec<ValidationFinding>,
    ) {
        colors.insert(node, Color::Gray);
        path.push(node);

        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                match colors.get(neighbor) {
                    Some(Color::Gray) => {
                        // Found a cycle — extract the cycle path
                        let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                        let cycle: Vec<&str> = path[cycle_start..].to_vec();
                        findings.push(ValidationFinding {
                            code: "CYCLE_DETECTED".into(),
                            severity: Severity::Error,
                            message: format!(
                                "Dependency cycle detected: {} -> {}",
                                cycle.join(" -> "),
                                neighbor
                            ),
                            step: Some(neighbor.to_string()),
                        });
                    }
                    Some(Color::White) | None => {
                        dfs(neighbor, adj, colors, path, findings);
                    }
                    Some(Color::Black) => {}
                }
            }
        }

        path.pop();
        colors.insert(node, Color::Black);
    }

    for &name in &step_names {
        if colors.get(name) == Some(&Color::White) {
            let mut path = Vec::new();
            dfs(name, &adj, &mut colors, &mut path, &mut findings);
        }
    }

    if findings.is_empty() {
        None
    } else {
        Some(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    #[test]
    fn test_valid_template_passes() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(report.valid);
        assert!(!report.has_cycles);
        assert_eq!(report.step_count, 5);
    }

    #[test]
    fn test_cycle_detected() {
        let yaml = r#"
name: cycle_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    depends_on:
      - step_b
  - name: step_b
    handler:
      callable: test.step_b
    depends_on:
      - step_a
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(!report.valid);
        assert!(report.has_cycles);
        assert!(report.findings.iter().any(|f| f.code == "CYCLE_DETECTED"));
    }

    #[test]
    fn test_missing_dep_ref() {
        let yaml = r#"
name: missing_dep
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    depends_on:
      - nonexistent_step
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(!report.valid);
        assert!(report.findings.iter().any(|f| f.code == "MISSING_DEP_REF"));
    }

    #[test]
    fn test_self_dependency() {
        let yaml = r#"
name: self_dep
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    depends_on:
      - step_a
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(!report.valid);
        assert!(report.findings.iter().any(|f| f.code == "SELF_DEPENDENCY"));
    }

    #[test]
    fn test_duplicate_step_names() {
        let yaml = r#"
name: dupe_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
  - name: step_a
    handler:
      callable: test.step_a_v2
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(!report.valid);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "DUPLICATE_STEP_NAME"));
    }

    #[test]
    fn test_namespace_too_long() {
        let yaml = r#"
name: ns_test
namespace_name: this_namespace_is_way_too_long_for_pgmq
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "NAMESPACE_TOO_LONG"));
    }

    #[test]
    fn test_orphan_step() {
        let yaml = r#"
name: orphan_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
  - name: step_b
    handler:
      callable: test.step_b
    depends_on:
      - step_a
  - name: step_orphan
    handler:
      callable: test.step_orphan
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "ORPHAN_STEP" && f.step.as_deref() == Some("step_orphan")));
    }

    #[test]
    fn test_no_result_schema_info() {
        let yaml = r#"
name: no_schema
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(report.valid);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "NO_RESULT_SCHEMA" && f.severity == Severity::Info));
    }

    #[test]
    fn test_schema_not_object() {
        let yaml = r#"
name: bad_schema
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: array
      items:
        type: string
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "SCHEMA_NOT_OBJECT"));
    }

    #[test]
    fn test_empty_callable() {
        let yaml = r#"
name: empty_callable
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: ""
"#;
        let template = parse_template_str(yaml).unwrap();
        let report = validate(&template);
        assert!(!report.valid);
        assert!(report.findings.iter().any(|f| f.code == "EMPTY_CALLABLE"));
    }
}
