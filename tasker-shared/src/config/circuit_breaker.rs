//! # Circuit Breaker Configuration Adapters
//!
//! Provides conversion methods from canonical config (tasker.rs) to resilience module types.
//! TAS-61: All TOML configs now in tasker.rs - this module only provides type conversions.

use std::time::Duration;

// Re-export canonical types
pub use crate::config::tasker::{
    CircuitBreakerComponentConfig, CircuitBreakerConfig, GlobalCircuitBreakerSettings,
};

// Type alias for backward compatibility (legacy name)
pub type CircuitBreakerGlobalSettings = GlobalCircuitBreakerSettings;

impl CircuitBreakerComponentConfig {
    /// Convert to resilience module's format
    ///
    /// TAS-221: timeout_seconds removed from per-component config (only failure/success thresholds used).
    /// Uses provided default timeout for the Duration conversion.
    pub fn to_resilience_config_with_timeout(
        &self,
        default_timeout_seconds: u32,
    ) -> crate::resilience::config::CircuitBreakerConfig {
        crate::resilience::config::CircuitBreakerConfig {
            failure_threshold: self.failure_threshold,
            timeout: Duration::from_secs(default_timeout_seconds as u64),
            success_threshold: self.success_threshold,
        }
    }
}

impl GlobalCircuitBreakerSettings {
    /// Convert to resilience module's format
    ///
    /// TAS-221: max_circuit_breakers removed from V2 config (not enforced at runtime).
    /// Uses hardcoded default of 50 for the resilience module conversion.
    pub fn to_resilience_config(&self) -> crate::resilience::config::GlobalCircuitBreakerSettings {
        crate::resilience::config::GlobalCircuitBreakerSettings {
            max_circuit_breakers: 50, // TAS-221: hardcoded default, was never enforced
            metrics_collection_interval: Duration::from_secs(
                self.metrics_collection_interval_seconds as u64,
            ),
            min_state_transition_interval: Duration::from_secs_f64(
                self.min_state_transition_interval_seconds,
            ),
        }
    }
}
