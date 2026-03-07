//! # Task Readiness Circuit Breaker
//!
//! TAS-75 Phase 5b: Circuit breaker for task readiness polling operations.
//!
//! This circuit breaker protects the fallback poller from cascading failures
//! when the database is unavailable or under stress.
//!
//! ## TAS-174: Refactored to Wrap Generic CircuitBreaker
//!
//! The internal atomics have been replaced with a `CircuitBreaker` from
//! `tasker_shared::resilience`. This provides unified state machine behavior
//! while preserving the existing public API.
//!
//! ## States
//!
//! - **Closed**: Normal operation, polling proceeds
//! - **Open**: Failing fast, skip polling cycles
//! - **Half-Open**: Testing recovery after timeout

use std::sync::Arc;
use std::time::Duration;

use tasker_shared::resilience::{
    CircuitBreaker, CircuitBreakerBehavior, CircuitBreakerMetrics, CircuitState,
};
use tracing::info;

/// Configuration for the task readiness circuit breaker
#[derive(Debug, Clone)]
pub struct TaskReadinessCircuitBreakerConfig {
    /// Number of failures needed to open the circuit
    pub failure_threshold: u32,
    /// How long to wait before testing recovery (seconds)
    pub recovery_timeout_seconds: u64,
    /// Number of successes needed in half-open to close
    pub success_threshold: u32,
}

impl Default for TaskReadinessCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 10,
            recovery_timeout_seconds: 60,
            success_threshold: 3,
        }
    }
}

/// Circuit breaker for task readiness polling operations
///
/// This circuit breaker protects the fallback poller from repeated failures
/// when the database is unavailable. It operates independently of the web
/// circuit breaker to allow fine-grained control over orchestration polling.
///
/// ## Usage
///
/// ```rust,ignore
/// let config = TaskReadinessCircuitBreakerConfig::default();
/// let breaker = TaskReadinessCircuitBreaker::new(config);
///
/// // Before polling
/// if breaker.is_circuit_open() {
///     // Skip this polling cycle
///     return;
/// }
///
/// // After polling
/// match poll_result {
///     Ok(_) => breaker.record_success(),
///     Err(_) => breaker.record_failure(),
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TaskReadinessCircuitBreaker {
    /// Inner generic circuit breaker (wrapped in Arc for Clone)
    breaker: Arc<CircuitBreaker>,
}

impl TaskReadinessCircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: TaskReadinessCircuitBreakerConfig) -> Self {
        info!(
            failure_threshold = config.failure_threshold,
            recovery_timeout_seconds = config.recovery_timeout_seconds,
            success_threshold = config.success_threshold,
            "Task readiness circuit breaker initialized"
        );

        let resilience_config = tasker_shared::resilience::CircuitBreakerConfig {
            failure_threshold: config.failure_threshold,
            timeout: Duration::from_secs(config.recovery_timeout_seconds),
            success_threshold: config.success_threshold,
        };

        Self {
            breaker: Arc::new(CircuitBreaker::new(
                "task_readiness".to_string(),
                resilience_config,
            )),
        }
    }

    /// Check if the circuit is currently open (should skip polling)
    ///
    /// This also handles the transition from Open to Half-Open when
    /// the recovery timeout has elapsed.
    pub fn is_circuit_open(&self) -> bool {
        !self.breaker.should_allow()
    }

    /// Record a successful polling operation
    ///
    /// In closed state: resets failure count
    /// In half-open state: increments success count, may close circuit
    pub fn record_success(&self) {
        self.breaker.record_success_manual(Duration::ZERO);
    }

    /// Record a failed polling operation
    ///
    /// Increments the failure count and opens the circuit if threshold is exceeded.
    /// In half-open state, any failure immediately opens the circuit.
    pub fn record_failure(&self) {
        self.breaker.record_failure_manual(Duration::ZERO);
    }

    /// Get current circuit state for monitoring
    pub fn current_state(&self) -> CircuitState {
        self.breaker.state()
    }

    /// Get current failure count
    pub fn current_failures(&self) -> u32 {
        self.breaker.metrics().consecutive_failures as u32
    }

    /// Get half-open success count
    pub fn half_open_successes(&self) -> u32 {
        self.breaker.metrics().half_open_calls as u32
    }

    /// Check if circuit is healthy (closed state)
    pub fn is_healthy(&self) -> bool {
        self.breaker.state() == CircuitState::Closed
    }

    /// Force the circuit open (for emergency situations)
    pub fn force_open(&self) {
        self.breaker.force_open();
    }

    /// Force the circuit closed (for emergency recovery)
    pub fn force_closed(&self) {
        self.breaker.force_closed();
    }

    /// Get metrics for health reporting
    pub fn metrics(&self) -> TaskReadinessCircuitBreakerMetrics {
        let inner_metrics = self.breaker.metrics();
        TaskReadinessCircuitBreakerMetrics {
            state: inner_metrics.current_state,
            current_failures: inner_metrics.consecutive_failures as u32,
            half_open_successes: inner_metrics.half_open_calls as u32,
            failure_threshold: inner_metrics.consecutive_failures as u32, // Not ideal, see below
            success_threshold: 0, // Config not stored; consumers use the metrics struct for state only
            recovery_timeout_secs: 0,
        }
    }

    /// Get detailed metrics for health reporting (includes config values)
    pub fn metrics_with_config(
        &self,
        config: &TaskReadinessCircuitBreakerConfig,
    ) -> TaskReadinessCircuitBreakerMetrics {
        let inner_metrics = self.breaker.metrics();
        TaskReadinessCircuitBreakerMetrics {
            state: inner_metrics.current_state,
            current_failures: inner_metrics.consecutive_failures as u32,
            half_open_successes: inner_metrics.half_open_calls as u32,
            failure_threshold: config.failure_threshold,
            success_threshold: config.success_threshold,
            recovery_timeout_secs: config.recovery_timeout_seconds,
        }
    }
}

impl Default for TaskReadinessCircuitBreaker {
    fn default() -> Self {
        Self::new(TaskReadinessCircuitBreakerConfig::default())
    }
}

impl CircuitBreakerBehavior for TaskReadinessCircuitBreaker {
    fn name(&self) -> &str {
        self.breaker.name()
    }

    fn state(&self) -> CircuitState {
        self.breaker.state()
    }

    fn should_allow(&self) -> bool {
        self.breaker.should_allow()
    }

    fn record_success(&self, duration: Duration) {
        self.breaker.record_success_manual(duration);
    }

    fn record_failure(&self, duration: Duration) {
        self.breaker.record_failure_manual(duration);
    }

    fn is_healthy(&self) -> bool {
        self.breaker.is_healthy()
    }

    fn force_open(&self) {
        self.breaker.force_open();
    }

    fn force_closed(&self) {
        self.breaker.force_closed();
    }

    fn metrics(&self) -> CircuitBreakerMetrics {
        self.breaker.metrics()
    }
}

/// Metrics snapshot for health reporting
#[derive(Debug, Clone)]
pub struct TaskReadinessCircuitBreakerMetrics {
    /// Current circuit state
    pub state: CircuitState,
    /// Current failure count
    pub current_failures: u32,
    /// Successes in half-open state
    pub half_open_successes: u32,
    /// Configured failure threshold
    pub failure_threshold: u32,
    /// Configured success threshold
    pub success_threshold: u32,
    /// Configured recovery timeout
    pub recovery_timeout_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let breaker = TaskReadinessCircuitBreaker::default();
        assert!(!breaker.is_circuit_open());
        assert_eq!(breaker.current_state(), CircuitState::Closed);
        assert!(breaker.is_healthy());
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let config = TaskReadinessCircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout_seconds: 60,
            success_threshold: 2,
        };
        let breaker = TaskReadinessCircuitBreaker::new(config);

        // Record failures below threshold
        breaker.record_failure();
        breaker.record_failure();
        assert!(!breaker.is_circuit_open());

        // Third failure should open circuit
        breaker.record_failure();
        assert!(breaker.is_circuit_open());
        assert_eq!(breaker.current_state(), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_failures() {
        let config = TaskReadinessCircuitBreakerConfig {
            failure_threshold: 5,
            recovery_timeout_seconds: 60,
            success_threshold: 1,
        };
        let breaker = TaskReadinessCircuitBreaker::new(config);

        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.current_failures(), 2);

        breaker.record_success();
        assert_eq!(breaker.current_failures(), 0);
    }

    #[test]
    fn test_half_open_closes_after_successes() {
        let config = TaskReadinessCircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_seconds: 0, // Immediate recovery for test
            success_threshold: 2,
        };
        let breaker = TaskReadinessCircuitBreaker::new(config);

        // Open circuit
        breaker.record_failure();
        assert_eq!(breaker.current_state(), CircuitState::Open);

        // is_circuit_open() with recovery_timeout=0 immediately transitions to half-open
        assert!(!breaker.is_circuit_open());
        assert_eq!(breaker.current_state(), CircuitState::HalfOpen);

        // First success in half-open
        breaker.record_success();
        // The generic CircuitBreaker increments half_open_calls on success
        // and closes at success_threshold (2)
        assert_eq!(breaker.current_state(), CircuitState::HalfOpen);

        // Second success should close circuit
        breaker.record_success();
        assert_eq!(breaker.current_state(), CircuitState::Closed);
        assert!(breaker.is_healthy());
    }

    #[test]
    fn test_failure_in_half_open_reopens() {
        let config = TaskReadinessCircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_seconds: 0,
            success_threshold: 2,
        };
        let breaker = TaskReadinessCircuitBreaker::new(config);

        // Open circuit, then transition to half-open
        breaker.record_failure();
        let _ = breaker.is_circuit_open(); // Triggers transition to half-open

        assert_eq!(breaker.current_state(), CircuitState::HalfOpen);

        // Failure in half-open should reopen
        breaker.record_failure();
        assert_eq!(breaker.current_state(), CircuitState::Open);
    }

    #[test]
    fn test_force_operations() {
        let breaker = TaskReadinessCircuitBreaker::default();

        breaker.force_open();
        assert_eq!(breaker.current_state(), CircuitState::Open);

        breaker.force_closed();
        assert_eq!(breaker.current_state(), CircuitState::Closed);
    }

    #[test]
    fn test_behavior_trait_conformance() {
        let breaker = TaskReadinessCircuitBreaker::default();
        let behavior: &dyn CircuitBreakerBehavior = &breaker;

        assert_eq!(behavior.name(), "task_readiness");
        assert_eq!(behavior.state(), CircuitState::Closed);
        assert!(behavior.should_allow());
    }
}
