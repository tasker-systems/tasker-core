//! Tier 3 — Write Tools with Confirmation semantics.
//!
//! Async functions that take a resolved `UnifiedOrchestrationClient`, the active profile name,
//! and param structs. Each implements the two-phase preview→confirm pattern.

use uuid::Uuid;

use tasker_client::{OrchestrationClient, UnifiedOrchestrationClient};
use tasker_sdk::operational::confirmation::{build_preview, handle_api_error, ConfirmationPhase};

use super::helpers::error_json;
use super::params::{
    DlqUpdateParams, StepCompleteParams, StepResolveParams, StepRetryParams, TaskCancelParams,
    TaskSubmitParams,
};

pub async fn task_submit(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: TaskSubmitParams,
) -> String {
    let version = params.version.as_deref().unwrap_or("0.1.0");

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => {
            let preview = build_preview(
                "task_submit",
                &format!(
                    "Submit task '{}' in namespace '{}' version '{}'",
                    params.name, params.namespace, version
                ),
                serde_json::json!({
                    "name": params.name,
                    "namespace": params.namespace,
                    "version": version,
                    "context_keys": params.context.as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default(),
                    "initiator": params.initiator.as_deref().unwrap_or("mcp-agent"),
                    "source_system": params.source_system.as_deref().unwrap_or("tasker-mcp"),
                    "tags": params.tags,
                    "priority": params.priority,
                }),
            );
            serde_json::to_string_pretty(&preview)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        ConfirmationPhase::Execute => {
            let request = tasker_shared::models::core::task_request::TaskRequest::builder()
                .name(params.name)
                .namespace(params.namespace)
                .version(version.to_string())
                .context(params.context)
                .initiator(params.initiator.unwrap_or_else(|| "mcp-agent".to_string()))
                .source_system(
                    params
                        .source_system
                        .unwrap_or_else(|| "tasker-mcp".to_string()),
                )
                .reason(
                    params
                        .reason
                        .unwrap_or_else(|| "Submitted via MCP".to_string()),
                )
                .tags(params.tags)
                .maybe_priority(params.priority)
                .build();

            match client.as_client().create_task(request).await {
                Ok(response) => serde_json::to_string_pretty(&serde_json::json!({
                    "status": "executed",
                    "action": "task_submit",
                    "result": response,
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                Err(e) => handle_api_error(&e.to_string(), "task_submit", profile_name),
            }
        }
    }
}

pub async fn task_cancel(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: TaskCancelParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => match client.as_client().get_task(task_uuid).await {
            Ok(task) => {
                let preview = build_preview(
                    "task_cancel",
                    &format!("Cancel task '{}'", params.task_uuid),
                    serde_json::json!({
                        "task_uuid": params.task_uuid,
                        "current_state": serde_json::to_value(&task)
                            .unwrap_or_else(|_| serde_json::json!({})),
                    }),
                );
                serde_json::to_string_pretty(&preview)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => handle_api_error(&e.to_string(), "task_cancel", profile_name),
        },
        ConfirmationPhase::Execute => match client.as_client().cancel_task(task_uuid).await {
            Ok(()) => serde_json::json!({
                "status": "executed",
                "action": "task_cancel",
                "task_uuid": params.task_uuid,
                "message": "Task cancelled successfully."
            })
            .to_string(),
            Err(e) => handle_api_error(&e.to_string(), "task_cancel", profile_name),
        },
    }
}

pub async fn step_retry(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: StepRetryParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };
    let step_uuid = match Uuid::parse_str(&params.step_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
    };

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => {
            match client.as_client().get_step(task_uuid, step_uuid).await {
                Ok(step) => {
                    let preview = build_preview(
                        "step_retry",
                        &format!(
                            "Reset step '{}' for retry on task '{}'",
                            params.step_uuid, params.task_uuid
                        ),
                        serde_json::json!({
                            "task_uuid": params.task_uuid,
                            "step_uuid": params.step_uuid,
                            "current_step": serde_json::to_value(&step)
                                .unwrap_or_else(|_| serde_json::json!({})),
                            "reason": params.reason,
                            "reset_by": params.reset_by,
                        }),
                    );
                    serde_json::to_string_pretty(&preview)
                        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                }
                Err(e) => handle_api_error(&e.to_string(), "step_retry", profile_name),
            }
        }
        ConfirmationPhase::Execute => {
            let action =
                tasker_shared::types::api::orchestration::StepManualAction::ResetForRetry {
                    reason: params.reason,
                    reset_by: params.reset_by,
                };

            match client
                .as_client()
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                    "status": "executed",
                    "action": "step_retry",
                    "result": step,
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                Err(e) => handle_api_error(&e.to_string(), "step_retry", profile_name),
            }
        }
    }
}

pub async fn step_resolve(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: StepResolveParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };
    let step_uuid = match Uuid::parse_str(&params.step_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
    };

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => {
            match client.as_client().get_step(task_uuid, step_uuid).await {
                Ok(step) => {
                    let preview = build_preview(
                        "step_resolve",
                        &format!(
                            "Mark step '{}' as manually resolved on task '{}'",
                            params.step_uuid, params.task_uuid
                        ),
                        serde_json::json!({
                            "task_uuid": params.task_uuid,
                            "step_uuid": params.step_uuid,
                            "current_step": serde_json::to_value(&step)
                                .unwrap_or_else(|_| serde_json::json!({})),
                            "reason": params.reason,
                            "resolved_by": params.resolved_by,
                        }),
                    );
                    serde_json::to_string_pretty(&preview)
                        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                }
                Err(e) => handle_api_error(&e.to_string(), "step_resolve", profile_name),
            }
        }
        ConfirmationPhase::Execute => {
            let action =
                tasker_shared::types::api::orchestration::StepManualAction::ResolveManually {
                    reason: params.reason,
                    resolved_by: params.resolved_by,
                };

            match client
                .as_client()
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                    "status": "executed",
                    "action": "step_resolve",
                    "result": step,
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                Err(e) => handle_api_error(&e.to_string(), "step_resolve", profile_name),
            }
        }
    }
}

pub async fn step_complete(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: StepCompleteParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };
    let step_uuid = match Uuid::parse_str(&params.step_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
    };

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => {
            match client.as_client().get_step(task_uuid, step_uuid).await {
                Ok(step) => {
                    let preview = build_preview(
                        "step_complete",
                        &format!(
                            "Manually complete step '{}' on task '{}'",
                            params.step_uuid, params.task_uuid
                        ),
                        serde_json::json!({
                            "task_uuid": params.task_uuid,
                            "step_uuid": params.step_uuid,
                            "current_step": serde_json::to_value(&step)
                                .unwrap_or_else(|_| serde_json::json!({})),
                            "result_data": params.result,
                            "reason": params.reason,
                            "completed_by": params.completed_by,
                        }),
                    );
                    serde_json::to_string_pretty(&preview)
                        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                }
                Err(e) => handle_api_error(&e.to_string(), "step_complete", profile_name),
            }
        }
        ConfirmationPhase::Execute => {
            let completion_data = tasker_shared::types::api::orchestration::ManualCompletionData {
                result: params.result,
                metadata: params.metadata,
            };
            let action =
                tasker_shared::types::api::orchestration::StepManualAction::CompleteManually {
                    completion_data,
                    reason: params.reason,
                    completed_by: params.completed_by,
                };

            match client
                .as_client()
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                    "status": "executed",
                    "action": "step_complete",
                    "result": step,
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                Err(e) => handle_api_error(&e.to_string(), "step_complete", profile_name),
            }
        }
    }
}

pub async fn dlq_update(
    client: &UnifiedOrchestrationClient,
    profile_name: &str,
    params: DlqUpdateParams,
) -> String {
    let dlq_entry_uuid = match Uuid::parse_str(&params.dlq_entry_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid dlq_entry_uuid: {}", e)),
    };

    match ConfirmationPhase::from_flag(params.confirm) {
        ConfirmationPhase::Preview => {
            let preview = build_preview(
                "dlq_update",
                &format!("Update DLQ entry '{}'", params.dlq_entry_uuid),
                serde_json::json!({
                    "dlq_entry_uuid": params.dlq_entry_uuid,
                    "resolution_status": params.resolution_status,
                    "resolution_notes": params.resolution_notes,
                    "resolved_by": params.resolved_by,
                    "has_metadata": params.metadata.is_some(),
                }),
            );
            serde_json::to_string_pretty(&preview)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        ConfirmationPhase::Execute => {
            let resolution_status = params
                .resolution_status
                .as_deref()
                .map(tasker_sdk::operational::enums::parse_dlq_resolution_status)
                .transpose()
                .map_err(|e| error_json("invalid_resolution_status", &e));

            let resolution_status = match resolution_status {
                Ok(s) => s,
                Err(e) => return e,
            };

            let update = tasker_shared::models::orchestration::DlqInvestigationUpdate {
                resolution_status,
                resolution_notes: params.resolution_notes,
                resolved_by: params.resolved_by,
                metadata: params.metadata,
            };

            match client
                .update_dlq_investigation(dlq_entry_uuid, update)
                .await
            {
                Ok(()) => serde_json::json!({
                    "status": "executed",
                    "action": "dlq_update",
                    "dlq_entry_uuid": params.dlq_entry_uuid,
                    "message": "DLQ entry updated successfully."
                })
                .to_string(),
                Err(e) => handle_api_error(&e.to_string(), "dlq_update", profile_name),
            }
        }
    }
}
