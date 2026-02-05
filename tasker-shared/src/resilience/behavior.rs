//! # Circuit Breaker Behavior Trait (TAS-174)
//!
//! Defines the `CircuitBreakerBehavior` trait that unifies all circuit breaker
//! implementations in the system. Each specialized circuit breaker (web database,
//! task readiness, FFI completion, messaging) implements this trait while retaining
//! domain-specific convenience methods.
//!
//! ## Design
//!
//! The trait is object-safe (`Send + Sync + Debug`) so that consumers can work with
//! `&dyn CircuitBreakerBehavior` when uniform access to any breaker is needed
//! (e.g., health reporting, metrics collection). Concrete types are preferred in
//! hot paths for zero-cost dispatch.

use crate::resilience::{CircuitBreakerMetrics, CircuitState};
use std::time::Duration;

/// Unified interface for all circuit breaker implementations.
///
/// Provides the core operations needed to protect a component:
/// - **Pre-flight check**: `should_allow()` — gate calls before attempting work
/// - **Recording**: `record_success()` / `record_failure()` — update state after work
/// - **Observability**: `state()`, `metrics()`, `is_healthy()`, `name()`
/// - **Emergency**: `force_open()`, `force_closed()`
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn CircuitBreakerBehavior`.
pub trait CircuitBreakerBehavior: Send + Sync + std::fmt::Debug {
    /// Get the component name this circuit breaker protects
    fn name(&self) -> &str;

    /// Get the current circuit state
    fn state(&self) -> CircuitState;

    /// Check if the circuit allows the next call.
    ///
    /// Returns `true` for Closed state, `true` for HalfOpen (limited), and
    /// `true` for Open only when the recovery timeout has elapsed (transitioning to HalfOpen).
    fn should_allow(&self) -> bool;

    /// Record a successful operation with its duration
    fn record_success(&self, duration: Duration);

    /// Record a failed operation with its duration
    fn record_failure(&self, duration: Duration);

    /// Check if the circuit breaker considers the component healthy
    fn is_healthy(&self) -> bool;

    /// Force the circuit to open state (emergency kill switch)
    fn force_open(&self);

    /// Force the circuit to closed state (emergency recovery)
    fn force_closed(&self);

    /// Get a metrics snapshot for observability
    fn metrics(&self) -> CircuitBreakerMetrics;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time proof that CircuitBreakerBehavior is object-safe
    fn _assert_object_safe(_: &dyn CircuitBreakerBehavior) {}
}
