//! Test for TOML-based circuit breaker configuration

#[cfg(test)]
mod tests {
    use crate::config::tasker::{
        CircuitBreakerComponentConfig, CircuitBreakerConfig, CircuitBreakerDefaultConfig,
        ComponentCircuitBreakerConfigs, GlobalCircuitBreakerSettings,
    };
    use crate::resilience::CircuitBreaker;

    /// Test that the TOML-based configuration correctly creates circuit breakers
    #[test]
    fn test_toml_based_circuit_breaker_configuration() {
        let component_configs = ComponentCircuitBreakerConfigs {
            task_readiness: CircuitBreakerComponentConfig {
                failure_threshold: 3,
                success_threshold: 2,
            },
            messaging: CircuitBreakerComponentConfig {
                failure_threshold: 2,
                success_threshold: 1,
            },
            web: CircuitBreakerComponentConfig {
                failure_threshold: 5,
                success_threshold: 2,
            },
            cache: CircuitBreakerComponentConfig {
                failure_threshold: 5,
                success_threshold: 2,
            },
        };

        let toml_config = CircuitBreakerConfig {
            global_settings: GlobalCircuitBreakerSettings {
                metrics_collection_interval_seconds: 15,
                min_state_transition_interval_seconds: 0.5,
            },
            default_config: CircuitBreakerDefaultConfig {
                failure_threshold: 4,
                timeout_seconds: 20,
                success_threshold: 2,
            },
            component_configs,
        };

        // Create circuit breakers directly from config (same pattern as SystemContext)
        let task_readiness_config = toml_config
            .config_for_component("task_readiness")
            .to_resilience_config_with_timeout(toml_config.default_config.timeout_seconds);
        let task_readiness_breaker =
            CircuitBreaker::new("task_readiness".to_string(), task_readiness_config);

        let messaging_config = toml_config
            .config_for_component("messaging")
            .to_resilience_config_with_timeout(toml_config.default_config.timeout_seconds);
        let messaging_breaker = CircuitBreaker::new("messaging".to_string(), messaging_config);

        // Unknown components fall back to default config
        let unknown_config = toml_config
            .config_for_component("unknown_component")
            .to_resilience_config_with_timeout(toml_config.default_config.timeout_seconds);
        let unknown_breaker = CircuitBreaker::new("unknown_component".to_string(), unknown_config);

        // Verify circuit breakers were created with correct names
        assert_eq!(task_readiness_breaker.name(), "task_readiness");
        assert_eq!(messaging_breaker.name(), "messaging");
        assert_eq!(unknown_breaker.name(), "unknown_component");

        // All circuit breakers should start healthy
        assert!(task_readiness_breaker.is_healthy());
        assert!(messaging_breaker.is_healthy());
        assert!(unknown_breaker.is_healthy());
    }

    /// Test that environment-specific configurations produce correctly configured breakers
    #[test]
    fn test_environment_specific_toml_configuration() {
        // Simulate test environment configuration with faster timeouts
        let component_configs = ComponentCircuitBreakerConfigs {
            task_readiness: CircuitBreakerComponentConfig {
                failure_threshold: 1,
                success_threshold: 1,
            },
            messaging: CircuitBreakerComponentConfig {
                failure_threshold: 1,
                success_threshold: 1,
            },
            web: CircuitBreakerComponentConfig {
                failure_threshold: 1,
                success_threshold: 1,
            },
            cache: CircuitBreakerComponentConfig {
                failure_threshold: 1,
                success_threshold: 1,
            },
        };

        let toml_config = CircuitBreakerConfig {
            global_settings: GlobalCircuitBreakerSettings {
                metrics_collection_interval_seconds: 1,
                min_state_transition_interval_seconds: 0.01,
            },
            default_config: CircuitBreakerDefaultConfig {
                failure_threshold: 1,
                timeout_seconds: 1,
                success_threshold: 1,
            },
            component_configs,
        };

        let test_config = toml_config
            .config_for_component("test_component")
            .to_resilience_config_with_timeout(toml_config.default_config.timeout_seconds);
        let test_breaker = CircuitBreaker::new("test_component".to_string(), test_config);

        assert_eq!(test_breaker.name(), "test_component");

        // Circuit breaker should start in closed state and be healthy
        let metrics = test_breaker.metrics();
        assert_eq!(metrics.total_calls, 0);
        assert_eq!(metrics.failure_count, 0);
    }
}
