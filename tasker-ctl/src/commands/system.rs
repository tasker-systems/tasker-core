//! System command handlers for the Tasker CLI

use tasker_client::{
    ClientConfig, ClientResult, OrchestrationApiClient, OrchestrationApiConfig, WorkerApiClient,
    WorkerApiConfig,
};

use crate::output;
use crate::SystemCommands;

pub(crate) async fn handle_system_command(
    cmd: SystemCommands,
    config: &ClientConfig,
) -> ClientResult<()> {
    match cmd {
        SystemCommands::Health {
            orchestration,
            workers,
        } => {
            if orchestration || !workers {
                output::dim("Checking orchestration health...");

                let orchestration_config = OrchestrationApiConfig {
                    base_url: config.orchestration.base_url.clone(),
                    timeout_ms: config.orchestration.timeout_ms,
                    max_retries: config.orchestration.max_retries,
                    auth: config.orchestration.resolve_web_auth_config(),
                };

                let orch_client = OrchestrationApiClient::new(orchestration_config)?;

                // Check basic health
                match orch_client.get_basic_health().await {
                    Ok(health) => {
                        output::status_icon(
                            true,
                            format!("Orchestration service is healthy: {}", health.status),
                        );
                    }
                    Err(e) => {
                        output::status_icon(
                            false,
                            format!("Orchestration service health check failed: {}", e),
                        );
                    }
                }

                // Get detailed health if available
                match orch_client.get_detailed_health().await {
                    Ok(detailed) => {
                        output::status_icon(true, "Detailed orchestration health:");
                        output::plain(format!(
                            "    Status: {} | Environment: {} | Version: {}",
                            detailed.status, detailed.info.environment, detailed.info.version
                        ));
                        output::plain(format!(
                            "    Operational state: {} | Circuit breaker: {}",
                            detailed.info.operational_state, detailed.info.circuit_breaker_state
                        ));
                        output::plain(format!(
                            "    DB pools - Web: {}, Orchestration: {}",
                            detailed.info.web_database_pool_size,
                            detailed.info.orchestration_database_pool_size
                        ));

                        // Print all health checks from typed struct
                        output::plain("    Health checks:");
                        let checks = [
                            ("web_database", &detailed.checks.web_database),
                            (
                                "orchestration_database",
                                &detailed.checks.orchestration_database,
                            ),
                            ("circuit_breaker", &detailed.checks.circuit_breaker),
                            (
                                "orchestration_system",
                                &detailed.checks.orchestration_system,
                            ),
                            ("command_processor", &detailed.checks.command_processor),
                            ("pool_utilization", &detailed.checks.pool_utilization),
                            ("queue_depth", &detailed.checks.queue_depth),
                            ("channel_saturation", &detailed.checks.channel_saturation),
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
                            if let Some(message) = &check_result.message {
                                output::dim(format!("        {}", message));
                            }
                        }
                    }
                    Err(e) => {
                        output::dim(format!("  Could not get detailed health info: {}", e));
                    }
                }
            }

            if workers || !orchestration {
                output::blank();
                output::dim("Checking worker health...");

                let worker_config = WorkerApiConfig {
                    base_url: config.worker.base_url.clone(),
                    timeout_ms: config.worker.timeout_ms,
                    max_retries: config.worker.max_retries,
                    auth: config.worker.resolve_web_auth_config(),
                };

                let worker_client = WorkerApiClient::new(worker_config)?;

                // Check basic worker service health
                match worker_client.health_check().await {
                    Ok(health) => {
                        output::status_icon(
                            true,
                            format!("Worker service is healthy: {}", health.status),
                        );
                        output::label("    Worker ID", &health.worker_id);
                    }
                    Err(e) => {
                        output::status_icon(
                            false,
                            format!("Worker service health check failed: {}", e),
                        );
                    }
                }

                // Get detailed worker health
                match worker_client.get_detailed_health().await {
                    Ok(health) => {
                        output::status_icon(true, "Worker detailed health:");
                        output::plain(format!(
                            "    Status: {} | Version: {} | Uptime: {}s",
                            health.status,
                            health.system_info.version,
                            health.system_info.uptime_seconds
                        ));
                        output::plain(format!(
                            "    Worker type: {} | Environment: {}",
                            health.system_info.worker_type, health.system_info.environment
                        ));
                        output::plain(format!(
                            "    Namespaces: {}",
                            health.system_info.supported_namespaces.join(", ")
                        ));

                        // TAS-76: Typed worker health checks
                        output::plain("    Health checks:");
                        let checks = [
                            ("database", &health.checks.database),
                            ("command_processor", &health.checks.command_processor),
                            ("queue_processing", &health.checks.queue_processing),
                            ("event_system", &health.checks.event_system),
                            ("step_processing", &health.checks.step_processing),
                            ("circuit_breakers", &health.checks.circuit_breakers),
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
                        }
                    }
                    Err(e) => {
                        output::status_icon(
                            false,
                            format!("Could not get detailed worker health: {}", e),
                        );
                    }
                }
            }

            if !orchestration && !workers {
                output::blank();
                output::plain(
                    "Overall system health: Both orchestration and worker services checked above",
                );
            }
        }
        SystemCommands::Info => {
            output::header("Tasker System Information");
            output::plain("================================");
            output::label("CLI Version", env!("CARGO_PKG_VERSION"));
            output::label("Build Target", std::env::consts::ARCH);
            output::blank();

            output::header("Configuration:");
            output::label("  Orchestration API", &config.orchestration.base_url);
            output::label("  Worker API", &config.worker.base_url);
            output::label(
                "  Request timeout",
                format!("{}ms", config.orchestration.timeout_ms),
            );
            output::label("  Max retries", config.orchestration.max_retries);
            output::blank();

            // Try to get version info from services
            output::header("Service Information:");

            // Orchestration service info
            let orchestration_config = OrchestrationApiConfig {
                base_url: config.orchestration.base_url.clone(),
                timeout_ms: config.orchestration.timeout_ms,
                max_retries: config.orchestration.max_retries,
                auth: config.orchestration.resolve_web_auth_config(),
            };

            if let Ok(orch_client) = OrchestrationApiClient::new(orchestration_config) {
                match orch_client.get_detailed_health().await {
                    Ok(health) => {
                        output::label(
                            "  Orchestration",
                            format!(
                                "{} v{} ({})",
                                health.status, health.info.version, health.info.environment
                            ),
                        );
                        output::label("    Operational state", &health.info.operational_state);
                        output::label(
                            "    Database pools",
                            format!(
                                "Web={}, Orch={}",
                                health.info.web_database_pool_size,
                                health.info.orchestration_database_pool_size
                            ),
                        );
                    }
                    Err(_) => {
                        output::dim("  Orchestration: Unable to retrieve service info");
                    }
                }
            } else {
                output::dim("  Orchestration: Configuration error");
            }

            // Worker service info
            let worker_config = WorkerApiConfig {
                base_url: config.worker.base_url.clone(),
                timeout_ms: config.worker.timeout_ms,
                max_retries: config.worker.max_retries,
                auth: config.worker.resolve_web_auth_config(),
            };

            if let Ok(worker_client) = WorkerApiClient::new(worker_config) {
                match worker_client.get_detailed_health().await {
                    Ok(health) => {
                        output::label(
                            "  Worker",
                            format!(
                                "{} v{} ({})",
                                health.status,
                                health.system_info.version,
                                health.system_info.environment
                            ),
                        );
                        output::label(
                            "    Worker type",
                            format!(
                                "{} | Uptime: {}s",
                                health.system_info.worker_type, health.system_info.uptime_seconds
                            ),
                        );
                        output::label(
                            "    Supported namespaces",
                            health.system_info.supported_namespaces.join(", "),
                        );
                    }
                    Err(_) => {
                        output::dim("  Worker: Unable to retrieve worker info");
                    }
                }
            } else {
                output::dim("  Worker: Configuration error");
            }
        }
    }
    Ok(())
}
