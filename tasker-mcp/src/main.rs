//! Tasker MCP Server binary.
//!
//! Model Context Protocol server exposing Tasker developer tooling
//! (template validation, code generation, schema inspection) and profile
//! management to LLM agents, developer IDEs, and operational tooling.

use clap::Parser;
use rmcp::ServiceExt;
use tasker_client::ProfileManager;
use tasker_mcp::server::TaskerMcpServer;
use tracing_subscriber::EnvFilter;

/// Tasker MCP Server — expose Tasker capabilities to LLM agents via MCP.
#[derive(Parser, Debug)]
#[command(name = "tasker-mcp", version, about)]
struct Cli {
    /// Named profile to use as the initial active profile.
    #[arg(long)]
    profile: Option<String>,

    /// Run in offline mode (Tier 1 developer tools only, no server connectivity).
    #[arg(long)]
    offline: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("tasker_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let server = if cli.offline {
        tracing::info!("tasker-mcp starting in offline mode (stdio transport)");
        TaskerMcpServer::offline()
    } else {
        let mut pm = match ProfileManager::load() {
            Ok(pm) => {
                let names = pm.list_profile_names();
                tracing::info!(
                    profiles = names.len(),
                    "Loaded {} profile(s): {}",
                    names.len(),
                    names.join(", ")
                );
                pm
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to load profiles, falling back to offline mode"
                );
                tracing::info!("tasker-mcp starting in offline mode (stdio transport)");
                return start_server(TaskerMcpServer::offline()).await;
            }
        };

        // Switch to requested profile if specified
        if let Some(ref profile_name) = cli.profile {
            pm.set_initial_profile(profile_name)?;
            tracing::info!(profile = %profile_name, "Set active profile");
        }

        // Probe health for all profiles at startup
        let health_results = pm.probe_all_health().await;
        for (name, snapshot) in &health_results {
            tracing::info!(
                profile = %name,
                status = %snapshot.status,
                orchestration = ?snapshot.orchestration_healthy,
                worker = ?snapshot.worker_healthy,
                "Profile health"
            );
        }

        tracing::info!(
            active_profile = %pm.active_profile_name(),
            "tasker-mcp starting with profile management (stdio transport)"
        );
        TaskerMcpServer::with_profile_manager(pm, false)
    };

    start_server(server).await
}

async fn start_server(server: TaskerMcpServer) -> anyhow::Result<()> {
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
