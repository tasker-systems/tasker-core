//! Shared test helpers for tasker-client integration tests.
//!
//! Provides URL resolution, client creation, and task request helpers
//! for integration tests requiring running services.

#![expect(
    dead_code,
    reason = "Test utilities shared across integration test modules"
)]

use std::env;

use tasker_client::{
    OrchestrationApiClient, OrchestrationApiConfig, WorkerApiClient, WorkerApiConfig,
};

#[cfg(feature = "grpc")]
use tasker_client::grpc_clients::{
    GrpcAuthConfig, GrpcClientConfig, OrchestrationGrpcClient, WorkerGrpcClient,
};

use tasker_shared::models::core::task_request::TaskRequest;

/// Get orchestration REST base URL from env or default.
pub fn orchestration_url() -> String {
    env::var("TASKER_TEST_ORCHESTRATION_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Get worker REST base URL from env or default.
pub fn worker_url() -> String {
    env::var("TASKER_TEST_WORKER_URL").unwrap_or_else(|_| "http://localhost:8081".to_string())
}

/// Get orchestration gRPC URL from env or default.
pub fn orchestration_grpc_url() -> String {
    env::var("TASKER_TEST_ORCHESTRATION_GRPC_URL")
        .unwrap_or_else(|_| "http://localhost:9190".to_string())
}

/// Get worker gRPC URL from env or default.
pub fn worker_grpc_url() -> String {
    env::var("TASKER_TEST_WORKER_GRPC_URL").unwrap_or_else(|_| "http://localhost:9191".to_string())
}

/// Get API key for authenticated requests from env or test default.
pub fn api_key() -> String {
    env::var("TASKER_TEST_API_KEY").unwrap_or_else(|_| "test-api-key-full-access".to_string())
}

/// Create an orchestration REST client with test defaults.
pub fn create_orchestration_client() -> OrchestrationApiClient {
    let config = OrchestrationApiConfig {
        base_url: orchestration_url(),
        timeout_ms: 30000,
        max_retries: 1,
        auth: None,
    };
    OrchestrationApiClient::new(config).expect("Failed to create orchestration client")
}

/// Create a worker REST client with test defaults.
pub fn create_worker_client() -> WorkerApiClient {
    let config = WorkerApiConfig {
        base_url: worker_url(),
        timeout_ms: 30000,
        max_retries: 1,
        auth: None,
    };
    WorkerApiClient::new(config).expect("Failed to create worker client")
}

/// Create an orchestration gRPC client with test defaults.
#[cfg(feature = "grpc")]
pub async fn create_orchestration_grpc_client() -> OrchestrationGrpcClient {
    let config = GrpcClientConfig::new(orchestration_grpc_url())
        .with_auth(GrpcAuthConfig::with_api_key(api_key()));
    OrchestrationGrpcClient::with_config(config)
        .await
        .expect("Failed to create orchestration gRPC client")
}

/// Create a worker gRPC client with test defaults.
#[cfg(feature = "grpc")]
pub async fn create_worker_grpc_client() -> WorkerGrpcClient {
    let config =
        GrpcClientConfig::new(worker_grpc_url()).with_auth(GrpcAuthConfig::with_api_key(api_key()));
    WorkerGrpcClient::with_config(config)
        .await
        .expect("Failed to create worker gRPC client")
}

/// Create a task request using the `mathematical_sequence` template.
///
/// The template name must match a registered template in the worker's fixture
/// directory. Uniqueness per test run comes from `_test_run_id` in the context,
/// not from the task name (which is the template lookup key).
pub fn create_task_request(run_id: &str) -> TaskRequest {
    TaskRequest::builder()
        .name("mathematical_sequence".to_string())
        .namespace("rust_e2e_linear".to_string())
        .version("1.0.0".to_string())
        .context(serde_json::json!({
            "_test_run_id": run_id,
            "even_number": 8
        }))
        .build()
}
