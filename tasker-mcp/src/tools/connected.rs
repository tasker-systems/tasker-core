//! Tier 2 — Connected Read-Only tool implementations.
//!
//! Async functions that take a resolved `UnifiedOrchestrationClient` and param structs,
//! returning JSON strings. Client resolution happens in server.rs before calling these.

use uuid::Uuid;

use tasker_client::{OrchestrationClient, UnifiedOrchestrationClient};
use tasker_sdk::operational::responses::{
    BottleneckFilter, BottleneckReport, DlqSummary, HealthReport, PerformanceReport, StepSummary,
    TaskDetail, TaskSummary,
};

use super::helpers::error_json;
use super::params::{
    AnalyticsBottlenecksParams, AnalyticsPerformanceParams, DlqInspectToolParams,
    DlqListToolParams, DlqQueueToolParams, DlqStatsToolParams, StalenessCheckParams,
    StepAuditParams, StepInspectToolParams, SystemConfigParams, SystemHealthParams,
    TaskInspectParams, TaskListParams, TemplateInspectRemoteParams, TemplateListRemoteParams,
};

pub async fn task_list(client: &UnifiedOrchestrationClient, params: TaskListParams) -> String {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    match client
        .as_client()
        .list_tasks(
            limit,
            offset,
            params.namespace.as_deref(),
            params.status.as_deref(),
        )
        .await
    {
        Ok(response) => {
            let summaries: Vec<TaskSummary> =
                response.tasks.iter().map(TaskSummary::from).collect();
            serde_json::to_string_pretty(&serde_json::json!({
                "tasks": summaries,
                "total_count": response.pagination.total_count,
                "limit": limit,
                "offset": offset,
            }))
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn task_inspect(
    client: &UnifiedOrchestrationClient,
    params: TaskInspectParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };

    let task = match client.as_client().get_task(task_uuid).await {
        Ok(t) => t,
        Err(e) => return error_json("api_error", &e.to_string()),
    };

    let steps = match client.as_client().list_task_steps(task_uuid).await {
        Ok(s) => s,
        Err(e) => return error_json("api_error", &format!("Task found but steps failed: {}", e)),
    };

    let detail = TaskDetail {
        task: serde_json::to_value(&task)
            .unwrap_or_else(|_| serde_json::json!({"error": "serialization_failed"})),
        steps: steps.iter().map(StepSummary::from).collect(),
    };

    serde_json::to_string_pretty(&detail)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}

pub async fn step_inspect(
    client: &UnifiedOrchestrationClient,
    params: StepInspectToolParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };
    let step_uuid = match Uuid::parse_str(&params.step_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
    };

    match client.as_client().get_step(task_uuid, step_uuid).await {
        Ok(step) => serde_json::to_string_pretty(&step)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn step_audit(client: &UnifiedOrchestrationClient, params: StepAuditParams) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };
    let step_uuid = match Uuid::parse_str(&params.step_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
    };

    match client
        .as_client()
        .get_step_audit_history(task_uuid, step_uuid)
        .await
    {
        Ok(audit) => serde_json::to_string_pretty(&audit)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn dlq_list(client: &UnifiedOrchestrationClient, params: DlqListToolParams) -> String {
    let dlq_params = {
        let status = params
            .resolution_status
            .as_deref()
            .and_then(|s| tasker_sdk::operational::enums::parse_dlq_resolution_status(s).ok());
        Some(tasker_shared::models::orchestration::DlqListParams {
            resolution_status: status,
            limit: params.limit.unwrap_or(20),
            offset: 0,
        })
    };

    match client.list_dlq_entries(dlq_params.as_ref()).await {
        Ok(entries) => {
            let summaries: Vec<DlqSummary> = entries.iter().map(DlqSummary::from).collect();
            serde_json::to_string_pretty(&serde_json::json!({
                "entries": summaries,
                "count": summaries.len(),
            }))
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn dlq_inspect(
    client: &UnifiedOrchestrationClient,
    params: DlqInspectToolParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };

    match client.get_dlq_entry(task_uuid).await {
        Ok(entry) => serde_json::to_string_pretty(&serde_json::json!({
            "dlq_entry_uuid": entry.dlq_entry_uuid.to_string(),
            "task_uuid": entry.task_uuid.to_string(),
            "original_state": entry.original_state,
            "dlq_reason": format!("{:?}", entry.dlq_reason),
            "dlq_timestamp": entry.dlq_timestamp.to_string(),
            "resolution_status": format!("{:?}", entry.resolution_status),
            "resolution_timestamp": entry.resolution_timestamp.map(|t| t.to_string()),
            "resolution_notes": entry.resolution_notes,
            "resolved_by": entry.resolved_by,
            "task_snapshot": entry.task_snapshot,
            "metadata": entry.metadata,
            "created_at": entry.created_at.to_string(),
            "updated_at": entry.updated_at.to_string(),
        }))
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn dlq_stats(client: &UnifiedOrchestrationClient, _params: DlqStatsToolParams) -> String {
    match client.get_dlq_stats().await {
        Ok(stats) => serde_json::to_string_pretty(&stats)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn dlq_queue(client: &UnifiedOrchestrationClient, params: DlqQueueToolParams) -> String {
    match client.get_investigation_queue(params.limit).await {
        Ok(queue) => serde_json::to_string_pretty(&queue)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn staleness_check(
    client: &UnifiedOrchestrationClient,
    params: StalenessCheckParams,
) -> String {
    match client.get_staleness_monitoring(params.limit).await {
        Ok(monitoring) => serde_json::to_string_pretty(&monitoring)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn analytics_performance(
    client: &UnifiedOrchestrationClient,
    params: AnalyticsPerformanceParams,
) -> String {
    let query = params
        .hours
        .map(|h| tasker_shared::types::api::orchestration::MetricsQuery { hours: Some(h) });

    match client.get_performance_metrics(query.as_ref()).await {
        Ok(metrics) => {
            let report = PerformanceReport {
                metrics: serde_json::to_value(&metrics).unwrap_or_else(|_| serde_json::json!({})),
                period_hours: params.hours,
            };
            serde_json::to_string_pretty(&report)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn analytics_bottlenecks(
    client: &UnifiedOrchestrationClient,
    params: AnalyticsBottlenecksParams,
) -> String {
    let query = if params.limit.is_some() || params.min_executions.is_some() {
        Some(tasker_shared::types::api::orchestration::BottleneckQuery {
            limit: params.limit,
            min_executions: params.min_executions,
        })
    } else {
        None
    };

    match client.get_bottlenecks(query.as_ref()).await {
        Ok(analysis) => {
            let report = BottleneckReport {
                analysis: serde_json::to_value(&analysis).unwrap_or_else(|_| serde_json::json!({})),
                filter: BottleneckFilter {
                    limit: params.limit,
                    min_executions: params.min_executions,
                },
            };
            serde_json::to_string_pretty(&report)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn system_health(
    client: &UnifiedOrchestrationClient,
    _params: SystemHealthParams,
) -> String {
    match client.as_client().get_detailed_health().await {
        Ok(health) => {
            let report = HealthReport {
                overall_status: health.status.clone(),
                timestamp: health.timestamp.clone(),
                components: serde_json::to_value(&health.checks)
                    .unwrap_or_else(|_| serde_json::json!({})),
                system_info: serde_json::to_value(&health.info)
                    .unwrap_or_else(|_| serde_json::json!({})),
            };
            serde_json::to_string_pretty(&report)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn system_config(
    client: &UnifiedOrchestrationClient,
    _params: SystemConfigParams,
) -> String {
    match client.get_config().await {
        Ok(config) => serde_json::to_string_pretty(&config)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn template_list_remote(
    client: &UnifiedOrchestrationClient,
    params: TemplateListRemoteParams,
) -> String {
    match client
        .as_client()
        .list_templates(params.namespace.as_deref())
        .await
    {
        Ok(templates) => serde_json::to_string_pretty(&templates)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}

pub async fn template_inspect_remote(
    client: &UnifiedOrchestrationClient,
    params: TemplateInspectRemoteParams,
) -> String {
    match client
        .as_client()
        .get_template(&params.namespace, &params.name, &params.version)
        .await
    {
        Ok(template) => serde_json::to_string_pretty(&template)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        Err(e) => error_json("api_error", &e.to_string()),
    }
}
