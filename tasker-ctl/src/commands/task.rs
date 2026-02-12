//! Task command handlers for the Tasker CLI

use std::str::FromStr;

use tasker_client::{ClientConfig, ClientResult, OrchestrationApiClient, OrchestrationApiConfig};
use tasker_shared::models::core::{task::TaskListQuery, task_request::TaskRequest};
use tasker_shared::types::api::orchestration::{ManualCompletionData, StepManualAction};
use uuid::Uuid;

use crate::output;
use crate::TaskCommands;

pub(crate) async fn handle_task_command(
    cmd: TaskCommands,
    config: &ClientConfig,
) -> ClientResult<()> {
    let orchestration_config = OrchestrationApiConfig {
        base_url: config.orchestration.base_url.clone(),
        timeout_ms: config.orchestration.timeout_ms,
        max_retries: config.orchestration.max_retries,
        auth: config.orchestration.resolve_web_auth_config(),
    };

    let client = OrchestrationApiClient::new(orchestration_config)?;

    match cmd {
        TaskCommands::Create {
            namespace,
            name,
            version,
            input,
            description: _,
            priority,
            correlation_id,
        } => {
            output::plain(format!("Creating task: {namespace}/{name} v{version}"));

            // Parse input as JSON
            let context: serde_json::Value = serde_json::from_str(&input).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid JSON input: {}", e))
            })?;

            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::now_v7().to_string());
            let correlation_id = Uuid::from_str(correlation_id.as_str()).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid correlation ID: {}", e))
            })?;

            let task_request = TaskRequest {
                namespace,
                name,
                version,
                context,
                initiator: "tasker-ctl".to_string(),
                source_system: "cli".to_string(),
                reason: "CLI task creation".to_string(),
                tags: Vec::new(),

                requested_at: chrono::Utc::now().naive_utc(),
                options: None,
                priority: Some(priority as i32),
                correlation_id,
                parent_correlation_id: None,
                idempotency_key: None,
            };

            match client.create_task(task_request).await {
                Ok(response) => {
                    output::success("Task created successfully!");
                    output::label("  Task UUID", &response.task_uuid);
                    output::label(
                        "  Name",
                        format!(
                            "{}/{} v{}",
                            response.namespace, response.name, response.version
                        ),
                    );
                    output::label("  Status", &response.status);
                    output::label("  Total Steps", response.total_steps);
                    output::label("  Created at", response.created_at);
                }
                Err(e) => {
                    output::error(format!("Failed to create task: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::Get { task_id } => {
            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
            })?;

            match client.get_task(task_uuid).await {
                Ok(response) => {
                    output::header("Task Details");
                    output::label("  UUID", &response.task_uuid);
                    output::label("  Name", &response.name);
                    output::label("  Namespace", &response.namespace);
                    output::label("  Version", &response.version);
                    output::label("  Status", &response.status);
                    if let Some(priority) = response.priority {
                        output::label("  Priority", priority);
                    }
                    output::label("  Created", response.created_at);
                    output::label("  Updated", response.updated_at);
                    if let Some(completed) = response.completed_at {
                        output::label("  Completed", completed);
                    }
                    output::label(
                        "  Steps",
                        format!(
                            "{}/{} completed",
                            response.completed_steps, response.total_steps
                        ),
                    );
                    output::label(
                        "  Progress",
                        format!("{:.1}%", response.completion_percentage),
                    );
                    output::label("  Health", &response.health_status);
                    output::label("  Recommended action", &response.recommended_action);
                    output::label("  Correlation ID", response.correlation_id);
                }
                Err(e) => {
                    output::error(format!("Failed to get task: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::List {
            namespace,
            status,
            limit,
        } => {
            output::dim(format!("Listing tasks (limit: {limit})"));

            let query = TaskListQuery {
                page: 1,
                per_page: limit,
                namespace,
                status,
                initiator: None,
                source_system: None,
            };

            match client.list_tasks(&query).await {
                Ok(response) => {
                    output::success(format!(
                        "Found {} tasks (page {} of {})",
                        response.tasks.len(),
                        response.pagination.page,
                        response.pagination.total_pages
                    ));
                    output::dim(format!(
                        "  Total: {} tasks",
                        response.pagination.total_count
                    ));
                    output::blank();

                    for task in response.tasks {
                        output::item(format!(
                            "{} - {}/{} v{}",
                            task.task_uuid, task.namespace, task.name, task.version
                        ));
                        output::plain(format!(
                            "    Status: {} | Progress: {:.1}% | Health: {}",
                            task.status, task.completion_percentage, task.health_status
                        ));
                        output::dim(format!(
                            "    Created: {} | Steps: {}/{}",
                            task.created_at, task.completed_steps, task.total_steps
                        ));
                        if let Some(ref tags) = task.tags {
                            if !tags.is_empty() {
                                output::dim(format!("    Tags: {}", tags.join(", ")));
                            }
                        }
                        output::blank();
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to list tasks: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::Cancel { task_id } => {
            output::plain(format!("Canceling task: {task_id}"));

            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
            })?;

            match client.cancel_task(task_uuid).await {
                Ok(()) => {
                    output::success(format!("Task {task_id} has been canceled successfully"));
                }
                Err(e) => {
                    output::error(format!("Failed to cancel task: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::Steps { task_id } => {
            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
            })?;

            match client.list_task_steps(task_uuid).await {
                Ok(steps) => {
                    output::success(format!("Found {} workflow steps:", steps.len()));
                    output::blank();
                    for step in steps {
                        output::item(format!("{} ({})", step.name, step.step_uuid));
                        output::label("    State", &step.current_state);
                        output::label("    Dependencies satisfied", step.dependencies_satisfied);
                        output::label("    Ready for execution", step.ready_for_execution);
                        output::label(
                            "    Attempts",
                            format!("{}/{}", step.attempts, step.max_attempts),
                        );
                        if step.retry_eligible {
                            output::warning("    Retry eligible");
                        }
                        output::blank();
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to list steps: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::Step { task_id, step_id } => {
            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid task UUID: {}", e))
            })?;

            let step_uuid = Uuid::parse_str(&step_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid step UUID: {}", e))
            })?;

            match client.get_step(task_uuid, step_uuid).await {
                Ok(step) => {
                    output::header("Step Details");
                    output::label("  UUID", &step.step_uuid);
                    output::label("  Name", &step.name);
                    output::label("  State", &step.current_state);
                    output::label("  Dependencies satisfied", step.dependencies_satisfied);
                    output::label("  Ready for execution", step.ready_for_execution);
                    output::label("  Retry eligible", step.retry_eligible);
                    output::label(
                        "  Attempts",
                        format!("{}/{}", step.attempts, step.max_attempts),
                    );
                    if let Some(last_failure) = step.last_failure_at {
                        output::label("  Last failure", last_failure);
                    }
                    if let Some(next_retry) = step.next_retry_at {
                        output::label("  Next retry", next_retry);
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to get step: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::ResetStep {
            task_id,
            step_id,
            reason,
            reset_by,
        } => {
            output::plain(format!("Resetting step {step_id} for retry..."));

            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid task UUID: {}", e))
            })?;

            let step_uuid = Uuid::parse_str(&step_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid step UUID: {}", e))
            })?;

            let action = StepManualAction::ResetForRetry {
                reason: reason.clone(),
                reset_by: reset_by.clone(),
            };

            match client
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => {
                    output::success("Step reset successfully!");
                    output::label("  New state", &step.current_state);
                    output::label("  Reason", &reason);
                    output::label("  Reset by", &reset_by);
                }
                Err(e) => {
                    output::error(format!("Failed to reset step: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::ResolveStep {
            task_id,
            step_id,
            reason,
            resolved_by,
        } => {
            output::plain(format!("Manually resolving step {step_id}..."));

            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid task UUID: {}", e))
            })?;

            let step_uuid = Uuid::parse_str(&step_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid step UUID: {}", e))
            })?;

            let action = StepManualAction::ResolveManually {
                reason: reason.clone(),
                resolved_by: resolved_by.clone(),
            };

            match client
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => {
                    output::success("Step resolved manually!");
                    output::label("  New state", &step.current_state);
                    output::label("  Reason", &reason);
                    output::label("  Resolved by", &resolved_by);
                }
                Err(e) => {
                    output::error(format!("Failed to resolve step: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::CompleteStep {
            task_id,
            step_id,
            result,
            metadata,
            reason,
            completed_by,
        } => {
            output::plain(format!(
                "Manually completing step {step_id} with results..."
            ));

            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid task UUID: {}", e))
            })?;

            let step_uuid = Uuid::parse_str(&step_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid step UUID: {}", e))
            })?;

            // Parse result JSON
            let result_value: serde_json::Value = serde_json::from_str(&result).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid result JSON: {}", e))
            })?;

            // Parse optional metadata JSON
            let metadata_value: Option<serde_json::Value> = if let Some(meta) = metadata {
                Some(serde_json::from_str(&meta).map_err(|e| {
                    tasker_client::ClientError::InvalidInput(format!(
                        "Invalid metadata JSON: {}",
                        e
                    ))
                })?)
            } else {
                None
            };

            let completion_data = ManualCompletionData {
                result: result_value,
                metadata: metadata_value,
            };

            let action = StepManualAction::CompleteManually {
                completion_data,
                reason: reason.clone(),
                completed_by: completed_by.clone(),
            };

            match client
                .resolve_step_manually(task_uuid, step_uuid, action)
                .await
            {
                Ok(step) => {
                    output::success("Step completed manually with results!");
                    output::label("  New state", &step.current_state);
                    output::label("  Reason", &reason);
                    output::label("  Completed by", &completed_by);
                }
                Err(e) => {
                    output::error(format!("Failed to complete step: {e}"));
                    return Err(e.into());
                }
            }
        }
        TaskCommands::StepAudit { task_id, step_id } => {
            let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid task UUID: {}", e))
            })?;

            let step_uuid = Uuid::parse_str(&step_id).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid step UUID: {}", e))
            })?;

            match client.get_step_audit_history(task_uuid, step_uuid).await {
                Ok(audit_records) => {
                    if audit_records.is_empty() {
                        output::warning("No audit records found for this step");
                        output::hint("  Audit records are created when step results are persisted");
                    } else {
                        output::success(format!("Found {} audit record(s):", audit_records.len()));
                        output::blank();
                        for (i, audit) in audit_records.iter().enumerate() {
                            output::header(format!("  Audit Record #{}", i + 1));
                            output::label("    UUID", &audit.audit_uuid);
                            output::label(
                                "    Step",
                                format!("{} ({})", audit.step_name, audit.workflow_step_uuid),
                            );
                            output::status_icon(
                                audit.success,
                                format!("Success: {}", if audit.success { "Yes" } else { "No" }),
                            );
                            output::label("    Recorded at", &audit.recorded_at);
                            output::label(
                                "    Transition",
                                format!(
                                    "{} -> {}",
                                    audit.from_state.as_deref().unwrap_or("(none)"),
                                    audit.to_state
                                ),
                            );
                            if let Some(ref worker) = audit.worker_uuid {
                                output::label("    Worker UUID", worker);
                            }
                            if let Some(ref correlation) = audit.correlation_id {
                                output::label("    Correlation ID", correlation);
                            }
                            if let Some(time_ms) = audit.execution_time_ms {
                                output::label("    Execution time", format!("{time_ms}ms"));
                            }
                            output::blank();
                        }
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to get audit history: {e}"));
                    return Err(e.into());
                }
            }
        }
    }
    Ok(())
}
