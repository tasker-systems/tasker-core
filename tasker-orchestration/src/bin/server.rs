//! # Tasker Orchestration Server
//!
//! Thin wrapper binary for running the orchestration system as a standalone server.
//! This is the production deployment target for the Tasker orchestration service.
//!
//! ## Usage
//!
//! ```bash
//! # Run with default configuration (REST API only)
//! cargo run --bin tasker-server --features web-api
//!
//! # Run with both REST and gRPC APIs
//! cargo run --bin tasker-server --features web-api,grpc-api
//!
//! # Run with specific environment
//! TASKER_ENV=production cargo run --bin tasker-server --features web-api,grpc-api
//! ```

use std::env;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info};

use tasker_orchestration::orchestration::bootstrap::OrchestrationBootstrap;
use tasker_shared::logging;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging first
    logging::init_tracing();

    info!("Starting Tasker Orchestration Server...");
    info!("   Version: {}", env!("CARGO_PKG_VERSION"));
    info!(
        "   Build Mode: {}",
        if cfg!(debug_assertions) {
            "Debug"
        } else {
            "Release"
        }
    );

    let mut orchestration_handle = OrchestrationBootstrap::bootstrap()
        .await
        .map_err(|e| format!("Failed to bootstrap orchestration: {e}"))?;

    info!("Orchestration Server started successfully!");

    if orchestration_handle.web_state.is_some() {
        info!("   REST API: Running");
    }

    #[cfg(feature = "grpc-api")]
    if orchestration_handle.grpc_server_handle.is_some() {
        info!("   gRPC API: Running");
    }

    info!(
        "   Environment: {}",
        orchestration_handle
            .tasker_config
            .common
            .execution
            .environment
    );
    info!("   Press Ctrl+C to shutdown gracefully");

    // Wait for shutdown signal
    shutdown_signal().await;

    info!("Shutdown signal received, initiating graceful shutdown...");

    // TAS-228: Read shutdown timeout from config (O-2 remediation)
    let shutdown_timeout_ms = orchestration_handle
        .tasker_config
        .orchestration
        .as_ref()
        .map(|o| o.shutdown_timeout_ms)
        .unwrap_or(30000);

    // Stop orchestration system with timeout to prevent hanging indefinitely
    info!(
        timeout_ms = shutdown_timeout_ms,
        "Stopping orchestration system..."
    );
    match tokio::time::timeout(
        Duration::from_millis(shutdown_timeout_ms),
        orchestration_handle.stop(),
    )
    .await
    {
        Ok(Ok(())) => {
            info!("Orchestration system stopped");
        }
        Ok(Err(e)) => {
            error!("Failed to stop orchestration cleanly: {}", e);
        }
        Err(_) => {
            error!(
                timeout_ms = shutdown_timeout_ms,
                "Graceful shutdown timed out, forcing exit"
            );
        }
    }

    info!("Orchestration Server shutdown complete");

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
    }
}
