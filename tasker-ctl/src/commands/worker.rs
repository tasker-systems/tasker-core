//! Worker command handlers for the Tasker CLI

use tasker_client::{ClientConfig, ClientResult, WorkerApiClient, WorkerApiConfig};

use crate::output;
use crate::WorkerCommands;

pub(crate) async fn handle_worker_command(
    cmd: WorkerCommands,
    config: &ClientConfig,
) -> ClientResult<()> {
    let worker_config = WorkerApiConfig {
        base_url: config.worker.base_url.clone(),
        timeout_ms: config.worker.timeout_ms,
        max_retries: config.worker.max_retries,
        auth: config.worker.resolve_web_auth_config(),
    };

    let client = WorkerApiClient::new(worker_config)?;

    match cmd {
        WorkerCommands::List { namespace } => {
            // List templates instead of workers (workers don't have a registry)
            output::dim("Listing worker templates and capabilities");
            if let Some(ref ns) = namespace {
                output::dim(format!("Namespace filter: {}", ns));
            }

            match client.list_templates(None).await {
                Ok(response) => {
                    output::success("Worker service information:");
                    output::label(
                        "  Supported namespaces",
                        response.supported_namespaces.join(", "),
                    );
                    output::label("  Cached templates", response.template_count);
                    output::label(
                        "  Worker capabilities",
                        response.worker_capabilities.join(", "),
                    );

                    if let Some(cache_stats) = response.cache_stats {
                        output::blank();
                        output::header("  Cache statistics:");
                        output::label("    Total cached", cache_stats.total_cached);
                        output::label("    Cache hits", cache_stats.cache_hits);
                        output::label("    Cache misses", cache_stats.cache_misses);
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to get worker info: {}", e));
                    return Err(e.into());
                }
            }
        }
        WorkerCommands::Status { worker_id: _ } => {
            // Get worker detailed health status (worker_id is ignored - single worker per service)
            output::dim("Getting worker status...");

            match client.get_detailed_health().await {
                Ok(response) => {
                    output::success("Worker status:");
                    output::label("  Worker ID", &response.worker_id);
                    output::label("  Status", &response.status);
                    output::label(
                        "  Version",
                        format!(
                            "{} ({})",
                            response.system_info.version, response.system_info.environment
                        ),
                    );
                    output::label(
                        "  Uptime",
                        format!("{} seconds", response.system_info.uptime_seconds),
                    );
                    output::label("  Worker type", &response.system_info.worker_type);
                    output::label("  DB pool size", response.system_info.database_pool_size);
                    output::label(
                        "  Command processor",
                        if response.system_info.command_processor_active {
                            "active"
                        } else {
                            "inactive"
                        },
                    );
                    output::label(
                        "  Namespaces",
                        response.system_info.supported_namespaces.join(", "),
                    );

                    // TAS-76: Typed worker health checks
                    output::blank();
                    output::header("  Health checks:");
                    let checks = [
                        ("database", &response.checks.database),
                        ("command_processor", &response.checks.command_processor),
                        ("queue_processing", &response.checks.queue_processing),
                        ("event_system", &response.checks.event_system),
                        ("step_processing", &response.checks.step_processing),
                        ("circuit_breakers", &response.checks.circuit_breakers),
                    ];
                    for (check_name, check_result) in checks {
                        let is_healthy = check_result.status == "healthy";
                        output::status_icon(
                            is_healthy,
                            format!(
                                "    {}: {} ({}ms)",
                                check_name, check_result.status, check_result.duration_ms
                            ),
                        );
                        if let Some(ref msg) = &check_result.message {
                            output::dim(format!("      {}", msg));
                        }
                    }
                }
                Err(e) => {
                    output::error(format!("Failed to get worker status: {}", e));
                    return Err(e.into());
                }
            }
        }
        WorkerCommands::Health {
            all: _,
            worker_id: _,
        } => {
            // Check worker health (--all and worker_id ignored - single worker per service)
            output::dim("Checking worker health...");

            // Basic health check first
            match client.health_check().await {
                Ok(basic) => {
                    output::success(format!("Worker basic health: {}", basic.status));
                    output::label("  Worker ID", &basic.worker_id);
                }
                Err(e) => {
                    output::error(format!("Worker health check failed: {}", e));
                    return Err(e.into());
                }
            }

            // Detailed health check
            match client.get_detailed_health().await {
                Ok(response) => {
                    output::blank();
                    output::success("Detailed worker health:");
                    output::label(
                        "  Status",
                        format!("{} | Timestamp: {}", response.status, response.timestamp),
                    );

                    output::blank();
                    output::header("  System info:");
                    output::label(
                        "    Version",
                        format!(
                            "{} | Environment: {}",
                            response.system_info.version, response.system_info.environment
                        ),
                    );
                    output::label(
                        "    Uptime",
                        format!(
                            "{} seconds | Worker type: {}",
                            response.system_info.uptime_seconds, response.system_info.worker_type
                        ),
                    );
                    output::label(
                        "    DB pool size",
                        format!(
                            "{} | Command processor: {}",
                            response.system_info.database_pool_size,
                            response.system_info.command_processor_active
                        ),
                    );
                    output::label(
                        "    Supported namespaces",
                        response.system_info.supported_namespaces.join(", "),
                    );

                    // TAS-76: Typed worker health checks
                    output::blank();
                    output::header("  Health checks:");
                    let checks = [
                        ("database", &response.checks.database),
                        ("command_processor", &response.checks.command_processor),
                        ("queue_processing", &response.checks.queue_processing),
                        ("event_system", &response.checks.event_system),
                        ("step_processing", &response.checks.step_processing),
                        ("circuit_breakers", &response.checks.circuit_breakers),
                    ];
                    for (check_name, check_result) in checks {
                        let is_healthy = check_result.status == "healthy";
                        output::status_icon(
                            is_healthy,
                            format!(
                                "    {}: {} ({}ms) - last checked: {}",
                                check_name,
                                check_result.status,
                                check_result.duration_ms,
                                check_result.last_checked
                            ),
                        );
                        if let Some(ref message) = &check_result.message {
                            output::dim(format!("      {}", message));
                        }
                    }
                }
                Err(e) => {
                    output::dim(format!("  Could not get detailed health info: {}", e));
                }
            }
        }
    }
    Ok(())
}
