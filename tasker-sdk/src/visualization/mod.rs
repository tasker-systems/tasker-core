//! Template visualization: Mermaid diagram and detail table generation.

mod detail_table;
mod mermaid;

use std::collections::HashMap;

use serde::Serialize;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Options controlling visualization output.
#[derive(Debug, Default)]
pub struct VisualizeOptions {
    /// When true, only the Mermaid graph is included (no detail table).
    pub graph_only: bool,
}

/// Output from template visualization.
#[derive(Debug, Serialize)]
pub struct VisualizationOutput {
    /// Raw Mermaid graph syntax (no fenced code block markers).
    pub mermaid: String,
    /// Markdown detail table (None when graph_only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_table: Option<String>,
    /// Complete markdown document (fenced Mermaid block + detail table).
    pub markdown: String,
    /// Warnings (e.g., annotation references unknown steps).
    pub warnings: Vec<String>,
}

/// Generate a Mermaid visualization of a task template.
///
/// `annotations` maps step names to developer notes rendered as callouts.
pub fn visualize_template(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> VisualizationOutput {
    let mut warnings = Vec::new();

    // Warn about annotations referencing unknown steps
    let step_names: std::collections::HashSet<&str> =
        template.steps.iter().map(|s| s.name.as_str()).collect();
    for key in annotations.keys() {
        if !step_names.contains(key.as_str()) {
            warnings.push(format!("Annotation references unknown step: '{key}'"));
        }
    }

    let mermaid = mermaid::generate_mermaid(template, annotations);
    let detail_table = if options.graph_only {
        None
    } else {
        Some(detail_table::generate_detail_table(template))
    };

    let markdown = build_markdown(&template.name, &mermaid, detail_table.as_deref());

    VisualizationOutput {
        mermaid,
        detail_table,
        markdown,
        warnings,
    }
}

fn build_markdown(name: &str, mermaid: &str, detail_table: Option<&str>) -> String {
    let mut doc = format!("# {name}\n\n```mermaid\n{mermaid}```\n");
    if let Some(table) = detail_table {
        doc.push_str("\n## Step Details\n\n");
        doc.push_str(table);
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    #[test]
    fn test_full_markdown_output() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        // Markdown contains fenced mermaid block
        assert!(output.markdown.contains("```mermaid"));
        assert!(output.markdown.contains("graph TD"));
        // Markdown contains detail table
        assert!(output.markdown.contains("## Step Details"));
        assert!(output.markdown.contains("| Step |"));
        // detail_table is present
        assert!(output.detail_table.is_some());
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_graph_only_mode() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let options = VisualizeOptions { graph_only: true };
        let output = visualize_template(&template, &HashMap::new(), &options);

        assert!(output.detail_table.is_none());
        assert!(!output.markdown.contains("## Step Details"));
        // But mermaid should still be present
        assert!(output.markdown.contains("```mermaid"));
    }

    #[test]
    fn test_annotation_warning_for_unknown_step() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("nonexistent_step".to_string(), "note".to_string());

        let output = visualize_template(&template, &annotations, &VisualizeOptions::default());

        assert_eq!(output.warnings.len(), 1);
        assert!(output.warnings[0].contains("nonexistent_step"));
    }

    #[test]
    fn test_diamond_dag_full_output() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output
            .mermaid
            .contains("diamond_start_py --> diamond_branch_b_py"));
        assert!(output.detail_table.is_some());
    }

    #[test]
    fn test_linear_chain_full_output() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output
            .mermaid
            .contains("linear_step_1_py --> linear_step_2_py"));
        assert!(output.detail_table.is_some());
    }
}
