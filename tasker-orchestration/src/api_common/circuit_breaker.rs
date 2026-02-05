//! # API Database Circuit Breaker
//!
//! Circuit breaker implementation for API database operations.
//! Protects against database connection failures and query timeouts without
//! interfering with the orchestration system's PGMQ operations.
//!
//! This module contains the core circuit breaker types that are shared between
//! REST and gRPC APIs. Transport-specific helper functions remain in their
//! respective modules (e.g., `web::circuit_breaker` for REST-specific helpers).
//!
//! ## TAS-174: Refactored to Wrap Generic CircuitBreaker
//!
//! The internal atomics have been replaced with a `CircuitBreaker` from
//! `tasker_shared::resilience`. This provides proper half-open → closed recovery
//! via `success_threshold` (previously a single success would close the circuit).

use std::sync::Arc;
use std::time::Duration;

pub use tasker_shared::resilience::CircuitState;
use tasker_shared::resilience::{
    CircuitBreaker, CircuitBreakerBehavior, CircuitBreakerConfig, CircuitBreakerMetrics,
};

/// API database circuit breaker for health monitoring
///
/// This circuit breaker is designed specifically for API database operations
/// and operates independently of the orchestration system's circuit breakers.
///
/// # States
/// - **Closed**: Normal operation, all requests pass through
/// - **Open**: Too many failures, reject requests with fast failure
/// - **Half-Open**: Testing if the database has recovered
///
/// # TAS-174
///
/// Now wraps the generic `CircuitBreaker` from `tasker_shared::resilience` and
/// implements `CircuitBreakerBehavior`. The backward-compatible convenience
/// methods (`is_circuit_open`, `record_success`, `record_failure`) are preserved.
#[derive(Debug, Clone)]
pub struct WebDatabaseCircuitBreaker {
    /// Inner generic circuit breaker (wrapped in Arc for Clone)
    breaker: Arc<CircuitBreaker>,
}

impl WebDatabaseCircuitBreaker {
    /// Create a new circuit breaker for API database operations
    ///
    /// # Arguments
    /// * `failure_threshold` - Number of failures before opening circuit
    /// * `recovery_timeout` - Duration to wait before testing recovery
    /// * `component_name` - Name for logging and monitoring
    pub fn new(
        failure_threshold: u32,
        recovery_timeout: Duration,
        component_name: impl Into<String>,
    ) -> Self {
        let config = CircuitBreakerConfig {
            failure_threshold,
            timeout: recovery_timeout,
            success_threshold: 2, // TAS-174: proper half-open recovery
        };
        Self {
            breaker: Arc::new(CircuitBreaker::new(component_name.into(), config)),
        }
    }

    /// Create from a resilience config (TAS-174: config-driven construction)
    pub fn from_config(component_name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            breaker: Arc::new(CircuitBreaker::new(component_name.into(), config)),
        }
    }

    /// Check if the circuit is currently open (failing fast)
    ///
    /// This also handles the transition from Open to Half-Open when
    /// the recovery timeout has elapsed.
    pub fn is_circuit_open(&self) -> bool {
        !self.breaker.should_allow()
    }

    /// Record a successful operation
    ///
    /// In closed state: resets failure count.
    /// In half-open state: counts toward success_threshold for closing.
    pub fn record_success(&self) {
        self.breaker.record_success_manual(Duration::ZERO);
    }

    /// Record a failed operation
    ///
    /// Increments the failure count and opens the circuit if threshold is exceeded.
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

    /// Get the component name
    pub fn component_name(&self) -> &str {
        self.breaker.name()
    }

    /// Force the circuit to open state
    pub fn force_open(&self) {
        self.breaker.force_open();
    }

    /// Force the circuit to closed state
    pub fn force_closed(&self) {
        self.breaker.force_closed();
    }

    /// Get circuit breaker metrics
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        self.breaker.metrics()
    }
}

impl Default for WebDatabaseCircuitBreaker {
    fn default() -> Self {
        Self::new(
            5,                       // failure_threshold
            Duration::from_secs(30), // recovery_timeout
            "web_database",          // component_name
        )
    }
}

impl CircuitBreakerBehavior for WebDatabaseCircuitBreaker {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = WebDatabaseCircuitBreaker::new(3, Duration::from_secs(5), "test");
        assert!(!cb.is_circuit_open());
        assert_eq!(cb.current_state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_opens_after_threshold_failures() {
        let cb = WebDatabaseCircuitBreaker::new(3, Duration::from_secs(5), "test");

        // Record failures below threshold
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.is_circuit_open());
        assert_eq!(cb.current_state(), CircuitState::Closed);

        // Record failure that exceeds threshold
        cb.record_failure();
        assert!(cb.is_circuit_open());
        assert_eq!(cb.current_state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_closes_on_success_via_half_open() {
        let cb = WebDatabaseCircuitBreaker::new(
            2,
            Duration::ZERO, // instant recovery timeout for testing
            "test",
        );

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);

        // With zero timeout, is_circuit_open() calls should_allow() which
        // transitions to half-open immediately, returning "not open" (allows call)
        assert!(!cb.is_circuit_open()); // transitions to HalfOpen

        // Record success_threshold (2) successes to close
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert_eq!(cb.current_failures(), 0);
    }

    #[test]
    fn test_circuit_state_from_u8_conversion() {
        assert_eq!(CircuitState::from(0), CircuitState::Closed);
        assert_eq!(CircuitState::from(1), CircuitState::Open);
        assert_eq!(CircuitState::from(2), CircuitState::HalfOpen);
        // Invalid values default to Open (safest)
        assert_eq!(CircuitState::from(3), CircuitState::Open);
        assert_eq!(CircuitState::from(255), CircuitState::Open);
    }

    #[test]
    fn test_default_circuit_breaker_configuration() {
        let cb = WebDatabaseCircuitBreaker::default();

        // Default values: 5 failures, 30s recovery, "web_database" component
        assert_eq!(cb.component_name(), "web_database");
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert_eq!(cb.current_failures(), 0);
        assert!(!cb.is_circuit_open());
    }

    #[test]
    fn test_component_name_accessor() {
        let cb = WebDatabaseCircuitBreaker::new(5, Duration::from_secs(30), "custom_component");
        assert_eq!(cb.component_name(), "custom_component");
    }

    #[test]
    fn test_failure_count_increments_correctly() {
        let cb = WebDatabaseCircuitBreaker::new(10, Duration::from_secs(30), "test");

        assert_eq!(cb.current_failures(), 0);
        cb.record_failure();
        assert_eq!(cb.current_failures(), 1);
        cb.record_failure();
        assert_eq!(cb.current_failures(), 2);
        cb.record_failure();
        assert_eq!(cb.current_failures(), 3);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let cb = WebDatabaseCircuitBreaker::new(10, Duration::from_secs(30), "test");

        // Accumulate some failures (but not enough to open)
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_failures(), 3);

        // Success should reset count
        cb.record_success();
        assert_eq!(cb.current_failures(), 0);
    }

    #[test]
    fn test_circuit_breaker_exact_threshold() {
        // Test that circuit opens at exactly the threshold, not before
        let cb = WebDatabaseCircuitBreaker::new(5, Duration::from_secs(30), "test");

        // 1 through 4 failures - should stay closed
        for i in 1..5 {
            cb.record_failure();
            assert!(
                !cb.is_circuit_open(),
                "Circuit should be closed at {} failures (threshold is 5)",
                i
            );
        }

        // 5th failure - should open
        cb.record_failure();
        assert!(
            cb.is_circuit_open(),
            "Circuit should be open at threshold (5 failures)"
        );
    }

    #[test]
    fn test_force_operations() {
        let cb = WebDatabaseCircuitBreaker::default();

        cb.force_open();
        assert_eq!(cb.current_state(), CircuitState::Open);

        cb.force_closed();
        assert_eq!(cb.current_state(), CircuitState::Closed);
    }

    #[test]
    fn test_behavior_trait_conformance() {
        let cb = WebDatabaseCircuitBreaker::new(3, Duration::from_secs(5), "trait_test");
        let behavior: &dyn CircuitBreakerBehavior = &cb;

        assert_eq!(behavior.name(), "trait_test");
        assert_eq!(behavior.state(), CircuitState::Closed);
        assert!(behavior.should_allow());

        behavior.record_failure(Duration::ZERO);
        behavior.record_failure(Duration::ZERO);
        behavior.record_failure(Duration::ZERO);
        assert_eq!(behavior.state(), CircuitState::Open);
        assert!(!behavior.should_allow());
    }
}
