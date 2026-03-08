//! # Task Summary — Computed View via SQL Function for Visualization
//!
//! **CRITICAL**: This is NOT a database table - it's a computed view via SQL functions.
//!
//! ## Overview
//!
//! The `TaskSummaryRow` represents a rich, single-query summary of a task including metadata,
//! step details, execution context, and DLQ status. Designed for task visualization rendering
//! (Mermaid/SVG).
//!
//! Data is computed on-demand by the `get_task_summary()` and `get_task_summaries()` SQL
//! functions, which aggregate data from tasks, steps, step readiness, and DLQ tables.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Rich task summary row returned by `get_task_summary`/`get_task_summaries` SQL functions.
///
/// Contains task metadata, step summaries as JSONB, execution context as JSONB,
/// and DLQ status as JSONB. Use the accessor methods to parse JSONB fields into
/// strongly-typed data structures.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskSummaryRow {
    pub task_uuid: Uuid,
    pub named_task_uuid: Uuid,
    pub task_name: String,
    pub task_version: String,
    pub namespace_name: String,
    pub task_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub initiator: Option<String>,
    pub source_system: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Uuid,
    pub template_configuration: Option<serde_json::Value>,
    /// JSONB array of step summary objects.
    pub step_summaries: serde_json::Value,
    /// JSONB object with execution counts, percentages, and status classification.
    pub execution_context: serde_json::Value,
    /// JSONB object with DLQ investigation status.
    pub dlq_status: serde_json::Value,
}

/// Typed representation of a single step summary from the `step_summaries` JSONB array.
///
/// Field names match the `json_build_object` keys in the SQL function.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepSummaryData {
    pub step_uuid: String,
    pub name: String,
    pub current_state: String,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_attempted_at: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub dependencies_satisfied: bool,
    pub retry_eligible: bool,
    pub error_type: Option<String>,
    pub error_retryable: Option<bool>,
    pub error_status_code: Option<i32>,
}

/// Typed representation of the `execution_context` JSONB object.
///
/// Contains step counts, completion percentage, and derived health/status classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionContextData {
    pub total_steps: i64,
    pub pending_steps: i64,
    pub in_progress_steps: i64,
    pub completed_steps: i64,
    pub failed_steps: i64,
    pub completion_percentage: f64,
    pub health_status: String,
    pub execution_status: String,
    pub recommended_action: Option<String>,
}

/// Typed representation of the `dlq_status` JSONB object.
///
/// Indicates whether the task has a pending DLQ investigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlqSummaryData {
    pub in_dlq: bool,
    pub dlq_reason: Option<String>,
    pub resolution_status: Option<String>,
}

impl TaskSummaryRow {
    /// Get summary for a single task using SQL function.
    ///
    /// Calls `get_task_summary(input_task_uuid)` which delegates to the batch function.
    pub async fn get_for_task(pool: &PgPool, task_uuid: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
            SELECT
                task_uuid as "task_uuid!: Uuid",
                named_task_uuid as "named_task_uuid!: Uuid",
                task_name as "task_name!: String",
                task_version as "task_version!: String",
                namespace_name as "namespace_name!: String",
                task_status as "task_status!: String",
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>",
                completed_at as "completed_at: DateTime<Utc>",
                initiator,
                source_system,
                reason,
                correlation_id as "correlation_id!: Uuid",
                template_configuration as "template_configuration: serde_json::Value",
                step_summaries as "step_summaries!: serde_json::Value",
                execution_context as "execution_context!: serde_json::Value",
                dlq_status as "dlq_status!: serde_json::Value"
            FROM get_task_summary($1::uuid)
            "#,
            task_uuid
        )
        .fetch_optional(pool)
        .await
    }

    /// Get summaries for multiple tasks using batch SQL function.
    ///
    /// Calls `get_task_summaries(input_task_uuids)` for efficient batch retrieval.
    pub async fn get_for_tasks(
        pool: &PgPool,
        task_uuids: &[Uuid],
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"
            SELECT
                task_uuid as "task_uuid!: Uuid",
                named_task_uuid as "named_task_uuid!: Uuid",
                task_name as "task_name!: String",
                task_version as "task_version!: String",
                namespace_name as "namespace_name!: String",
                task_status as "task_status!: String",
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>",
                completed_at as "completed_at: DateTime<Utc>",
                initiator,
                source_system,
                reason,
                correlation_id as "correlation_id!: Uuid",
                template_configuration as "template_configuration: serde_json::Value",
                step_summaries as "step_summaries!: serde_json::Value",
                execution_context as "execution_context!: serde_json::Value",
                dlq_status as "dlq_status!: serde_json::Value"
            FROM get_task_summaries($1::uuid[])
            "#,
            task_uuids
        )
        .fetch_all(pool)
        .await
    }

    /// Parse `step_summaries` JSONB into typed data.
    pub fn step_summaries(&self) -> Vec<StepSummaryData> {
        serde_json::from_value(self.step_summaries.clone()).unwrap_or_default()
    }

    /// Parse `execution_context` JSONB into typed data.
    pub fn execution_context(&self) -> ExecutionContextData {
        serde_json::from_value(self.execution_context.clone()).unwrap_or_else(|_| {
            ExecutionContextData {
                total_steps: 0,
                pending_steps: 0,
                in_progress_steps: 0,
                completed_steps: 0,
                failed_steps: 0,
                completion_percentage: 0.0,
                health_status: "unknown".to_string(),
                execution_status: "unknown".to_string(),
                recommended_action: None,
            }
        })
    }

    /// Parse `dlq_status` JSONB into typed data.
    pub fn dlq(&self) -> DlqSummaryData {
        serde_json::from_value(self.dlq_status.clone()).unwrap_or(DlqSummaryData {
            in_dlq: false,
            dlq_reason: None,
            resolution_status: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_step_summary_deserialization() {
        let json_data = json!({
            "step_uuid": "550e8400-e29b-41d4-a716-446655440000",
            "name": "fetch_data",
            "current_state": "complete",
            "created_at": "2026-03-06T10:00:00Z",
            "completed_at": "2026-03-06T10:01:00Z",
            "last_attempted_at": "2026-03-06T10:00:30Z",
            "attempts": 1,
            "max_attempts": 3,
            "dependencies_satisfied": true,
            "retry_eligible": false,
            "error_type": null,
            "error_retryable": null,
            "error_status_code": null
        });

        let step: StepSummaryData = serde_json::from_value(json_data).unwrap();
        assert_eq!(step.name, "fetch_data");
        assert_eq!(step.current_state, "complete");
        assert_eq!(step.attempts, 1);
        assert_eq!(step.max_attempts, 3);
        assert!(step.dependencies_satisfied);
        assert!(!step.retry_eligible);
        assert!(step.error_type.is_none());
        assert!(step.error_retryable.is_none());
        assert!(step.error_status_code.is_none());
        assert_eq!(step.completed_at.as_deref(), Some("2026-03-06T10:01:00Z"));
    }

    #[test]
    fn test_step_summary_with_error() {
        let json_data = json!({
            "step_uuid": "550e8400-e29b-41d4-a716-446655440001",
            "name": "process_payment",
            "current_state": "error",
            "created_at": "2026-03-06T10:00:00Z",
            "completed_at": null,
            "last_attempted_at": "2026-03-06T10:02:00Z",
            "attempts": 3,
            "max_attempts": 3,
            "dependencies_satisfied": true,
            "retry_eligible": false,
            "error_type": "timeout",
            "error_retryable": true,
            "error_status_code": 504
        });

        let step: StepSummaryData = serde_json::from_value(json_data).unwrap();
        assert_eq!(step.current_state, "error");
        assert_eq!(step.attempts, 3);
        assert_eq!(step.error_type.as_deref(), Some("timeout"));
        assert_eq!(step.error_retryable, Some(true));
        assert_eq!(step.error_status_code, Some(504));
        assert!(step.completed_at.is_none());
    }

    #[test]
    fn test_step_summaries_array_deserialization() {
        let json_data = json!([
            {
                "step_uuid": "aaa",
                "name": "step_one",
                "current_state": "complete",
                "created_at": null,
                "completed_at": null,
                "last_attempted_at": null,
                "attempts": 1,
                "max_attempts": 3,
                "dependencies_satisfied": true,
                "retry_eligible": false,
                "error_type": null,
                "error_retryable": null,
                "error_status_code": null
            },
            {
                "step_uuid": "bbb",
                "name": "step_two",
                "current_state": "pending",
                "created_at": null,
                "completed_at": null,
                "last_attempted_at": null,
                "attempts": 0,
                "max_attempts": 3,
                "dependencies_satisfied": false,
                "retry_eligible": true,
                "error_type": null,
                "error_retryable": null,
                "error_status_code": null
            }
        ]);

        let steps: Vec<StepSummaryData> = serde_json::from_value(json_data).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "step_one");
        assert_eq!(steps[1].name, "step_two");
        assert!(!steps[1].dependencies_satisfied);
    }

    #[test]
    fn test_execution_context_deserialization() {
        let json_data = json!({
            "total_steps": 5,
            "pending_steps": 1,
            "in_progress_steps": 2,
            "completed_steps": 2,
            "failed_steps": 0,
            "completion_percentage": 40.0,
            "health_status": "healthy",
            "execution_status": "processing",
            "recommended_action": "wait_for_completion"
        });

        let ctx: ExecutionContextData = serde_json::from_value(json_data).unwrap();
        assert_eq!(ctx.total_steps, 5);
        assert_eq!(ctx.pending_steps, 1);
        assert_eq!(ctx.in_progress_steps, 2);
        assert_eq!(ctx.completed_steps, 2);
        assert_eq!(ctx.failed_steps, 0);
        assert!((ctx.completion_percentage - 40.0).abs() < f64::EPSILON);
        assert_eq!(ctx.health_status, "healthy");
        assert_eq!(ctx.execution_status, "processing");
        assert_eq!(
            ctx.recommended_action.as_deref(),
            Some("wait_for_completion")
        );
    }

    #[test]
    fn test_execution_context_all_complete() {
        let json_data = json!({
            "total_steps": 3,
            "pending_steps": 0,
            "in_progress_steps": 0,
            "completed_steps": 3,
            "failed_steps": 0,
            "completion_percentage": 100.0,
            "health_status": "healthy",
            "execution_status": "all_complete",
            "recommended_action": "finalize_task"
        });

        let ctx: ExecutionContextData = serde_json::from_value(json_data).unwrap();
        assert_eq!(ctx.completed_steps, ctx.total_steps);
        assert!((ctx.completion_percentage - 100.0).abs() < f64::EPSILON);
        assert_eq!(ctx.execution_status, "all_complete");
    }

    #[test]
    fn test_execution_context_with_null_recommended_action() {
        let json_data = json!({
            "total_steps": 0,
            "pending_steps": 0,
            "in_progress_steps": 0,
            "completed_steps": 0,
            "failed_steps": 0,
            "completion_percentage": 0.0,
            "health_status": "unknown",
            "execution_status": "waiting_for_dependencies",
            "recommended_action": null
        });

        let ctx: ExecutionContextData = serde_json::from_value(json_data).unwrap();
        assert!(ctx.recommended_action.is_none());
    }

    #[test]
    fn test_dlq_summary_not_in_dlq() {
        let json_data = json!({
            "in_dlq": false,
            "dlq_reason": null,
            "resolution_status": null
        });

        let dlq: DlqSummaryData = serde_json::from_value(json_data).unwrap();
        assert!(!dlq.in_dlq);
        assert!(dlq.dlq_reason.is_none());
        assert!(dlq.resolution_status.is_none());
    }

    #[test]
    fn test_dlq_summary_in_dlq() {
        let json_data = json!({
            "in_dlq": true,
            "dlq_reason": "step_failures_exhausted",
            "resolution_status": "pending"
        });

        let dlq: DlqSummaryData = serde_json::from_value(json_data).unwrap();
        assert!(dlq.in_dlq);
        assert_eq!(dlq.dlq_reason.as_deref(), Some("step_failures_exhausted"));
        assert_eq!(dlq.resolution_status.as_deref(), Some("pending"));
    }

    #[test]
    fn test_step_summaries_default_on_invalid_json() {
        let row = make_test_row(json!("not an array"), json!({}), json!({}));
        let steps = row.step_summaries();
        assert!(steps.is_empty());
    }

    #[test]
    fn test_execution_context_default_on_invalid_json() {
        let row = make_test_row(json!([]), json!("invalid"), json!({}));
        let ctx = row.execution_context();
        assert_eq!(ctx.total_steps, 0);
        assert_eq!(ctx.health_status, "unknown");
        assert_eq!(ctx.execution_status, "unknown");
    }

    #[test]
    fn test_dlq_default_on_invalid_json() {
        let row = make_test_row(json!([]), json!({}), json!("invalid"));
        let dlq = row.dlq();
        assert!(!dlq.in_dlq);
        assert!(dlq.dlq_reason.is_none());
    }

    /// Helper to construct a `TaskSummaryRow` for unit tests without a database.
    fn make_test_row(
        step_summaries: serde_json::Value,
        execution_context: serde_json::Value,
        dlq_status: serde_json::Value,
    ) -> TaskSummaryRow {
        TaskSummaryRow {
            task_uuid: Uuid::nil(),
            named_task_uuid: Uuid::nil(),
            task_name: "test_task".to_string(),
            task_version: "1.0.0".to_string(),
            namespace_name: "default".to_string(),
            task_status: "pending".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            initiator: None,
            source_system: None,
            reason: None,
            correlation_id: Uuid::nil(),
            template_configuration: None,
            step_summaries,
            execution_context,
            dlq_status,
        }
    }
}
