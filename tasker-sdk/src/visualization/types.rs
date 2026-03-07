//! Shared types for template and task visualization.

use serde::Serialize;

/// Structured graph data for Mermaid rendering.
#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// A single node in the visualization graph.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub visual_category: VisualCategory,
    pub node_shape: NodeShape,
    pub resource_path: Option<String>,
}

/// Visual styling category for a node, reflecting execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualCategory {
    Completed,
    InProgress,
    Pending,
    Error,
    Retrying,
    Annotated,
    Untraversed,
    Default,
}

/// Shape of a node in the Mermaid diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    Rectangle,
    Diamond,
    Trapezoid,
    Subroutine,
}

/// A directed edge between two nodes.
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_style: EdgeStyle,
}

/// Line style for an edge in the Mermaid diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeStyle {
    Solid,
    Dashed,
}

/// Structured table data for detail table rendering.
#[derive(Debug, Clone, Serialize)]
pub struct TableData {
    pub header: Option<TableHeader>,
    pub rows: Vec<TableRow>,
}

/// Header metadata for the detail table (template or task info).
#[derive(Debug, Clone, Serialize)]
pub struct TableHeader {
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: Option<String>,
    pub completion_percentage: Option<f64>,
    pub health_status: Option<String>,
    pub elapsed: Option<String>,
    pub dlq_status: Option<String>,
    pub task_link: Option<String>,
    pub dlq_link: Option<String>,
}

/// A single row in the detail table, representing one step.
#[derive(Debug, Clone, Serialize)]
pub struct TableRow {
    pub name: String,
    pub step_type: String,
    pub handler: String,
    // Template-only columns
    pub dependencies: Option<String>,
    pub schema_fields: Option<String>,
    pub retry_info: Option<String>,
    // Task-only columns
    pub state: Option<String>,
    pub visual_category_label: Option<String>,
    pub duration: Option<String>,
    pub attempts: Option<String>,
    pub error_type: Option<String>,
    pub step_link: Option<String>,
}

/// Combined visualization output with structured graph and table data.
#[derive(Debug, Clone, Serialize)]
pub struct VisualizationOutput {
    pub graph: GraphData,
    pub table: TableData,
    pub warnings: Vec<String>,
}
