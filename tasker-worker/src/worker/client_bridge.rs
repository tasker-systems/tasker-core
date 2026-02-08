//! # FFI Client Bridge
//!
//! TAS-231: Shared abstraction that all FFI workers use, providing sync wrappers
//! over the async `OrchestrationClient` trait from `tasker-client`.
//!
//! This bridge is initialized during bootstrap and provides a synchronous API
//! suitable for FFI consumption. It uses `runtime_handle.block_on()` internally
//! to bridge async client methods to sync FFI boundaries.

use std::sync::Arc;

use serde_json::Value as JsonValue;
use tasker_client::{ClientConfig, ClientError, OrchestrationClient, UnifiedOrchestrationClient};
use tasker_shared::config::tasker::OrchestrationClientConfig;
use tasker_shared::models::core::task_request::TaskRequest;
use tokio::runtime::Handle;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Result type for FFI client bridge operations.
///
/// Wraps a JSON value on success, or a structured error on failure.
pub type FfiClientResult = Result<JsonValue, FfiClientError>;

/// Error from FFI client bridge operations.
///
/// Preserves the `is_recoverable` flag from the underlying `ClientError`
/// so FFI consumers can implement retry logic.
#[derive(Debug)]
pub struct FfiClientError {
    /// Human-readable error message
    pub message: String,
    /// Whether this error is worth retrying
    pub is_recoverable: bool,
}

impl std::fmt::Display for FfiClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FfiClientError {}

impl From<ClientError> for FfiClientError {
    fn from(err: ClientError) -> Self {
        let is_recoverable = err.is_recoverable();
        Self {
            message: err.to_string(),
            is_recoverable,
        }
    }
}

/// Synchronous bridge over the async `OrchestrationClient` for FFI usage.
///
/// All methods block on the provided Tokio runtime handle and return
/// `serde_json::Value` for easy conversion to language-specific types
/// at the FFI boundary.
#[derive(Debug)]
pub struct FfiClientBridge {
    client: Arc<UnifiedOrchestrationClient>,
    runtime_handle: Handle,
}

impl FfiClientBridge {
    /// Create a new bridge from an already-constructed client.
    pub fn new(client: Arc<UnifiedOrchestrationClient>, runtime_handle: Handle) -> Self {
        Self {
            client,
            runtime_handle,
        }
    }

    /// Create a bridge from worker TOML configuration.
    ///
    /// Extracts `orchestration_client` config from the worker TOML and builds
    /// a `UnifiedOrchestrationClient`. Returns `None` if the client cannot be
    /// created (logs a warning instead of failing bootstrap).
    ///
    /// This is async because client construction may perform I/O (e.g., gRPC
    /// channel setup). Callers in FFI bootstrap already run inside
    /// `runtime.block_on(async { ... })`, so this avoids nested `block_on`.
    pub async fn from_worker_config(
        orch_config: &OrchestrationClientConfig,
        runtime_handle: Handle,
    ) -> Option<Self> {
        // Build ClientConfig from the worker's orchestration_client section
        let client_config: ClientConfig = orch_config.into();

        info!(
            base_url = %orch_config.base_url,
            "Creating FFI client bridge from worker config"
        );

        match UnifiedOrchestrationClient::from_config(&client_config).await {
            Ok(client) => {
                info!(
                    transport = %client.transport_name(),
                    endpoint = %client.endpoint(),
                    "FFI client bridge created successfully"
                );
                Some(Self {
                    client: Arc::new(client),
                    runtime_handle,
                })
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Failed to create FFI client bridge — client operations will be unavailable"
                );
                None
            }
        }
    }

    /// Get the transport name (e.g., "REST" or "gRPC").
    pub fn transport_name(&self) -> &'static str {
        self.client.transport_name()
    }

    /// Get the endpoint URL.
    pub fn endpoint(&self) -> &str {
        self.client.endpoint()
    }

    // =========================================================================
    // Task Operations
    // =========================================================================

    /// Create a new task.
    pub fn create_task(&self, request: TaskRequest) -> FfiClientResult {
        debug!(name = %request.name, "FFI client: create_task");
        let response = self
            .runtime_handle
            .block_on(self.client.create_task(request))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    /// Get a task by UUID.
    pub fn get_task(&self, task_uuid: Uuid) -> FfiClientResult {
        debug!(%task_uuid, "FFI client: get_task");
        let response = self
            .runtime_handle
            .block_on(self.client.get_task(task_uuid))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    /// List tasks with pagination and optional filters.
    pub fn list_tasks(
        &self,
        limit: i32,
        offset: i32,
        namespace: Option<&str>,
        status: Option<&str>,
    ) -> FfiClientResult {
        debug!(limit, offset, ?namespace, ?status, "FFI client: list_tasks");
        let response = self
            .runtime_handle
            .block_on(self.client.list_tasks(limit, offset, namespace, status))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    /// Cancel a task.
    pub fn cancel_task(&self, task_uuid: Uuid) -> FfiClientResult {
        debug!(%task_uuid, "FFI client: cancel_task");
        self.runtime_handle
            .block_on(self.client.cancel_task(task_uuid))
            .map_err(FfiClientError::from)?;
        Ok(serde_json::json!({ "cancelled": true }))
    }

    // =========================================================================
    // Step Operations
    // =========================================================================

    /// List workflow steps for a task.
    pub fn list_task_steps(&self, task_uuid: Uuid) -> FfiClientResult {
        debug!(%task_uuid, "FFI client: list_task_steps");
        let response = self
            .runtime_handle
            .block_on(self.client.list_task_steps(task_uuid))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    /// Get a specific workflow step.
    pub fn get_step(&self, task_uuid: Uuid, step_uuid: Uuid) -> FfiClientResult {
        debug!(%task_uuid, %step_uuid, "FFI client: get_step");
        let response = self
            .runtime_handle
            .block_on(self.client.get_step(task_uuid, step_uuid))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    /// Get audit history for a workflow step.
    pub fn get_step_audit_history(&self, task_uuid: Uuid, step_uuid: Uuid) -> FfiClientResult {
        debug!(%task_uuid, %step_uuid, "FFI client: get_step_audit_history");
        let response = self
            .runtime_handle
            .block_on(self.client.get_step_audit_history(task_uuid, step_uuid))
            .map_err(FfiClientError::from)?;
        serde_json::to_value(response).map_err(|e| FfiClientError {
            message: format!("Serialization error: {e}"),
            is_recoverable: false,
        })
    }

    // =========================================================================
    // Health Operations
    // =========================================================================

    /// Check if the orchestration API is healthy.
    pub fn health_check(&self) -> FfiClientResult {
        debug!("FFI client: health_check");
        self.runtime_handle
            .block_on(self.client.health_check())
            .map_err(FfiClientError::from)?;
        Ok(serde_json::json!({ "healthy": true }))
    }
}

/// Helper to create an `FfiClientBridge` during FFI bootstrap.
///
/// Extracts the `orchestration_client` config from the worker's `TaskerConfig`
/// and attempts to build a client. Returns `None` on failure (logs a warning).
pub async fn create_ffi_client_bridge(
    worker_core: &crate::worker::WorkerCore,
    runtime_handle: Handle,
) -> Option<Arc<FfiClientBridge>> {
    let tasker_config = worker_core.context.tasker_config.as_ref();
    let orch_config = tasker_config
        .worker
        .as_ref()
        .and_then(|w| w.orchestration_client.as_ref());

    match orch_config {
        Some(config) => FfiClientBridge::from_worker_config(config, runtime_handle)
            .await
            .map(Arc::new),
        None => {
            warn!("No orchestration_client config found in worker config — client operations unavailable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_client_error_from_client_error() {
        let err = ClientError::api_error(503, "service unavailable");
        let ffi_err = FfiClientError::from(err);
        assert!(ffi_err.is_recoverable);
        assert!(ffi_err.message.contains("503"));
    }

    #[test]
    fn test_ffi_client_error_not_recoverable() {
        let err = ClientError::api_error(400, "bad request");
        let ffi_err = FfiClientError::from(err);
        assert!(!ffi_err.is_recoverable);
    }

    #[test]
    fn test_ffi_client_error_display() {
        let err = FfiClientError {
            message: "something failed".to_string(),
            is_recoverable: false,
        };
        assert_eq!(format!("{err}"), "something failed");
    }

    #[tokio::test]
    async fn test_bridge_from_config_creates_rest_client() {
        let config = OrchestrationClientConfig {
            base_url: "http://localhost:8080".to_string(),
            transport: tasker_shared::config::tasker::ClientTransport::Rest,
            timeout_ms: 5000,
            max_retries: 1,
            auth: None,
        };

        let handle = Handle::current();
        let bridge = FfiClientBridge::from_worker_config(&config, handle).await;

        // Should succeed since REST client creation doesn't require a running server
        assert!(bridge.is_some());
        let bridge = bridge.unwrap();
        assert_eq!(bridge.transport_name(), "REST");
    }
}
