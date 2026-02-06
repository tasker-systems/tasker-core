//! # Message Router
//!
//! Queue name routing abstraction for namespace-based queue organization.

use crate::config::queues::QueuesConfig;
use crate::messaging::MessagingError;
use crate::validation::validate_queue_name;

/// Namespace-based queue routing trait
///
/// Separates queue name generation from messaging operations,
/// allowing different naming conventions for different deployments.
pub trait MessageRouter: Send + Sync {
    /// Get the step execution queue for a namespace
    ///
    /// Default pattern: `worker_{namespace}_queue`
    ///
    /// Returns `Err(MessagingError::InvalidQueueName)` if the constructed queue
    /// name contains invalid characters or exceeds the PGMQ length limit.
    fn step_queue(&self, namespace: &str) -> Result<String, MessagingError>;

    /// Get the step results queue (orchestration consumes)
    fn result_queue(&self) -> String;

    /// Get the task request queue (orchestration consumes)
    fn task_request_queue(&self) -> String;

    /// Get the task finalization queue (orchestration consumes)
    fn task_finalization_queue(&self) -> String;

    /// Get the domain event queue for a namespace
    ///
    /// Default pattern: `{namespace}_domain_events`
    ///
    /// Returns `Err(MessagingError::InvalidQueueName)` if the constructed queue
    /// name contains invalid characters or exceeds the PGMQ length limit.
    fn domain_event_queue(&self, namespace: &str) -> Result<String, MessagingError>;

    /// Extract namespace from a queue name (reverse of step_queue)
    ///
    /// Returns None if the queue name doesn't match the expected pattern.
    fn extract_namespace(&self, queue_name: &str) -> Option<String>;
}

/// Default router implementation using config-based queue names
///
/// Uses the standard Tasker naming conventions:
/// - Worker queues: `worker_{namespace}_queue`
/// - Orchestration queues: configured names from TOML
#[derive(Debug, Clone)]
pub struct DefaultMessageRouter {
    /// Prefix for worker queues (default: "worker")
    worker_queue_prefix: String,

    /// Step results queue name
    result_queue: String,

    /// Task request queue name
    task_request_queue: String,

    /// Task finalization queue name
    task_finalization_queue: String,
}

impl DefaultMessageRouter {
    /// Create a router from queue configuration
    pub fn from_config(config: &QueuesConfig) -> Self {
        Self {
            worker_queue_prefix: config.worker_namespace.clone(),
            result_queue: config.orchestration_queues.step_results.clone(),
            task_request_queue: config.orchestration_queues.task_requests.clone(),
            task_finalization_queue: config.orchestration_queues.task_finalizations.clone(),
        }
    }

    /// Create a router with explicit queue names
    pub fn new(
        worker_queue_prefix: impl Into<String>,
        result_queue: impl Into<String>,
        task_request_queue: impl Into<String>,
        task_finalization_queue: impl Into<String>,
    ) -> Self {
        Self {
            worker_queue_prefix: worker_queue_prefix.into(),
            result_queue: result_queue.into(),
            task_request_queue: task_request_queue.into(),
            task_finalization_queue: task_finalization_queue.into(),
        }
    }
}

impl Default for DefaultMessageRouter {
    fn default() -> Self {
        Self {
            worker_queue_prefix: "worker".to_string(),
            result_queue: "orchestration_step_results".to_string(),
            task_request_queue: "orchestration_task_requests".to_string(),
            task_finalization_queue: "orchestration_task_finalizations".to_string(),
        }
    }
}

impl MessageRouter for DefaultMessageRouter {
    fn step_queue(&self, namespace: &str) -> Result<String, MessagingError> {
        let name = format!("{}_{}_queue", self.worker_queue_prefix, namespace);
        validate_queue_name(&name)?;
        Ok(name)
    }

    fn result_queue(&self) -> String {
        self.result_queue.clone()
    }

    fn task_request_queue(&self) -> String {
        self.task_request_queue.clone()
    }

    fn task_finalization_queue(&self) -> String {
        self.task_finalization_queue.clone()
    }

    fn domain_event_queue(&self, namespace: &str) -> Result<String, MessagingError> {
        let name = format!("{}_domain_events", namespace);
        validate_queue_name(&name)?;
        Ok(name)
    }

    fn extract_namespace(&self, queue_name: &str) -> Option<String> {
        queue_name
            .strip_prefix(&format!("{}_", self.worker_queue_prefix))
            .and_then(|s| s.strip_suffix("_queue"))
            .map(String::from)
    }
}

/// Enum dispatch for MessageRouter
///
/// Uses enum dispatch instead of `Arc<dyn MessageRouter>` to maintain
/// consistency with `MessagingProvider` and avoid vtable overhead.
/// While router operations are cheap (string formatting), using enums
/// keeps the pattern uniform across the messaging layer.
#[derive(Debug, Clone)]
pub enum MessageRouterKind {
    /// Default router with standard Tasker naming conventions
    Default(DefaultMessageRouter),
    // Future variants can be added as needed:
    // Custom(CustomRouter),
    // Prefixed(PrefixedRouter),
}

impl MessageRouterKind {
    /// Create a router from queue configuration
    ///
    /// Creates a `Default` variant using the configuration values.
    pub fn from_config(config: &QueuesConfig) -> Self {
        Self::Default(DefaultMessageRouter::from_config(config))
    }

    /// Get the step execution queue for a namespace
    pub fn step_queue(&self, namespace: &str) -> Result<String, MessagingError> {
        match self {
            Self::Default(r) => r.step_queue(namespace),
        }
    }

    /// Get the step results queue
    pub fn result_queue(&self) -> String {
        match self {
            Self::Default(r) => r.result_queue(),
        }
    }

    /// Get the task request queue
    pub fn task_request_queue(&self) -> String {
        match self {
            Self::Default(r) => r.task_request_queue(),
        }
    }

    /// Get the task finalization queue
    pub fn task_finalization_queue(&self) -> String {
        match self {
            Self::Default(r) => r.task_finalization_queue(),
        }
    }

    /// Get the domain event queue for a namespace
    pub fn domain_event_queue(&self, namespace: &str) -> Result<String, MessagingError> {
        match self {
            Self::Default(r) => r.domain_event_queue(namespace),
        }
    }

    /// Extract namespace from a queue name
    pub fn extract_namespace(&self, queue_name: &str) -> Option<String> {
        match self {
            Self::Default(r) => r.extract_namespace(queue_name),
        }
    }
}

impl Default for MessageRouterKind {
    fn default() -> Self {
        Self::Default(DefaultMessageRouter::default())
    }
}

impl From<DefaultMessageRouter> for MessageRouterKind {
    fn from(router: DefaultMessageRouter) -> Self {
        Self::Default(router)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_router_step_queue() {
        let router = DefaultMessageRouter::default();

        assert_eq!(
            router.step_queue("payments").unwrap(),
            "worker_payments_queue"
        );
        assert_eq!(
            router.step_queue("fulfillment").unwrap(),
            "worker_fulfillment_queue"
        );
    }

    #[test]
    fn test_default_router_orchestration_queues() {
        let router = DefaultMessageRouter::default();

        assert_eq!(router.result_queue(), "orchestration_step_results");
        assert_eq!(router.task_request_queue(), "orchestration_task_requests");
        assert_eq!(
            router.task_finalization_queue(),
            "orchestration_task_finalizations"
        );
    }

    #[test]
    fn test_default_router_domain_event_queue() {
        let router = DefaultMessageRouter::default();

        assert_eq!(
            router.domain_event_queue("payments").unwrap(),
            "payments_domain_events"
        );
    }

    #[test]
    fn test_extract_namespace() {
        let router = DefaultMessageRouter::default();

        assert_eq!(
            router.extract_namespace("worker_payments_queue"),
            Some("payments".to_string())
        );
        assert_eq!(
            router.extract_namespace("worker_fulfillment_queue"),
            Some("fulfillment".to_string())
        );

        // Should return None for non-matching patterns
        assert_eq!(router.extract_namespace("orchestration_step_results"), None);
        assert_eq!(router.extract_namespace("random_queue"), None);
    }

    #[test]
    fn test_router_kind_delegates() {
        let kind = MessageRouterKind::default();

        assert_eq!(
            kind.step_queue("payments").unwrap(),
            "worker_payments_queue"
        );
        assert_eq!(kind.result_queue(), "orchestration_step_results");
        assert_eq!(
            kind.extract_namespace("worker_payments_queue"),
            Some("payments".to_string())
        );
    }

    #[test]
    fn test_custom_router() {
        let router =
            DefaultMessageRouter::new("custom", "my_results", "my_requests", "my_finalizations");

        assert_eq!(router.step_queue("test").unwrap(), "custom_test_queue");
        assert_eq!(router.result_queue(), "my_results");
        assert_eq!(
            router.extract_namespace("custom_test_queue"),
            Some("test".to_string())
        );
    }

    // =========================================================================
    // TAS-226: Validation Tests
    // =========================================================================

    #[test]
    fn test_step_queue_rejects_invalid_namespace() {
        let router = DefaultMessageRouter::default();

        // Namespace with hyphens produces invalid queue name
        assert!(router.step_queue("bad-namespace").is_err());

        // Namespace with spaces
        assert!(router.step_queue("bad namespace").is_err());

        // Namespace with SQL injection attempt
        assert!(router.step_queue("bad;DROP TABLE").is_err());
    }

    #[test]
    fn test_domain_event_queue_rejects_invalid_namespace() {
        let router = DefaultMessageRouter::default();

        assert!(router.domain_event_queue("bad-namespace").is_err());
        assert!(router.domain_event_queue("bad;DROP TABLE").is_err());
    }

    #[test]
    fn test_valid_namespaces_produce_correct_queue_names() {
        let router = DefaultMessageRouter::default();

        assert_eq!(
            router.step_queue("payments").unwrap(),
            "worker_payments_queue"
        );
        assert_eq!(
            router.domain_event_queue("orders").unwrap(),
            "orders_domain_events"
        );
    }
}
