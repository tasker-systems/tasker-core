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
