//! # Circuit Breaker Metrics
//!
//! Provides comprehensive metrics collection for circuit breaker operations.
//! These metrics enable monitoring, alerting, and performance analysis of
//! circuit breaker behavior in production.

use crate::resilience::CircuitState;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Metrics for a single circuit breaker instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerMetrics {
    /// Total number of calls attempted
    pub total_calls: u64,

    /// Number of successful calls
    pub success_count: u64,

    /// Number of failed calls
    pub failure_count: u64,

    /// Current consecutive failure count
    pub consecutive_failures: u64,

    /// Number of calls made in half-open state
    pub half_open_calls: u64,

    /// Total duration of all operations
    pub total_duration: Duration,

    /// Current circuit breaker state
    pub current_state: CircuitState,

    /// Calculated failure rate (0.0 to 1.0)
    pub failure_rate: f64,

    /// Calculated success rate (0.0 to 1.0)
    pub success_rate: f64,

    /// Average operation duration
    pub average_duration: Duration,
}

impl CircuitBreakerMetrics {
    /// Create new metrics instance with zero values
    pub fn new() -> Self {
        Self {
            total_calls: 0,
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            half_open_calls: 0,
            total_duration: Duration::ZERO,
            current_state: CircuitState::Closed,
            failure_rate: 0.0,
            success_rate: 0.0,
            average_duration: Duration::ZERO,
        }
    }

    /// Calculate calls per second based on total duration
    pub fn calls_per_second(&self) -> f64 {
        if self.total_duration.is_zero() {
            return 0.0;
        }

        self.total_calls as f64 / self.total_duration.as_secs_f64()
    }

    /// Check if metrics indicate healthy operation
    pub fn is_healthy(&self) -> bool {
        match self.current_state {
            CircuitState::Closed => {
                // Closed is healthy if failure rate is reasonable
                self.failure_rate < 0.1 // Less than 10% failure rate
            }
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // Half-open is attempting recovery
        }
    }

    /// Get human-readable state description
    pub fn state_description(&self) -> &'static str {
        match self.current_state {
            CircuitState::Closed => "Healthy - Normal operation",
            CircuitState::Open => "Failing - Rejecting all calls",
            CircuitState::HalfOpen => "Recovering - Testing system health",
        }
    }

    /// Format metrics for logging
    pub fn format_summary(&self) -> String {
        format!(
            "State: {} | Calls: {} | Success: {:.1}% | Failures: {} | Avg Duration: {:.2}ms",
            self.state_description(),
            self.total_calls,
            self.success_rate * 100.0,
            self.failure_count,
            self.average_duration.as_millis()
        )
    }
}

impl Default for CircuitBreakerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics collection trait for integration with monitoring systems
pub trait MetricsCollector {
    /// Record circuit breaker metrics
    fn record_circuit_breaker_metrics(&self, name: &str, metrics: &CircuitBreakerMetrics);

    /// Record circuit breaker state transition
    fn record_state_transition(&self, name: &str, from: CircuitState, to: CircuitState);

    /// Record operation timing
    fn record_operation_timing(&self, name: &str, duration: Duration, success: bool);
}

/// Prometheus-style metrics exporter
#[derive(Debug)]
pub struct PrometheusMetricsExporter;

impl MetricsCollector for PrometheusMetricsExporter {
    fn record_circuit_breaker_metrics(&self, name: &str, metrics: &CircuitBreakerMetrics) {
        // In a real implementation, this would export to Prometheus
        tracing::info!(
            circuit_breaker = name,
            total_calls = metrics.total_calls,
            success_count = metrics.success_count,
            failure_count = metrics.failure_count,
            failure_rate = metrics.failure_rate,
            state = ?metrics.current_state,
            "Circuit breaker metrics"
        );
    }

    fn record_state_transition(&self, name: &str, from: CircuitState, to: CircuitState) {
        tracing::info!(
            circuit_breaker = name,
            from_state = ?from,
            to_state = ?to,
            "Circuit breaker state transition"
        );
    }

    fn record_operation_timing(&self, name: &str, duration: Duration, success: bool) {
        tracing::debug!(
            circuit_breaker = name,
            duration_ms = duration.as_millis(),
            success = success,
            "Operation timing"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_metrics_creation() {
        let metrics = CircuitBreakerMetrics::new();

        assert_eq!(metrics.total_calls, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(metrics.current_state, CircuitState::Closed);
        assert!(metrics.is_healthy());
    }

    #[test]
    fn test_metrics_health_calculation() {
        let mut metrics = CircuitBreakerMetrics::new();

        // Healthy closed state
        metrics.current_state = CircuitState::Closed;
        metrics.failure_rate = 0.05;
        assert!(metrics.is_healthy());

        // Unhealthy closed state (high failure rate)
        metrics.failure_rate = 0.15;
        assert!(!metrics.is_healthy());

        // Open state is never healthy
        metrics.current_state = CircuitState::Open;
        metrics.failure_rate = 0.0;
        assert!(!metrics.is_healthy());

        // Half-open is considered healthy (recovering)
        metrics.current_state = CircuitState::HalfOpen;
        assert!(metrics.is_healthy());
    }
}
