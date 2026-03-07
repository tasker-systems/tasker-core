//! Task execution visualization.
//!
//! Converts a [`TaskSummaryResponse`] into structured [`VisualizationOutput`]
//! containing [`GraphData`] and [`TableData`] that can be rendered by the
//! shared renderers.

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use tasker_shared::types::api::orchestration::{
    DlqSummaryInfo, StepSummaryInfo, TaskSummaryLinks, TaskSummaryMetadata, TaskSummaryResponse,
    TemplateStepSummary, TemplateSummary,
};

use super::types::{
    EdgeStyle, GraphData, GraphEdge, GraphNode, NodeShape, TableData, TableHeader, TableRow,
    VisualCategory, VisualizationOutput,
};

/// Input for task visualization, adapted from [`TaskSummaryResponse`].
#[derive(Debug, Clone)]
pub struct TaskVisualizationInput {
    pub task: TaskSummaryMetadata,
    pub template: TemplateSummary,
    pub steps: Vec<StepSummaryInfo>,
    pub dlq: DlqSummaryInfo,
    pub links: TaskSummaryLinks,
}

impl From<&TaskSummaryResponse> for TaskVisualizationInput {
    fn from(response: &TaskSummaryResponse) -> Self {
        TaskVisualizationInput {
            task: response.task.clone(),
            template: response.template.clone(),
            steps: response.steps.clone(),
            dlq: response.dlq.clone(),
            links: response.links.clone(),
        }
    }
}

/// Build structured visualization data from a task execution summary.
///
/// Produces [`GraphData`] (nodes + edges) and [`TableData`] (topologically ordered rows)
/// suitable for rendering via [`render_mermaid`](super::render::render_mermaid) and
/// [`render_detail_table`](super::render::render_detail_table).
pub fn visualize_task(input: &TaskVisualizationInput) -> VisualizationOutput {
    let mut warnings = Vec::new();

    // Build lookups
    let template_step_map: HashMap<&str, &TemplateStepSummary> = input
        .template
        .steps
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    let execution_step_map: HashMap<&str, &StepSummaryInfo> =
        input.steps.iter().map(|s| (s.name.as_str(), s)).collect();

    // Identify decision step names for untraversed detection
    let decision_steps: HashSet<&str> = input
        .template
        .steps
        .iter()
        .filter(|s| s.step_type == "decision")
        .map(|s| s.name.as_str())
        .collect();

    // Track template step names
    let template_step_names: HashSet<&str> = input
        .template
        .steps
        .iter()
        .map(|s| s.name.as_str())
        .collect();

    // Build graph
    let graph = build_graph(
        input,
        &template_step_map,
        &execution_step_map,
        &decision_steps,
    );

    // Build table
    let table = build_table(
        input,
        &template_step_map,
        &execution_step_map,
        &decision_steps,
        &template_step_names,
    );

    // Add DLQ warning
    if input.dlq.in_dlq {
        let reason = input.dlq.dlq_reason.as_deref().unwrap_or("unknown reason");
        warnings.push(format!("Task is in DLQ: {reason}"));
    }

    VisualizationOutput {
        graph,
        table,
        warnings,
    }
}

fn build_graph(
    input: &TaskVisualizationInput,
    _template_step_map: &HashMap<&str, &TemplateStepSummary>,
    execution_step_map: &HashMap<&str, &StepSummaryInfo>,
    decision_steps: &HashSet<&str>,
) -> GraphData {
    let mut nodes = Vec::new();
    let template_step_names: HashSet<&str> = input
        .template
        .steps
        .iter()
        .map(|s| s.name.as_str())
        .collect();

    // Add nodes for each template step
    for ts in &input.template.steps {
        let visual_category = match execution_step_map.get(ts.name.as_str()) {
            Some(exec) => state_to_visual_category(&exec.current_state),
            None => {
                // Check if parent is a decision step (untraversed path)
                if has_decision_parent(&ts.dependencies, decision_steps) {
                    VisualCategory::Untraversed
                } else {
                    VisualCategory::Pending
                }
            }
        };

        let node_shape = step_type_to_shape(&ts.step_type);

        let label = match &visual_category {
            VisualCategory::Untraversed => format!("{}\\n(untraversed)", ts.name),
            _ => ts.name.clone(),
        };

        let resource_path = execution_step_map.get(ts.name.as_str()).map(|exec| {
            format!(
                "/v1/tasks/{}/workflow_steps/{}",
                input.task.task_uuid, exec.step_uuid
            )
        });

        nodes.push(GraphNode {
            id: ts.name.clone(),
            label,
            visual_category,
            node_shape,
            resource_path,
        });
    }

    // Add nodes for dynamic batch workers (execution steps not in template)
    for exec_step in &input.steps {
        if !template_step_names.contains(exec_step.name.as_str()) {
            let visual_category = state_to_visual_category(&exec_step.current_state);
            let resource_path = Some(format!(
                "/v1/tasks/{}/workflow_steps/{}",
                input.task.task_uuid, exec_step.step_uuid
            ));

            nodes.push(GraphNode {
                id: exec_step.name.clone(),
                label: exec_step.name.clone(),
                visual_category,
                node_shape: NodeShape::Subroutine,
                resource_path,
            });
        }
    }

    // Build edges from template dependencies
    let mut edges = Vec::new();
    for ts in &input.template.steps {
        let mut deps: Vec<&str> = ts.dependencies.iter().map(|s| s.as_str()).collect();
        deps.sort();
        for dep in deps {
            let target_category = match execution_step_map.get(ts.name.as_str()) {
                Some(exec) => state_to_visual_category(&exec.current_state),
                None => {
                    if has_decision_parent(&ts.dependencies, decision_steps) {
                        VisualCategory::Untraversed
                    } else {
                        VisualCategory::Pending
                    }
                }
            };

            let dep_completed = execution_step_map
                .get(dep)
                .map(|e| state_to_visual_category(&e.current_state) == VisualCategory::Completed)
                .unwrap_or(false);

            let edge_style = if target_category == VisualCategory::Untraversed {
                EdgeStyle::Dashed
            } else if dep_completed {
                EdgeStyle::Solid
            } else {
                EdgeStyle::Dashed
            };

            edges.push(GraphEdge {
                from: dep.to_string(),
                to: ts.name.clone(),
                edge_style,
            });
        }
    }

    // Add edges for dynamic batch workers — link to batchable parent if identifiable
    // Dynamic workers have names like "parent_batch_0", "parent_batch_1", etc.
    for exec_step in &input.steps {
        if !template_step_names.contains(exec_step.name.as_str()) {
            // Try to find a batchable parent by name prefix
            for ts in &input.template.steps {
                if ts.step_type == "batchable" && exec_step.name.starts_with(&ts.name) {
                    edges.push(GraphEdge {
                        from: ts.name.clone(),
                        to: exec_step.name.clone(),
                        edge_style: EdgeStyle::Solid,
                    });
                    break;
                }
            }
        }
    }

    GraphData { nodes, edges }
}

fn build_table(
    input: &TaskVisualizationInput,
    template_step_map: &HashMap<&str, &TemplateStepSummary>,
    execution_step_map: &HashMap<&str, &StepSummaryInfo>,
    decision_steps: &HashSet<&str>,
    template_step_names: &HashSet<&str>,
) -> TableData {
    let order = topological_order(&input.template);

    let mut rows: Vec<TableRow> = order
        .iter()
        .filter_map(|name| {
            let ts = template_step_map.get(name.as_str())?;
            let exec = execution_step_map.get(name.as_str());

            let visual_category = match exec {
                Some(e) => state_to_visual_category(&e.current_state),
                None => {
                    if has_decision_parent(&ts.dependencies, decision_steps) {
                        VisualCategory::Untraversed
                    } else {
                        VisualCategory::Pending
                    }
                }
            };

            let state = exec.map(|e| e.current_state.clone());
            let visual_category_label = Some(visual_category_to_label(visual_category));
            let duration = exec.and_then(|e| calculate_duration(e));
            let attempts = exec.map(|e| format!("{}/{}", e.attempts, e.max_attempts));
            let error_type = exec
                .and_then(|e| e.error.as_ref())
                .and_then(|err| err.error_type.clone());
            let step_link = exec.map(|e| {
                format!(
                    "/v1/tasks/{}/workflow_steps/{}",
                    input.task.task_uuid, e.step_uuid
                )
            });

            Some(TableRow {
                name: ts.name.clone(),
                step_type: ts.step_type.clone(),
                handler: ts.handler.clone(),
                dependencies: None,
                schema_fields: None,
                retry_info: None,
                state,
                visual_category_label,
                duration,
                attempts,
                error_type,
                step_link,
            })
        })
        .collect();

    // Add dynamic batch worker rows
    for exec_step in &input.steps {
        if !template_step_names.contains(exec_step.name.as_str()) {
            let visual_category = state_to_visual_category(&exec_step.current_state);
            let duration = calculate_duration(exec_step);
            let step_link = Some(format!(
                "/v1/tasks/{}/workflow_steps/{}",
                input.task.task_uuid, exec_step.step_uuid
            ));

            rows.push(TableRow {
                name: exec_step.name.clone(),
                step_type: "batch_worker".to_string(),
                handler: String::new(),
                dependencies: None,
                schema_fields: None,
                retry_info: None,
                state: Some(exec_step.current_state.clone()),
                visual_category_label: Some(visual_category_to_label(visual_category)),
                duration,
                attempts: Some(format!("{}/{}", exec_step.attempts, exec_step.max_attempts)),
                error_type: exec_step.error.as_ref().and_then(|e| e.error_type.clone()),
                step_link,
            });
        }
    }

    let header = Some(build_table_header(input));

    TableData { header, rows }
}

fn build_table_header(input: &TaskVisualizationInput) -> TableHeader {
    let elapsed = calculate_task_elapsed(input);

    TableHeader {
        name: input.task.name.clone(),
        namespace: input.task.namespace.clone(),
        version: input.task.version.clone(),
        status: Some(input.task.status.clone()),
        completion_percentage: Some(input.task.completion_percentage),
        health_status: Some(input.task.health_status.clone()),
        elapsed,
        dlq_status: if input.dlq.in_dlq {
            Some("In DLQ".to_string())
        } else {
            None
        },
        task_link: Some(input.links.task.clone()),
        dlq_link: if input.dlq.in_dlq {
            Some(input.links.dlq.clone())
        } else {
            None
        },
    }
}

fn calculate_task_elapsed(input: &TaskVisualizationInput) -> Option<String> {
    let end = input.task.completed_at.unwrap_or_else(Utc::now);
    let duration = end - input.task.created_at;
    Some(format_duration_chrono(duration))
}

fn state_to_visual_category(state: &str) -> VisualCategory {
    match state {
        "complete" | "resolved_manually" => VisualCategory::Completed,
        "in_progress" => VisualCategory::InProgress,
        "pending" => VisualCategory::Pending,
        "error" | "cancelled" => VisualCategory::Error,
        "waiting_for_retry"
        | "enqueued"
        | "enqueued_for_orchestration"
        | "enqueued_as_error_for_orchestration" => VisualCategory::Retrying,
        _ => VisualCategory::Default,
    }
}

fn visual_category_to_label(category: VisualCategory) -> String {
    match category {
        VisualCategory::Completed => "Completed",
        VisualCategory::InProgress => "In Progress",
        VisualCategory::Pending => "Pending",
        VisualCategory::Error => "Error",
        VisualCategory::Retrying => "Retrying",
        VisualCategory::Untraversed => "Untraversed",
        VisualCategory::Annotated => "Annotated",
        VisualCategory::Default => "Unknown",
    }
    .to_string()
}

fn step_type_to_shape(step_type: &str) -> NodeShape {
    match step_type {
        "standard" => NodeShape::Rectangle,
        "decision" => NodeShape::Diamond,
        "deferred_convergence" => NodeShape::Trapezoid,
        "batchable" => NodeShape::Trapezoid,
        "batch_worker" => NodeShape::Subroutine,
        _ => NodeShape::Rectangle,
    }
}

fn has_decision_parent(dependencies: &[String], decision_steps: &HashSet<&str>) -> bool {
    dependencies
        .iter()
        .any(|dep| decision_steps.contains(dep.as_str()))
}

fn topological_order(template: &TemplateSummary) -> Vec<String> {
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

fn calculate_duration(step: &StepSummaryInfo) -> Option<String> {
    let completed_str = step.completed_at.as_deref()?;
    let completed = parse_datetime(completed_str)?;

    let start = step
        .last_attempted_at
        .as_deref()
        .and_then(parse_datetime)
        .or_else(|| step.created_at.as_deref().and_then(parse_datetime))?;

    let duration = completed - start;
    Some(format_duration_chrono(duration))
}

fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    // Try RFC 3339 first, then common formats
    s.parse::<DateTime<Utc>>().ok()
}

fn format_duration_chrono(duration: chrono::TimeDelta) -> String {
    let total_secs = duration.num_seconds();
    if total_secs < 0 {
        return "0s".to_string();
    }
    let millis = duration.num_milliseconds() % 1000;

    if total_secs == 0 {
        if millis > 0 {
            return format!("{millis}ms");
        }
        return "0s".to_string();
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if secs > 0 || (hours == 0 && minutes == 0) {
        if millis > 0 && hours == 0 {
            let fractional = format!("{secs}.{millis}s");
            parts.push(fractional);
        } else {
            parts.push(format!("{secs}s"));
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tasker_shared::types::api::orchestration::StepErrorSummary;
    use uuid::Uuid;

    // ── Fixture helpers ──

    fn make_task_metadata() -> TaskSummaryMetadata {
        TaskSummaryMetadata {
            task_uuid: "task-uuid-001".to_string(),
            name: "order_processing".to_string(),
            namespace: "ecommerce".to_string(),
            version: "1.0.0".to_string(),
            status: "in_progress".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 3, 6, 10, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 6, 10, 5, 0).unwrap(),
            completed_at: None,
            initiator: "api".to_string(),
            source_system: "web".to_string(),
            reason: "New order".to_string(),
            correlation_id: Uuid::nil(),
            total_steps: 4,
            pending_steps: 1,
            in_progress_steps: 1,
            completed_steps: 2,
            failed_steps: 0,
            completion_percentage: 50.0,
            health_status: "healthy".to_string(),
            execution_status: "in_progress".to_string(),
            recommended_action: None,
        }
    }

    fn make_template_steps() -> Vec<TemplateStepSummary> {
        vec![
            TemplateStepSummary {
                name: "validate_order".to_string(),
                step_type: "standard".to_string(),
                handler: "orders.validate".to_string(),
                dependencies: vec![],
                retryable: true,
                max_attempts: 3,
            },
            TemplateStepSummary {
                name: "process_payment".to_string(),
                step_type: "standard".to_string(),
                handler: "payments.process".to_string(),
                dependencies: vec!["validate_order".to_string()],
                retryable: true,
                max_attempts: 3,
            },
            TemplateStepSummary {
                name: "enrich_order".to_string(),
                step_type: "standard".to_string(),
                handler: "orders.enrich".to_string(),
                dependencies: vec!["validate_order".to_string()],
                retryable: false,
                max_attempts: 1,
            },
            TemplateStepSummary {
                name: "generate_report".to_string(),
                step_type: "standard".to_string(),
                handler: "reports.generate".to_string(),
                dependencies: vec!["process_payment".to_string(), "enrich_order".to_string()],
                retryable: true,
                max_attempts: 3,
            },
        ]
    }

    fn make_execution_steps() -> Vec<StepSummaryInfo> {
        vec![
            StepSummaryInfo {
                step_uuid: "step-uuid-001".to_string(),
                name: "validate_order".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:00Z".to_string()),
                completed_at: Some("2026-03-06T10:00:01.500Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
                attempts: 1,
                max_attempts: 3,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "step-uuid-002".to_string(),
                name: "process_payment".to_string(),
                current_state: "in_progress".to_string(),
                created_at: Some("2026-03-06T10:00:02Z".to_string()),
                completed_at: None,
                last_attempted_at: Some("2026-03-06T10:00:02Z".to_string()),
                attempts: 1,
                max_attempts: 3,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "step-uuid-003".to_string(),
                name: "enrich_order".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:02Z".to_string()),
                completed_at: Some("2026-03-06T10:00:04Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:02Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "step-uuid-004".to_string(),
                name: "generate_report".to_string(),
                current_state: "pending".to_string(),
                created_at: Some("2026-03-06T10:00:00Z".to_string()),
                completed_at: None,
                last_attempted_at: None,
                attempts: 0,
                max_attempts: 3,
                dependencies_satisfied: false,
                retry_eligible: false,
                error: None,
            },
        ]
    }

    fn make_dlq_info(in_dlq: bool) -> DlqSummaryInfo {
        DlqSummaryInfo {
            in_dlq,
            dlq_reason: if in_dlq {
                Some("Max retries exceeded".to_string())
            } else {
                None
            },
            resolution_status: None,
        }
    }

    fn make_links() -> TaskSummaryLinks {
        TaskSummaryLinks {
            task: "/v1/tasks/task-uuid-001".to_string(),
            steps: "/v1/tasks/task-uuid-001/workflow_steps".to_string(),
            dlq: "/v1/tasks/task-uuid-001/dead_letter_queue".to_string(),
        }
    }

    fn make_input() -> TaskVisualizationInput {
        TaskVisualizationInput {
            task: make_task_metadata(),
            template: TemplateSummary {
                steps: make_template_steps(),
            },
            steps: make_execution_steps(),
            dlq: make_dlq_info(false),
            links: make_links(),
        }
    }

    // ── 1. State mapping tests ──

    #[test]
    fn test_state_to_visual_category_complete() {
        assert_eq!(
            state_to_visual_category("complete"),
            VisualCategory::Completed
        );
    }

    #[test]
    fn test_state_to_visual_category_resolved_manually() {
        assert_eq!(
            state_to_visual_category("resolved_manually"),
            VisualCategory::Completed
        );
    }

    #[test]
    fn test_state_to_visual_category_in_progress() {
        assert_eq!(
            state_to_visual_category("in_progress"),
            VisualCategory::InProgress
        );
    }

    #[test]
    fn test_state_to_visual_category_pending() {
        assert_eq!(state_to_visual_category("pending"), VisualCategory::Pending);
    }

    #[test]
    fn test_state_to_visual_category_error() {
        assert_eq!(state_to_visual_category("error"), VisualCategory::Error);
    }

    #[test]
    fn test_state_to_visual_category_cancelled() {
        assert_eq!(state_to_visual_category("cancelled"), VisualCategory::Error);
    }

    #[test]
    fn test_state_to_visual_category_retrying_states() {
        for state in &[
            "waiting_for_retry",
            "enqueued",
            "enqueued_for_orchestration",
            "enqueued_as_error_for_orchestration",
        ] {
            assert_eq!(
                state_to_visual_category(state),
                VisualCategory::Retrying,
                "Expected Retrying for state '{state}'"
            );
        }
    }

    #[test]
    fn test_state_to_visual_category_unknown() {
        assert_eq!(
            state_to_visual_category("some_unknown_state"),
            VisualCategory::Default
        );
    }

    // ── 2. Node shape tests ──

    #[test]
    fn test_step_type_to_shape_standard() {
        assert_eq!(step_type_to_shape("standard"), NodeShape::Rectangle);
    }

    #[test]
    fn test_step_type_to_shape_decision() {
        assert_eq!(step_type_to_shape("decision"), NodeShape::Diamond);
    }

    #[test]
    fn test_step_type_to_shape_deferred_convergence() {
        assert_eq!(
            step_type_to_shape("deferred_convergence"),
            NodeShape::Trapezoid
        );
    }

    #[test]
    fn test_step_type_to_shape_batchable() {
        assert_eq!(step_type_to_shape("batchable"), NodeShape::Trapezoid);
    }

    #[test]
    fn test_step_type_to_shape_batch_worker() {
        assert_eq!(step_type_to_shape("batch_worker"), NodeShape::Subroutine);
    }

    // ── 3. Edge styling tests ──

    #[test]
    fn test_edges_solid_for_satisfied_dependencies() {
        let input = make_input();
        let output = visualize_task(&input);

        // validate_order -> process_payment: validate_order is complete, so solid
        let edge = output
            .graph
            .edges
            .iter()
            .find(|e| e.from == "validate_order" && e.to == "process_payment")
            .expect("Edge validate_order -> process_payment should exist");
        assert_eq!(edge.edge_style, EdgeStyle::Solid);
    }

    #[test]
    fn test_edges_dashed_for_unsatisfied_dependencies() {
        let input = make_input();
        let output = visualize_task(&input);

        // process_payment -> generate_report: process_payment is in_progress, so dashed
        let edge = output
            .graph
            .edges
            .iter()
            .find(|e| e.from == "process_payment" && e.to == "generate_report")
            .expect("Edge process_payment -> generate_report should exist");
        assert_eq!(edge.edge_style, EdgeStyle::Dashed);
    }

    // ── 4. Decision workflow / untraversed tests ──

    #[test]
    fn test_decision_workflow_untraversed_branch() {
        let template_steps = vec![
            TemplateStepSummary {
                name: "start".to_string(),
                step_type: "standard".to_string(),
                handler: "test.start".to_string(),
                dependencies: vec![],
                retryable: false,
                max_attempts: 1,
            },
            TemplateStepSummary {
                name: "decide".to_string(),
                step_type: "decision".to_string(),
                handler: "test.decide".to_string(),
                dependencies: vec!["start".to_string()],
                retryable: false,
                max_attempts: 1,
            },
            TemplateStepSummary {
                name: "branch_a".to_string(),
                step_type: "standard".to_string(),
                handler: "test.branch_a".to_string(),
                dependencies: vec!["decide".to_string()],
                retryable: false,
                max_attempts: 1,
            },
            TemplateStepSummary {
                name: "branch_b".to_string(),
                step_type: "standard".to_string(),
                handler: "test.branch_b".to_string(),
                dependencies: vec!["decide".to_string()],
                retryable: false,
                max_attempts: 1,
            },
        ];

        let execution_steps = vec![
            StepSummaryInfo {
                step_uuid: "s1".to_string(),
                name: "start".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:00Z".to_string()),
                completed_at: Some("2026-03-06T10:00:01Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "s2".to_string(),
                name: "decide".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:01Z".to_string()),
                completed_at: Some("2026-03-06T10:00:02Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:01Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "s3".to_string(),
                name: "branch_a".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:02Z".to_string()),
                completed_at: Some("2026-03-06T10:00:03Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:02Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            // branch_b is NOT executed (untraversed)
        ];

        let input = TaskVisualizationInput {
            task: make_task_metadata(),
            template: TemplateSummary {
                steps: template_steps,
            },
            steps: execution_steps,
            dlq: make_dlq_info(false),
            links: make_links(),
        };

        let output = visualize_task(&input);

        // branch_b should be Untraversed
        let branch_b = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "branch_b")
            .expect("branch_b node should exist");
        assert_eq!(branch_b.visual_category, VisualCategory::Untraversed);

        // branch_a should be Completed
        let branch_a = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "branch_a")
            .expect("branch_a node should exist");
        assert_eq!(branch_a.visual_category, VisualCategory::Completed);

        // Edge to branch_b should be dashed
        let edge_to_b = output
            .graph
            .edges
            .iter()
            .find(|e| e.to == "branch_b")
            .expect("Edge to branch_b should exist");
        assert_eq!(edge_to_b.edge_style, EdgeStyle::Dashed);
    }

    // ── 5. Batch workers (dynamic steps) ──

    #[test]
    fn test_dynamic_batch_workers_included_as_subroutine() {
        let template_steps = vec![TemplateStepSummary {
            name: "process_batch".to_string(),
            step_type: "batchable".to_string(),
            handler: "batch.process".to_string(),
            dependencies: vec![],
            retryable: false,
            max_attempts: 1,
        }];

        let execution_steps = vec![
            StepSummaryInfo {
                step_uuid: "s1".to_string(),
                name: "process_batch".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:00Z".to_string()),
                completed_at: Some("2026-03-06T10:00:01Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "s2".to_string(),
                name: "process_batch_worker_0".to_string(),
                current_state: "complete".to_string(),
                created_at: Some("2026-03-06T10:00:01Z".to_string()),
                completed_at: Some("2026-03-06T10:00:02Z".to_string()),
                last_attempted_at: Some("2026-03-06T10:00:01Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
            StepSummaryInfo {
                step_uuid: "s3".to_string(),
                name: "process_batch_worker_1".to_string(),
                current_state: "in_progress".to_string(),
                created_at: Some("2026-03-06T10:00:01Z".to_string()),
                completed_at: None,
                last_attempted_at: Some("2026-03-06T10:00:01Z".to_string()),
                attempts: 1,
                max_attempts: 1,
                dependencies_satisfied: true,
                retry_eligible: false,
                error: None,
            },
        ];

        let input = TaskVisualizationInput {
            task: make_task_metadata(),
            template: TemplateSummary {
                steps: template_steps,
            },
            steps: execution_steps,
            dlq: make_dlq_info(false),
            links: make_links(),
        };

        let output = visualize_task(&input);

        // Dynamic workers should be Subroutine shaped
        let worker_0 = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "process_batch_worker_0")
            .expect("worker_0 should exist");
        assert_eq!(worker_0.node_shape, NodeShape::Subroutine);
        assert_eq!(worker_0.visual_category, VisualCategory::Completed);

        let worker_1 = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "process_batch_worker_1")
            .expect("worker_1 should exist");
        assert_eq!(worker_1.node_shape, NodeShape::Subroutine);
        assert_eq!(worker_1.visual_category, VisualCategory::InProgress);

        // Dynamic workers should also appear in table rows
        let table_names: Vec<&str> = output.table.rows.iter().map(|r| r.name.as_str()).collect();
        assert!(table_names.contains(&"process_batch_worker_0"));
        assert!(table_names.contains(&"process_batch_worker_1"));
    }

    // ── 6. DLQ annotation ──

    #[test]
    fn test_dlq_warning_when_in_dlq() {
        let mut input = make_input();
        input.dlq = make_dlq_info(true);

        let output = visualize_task(&input);

        assert!(!output.warnings.is_empty());
        assert!(output.warnings.iter().any(|w| w.contains("DLQ")));
    }

    #[test]
    fn test_no_dlq_warning_when_not_in_dlq() {
        let input = make_input();
        let output = visualize_task(&input);

        assert!(output
            .warnings
            .iter()
            .all(|w| !w.to_lowercase().contains("dlq")));
    }

    #[test]
    fn test_dlq_status_in_table_header() {
        let mut input = make_input();
        input.dlq = make_dlq_info(true);

        let output = visualize_task(&input);
        let header = output.table.header.as_ref().unwrap();
        assert_eq!(header.dlq_status.as_deref(), Some("In DLQ"));
        assert!(header.dlq_link.is_some());
    }

    #[test]
    fn test_no_dlq_status_when_not_in_dlq() {
        let input = make_input();
        let output = visualize_task(&input);
        let header = output.table.header.as_ref().unwrap();
        assert!(header.dlq_status.is_none());
        assert!(header.dlq_link.is_none());
    }

    // ── 7. Resource paths ──

    #[test]
    fn test_resource_path_format() {
        let input = make_input();
        let output = visualize_task(&input);

        let validate_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "validate_order")
            .unwrap();
        assert_eq!(
            validate_node.resource_path.as_deref(),
            Some("/v1/tasks/task-uuid-001/workflow_steps/step-uuid-001")
        );
    }

    #[test]
    fn test_no_resource_path_for_unexecuted_steps() {
        // Create input where generate_report has no execution data and no decision parent
        let mut input = make_input();
        // Remove the generate_report execution step
        input.steps.retain(|s| s.name != "generate_report");
        // Also remove the decision parent condition — generate_report depends on
        // process_payment and enrich_order (not decision steps), so it will be Pending

        let output = visualize_task(&input);

        let report_node = output
            .graph
            .nodes
            .iter()
            .find(|n| n.id == "generate_report")
            .unwrap();
        assert!(report_node.resource_path.is_none());
    }

    // ── 8. Table rows ──

    #[test]
    fn test_table_rows_populated_correctly() {
        let input = make_input();
        let output = visualize_task(&input);

        let validate_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "validate_order")
            .expect("validate_order row should exist");
        assert_eq!(validate_row.step_type, "standard");
        assert_eq!(validate_row.handler, "orders.validate");
        assert_eq!(validate_row.state.as_deref(), Some("complete"));
        assert_eq!(
            validate_row.visual_category_label.as_deref(),
            Some("Completed")
        );
        assert_eq!(validate_row.attempts.as_deref(), Some("1/3"));
        assert!(validate_row.step_link.is_some());
    }

    #[test]
    fn test_table_rows_in_topological_order() {
        let input = make_input();
        let output = visualize_task(&input);

        let names: Vec<&str> = output.table.rows.iter().map(|r| r.name.as_str()).collect();
        let validate_pos = names.iter().position(|n| *n == "validate_order").unwrap();
        let payment_pos = names.iter().position(|n| *n == "process_payment").unwrap();
        let report_pos = names.iter().position(|n| *n == "generate_report").unwrap();

        assert!(validate_pos < payment_pos);
        assert!(payment_pos < report_pos);
    }

    #[test]
    fn test_table_header_populated() {
        let input = make_input();
        let output = visualize_task(&input);

        let header = output.table.header.as_ref().unwrap();
        assert_eq!(header.name, "order_processing");
        assert_eq!(header.namespace, "ecommerce");
        assert_eq!(header.version, "1.0.0");
        assert_eq!(header.status.as_deref(), Some("in_progress"));
        assert_eq!(header.completion_percentage, Some(50.0));
        assert_eq!(header.health_status.as_deref(), Some("healthy"));
        assert!(header.elapsed.is_some());
        assert_eq!(header.task_link.as_deref(), Some("/v1/tasks/task-uuid-001"));
    }

    #[test]
    fn test_error_step_in_table() {
        let mut input = make_input();
        // Make process_payment an error step
        let payment_step = input
            .steps
            .iter_mut()
            .find(|s| s.name == "process_payment")
            .unwrap();
        payment_step.current_state = "error".to_string();
        payment_step.error = Some(StepErrorSummary {
            error_type: Some("TimeoutError".to_string()),
            retryable: true,
            status_code: Some(504),
        });

        let output = visualize_task(&input);

        let payment_row = output
            .table
            .rows
            .iter()
            .find(|r| r.name == "process_payment")
            .unwrap();
        assert_eq!(payment_row.visual_category_label.as_deref(), Some("Error"));
        assert_eq!(payment_row.error_type.as_deref(), Some("TimeoutError"));
    }

    // ── 9. Duration calculation ──

    #[test]
    fn test_duration_calculation_seconds() {
        let step = StepSummaryInfo {
            step_uuid: "s1".to_string(),
            name: "test".to_string(),
            current_state: "complete".to_string(),
            created_at: Some("2026-03-06T10:00:00Z".to_string()),
            completed_at: Some("2026-03-06T10:00:01.500Z".to_string()),
            last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
            attempts: 1,
            max_attempts: 1,
            dependencies_satisfied: true,
            retry_eligible: false,
            error: None,
        };

        let duration = calculate_duration(&step).unwrap();
        assert_eq!(duration, "1.500s");
    }

    #[test]
    fn test_duration_calculation_minutes() {
        let step = StepSummaryInfo {
            step_uuid: "s1".to_string(),
            name: "test".to_string(),
            current_state: "complete".to_string(),
            created_at: Some("2026-03-06T10:00:00Z".to_string()),
            completed_at: Some("2026-03-06T10:02:30Z".to_string()),
            last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
            attempts: 1,
            max_attempts: 1,
            dependencies_satisfied: true,
            retry_eligible: false,
            error: None,
        };

        let duration = calculate_duration(&step).unwrap();
        assert_eq!(duration, "2m 30s");
    }

    #[test]
    fn test_duration_none_for_incomplete_step() {
        let step = StepSummaryInfo {
            step_uuid: "s1".to_string(),
            name: "test".to_string(),
            current_state: "in_progress".to_string(),
            created_at: Some("2026-03-06T10:00:00Z".to_string()),
            completed_at: None,
            last_attempted_at: Some("2026-03-06T10:00:00Z".to_string()),
            attempts: 1,
            max_attempts: 1,
            dependencies_satisfied: true,
            retry_eligible: false,
            error: None,
        };

        assert!(calculate_duration(&step).is_none());
    }

    #[test]
    fn test_duration_uses_created_at_as_fallback() {
        let step = StepSummaryInfo {
            step_uuid: "s1".to_string(),
            name: "test".to_string(),
            current_state: "complete".to_string(),
            created_at: Some("2026-03-06T10:00:00Z".to_string()),
            completed_at: Some("2026-03-06T10:00:05Z".to_string()),
            last_attempted_at: None,
            attempts: 1,
            max_attempts: 1,
            dependencies_satisfied: true,
            retry_eligible: false,
            error: None,
        };

        let duration = calculate_duration(&step).unwrap();
        assert_eq!(duration, "5s");
    }

    // ── 10. Visual category labels ──

    #[test]
    fn test_visual_category_labels() {
        assert_eq!(
            visual_category_to_label(VisualCategory::Completed),
            "Completed"
        );
        assert_eq!(
            visual_category_to_label(VisualCategory::InProgress),
            "In Progress"
        );
        assert_eq!(visual_category_to_label(VisualCategory::Pending), "Pending");
        assert_eq!(visual_category_to_label(VisualCategory::Error), "Error");
        assert_eq!(
            visual_category_to_label(VisualCategory::Retrying),
            "Retrying"
        );
        assert_eq!(
            visual_category_to_label(VisualCategory::Untraversed),
            "Untraversed"
        );
        assert_eq!(
            visual_category_to_label(VisualCategory::Annotated),
            "Annotated"
        );
        assert_eq!(visual_category_to_label(VisualCategory::Default), "Unknown");
    }

    // ── 11. From implementation ──

    #[test]
    fn test_from_task_summary_response() {
        let response = TaskSummaryResponse {
            task: make_task_metadata(),
            template: TemplateSummary {
                steps: make_template_steps(),
            },
            steps: make_execution_steps(),
            dlq: make_dlq_info(false),
            links: make_links(),
        };

        let input = TaskVisualizationInput::from(&response);
        assert_eq!(input.task.task_uuid, "task-uuid-001");
        assert_eq!(input.template.steps.len(), 4);
        assert_eq!(input.steps.len(), 4);
    }

    // ── 12. Full integration test ──

    #[test]
    fn test_full_visualization_output() {
        let input = make_input();
        let output = visualize_task(&input);

        // Graph should have 4 nodes (all template steps)
        assert_eq!(output.graph.nodes.len(), 4);

        // Graph should have 4 edges
        assert_eq!(output.graph.edges.len(), 4);

        // Table should have header
        assert!(output.table.header.is_some());

        // Table should have 4 rows
        assert_eq!(output.table.rows.len(), 4);

        // No warnings (not in DLQ)
        assert!(output.warnings.is_empty());
    }
}
