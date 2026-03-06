//! Mermaid flowchart generation from TaskTemplate.

use std::collections::HashMap;
use std::fmt::Write;

use tasker_shared::models::core::task_template::{StepType, TaskTemplate};

pub(super) fn generate_mermaid(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
) -> String {
    let mut out = String::new();
    writeln!(out, "graph TD").unwrap();

    // Class definitions for annotated nodes
    if !annotations.is_empty() {
        writeln!(out, "    classDef annotated fill:#fff3cd,stroke:#ffc107").unwrap();
    }

    // Node definitions
    for step in &template.steps {
        let annotation = annotations.get(&step.name);
        let node = format_node(&step.name, step.step_type.clone(), annotation);
        writeln!(out, "    {node}").unwrap();
    }

    writeln!(out).unwrap();

    // Edge definitions (sorted for deterministic output)
    let mut edges: Vec<String> = Vec::new();
    for step in &template.steps {
        let mut deps: Vec<&str> = step.dependencies.iter().map(|s| s.as_str()).collect();
        deps.sort();
        for dep in deps {
            edges.push(format!("    {dep} --> {}", step.name));
        }
    }
    for edge in &edges {
        writeln!(out, "{edge}").unwrap();
    }

    out
}

fn format_node(name: &str, step_type: StepType, annotation: Option<&String>) -> String {
    match (step_type, annotation) {
        (StepType::Decision, Some(text)) => {
            format!("{name}{{\"{}\\n\u{26a0} {text}\"}}", name)
        }
        (StepType::Decision, None) => {
            format!("{name}{{{name}}}")
        }
        (_, Some(text)) => {
            format!("{name}[\"{name}\\n\u{26a0} {text}\"]:::annotated")
        }
        (_, None) => {
            format!("{name}[{name}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    #[test]
    fn test_diamond_dag_generates_valid_mermaid() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        assert!(result.starts_with("graph TD\n"));
        // Should have 4 nodes
        assert!(result.contains("diamond_start_py[diamond_start_py]"));
        assert!(result.contains("diamond_branch_b_py[diamond_branch_b_py]"));
        assert!(result.contains("diamond_branch_c_py[diamond_branch_c_py]"));
        assert!(result.contains("diamond_end_py[diamond_end_py]"));
        // Should have 4 edges
        assert!(result.contains("diamond_start_py --> diamond_branch_b_py"));
        assert!(result.contains("diamond_start_py --> diamond_branch_c_py"));
        assert!(result.contains("diamond_branch_b_py --> diamond_end_py"));
        assert!(result.contains("diamond_branch_c_py --> diamond_end_py"));
    }

    #[test]
    fn test_linear_chain_generates_sequential_edges() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        assert!(result.contains("linear_step_1_py --> linear_step_2_py"));
        assert!(result.contains("linear_step_2_py --> linear_step_3_py"));
        assert!(result.contains("linear_step_3_py --> linear_step_4_py"));
    }

    #[test]
    fn test_convergent_dag_with_schema() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        // Convergence: both enrich_order and process_payment feed into generate_report
        assert!(result.contains("enrich_order --> generate_report"));
        assert!(result.contains("process_payment --> generate_report"));
    }

    #[test]
    fn test_annotations_render_in_node_label() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("process_payment".to_string(), "Not retry-safe".to_string());

        let result = generate_mermaid(&template, &annotations);

        // Annotated node should have the annotation text and the annotated class
        assert!(result.contains("classDef annotated fill:#fff3cd,stroke:#ffc107"));
        assert!(result.contains("Not retry-safe"));
        assert!(result.contains(":::annotated"));
    }

    #[test]
    fn test_decision_step_uses_diamond_shape() {
        let yaml = r#"
name: decision_test
namespace_name: test
version: "1.0.0"
steps:
  - name: check_input
    handler:
      callable: test.check
  - name: decide
    handler:
      callable: test.decide
    type: decision
    depends_on: [check_input]
"#;
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        // Decision step should use diamond shape
        assert!(result.contains("decide{decide}"));
        // Standard step should use rectangle
        assert!(result.contains("check_input[check_input]"));
    }
}
