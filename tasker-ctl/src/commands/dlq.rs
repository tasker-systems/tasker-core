//! DLQ (Dead Letter Queue) command handlers for the Tasker CLI

use tasker_client::{ClientConfig, ClientResult, OrchestrationApiClient, OrchestrationApiConfig};
use uuid::Uuid;

use crate::output;
use crate::DlqCommands;

pub(crate) async fn handle_dlq_command(
    cmd: DlqCommands,
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
        DlqCommands::List {
            status,
            limit,
            offset,
        } => {
            output::dim(format!(
                "Listing DLQ entries (limit: {}, offset: {})",
                limit, offset
            ));

            // Parse status if provided
            let resolution_status = if let Some(status_str) = status {
                Some(match status_str.as_str() {
                    "pending" => tasker_shared::models::orchestration::DlqResolutionStatus::Pending,
                    "manually_resolved" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::ManuallyResolved
                    }
                    "permanently_failed" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::PermanentlyFailed
                    }
                    "cancelled" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::Cancelled
                    }
                    _ => {
                        output::error(format!(
                            "Invalid status '{}'. Valid: pending, manually_resolved, permanently_failed, cancelled",
                            status_str
                        ));
                        return Err(tasker_client::ClientError::InvalidInput(format!(
                            "Invalid status: {}",
                            status_str
                        )));
                    }
                })
            } else {
                None
            };

            let params = tasker_shared::models::orchestration::DlqListParams {
                resolution_status,
                limit,
                offset,
            };

            match client.list_dlq_entries(Some(&params)).await {
                Ok(entries) => {
                    output::success(format!("Found {} DLQ entries", entries.len()));
                    output::blank();

                    for entry in entries {
                        output::item(format!("DLQ Entry: {}", entry.dlq_entry_uuid));
                        output::label("    Task UUID", entry.task_uuid);
                        output::label("    Reason", format!("{:?}", entry.dlq_reason));
                        output::label("    Original state", &entry.original_state);
                        output::label(
                            "    Resolution status",
                            format!("{:?}", entry.resolution_status),
                        );
                        output::label("    DLQ timestamp", entry.dlq_timestamp);
                        if let Some(ref notes) = entry.resolution_notes {
                            output::label("    Notes", notes);
                        }
                        if let Some(ref resolved_by) = entry.resolved_by {
                            output::label("    Resolved by", resolved_by);
                        }
                        output::blank();
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to list DLQ entries: {}", e));
                    return Err(e.into());
                }
            }
        }
        DlqCommands::Get { task_uuid } => {
            output::dim(format!("Getting DLQ entry for task: {}", task_uuid));

            let task_uuid = Uuid::parse_str(&task_uuid).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
            })?;

            match client.get_dlq_entry(task_uuid).await {
                Ok(entry) => {
                    output::header("DLQ Entry Details:");
                    output::blank();
                    output::label("  DLQ Entry UUID", entry.dlq_entry_uuid);
                    output::label("  Task UUID", entry.task_uuid);
                    output::label("  Reason", format!("{:?}", entry.dlq_reason));
                    output::label("  Original state", &entry.original_state);
                    output::label(
                        "  Resolution status",
                        format!("{:?}", entry.resolution_status),
                    );
                    output::label("  DLQ timestamp", entry.dlq_timestamp);

                    if let Some(resolution_ts) = entry.resolution_timestamp {
                        output::label("  Resolution timestamp", resolution_ts);
                    }
                    if let Some(ref notes) = entry.resolution_notes {
                        output::label("  Resolution notes", notes);
                    }
                    if let Some(ref resolved_by) = entry.resolved_by {
                        output::label("  Resolved by", resolved_by);
                    }

                    output::blank();
                    output::header("  Task Snapshot:");
                    output::plain(serde_json::to_string_pretty(&entry.task_snapshot).unwrap());
                }
                Err(e) => {
                    output::error(format!("Failed to get DLQ entry: {}", e));
                    return Err(e.into());
                }
            }
        }
        DlqCommands::Update {
            dlq_entry_uuid,
            status,
            notes,
            resolved_by,
        } => {
            output::dim(format!("Updating DLQ investigation: {}", dlq_entry_uuid));

            let dlq_entry_uuid = Uuid::parse_str(&dlq_entry_uuid).map_err(|e| {
                tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
            })?;

            // Parse status if provided
            let resolution_status = if let Some(status_str) = status {
                Some(match status_str.as_str() {
                    "pending" => tasker_shared::models::orchestration::DlqResolutionStatus::Pending,
                    "manually_resolved" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::ManuallyResolved
                    }
                    "permanently_failed" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::PermanentlyFailed
                    }
                    "cancelled" => {
                        tasker_shared::models::orchestration::DlqResolutionStatus::Cancelled
                    }
                    _ => {
                        output::error(format!(
                            "Invalid status '{}'. Valid: pending, manually_resolved, permanently_failed, cancelled",
                            status_str
                        ));
                        return Err(tasker_client::ClientError::InvalidInput(format!(
                            "Invalid status: {}",
                            status_str
                        )));
                    }
                })
            } else {
                None
            };

            let update = tasker_shared::models::orchestration::DlqInvestigationUpdate {
                resolution_status,
                resolution_notes: notes,
                resolved_by,
                metadata: None,
            };

            match client
                .update_dlq_investigation(dlq_entry_uuid, update)
                .await
            {
                Ok(()) => {
                    output::success("DLQ investigation updated successfully");
                }
                Err(e) => {
                    output::error(format!("Failed to update DLQ investigation: {}", e));
                    return Err(e.into());
                }
            }
        }
        DlqCommands::Stats => {
            output::dim("Getting DLQ statistics...");
            output::blank();

            match client.get_dlq_stats().await {
                Ok(stats) => {
                    output::success("DLQ Statistics by Reason:");
                    output::blank();

                    for stat in stats {
                        output::header(format!("  Reason: {:?}", stat.dlq_reason));
                        output::label("    Total entries", stat.total_entries);
                        output::label("    Pending", stat.pending);
                        output::label("    Manually resolved", stat.manually_resolved);
                        output::label("    Permanent failures", stat.permanent_failures);

                        if let Some(oldest) = stat.oldest_entry {
                            output::label("    Oldest entry", oldest);
                        }
                        if let Some(newest) = stat.newest_entry {
                            output::label("    Newest entry", newest);
                        }
                        output::blank();
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to get DLQ statistics: {}", e));
                    return Err(e.into());
                }
            }
        }
    }
    Ok(())
}
