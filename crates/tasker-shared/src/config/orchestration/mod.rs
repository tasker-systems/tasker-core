//! # Orchestration Configuration
//!
//! Configuration types for the orchestration system that coordinates task and step execution.
//!
//! ## Overview
//!
//! This module provides configuration for orchestration behavior including:
//! - **Step Enqueueing**: Batch sizes, delays, and timeouts for step queue operations
//! - **Result Processing**: Queue names, batch sizes, and retry policies
//! - **Batch Processing**: Configuration for batch workflow processing
//! - **Event Systems**: Orchestration-specific event system settings
//!
//! ## Structure
//!
//! ```text
//! orchestration/
//! ├── mod.rs                      # OrchestrationConfig
//! ├── step_enqueuer.rs            # StepEnqueuerConfig
//! ├── step_result_processor.rs    # StepResultProcessorConfig
//! ├── batch_processing.rs         # BatchProcessingConfig
//! ├── event_systems.rs            # OrchestrationEventSystemConfig
//! └── task_claim_step_enqueuer.rs # TaskClaimStepEnqueuerConfig
//! ```
//!
//! ## Configuration Loading
//!
//! Orchestration configuration is loaded from `config/tasker/base/orchestration.toml`
//! and environment-specific overrides in `config/tasker/environments/{env}/orchestration.toml`.
//!
//! ## Example
//!
//! ```toml
//! [orchestration]
//! enable_performance_logging = false
//!
//! [orchestration.web]
//! enabled = true
//! host = "0.0.0.0"
//! port = 3000
//! ```

use serde::{Deserialize, Serialize};

// TAS-61 Phase 6C: decision_points moved to tasker::DecisionPointsConfig
pub mod task_claim_step_enqueuer;
pub use task_claim_step_enqueuer::TaskClaimStepEnqueuerConfig;
pub mod step_enqueuer;
pub use step_enqueuer::StepEnqueuerConfig;
pub mod step_result_processor;
pub use step_result_processor::StepResultProcessorConfig;
pub mod batch_processing;
pub use batch_processing::BatchProcessingConfig;
pub mod event_systems;
pub use crate::config::web::WebConfig;
pub use event_systems::OrchestrationEventSystemConfig;

/// Orchestration system configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrchestrationConfig {
    #[serde(default)]
    pub enable_performance_logging: bool,
    /// Web API configuration
    #[serde(default)]
    pub web: WebConfig,
}

impl OrchestrationConfig {
    /// Get web configuration with fallback to defaults
    pub fn web_config(&self) -> WebConfig {
        self.web.clone()
    }

    /// Check if web API is enabled
    pub fn web_enabled(&self) -> bool {
        self.web.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestration_config_default_values() {
        let config = OrchestrationConfig::default();
        assert!(!config.enable_performance_logging);
    }

    #[test]
    fn test_orchestration_config_web_config() {
        let config = OrchestrationConfig::default();
        let web = config.web_config();
        // web_config returns a clone of the web field
        assert_eq!(web.enabled, config.web.enabled);
    }

    #[test]
    fn test_orchestration_config_web_enabled() {
        let mut config = OrchestrationConfig::default();
        // Default web config has enabled field from WebConfig::default()
        let default_enabled = config.web_enabled();

        // Verify web_enabled reflects the web.enabled field
        config.web.enabled = !default_enabled;
        assert_eq!(config.web_enabled(), !default_enabled);
    }
}
