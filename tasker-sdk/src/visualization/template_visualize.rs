//! Template-to-structured-data visualization.
//!
//! Converts a [`TaskTemplate`] into structured [`VisualizationOutput`] containing
//! [`GraphData`] and [`TableData`] that can be rendered by the shared renderers.

use std::collections::{HashMap, VecDeque};

use tasker_shared::models::core::task_template::{StepType, TaskTemplate};

use super::types::{
    EdgeStyle, GraphData, GraphEdge, GraphNode, NodeShape, TableData, TableRow, VisualCategory,
    VisualizationOutput,
};

/// Build structured visualization data from a task template.
///
/// Produces [`GraphData`] (nodes + edges) and [`TableData`] (topologically ordered rows)
/// suitable for rendering via [`render_mermaid`](super::render::render_mermaid) and
/// [`render_detail_table`](super::render::render_detail_table).
pub(super) fn build_template_visualization(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
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

    let graph = build_graph(template, annotations);
    let table = build_table(template);

    VisualizationOutput {
        graph,
        table,
        warnings,
    }
}

fn build_graph(template: &TaskTemplate, annotations: &HashMap<String, String>) -> GraphData {
    let mut nodes = Vec::new();
    for step in &template.steps {
        let annotation = annotations.get(&step.name);
        let node_shape = step_type_to_shape(&step.step_type);
        let visual_category = if annotation.is_some() {
            VisualCategory::Annotated
        } else {
            VisualCategory::Default
        };

        let label = match annotation {
            Some(text) => format!("{}\\n\u{26a0} {text}", step.name),
            None => step.name.clone(),
        };

        nodes.push(GraphNode {
            id: step.name.clone(),
            label,
            visual_category,
            node_shape,
            resource_path: None,
        });
    }

    let mut edges = Vec::new();
    for step in &template.steps {
        let mut deps: Vec<&str> = step.dependencies.iter().map(|s| s.as_str()).collect();
        deps.sort();
        for dep in deps {
            edges.push(GraphEdge {
                from: dep.to_string(),
                to: step.name.clone(),
                edge_style: EdgeStyle::Solid,
            });
        }
    }

    GraphData { nodes, edges }
}

fn build_table(template: &TaskTemplate) -> TableData {
    let order = topological_order(template);

    let rows = order
        .iter()
        .filter_map(|name| {
            let step = template.steps.iter().find(|s| &s.name == name)?;
            let step_type = format!("{:?}", step.step_type);
            let handler = step.handler.callable.clone();
            let dependencies = if step.dependencies.is_empty() {
                Some("\u{2014}".to_string())
            } else {
                Some(step.dependencies.join(", "))
            };
            let schema_fields = Some(extract_schema_field_names(&step.result_schema));
            let retry_info = Some(format_retry(&step.retry));

            Some(TableRow {
                name: step.name.clone(),
                step_type,
                handler,
                dependencies,
                schema_fields,
                retry_info,
                state: None,
                visual_category_label: None,
                duration: None,
                attempts: None,
                error_type: None,
                step_link: None,
            })
        })
        .collect();

    TableData { header: None, rows }
}

fn step_type_to_shape(step_type: &StepType) -> NodeShape {
    match step_type {
        StepType::Standard => NodeShape::Rectangle,
        StepType::Decision => NodeShape::Diamond,
        StepType::DeferredConvergence => NodeShape::Trapezoid,
        StepType::Batchable => NodeShape::Trapezoid,
        StepType::BatchWorker => NodeShape::Subroutine,
    }
}

fn topological_order(template: &TaskTemplate) -> Vec<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &template.steps {
        in_degree.entry(&step.name).or_insert(0);
        for dep in &step.dependencies {
            adj.entry(dep.as_str()).or_default().push(&step.name);
            *in_degree.entry(&step.name).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = {
        let mut roots: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name)
            .collect();
        roots.sort();
        roots.into_iter().collect()
    };

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next: Vec<&str> = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(neighbor);
                    }
                }
            }
            next.sort();
            queue.extend(next);
        }
    }

    result
}

fn extract_schema_field_names(schema: &Option<serde_json::Value>) -> String {
    let Some(schema) = schema else {
        return "\u{2014}".to_string();
    };
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return "\u{2014}".to_string();
    };
    let mut names: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
    names.sort();
    if names.is_empty() {
        "\u{2014}".to_string()
    } else {
        names.join(", ")
    }
}

fn format_retry(retry: &tasker_shared::models::core::task_template::RetryConfiguration) -> String {
    if !retry.retryable {
        return "\u{2014}".to_string();
    }
    if retry.max_attempts == 3
        && retry.backoff == tasker_shared::models::core::task_template::BackoffStrategy::Exponential
    {
        return "\u{2014}".to_string();
    }
    format!("{}x {:?}", retry.max_attempts, retry.backoff).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    // ── Tests migrated from mermaid.rs (now testing structured output) ──

    #[test]
    fn test_diamond_dag_produces_correct_graph_structure() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        // Should have 4 nodes
        let node_ids: Vec<&str> = output.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(node_ids.contains(&"diamond_start_py"));
        assert!(node_ids.contains(&"diamond_branch_b_py"));
        assert!(node_ids.contains(&"diamond_branch_c_py"));
        assert!(node_ids.contains(&"diamond_end_py"));

        // Should have 4 edges
        assert_eq!(output.graph.edges.len(), 4);
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "diamond_start_py" && e.to == "diamond_branch_b_py"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "diamond_start_py" && e.to == "diamond_branch_c_py"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "diamond_branch_b_py" && e.to == "diamond_end_py"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "diamond_branch_c_py" && e.to == "diamond_end_py"));
    }

    #[test]
    fn test_linear_chain_produces_sequential_edges() {
        let yaml = include_str!(
            "../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml"
        );
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "linear_step_1_py" && e.to == "linear_step_2_py"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "linear_step_2_py" && e.to == "linear_step_3_py"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "linear_step_3_py" && e.to == "linear_step_4_py"));
    }

    #[test]
    fn test_convergent_dag_edges() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "enrich_order" && e.to == "generate_report"));
        assert!(output
            .graph
            .edges
            .iter()
            .any(|e| e.from == "process_payment" && e.to == "generate_report"));
    }

    #[test]
    fn test_annotations_set_annotated_category() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("process_payment".to_string(), "Not retry-safe".to_string());

        let output = build_template_visualization(&template, &annotations);

        let annotated_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "process_payment")
            .unwrap();
        assert_eq!(annotated_node.visual_category, VisualCategory::Annotated);
        assert!(annotated_node.label.contains("Not retry-safe"));

        // Non-annotated nodes should be Default
        let other_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "validate_order")
            .unwrap();
        assert_eq!(other_node.visual_category, VisualCategory::Default);
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
        let output = build_template_visualization(&template, &HashMap::new());

        let decide_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "decide")
            .unwrap();
        assert_eq!(decide_node.node_shape, NodeShape::Diamond);

        let check_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "check_input")
            .unwrap();
        assert_eq!(check_node.node_shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_all_edges_are_solid_for_templates() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        for edge in &output.graph.edges {
            assert_eq!(edge.edge_style, EdgeStyle::Solid);
        }
    }

    #[test]
    fn test_no_resource_path_for_template_nodes() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        for node in &output.graph.nodes {
            assert!(node.resource_path.is_none());
        }
    }

    // ── Tests migrated from detail_table.rs (now testing structured output) ──

    #[test]
    fn test_table_rows_in_topological_order() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        let names: Vec<&str> = output.table.rows.iter().map(|r| r.name.as_str()).collect();
        let validate_pos = names.iter().position(|n| *n == "validate_order").unwrap();
        let enrich_pos = names.iter().position(|n| *n == "enrich_order").unwrap();
        assert!(validate_pos < enrich_pos);
    }

    #[test]
    fn test_table_rows_have_schema_fields() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        let validate_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "validate_order")
            .unwrap();
        let schema = validate_row.schema_fields.as_deref().unwrap();
        assert!(schema.contains("validated"));
        assert!(schema.contains("order_total"));
    }

    #[test]
    fn test_table_root_step_shows_dash_for_deps() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        let validate_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "validate_order")
            .unwrap();
        assert_eq!(validate_row.dependencies.as_deref(), Some("\u{2014}"));
    }

    #[test]
    fn test_table_shows_retry_config() {
        let yaml = r#"
name: retry_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_with_retry
    handler:
      callable: test.retry_step
    retry:
      retryable: true
      max_attempts: 5
      backoff: exponential
  - name: step_no_retry
    handler:
      callable: test.no_retry
    retry:
      retryable: false
    depends_on: [step_with_retry]
"#;
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        let retry_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "step_with_retry")
            .unwrap();
        assert_eq!(retry_row.retry_info.as_deref(), Some("5x exponential"));

        let no_retry_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "step_no_retry")
            .unwrap();
        assert_eq!(no_retry_row.retry_info.as_deref(), Some("\u{2014}"));
    }

    #[test]
    fn test_table_header_is_none_for_templates() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = build_template_visualization(&template, &HashMap::new());

        assert!(output.table.header.is_none());
    }

    #[test]
    fn test_annotation_warning_for_unknown_step() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("nonexistent_step".to_string(), "note".to_string());

        let output = build_template_visualization(&template, &annotations);

        assert_eq!(output.warnings.len(), 1);
        assert!(output.warnings[0].contains("nonexistent_step"));
    }
}
