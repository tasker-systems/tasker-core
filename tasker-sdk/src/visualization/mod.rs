//! Template visualization: Mermaid diagram and detail table generation.

pub mod render;
pub mod task_visualize;
mod template_visualize;
pub mod types;

pub use types::{
    EdgeStyle, GraphData, GraphEdge, GraphNode, NodeShape, TableData, TableHeader, TableRow,
    VisualCategory, VisualizationOutput,
};

pub use render::{render_detail_table, render_markdown, render_mermaid};
pub use task_visualize::{visualize_task, TaskVisualizationInput};

use std::collections::HashMap;

use serde::Serialize;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Options controlling visualization output.
#[derive(Debug, Default)]
pub struct VisualizeOptions {
    /// When true, only the Mermaid graph is included (no detail table).
    pub graph_only: bool,
}

/// Pre-rendered visualization output (backward-compatible).
#[derive(Debug, Serialize)]
pub struct RenderedOutput {
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

/// Generate structured visualization data from a template.
pub fn visualize_template(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    _options: &VisualizeOptions,
) -> types::VisualizationOutput {
    template_visualize::build_template_visualization(template, annotations)
}

/// Generate pre-rendered visualization output (backward-compatible wrapper).
pub fn visualize_template_rendered(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> RenderedOutput {
    let viz = visualize_template(template, annotations, options);
    let mermaid = render::render_mermaid(&viz.graph);
    let detail_table = if options.graph_only {
        None
    } else {
        Some(render::render_detail_table(&viz.table, None))
    };
    let markdown = render::render_markdown(&template.name, &viz, None, options.graph_only);

    RenderedOutput {
        mermaid,
        detail_table,
        markdown,
        warnings: viz.warnings,
    }
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
        let output =
            visualize_template_rendered(&template, &HashMap::new(), &VisualizeOptions::default());

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
        let output = visualize_template_rendered(&template, &HashMap::new(), &options);

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

        let output =
            visualize_template_rendered(&template, &annotations, &VisualizeOptions::default());

        assert_eq!(output.warnings.len(), 1);
        assert!(output.warnings[0].contains("nonexistent_step"));
    }

    #[test]
    fn test_diamond_dag_full_output() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output =
            visualize_template_rendered(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output.mermaid.contains("diamond_start_py"));
        assert!(output.detail_table.is_some());
    }

    #[test]
    fn test_linear_chain_full_output() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output =
            visualize_template_rendered(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output.mermaid.contains("linear_step_1_py"));
        assert!(output.detail_table.is_some());
    }
}
