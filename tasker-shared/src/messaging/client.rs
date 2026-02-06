//! # MessageClient Domain Facade (TAS-133c, TAS-174)
//!
//! Domain-level messaging client that provides convenient, Tasker-specific
//! messaging methods. Wraps `MessagingProvider` (enum) and `MessageRouterKind`
//! (enum) - no trait objects, all enum dispatch.
//!
//! ## TAS-174: Circuit Breaker Integration
//!
//! The client optionally wraps send/receive operations with circuit breaker
//! protection. When the breaker is open, protected operations fail fast with
//! `MessagingError::CircuitBreakerOpen`. Unprotected operations (ack, nack,
//! health_check, queue management) bypass the breaker.
//!
//! ## Design
//!
//! This is a **struct**, not a trait. The struct pattern is simpler and the
//! trait was only used polymorphically in one place (which has been refactored).
//!
//! ```text
//! MessageClient
//!   ├── provider: Arc<MessagingProvider>           <- Actual messaging backend
//!   ├── router: MessageRouterKind                  <- Queue name resolution
//!   └── circuit_breaker: Option<Arc<CircuitBreaker>> <- TAS-174: fault isolation
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::message::StepMessage;
use super::orchestration_messages::{StepResultMessage, TaskRequestMessage};
use super::service::{
    MessageId, MessageRouterKind, MessagingError, MessagingProvider, QueueMessage, QueueStats,
    QueuedMessage, ReceiptHandle,
};
use crate::resilience::CircuitBreaker;
use crate::TaskerResult;

/// Domain-level messaging client for Tasker
///
/// Provides convenient, Tasker-specific methods for messaging operations.
/// Wraps a `MessagingProvider` (enum) and `MessageRouterKind` (enum) for
/// zero-cost dispatch without trait objects.
///
/// ## Thread Safety
///
/// The client is `Send + Sync` and can be safely shared across threads.
/// The inner `MessagingProvider` is wrapped in `Arc` for efficient cloning.
///
/// ## TAS-174: Circuit Breaker
///
/// When constructed with `with_circuit_breaker`, send and receive operations
/// are protected. The breaker trips after repeated failures, causing subsequent
/// calls to return `MessagingError::CircuitBreakerOpen` immediately.
#[derive(Debug, Clone)]
pub struct MessageClient {
    /// The underlying messaging provider
    provider: Arc<MessagingProvider>,
    /// Queue name router
    router: MessageRouterKind,
    /// Optional circuit breaker for messaging operations (TAS-174)
    circuit_breaker: Option<Arc<CircuitBreaker>>,
}

impl MessageClient {
    /// Create a new MessageClient without circuit breaker protection
    pub fn new(provider: Arc<MessagingProvider>, router: MessageRouterKind) -> Self {
        Self {
            provider,
            router,
            circuit_breaker: None,
        }
    }

    /// Create a new MessageClient with circuit breaker protection (TAS-174)
    ///
    /// Protected operations (send/receive) will be gated by the circuit breaker.
    /// Unprotected operations (ack, nack, health_check, queue management) bypass it.
    pub fn with_circuit_breaker(
        provider: Arc<MessagingProvider>,
        router: MessageRouterKind,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Self {
        Self {
            provider,
            router,
            circuit_breaker: Some(circuit_breaker),
        }
    }

    /// Get the underlying messaging provider
    pub fn provider(&self) -> &Arc<MessagingProvider> {
        &self.provider
    }

    /// Get the router for queue name lookups
    pub fn router(&self) -> &MessageRouterKind {
        &self.router
    }

    /// Get the provider name for logging/metrics
    pub fn provider_name(&self) -> &'static str {
        self.provider.provider_name()
    }

    /// Get the circuit breaker reference, if configured (TAS-174)
    pub fn circuit_breaker(&self) -> Option<&Arc<CircuitBreaker>> {
        self.circuit_breaker.as_ref()
    }

    // =========================================================================
    // Circuit Breaker Helper (TAS-174)
    // =========================================================================

    /// Execute an async operation with circuit breaker protection.
    ///
    /// If no circuit breaker is configured, the operation runs directly.
    /// If the breaker is open, returns `MessagingError::CircuitBreakerOpen`.
    /// Success/failure is recorded on the breaker for state transitions.
    async fn with_breaker<F, T, Fut>(&self, op: F) -> Result<T, MessagingError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, MessagingError>>,
    {
        if let Some(cb) = &self.circuit_breaker {
            if !cb.should_allow() {
                return Err(MessagingError::circuit_breaker_open("messaging"));
            }
            let start = Instant::now();
            let result = op().await;
            match &result {
                Ok(_) => cb.record_success_manual(start.elapsed()),
                Err(_) => cb.record_failure_manual(start.elapsed()),
            }
            result
        } else {
            op().await
        }
    }

    /// Execute an async operation with circuit breaker, mapping MessagingError to TaskerError.
    async fn with_breaker_tasker<F, T, Fut>(&self, op: F) -> TaskerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, MessagingError>>,
    {
        self.with_breaker(op)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))
    }

    // =========================================================================
    // Domain Methods - Step Messages (PROTECTED)
    // =========================================================================

    /// Send a step message to the appropriate worker queue
    pub async fn send_step_message(
        &self,
        namespace: &str,
        message: StepMessage,
    ) -> TaskerResult<()> {
        let queue = self
            .router
            .step_queue(namespace)
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;
        let provider = self.provider.clone();
        self.with_breaker_tasker(|| async move {
            provider.send_message(&queue, &message).await?;
            Ok(())
        })
        .await
    }

    /// Receive step messages from a namespace queue
    pub async fn receive_step_messages(
        &self,
        namespace: &str,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<StepMessage>>, MessagingError> {
        let queue = self.router.step_queue(namespace)?;
        let provider = self.provider.clone();
        self.with_breaker(|| async move {
            provider
                .receive_messages(&queue, max_messages, visibility_timeout)
                .await
        })
        .await
    }

    // =========================================================================
    // Domain Methods - Step Results (PROTECTED)
    // =========================================================================

    /// Send a step result to the orchestration results queue
    pub async fn send_step_result(&self, result: StepResultMessage) -> TaskerResult<()> {
        let queue = self.router.result_queue();
        let provider = self.provider.clone();
        self.with_breaker_tasker(|| async move {
            provider.send_message(&queue, &result).await?;
            Ok(())
        })
        .await
    }

    /// Receive step results from the orchestration results queue
    pub async fn receive_step_results(
        &self,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<StepResultMessage>>, MessagingError> {
        let queue = self.router.result_queue();
        let provider = self.provider.clone();
        self.with_breaker(|| async move {
            provider
                .receive_messages(&queue, max_messages, visibility_timeout)
                .await
        })
        .await
    }

    // =========================================================================
    // Domain Methods - Task Requests (PROTECTED)
    // =========================================================================

    /// Send a task request to the orchestration task requests queue
    pub async fn send_task_request(&self, request: TaskRequestMessage) -> TaskerResult<()> {
        let queue = self.router.task_request_queue();
        let provider = self.provider.clone();
        self.with_breaker_tasker(|| async move {
            provider.send_message(&queue, &request).await?;
            Ok(())
        })
        .await
    }

    /// Receive task requests from the orchestration task requests queue
    pub async fn receive_task_requests(
        &self,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<TaskRequestMessage>>, MessagingError> {
        let queue = self.router.task_request_queue();
        let provider = self.provider.clone();
        self.with_breaker(|| async move {
            provider
                .receive_messages(&queue, max_messages, visibility_timeout)
                .await
        })
        .await
    }

    // =========================================================================
    // Domain Methods - Task Finalization (PROTECTED)
    // =========================================================================

    /// Send a task finalization message
    pub async fn send_task_finalization<T: QueueMessage>(&self, message: &T) -> TaskerResult<()> {
        // TAS-174: Inline circuit breaker check (can't use with_breaker due to &T lifetime)
        if let Some(cb) = &self.circuit_breaker {
            if !cb.should_allow() {
                return Err(crate::TaskerError::MessagingError(
                    MessagingError::circuit_breaker_open("messaging").to_string(),
                ));
            }
            let start = Instant::now();
            let queue = self.router.task_finalization_queue();
            let result = self
                .provider
                .send_message(&queue, message)
                .await
                .map(|_| ())
                .map_err(|e| crate::TaskerError::MessagingError(e.to_string()));
            match &result {
                Ok(_) => cb.record_success_manual(start.elapsed()),
                Err(_) => cb.record_failure_manual(start.elapsed()),
            }
            result
        } else {
            let queue = self.router.task_finalization_queue();
            self.provider
                .send_message(&queue, message)
                .await
                .map(|_| ())
                .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))
        }
    }

    /// Receive task finalization messages
    pub async fn receive_task_finalizations<T: QueueMessage>(
        &self,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<T>>, MessagingError> {
        let queue = self.router.task_finalization_queue();
        let provider = self.provider.clone();
        self.with_breaker(|| async move {
            provider
                .receive_messages(&queue, max_messages, visibility_timeout)
                .await
        })
        .await
    }

    // =========================================================================
    // Queue Management (UNPROTECTED — admin/startup ops)
    // =========================================================================

    /// Initialize queues for the given namespaces
    pub async fn initialize_namespace_queues(&self, namespaces: &[&str]) -> TaskerResult<()> {
        let mut worker_queues = Vec::with_capacity(namespaces.len());
        for ns in namespaces {
            worker_queues.push(
                self.router
                    .step_queue(ns)
                    .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?,
            );
        }

        let orchestration_queues = vec![
            self.router.result_queue(),
            self.router.task_request_queue(),
            self.router.task_finalization_queue(),
        ];

        let mut all_queues = worker_queues;
        all_queues.extend(orchestration_queues);

        self.provider
            .ensure_queues(&all_queues)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;

        Ok(())
    }

    /// Ensure a single queue exists
    pub async fn ensure_queue(&self, queue_name: &str) -> TaskerResult<()> {
        self.provider
            .ensure_queue(queue_name)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;
        Ok(())
    }

    // =========================================================================
    // Message Lifecycle (UNPROTECTED — safe to fail, causes redelivery)
    // =========================================================================

    /// Acknowledge (delete) a processed message
    pub async fn ack_message(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
    ) -> TaskerResult<()> {
        self.provider
            .ack_message(queue_name, receipt_handle)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;
        Ok(())
    }

    /// Negative acknowledge a message (release it back to queue or discard)
    pub async fn nack_message(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
        requeue: bool,
    ) -> TaskerResult<()> {
        self.provider
            .nack_message(queue_name, receipt_handle, requeue)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;
        Ok(())
    }

    /// Extend the visibility timeout for a message
    pub async fn extend_visibility(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
        extension: Duration,
    ) -> TaskerResult<()> {
        self.provider
            .extend_visibility(queue_name, receipt_handle, extension)
            .await
            .map_err(|e| crate::TaskerError::MessagingError(e.to_string()))?;
        Ok(())
    }

    // =========================================================================
    // Queue Metrics (UNPROTECTED — observability should work when breaker open)
    // =========================================================================

    /// Get statistics for a queue
    pub async fn get_queue_stats(&self, queue_name: &str) -> Result<QueueStats, MessagingError> {
        self.provider.queue_stats(queue_name).await
    }

    /// Get statistics for a namespace's worker queue
    pub async fn get_namespace_queue_stats(
        &self,
        namespace: &str,
    ) -> Result<QueueStats, MessagingError> {
        let queue = self.router.step_queue(namespace)?;
        self.provider.queue_stats(&queue).await
    }

    /// Health check for the messaging provider (UNPROTECTED — must work when breaker open)
    pub async fn health_check(&self) -> Result<bool, MessagingError> {
        self.provider.health_check().await
    }

    // =========================================================================
    // Generic Messaging (PROTECTED)
    // =========================================================================

    /// Send a generic message to any queue
    pub async fn send_message<T: QueueMessage>(
        &self,
        queue_name: &str,
        message: &T,
    ) -> TaskerResult<MessageId> {
        let provider = self.provider.clone();
        let queue = queue_name.to_string();
        self.with_breaker_tasker(|| async move {
            let msg_id = provider.send_message(&queue, message).await?;
            Ok(msg_id)
        })
        .await
    }

    /// Receive generic messages from any queue
    pub async fn receive_messages<T: QueueMessage>(
        &self,
        queue_name: &str,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<T>>, MessagingError> {
        let provider = self.provider.clone();
        let queue = queue_name.to_string();
        self.with_breaker(|| async move {
            provider
                .receive_messages(&queue, max_messages, visibility_timeout)
                .await
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resilience::CircuitBreakerConfig;
    use uuid::Uuid;

    fn create_test_client() -> MessageClient {
        let provider = Arc::new(MessagingProvider::new_in_memory());
        let router = MessageRouterKind::default();
        MessageClient::new(provider, router)
    }

    fn create_test_client_with_breaker(
        failure_threshold: u32,
        success_threshold: u32,
    ) -> (MessageClient, Arc<CircuitBreaker>) {
        let provider = Arc::new(MessagingProvider::new_in_memory());
        let router = MessageRouterKind::default();
        let config = CircuitBreakerConfig {
            failure_threshold,
            timeout: Duration::from_millis(100),
            success_threshold,
        };
        let breaker = Arc::new(CircuitBreaker::new("messaging".to_string(), config));
        let client = MessageClient::with_circuit_breaker(provider, router, breaker.clone());
        (client, breaker)
    }

    #[test]
    fn test_message_client_creation() {
        let client = create_test_client();
        assert_eq!(client.provider_name(), "in_memory");
        assert!(client.circuit_breaker().is_none());
    }

    #[test]
    fn test_client_with_circuit_breaker() {
        let (client, _breaker) = create_test_client_with_breaker(5, 2);
        assert!(client.circuit_breaker().is_some());
    }

    #[test]
    fn test_router_queue_names() {
        let client = create_test_client();

        assert_eq!(
            client.router().step_queue("payments").unwrap(),
            "worker_payments_queue"
        );
        assert_eq!(client.router().result_queue(), "orchestration_step_results");
        assert_eq!(
            client.router().task_request_queue(),
            "orchestration_task_requests"
        );
    }

    #[tokio::test]
    async fn test_send_step_message() {
        let client = create_test_client();

        client.ensure_queue("worker_payments_queue").await.unwrap();
        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let result = client.send_step_message("payments", msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_initialize_namespace_queues() {
        let client = create_test_client();
        let result = client
            .initialize_namespace_queues(&["payments", "fulfillment"])
            .await;
        assert!(result.is_ok());
        assert!(client.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_send_and_receive_step_messages() {
        let client = create_test_client();

        let queue_name = client.router().step_queue("test").unwrap();
        client.ensure_queue(&queue_name).await.unwrap();

        let original_msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        client
            .send_step_message("test", original_msg.clone())
            .await
            .unwrap();

        let messages = client
            .receive_step_messages("test", 10, Duration::from_secs(30))
            .await
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message.step_uuid, original_msg.step_uuid);
    }

    #[tokio::test]
    async fn test_ack_message() {
        let client = create_test_client();

        let queue_name = client.router().step_queue("test").unwrap();
        client.ensure_queue(&queue_name).await.unwrap();

        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        client.send_step_message("test", msg).await.unwrap();

        let messages = client
            .receive_step_messages("test", 10, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        let result = client
            .ack_message(&queue_name, &messages[0].receipt_handle)
            .await;
        assert!(result.is_ok());

        let messages_after = client
            .receive_step_messages("test", 10, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(messages_after.len(), 0);
    }

    // =========================================================================
    // TAS-174: Circuit Breaker Tests
    // =========================================================================

    #[tokio::test]
    async fn test_send_blocked_when_circuit_open() {
        let (client, breaker) = create_test_client_with_breaker(1, 1);

        // Initialize queue first
        client.ensure_queue("worker_test_queue").await.unwrap();

        // Force circuit open
        breaker.force_open();

        // Send should be rejected
        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let result = client.send_step_message("test", msg).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Circuit breaker"),
            "Expected circuit breaker error, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_receive_blocked_when_circuit_open() {
        let (client, breaker) = create_test_client_with_breaker(1, 1);

        breaker.force_open();

        let result = client
            .receive_step_messages("test", 10, Duration::from_secs(30))
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MessagingError::CircuitBreakerOpen { .. }
        ));
    }

    #[tokio::test]
    async fn test_ack_bypasses_circuit_breaker() {
        let (client, breaker) = create_test_client_with_breaker(1, 1);

        let queue_name = client.router().step_queue("test").unwrap();
        client.ensure_queue(&queue_name).await.unwrap();

        // Send a message before opening the breaker
        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        client.send_step_message("test", msg).await.unwrap();

        let messages = client
            .receive_step_messages("test", 10, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        // Force circuit open
        breaker.force_open();

        // Ack should still work (bypasses breaker)
        let result = client
            .ack_message(&queue_name, &messages[0].receipt_handle)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_bypasses_circuit_breaker() {
        let (client, breaker) = create_test_client_with_breaker(1, 1);

        breaker.force_open();

        // Health check should still work
        let result = client.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_success_updates_breaker_metrics() {
        let (client, breaker) = create_test_client_with_breaker(5, 2);

        client.ensure_queue("worker_test_queue").await.unwrap();

        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        client.send_step_message("test", msg).await.unwrap();

        let metrics = breaker.metrics();
        assert_eq!(metrics.success_count, 1);
        assert_eq!(metrics.failure_count, 0);
    }

    #[tokio::test]
    async fn test_no_breaker_passthrough() {
        // Client without circuit breaker should work normally
        let client = create_test_client();
        assert!(client.circuit_breaker().is_none());

        client.ensure_queue("worker_test_queue").await.unwrap();

        let msg = StepMessage::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let result = client.send_step_message("test", msg).await;
        assert!(result.is_ok());
    }
}
