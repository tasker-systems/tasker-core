//! Shared renderers that consume structured visualization types.
//!
//! These renderers transform [`GraphData`] and [`TableData`] into Mermaid syntax,
//! markdown detail tables, and combined markdown documents.

use std::fmt::Write;

use super::types::{
    EdgeStyle, GraphData, NodeShape, TableData, VisualCategory, VisualizationOutput,
};

/// Render structured [`GraphData`] into Mermaid flowchart syntax.
///
/// Produces a complete Mermaid `graph TD` block with classDef lines for all
/// visual categories, shaped nodes, and sorted edges.
pub fn render_mermaid(graph: &GraphData) -> String {
    let mut out = String::new();
    writeln!(out, "graph TD").unwrap();

    // Class definitions for all visual categories
    writeln!(out, "    classDef completed fill:#d4edda,stroke:#28a745").unwrap();
    writeln!(out, "    classDef in_progress fill:#cce5ff,stroke:#007bff").unwrap();
    writeln!(out, "    classDef pending fill:#e2e3e5,stroke:#6c757d").unwrap();
    writeln!(out, "    classDef error fill:#f8d7da,stroke:#dc3545").unwrap();
    writeln!(out, "    classDef retrying fill:#fff3cd,stroke:#ffc107").unwrap();
    writeln!(out, "    classDef annotated fill:#fff3cd,stroke:#ffc107").unwrap();
    writeln!(
        out,
        "    classDef untraversed fill:#f8f9fa,stroke:#dee2e6,stroke-dasharray:5"
    )
    .unwrap();
    writeln!(out, "    classDef default fill:#ffffff,stroke:#333333").unwrap();

    writeln!(out).unwrap();

    // Node definitions
    for node in &graph.nodes {
        let shape = format_node_shape(&node.id, &node.label, node.node_shape);
        let class = category_class_name(node.visual_category);
        writeln!(out, "    {shape}:::{class}").unwrap();
    }

    writeln!(out).unwrap();

    // Edge definitions (sorted for deterministic output)
    let mut edges: Vec<_> = graph.edges.iter().collect();
    edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

    for edge in &edges {
        let arrow = match edge.edge_style {
            EdgeStyle::Solid => "-->",
            EdgeStyle::Dashed => "-.->",
        };
        writeln!(out, "    {} {arrow} {}", edge.from, edge.to).unwrap();
    }

    out
}

/// Render structured [`TableData`] into a markdown table.
///
/// Detects template vs task mode based on whether any row has `state` set.
/// When `base_url` is provided, it is prepended to resource paths for links.
pub fn render_detail_table(table: &TableData, base_url: Option<&str>) -> String {
    let mut out = String::new();

    // Header metadata block
    if let Some(header) = &table.header {
        writeln!(
            out,
            "**Task**: {} | **Namespace**: {} | **Version**: {}",
            header.name, header.namespace, header.version
        )
        .unwrap();

        let mut meta_parts: Vec<String> = Vec::new();
        if let Some(status) = &header.status {
            meta_parts.push(format!("**Status**: {status}"));
        }
        if let Some(pct) = header.completion_percentage {
            meta_parts.push(format!("**Completion**: {pct:.0}%"));
        }
        if let Some(health) = &header.health_status {
            meta_parts.push(format!("**Health**: {health}"));
        }
        if let Some(elapsed) = &header.elapsed {
            meta_parts.push(format!("**Elapsed**: {elapsed}"));
        }
        if let Some(dlq) = &header.dlq_status {
            meta_parts.push(format!("**DLQ**: {dlq}"));
        }
        if !meta_parts.is_empty() {
            writeln!(out, "{}", meta_parts.join(" | ")).unwrap();
        }

        // Links
        let mut link_parts: Vec<String> = Vec::new();
        if let Some(task_link) = &header.task_link {
            let url = resolve_url(task_link, base_url);
            link_parts.push(format!("[Task Detail]({url})"));
        }
        if let Some(dlq_link) = &header.dlq_link {
            let url = resolve_url(dlq_link, base_url);
            link_parts.push(format!("[DLQ]({url})"));
        }
        if !link_parts.is_empty() {
            writeln!(out, "{}", link_parts.join(" | ")).unwrap();
        }

        writeln!(out).unwrap();
    }

    // Detect mode: task mode if any row has state set
    let is_task_mode = table.rows.iter().any(|r| r.state.is_some());

    if is_task_mode {
        writeln!(
            out,
            "| Step | Type | Handler | State | Duration | Attempts | Error | Link |"
        )
        .unwrap();
        writeln!(
            out,
            "|------|------|---------|-------|----------|----------|-------|------|"
        )
        .unwrap();

        for row in &table.rows {
            let state = row.state.as_deref().unwrap_or("\u{2014}");
            let duration = row.duration.as_deref().unwrap_or("\u{2014}");
            let attempts = row.attempts.as_deref().unwrap_or("\u{2014}");
            let error = row.error_type.as_deref().unwrap_or("\u{2014}");
            let link = row
                .step_link
                .as_ref()
                .map(|l| {
                    let url = resolve_url(l, base_url);
                    format!("[view]({url})")
                })
                .unwrap_or_else(|| "\u{2014}".to_string());

            writeln!(
                out,
                "| {} | {} | {} | {state} | {duration} | {attempts} | {error} | {link} |",
                row.name, row.step_type, row.handler
            )
            .unwrap();
        }
    } else {
        writeln!(
            out,
            "| Step | Type | Handler | Dependencies | Schema Fields | Retry |"
        )
        .unwrap();
        writeln!(
            out,
            "|------|------|---------|--------------|---------------|-------|"
        )
        .unwrap();

        for row in &table.rows {
            let deps = row.dependencies.as_deref().unwrap_or("\u{2014}");
            let schema = row.schema_fields.as_deref().unwrap_or("\u{2014}");
            let retry = row.retry_info.as_deref().unwrap_or("\u{2014}");

            writeln!(
                out,
                "| {} | {} | {} | {deps} | {schema} | {retry} |",
                row.name, row.step_type, row.handler
            )
            .unwrap();
        }
    }

    out
}

/// Render a combined markdown document with Mermaid graph and detail table.
///
/// When `graph_only` is true, the "Step Details" section is omitted.
pub fn render_markdown(
    title: &str,
    output: &VisualizationOutput,
    base_url: Option<&str>,
    graph_only: bool,
) -> String {
    let mermaid = render_mermaid(&output.graph);
    let mut doc = format!("# {title}\n\n```mermaid\n{mermaid}```\n");

    if !graph_only {
        let table = render_detail_table(&output.table, base_url);
        doc.push_str("\n## Step Details\n\n");
        doc.push_str(&table);
    }

    doc
}

fn format_node_shape(id: &str, label: &str, shape: NodeShape) -> String {
    match shape {
        NodeShape::Rectangle => format!("{id}[\"{label}\"]"),
        NodeShape::Diamond => format!("{id}{{\"{label}\"}}"),
        NodeShape::Trapezoid => format!("{id}[/\"{label}\"/]"),
        NodeShape::Subroutine => format!("{id}[[\"{label}\"]]"),
    }
}

fn category_class_name(category: VisualCategory) -> &'static str {
    match category {
        VisualCategory::Completed => "completed",
        VisualCategory::InProgress => "in_progress",
        VisualCategory::Pending => "pending",
        VisualCategory::Error => "error",
        VisualCategory::Retrying => "retrying",
        VisualCategory::Annotated => "annotated",
        VisualCategory::Untraversed => "untraversed",
        VisualCategory::Default => "default",
    }
}

fn resolve_url(path: &str, base_url: Option<&str>) -> String {
    match base_url {
        Some(base) => format!("{base}{path}"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visualization::types::*;

    // ── Helper builders ──────────────────────────────────────────

    fn node(id: &str, shape: NodeShape, category: VisualCategory) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            visual_category: category,
            node_shape: shape,
            resource_path: None,
        }
    }

    fn edge(from: &str, to: &str, style: EdgeStyle) -> GraphEdge {
        GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            edge_style: style,
        }
    }

    fn template_row(name: &str, deps: Option<&str>, schema: Option<&str>) -> TableRow {
        TableRow {
            name: name.to_string(),
            step_type: "Step".to_string(),
            handler: "test.handler".to_string(),
            dependencies: deps.map(|s| s.to_string()),
            schema_fields: schema.map(|s| s.to_string()),
            retry_info: None,
            state: None,
            visual_category_label: None,
            duration: None,
            attempts: None,
            error_type: None,
            step_link: None,
        }
    }

    fn task_row(name: &str, state: &str, link: Option<&str>) -> TableRow {
        TableRow {
            name: name.to_string(),
            step_type: "Step".to_string(),
            handler: "test.handler".to_string(),
            dependencies: None,
            schema_fields: None,
            retry_info: None,
            state: Some(state.to_string()),
            visual_category_label: Some("completed".to_string()),
            duration: Some("1.2s".to_string()),
            attempts: Some("1".to_string()),
            error_type: None,
            step_link: link.map(|s| s.to_string()),
        }
    }

    fn sample_graph() -> GraphData {
        GraphData {
            nodes: vec![
                node("start", NodeShape::Rectangle, VisualCategory::Completed),
                node("decide", NodeShape::Diamond, VisualCategory::InProgress),
                node("process", NodeShape::Trapezoid, VisualCategory::Pending),
                node("finalize", NodeShape::Subroutine, VisualCategory::Error),
            ],
            edges: vec![
                edge("start", "decide", EdgeStyle::Solid),
                edge("decide", "process", EdgeStyle::Dashed),
                edge("process", "finalize", EdgeStyle::Solid),
            ],
        }
    }

    // ── render_mermaid tests ─────────────────────────────────────

    #[test]
    fn test_mermaid_all_classdef_lines_present() {
        let graph = GraphData {
            nodes: vec![],
            edges: vec![],
        };
        let result = render_mermaid(&graph);

        assert!(result.contains("classDef completed fill:#d4edda,stroke:#28a745"));
        assert!(result.contains("classDef in_progress fill:#cce5ff,stroke:#007bff"));
        assert!(result.contains("classDef pending fill:#e2e3e5,stroke:#6c757d"));
        assert!(result.contains("classDef error fill:#f8d7da,stroke:#dc3545"));
        assert!(result.contains("classDef retrying fill:#fff3cd,stroke:#ffc107"));
        assert!(result.contains("classDef annotated fill:#fff3cd,stroke:#ffc107"));
        assert!(
            result.contains("classDef untraversed fill:#f8f9fa,stroke:#dee2e6,stroke-dasharray:5")
        );
        assert!(result.contains("classDef default fill:#ffffff,stroke:#333333"));
    }

    #[test]
    fn test_mermaid_node_shapes_render_correctly() {
        let graph = sample_graph();
        let result = render_mermaid(&graph);

        // Rectangle: id["label"]
        assert!(result.contains("start[\"start\"]"));
        // Diamond: id{"label"}
        assert!(result.contains("decide{\"decide\"}"));
        // Trapezoid: id[/"label"/]
        assert!(result.contains("process[/\"process\"/]"));
        // Subroutine: id[["label"]]
        assert!(result.contains("finalize[[\"finalize\"]]"));
    }

    #[test]
    fn test_mermaid_category_class_assignment() {
        let graph = sample_graph();
        let result = render_mermaid(&graph);

        assert!(result.contains(":::completed"));
        assert!(result.contains(":::in_progress"));
        assert!(result.contains(":::pending"));
        assert!(result.contains(":::error"));
    }

    #[test]
    fn test_mermaid_solid_vs_dashed_edges() {
        let graph = sample_graph();
        let result = render_mermaid(&graph);

        assert!(result.contains("start --> decide"));
        assert!(result.contains("decide -.-> process"));
        assert!(result.contains("process --> finalize"));
    }

    #[test]
    fn test_mermaid_edges_sorted_deterministically() {
        let graph = GraphData {
            nodes: vec![
                node("c", NodeShape::Rectangle, VisualCategory::Default),
                node("a", NodeShape::Rectangle, VisualCategory::Default),
                node("b", NodeShape::Rectangle, VisualCategory::Default),
            ],
            edges: vec![
                // Deliberately out of order
                edge("c", "a", EdgeStyle::Solid),
                edge("a", "b", EdgeStyle::Solid),
                edge("b", "c", EdgeStyle::Solid),
            ],
        };
        let result = render_mermaid(&graph);

        let edge_lines: Vec<&str> = result.lines().filter(|l| l.contains("-->")).collect();

        // Should be sorted by (from, to): a->b, b->c, c->a
        assert_eq!(edge_lines.len(), 3);
        assert!(edge_lines[0].contains("a --> b"));
        assert!(edge_lines[1].contains("b --> c"));
        assert!(edge_lines[2].contains("c --> a"));
    }

    #[test]
    fn test_mermaid_starts_with_graph_td() {
        let graph = sample_graph();
        let result = render_mermaid(&graph);
        assert!(result.starts_with("graph TD\n"));
    }

    // ── render_detail_table tests ────────────────────────────────

    #[test]
    fn test_detail_table_template_mode_columns() {
        let table = TableData {
            header: None,
            rows: vec![template_row(
                "step_a",
                Some("step_b"),
                Some("field1, field2"),
            )],
        };
        let result = render_detail_table(&table, None);

        assert!(result.contains("| Step | Type | Handler | Dependencies | Schema Fields | Retry |"));
        assert!(result.contains("| step_a |"));
        assert!(result.contains("| step_b |"));
        assert!(result.contains("| field1, field2 |"));
    }

    #[test]
    fn test_detail_table_task_mode_columns() {
        let table = TableData {
            header: None,
            rows: vec![task_row("step_a", "complete", None)],
        };
        let result = render_detail_table(&table, None);

        assert!(result
            .contains("| Step | Type | Handler | State | Duration | Attempts | Error | Link |"));
        assert!(result.contains("| step_a |"));
        assert!(result.contains("| complete |"));
        assert!(result.contains("| 1.2s |"));
    }

    #[test]
    fn test_detail_table_base_url_prepends_to_links() {
        let table = TableData {
            header: None,
            rows: vec![task_row("step_a", "complete", Some("/api/v1/steps/42"))],
        };
        let result = render_detail_table(&table, Some("https://tasker.example.com"));

        assert!(result.contains("https://tasker.example.com/api/v1/steps/42"));
    }

    #[test]
    fn test_detail_table_no_base_url_uses_raw_path() {
        let table = TableData {
            header: None,
            rows: vec![task_row("step_a", "complete", Some("/api/v1/steps/42"))],
        };
        let result = render_detail_table(&table, None);

        assert!(result.contains("/api/v1/steps/42"));
        assert!(!result.contains("https://"));
    }

    #[test]
    fn test_detail_table_header_renders_metadata() {
        let table = TableData {
            header: Some(TableHeader {
                name: "my_task".to_string(),
                namespace: "test".to_string(),
                version: "1.0.0".to_string(),
                status: Some("in_progress".to_string()),
                completion_percentage: Some(75.0),
                health_status: Some("healthy".to_string()),
                elapsed: Some("5m 30s".to_string()),
                dlq_status: None,
                task_link: Some("/api/v1/tasks/1".to_string()),
                dlq_link: None,
            }),
            rows: vec![template_row("step_a", None, None)],
        };
        let result = render_detail_table(&table, None);

        assert!(result.contains("**Task**: my_task"));
        assert!(result.contains("**Namespace**: test"));
        assert!(result.contains("**Version**: 1.0.0"));
        assert!(result.contains("**Status**: in_progress"));
        assert!(result.contains("**Completion**: 75%"));
        assert!(result.contains("**Health**: healthy"));
        assert!(result.contains("**Elapsed**: 5m 30s"));
        assert!(result.contains("[Task Detail](/api/v1/tasks/1)"));
    }

    // ── render_markdown tests ────────────────────────────────────

    #[test]
    fn test_markdown_contains_fenced_mermaid_block() {
        let output = VisualizationOutput {
            graph: sample_graph(),
            table: TableData {
                header: None,
                rows: vec![template_row("step_a", None, None)],
            },
            warnings: vec![],
        };
        let result = render_markdown("Test Template", &output, None, false);

        assert!(result.contains("```mermaid"));
        assert!(result.contains("graph TD"));
    }

    #[test]
    fn test_markdown_contains_detail_table_when_not_graph_only() {
        let output = VisualizationOutput {
            graph: sample_graph(),
            table: TableData {
                header: None,
                rows: vec![template_row("step_a", None, None)],
            },
            warnings: vec![],
        };
        let result = render_markdown("Test Template", &output, None, false);

        assert!(result.contains("## Step Details"));
        assert!(result.contains("| Step |"));
    }

    #[test]
    fn test_markdown_omits_detail_table_when_graph_only() {
        let output = VisualizationOutput {
            graph: sample_graph(),
            table: TableData {
                header: None,
                rows: vec![template_row("step_a", None, None)],
            },
            warnings: vec![],
        };
        let result = render_markdown("Test Template", &output, None, true);

        assert!(!result.contains("## Step Details"));
        assert!(!result.contains("| Step |"));
        // But mermaid should still be present
        assert!(result.contains("```mermaid"));
    }

    #[test]
    fn test_markdown_title_in_heading() {
        let output = VisualizationOutput {
            graph: GraphData {
                nodes: vec![],
                edges: vec![],
            },
            table: TableData {
                header: None,
                rows: vec![],
            },
            warnings: vec![],
        };
        let result = render_markdown("My Workflow", &output, None, false);

        assert!(result.starts_with("# My Workflow\n"));
    }
}
