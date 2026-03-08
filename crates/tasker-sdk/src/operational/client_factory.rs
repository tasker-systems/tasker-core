//! Client construction helpers for building transport-agnostic Tasker clients.

pub use tasker_client::config::ClientConfig;
pub use tasker_client::{
    ClientError, ClientResult, UnifiedOrchestrationClient, UnifiedWorkerClient,
};

/// Build an orchestration client from a resolved profile config.
pub async fn build_orchestration_client(
    config: &ClientConfig,
) -> ClientResult<UnifiedOrchestrationClient> {
    UnifiedOrchestrationClient::from_config(config).await
}

/// Build a worker client from a resolved profile config.
pub async fn build_worker_client(config: &ClientConfig) -> ClientResult<UnifiedWorkerClient> {
    UnifiedWorkerClient::from_config(config).await
}
