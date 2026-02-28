//! Shared response types consumed by both MCP (JSON serialization) and ctl (terminal formatting).
//!
//! These types provide a stable, simplified view of API responses suitable for
//! LLM consumption and CLI output formatting.

use serde::Serialize;

/// Compact task summary for list views.
#[derive(Debug, Serialize)]
pub struct TaskSummary {
    pub task_uuid: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: String,
    pub completion_percentage: f64,
    pub health_status: String,
    pub total_steps: i64,
    pub completed_steps: i64,
    pub created_at: String,
}

impl From<&tasker_shared::types::api::orchestration::TaskResponse> for TaskSummary {
    fn from(task: &tasker_shared::types::api::orchestration::TaskResponse) -> Self {
        Self {
            task_uuid: task.task_uuid.clone(),
            name: task.name.clone(),
            namespace: task.namespace.clone(),
            version: task.version.clone(),
            status: task.status.clone(),
            completion_percentage: task.completion_percentage,
            health_status: task.health_status.clone(),
            total_steps: task.total_steps,
            completed_steps: task.completed_steps,
            created_at: task.created_at.to_rfc3339(),
        }
    }
}

/// Detailed task view with step breakdown.
#[derive(Debug, Serialize)]
pub struct TaskDetail {
    pub task: serde_json::Value,
    pub steps: Vec<StepSummary>,
}

/// Step summary for task detail views.
#[derive(Debug, Serialize)]
pub struct StepSummary {
    pub step_uuid: String,
    pub name: String,
    pub status: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub retry_eligible: bool,
    pub dependencies_satisfied: bool,
}

impl From<&tasker_shared::types::api::orchestration::StepResponse> for StepSummary {
    fn from(step: &tasker_shared::types::api::orchestration::StepResponse) -> Self {
        Self {
            step_uuid: step.step_uuid.clone(),
            name: step.name.clone(),
            status: step.current_state.clone(),
            attempt_count: step.attempts,
            max_attempts: step.max_attempts,
            retry_eligible: step.retry_eligible,
            dependencies_satisfied: step.dependencies_satisfied,
        }
    }
}

/// DLQ entry summary for list views.
#[derive(Debug, Serialize)]
pub struct DlqSummary {
    pub dlq_entry_uuid: String,
    pub task_uuid: String,
    pub dlq_reason: String,
    pub resolution_status: String,
    pub created_at: String,
}

impl From<&tasker_shared::models::orchestration::DlqEntry> for DlqSummary {
    fn from(entry: &tasker_shared::models::orchestration::DlqEntry) -> Self {
        Self {
            dlq_entry_uuid: entry.dlq_entry_uuid.to_string(),
            task_uuid: entry.task_uuid.to_string(),
            dlq_reason: format!("{:?}", entry.dlq_reason),
            resolution_status: format!("{:?}", entry.resolution_status),
            created_at: entry.created_at.to_string(),
        }
    }
}

/// System health report.
#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub overall_status: String,
    pub timestamp: String,
    pub components: serde_json::Value,
    pub system_info: serde_json::Value,
}

/// Performance analytics report.
#[derive(Debug, Serialize)]
pub struct PerformanceReport {
    pub metrics: serde_json::Value,
    pub period_hours: Option<u32>,
}

/// Bottleneck analysis report.
#[derive(Debug, Serialize)]
pub struct BottleneckReport {
    pub analysis: serde_json::Value,
    pub filter: BottleneckFilter,
}

/// Filter parameters used for bottleneck analysis.
#[derive(Debug, Serialize)]
pub struct BottleneckFilter {
    pub limit: Option<i32>,
    pub min_executions: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_task_summary_from_task_response() {
        let task = tasker_shared::types::api::orchestration::TaskResponse {
            task_uuid: "abc-123".to_string(),
            name: "test_task".to_string(),
            namespace: "default".to_string(),
            version: "1.0.0".to_string(),
            status: "complete".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            context: serde_json::json!({}),
            initiator: "test".to_string(),
            source_system: "test".to_string(),
            reason: "test".to_string(),
            priority: None,
            tags: None,
            correlation_id: Uuid::new_v4(),
            parent_correlation_id: None,
            total_steps: 5,
            pending_steps: 0,
            in_progress_steps: 0,
            completed_steps: 5,
            failed_steps: 0,
            ready_steps: 0,
            execution_status: "complete".to_string(),
            recommended_action: "none".to_string(),
            completion_percentage: 100.0,
            health_status: "healthy".to_string(),
            steps: vec![],
        };

        let summary = TaskSummary::from(&task);
        assert_eq!(summary.name, "test_task");
        assert_eq!(summary.total_steps, 5);
        assert_eq!(summary.completed_steps, 5);
        assert_eq!(summary.completion_percentage, 100.0);
    }

    #[test]
    fn test_step_summary_from_step_response() {
        let step = tasker_shared::types::api::orchestration::StepResponse {
            step_uuid: "step-123".to_string(),
            task_uuid: "task-123".to_string(),
            name: "validate_order".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:01:00Z".to_string(),
            completed_at: None,
            results: None,
            current_state: "complete".to_string(),
            dependencies_satisfied: true,
            retry_eligible: false,
            ready_for_execution: false,
            total_parents: 0,
            completed_parents: 0,
            attempts: 1,
            max_attempts: 3,
            last_failure_at: None,
            next_retry_at: None,
            last_attempted_at: None,
        };

        let summary = StepSummary::from(&step);
        assert_eq!(summary.name, "validate_order");
        assert_eq!(summary.attempt_count, 1);
        assert!(summary.dependencies_satisfied);
    }
}
