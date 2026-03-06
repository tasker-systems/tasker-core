use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::categories::MutationProfile;

/// A registered capability in the vocabulary.
///
/// Capabilities are the concrete, composable units that agents discover
/// and assemble into workflows. Each capability belongs to a grammar
/// category and declares its contracts via JSON Schema.
///
/// Every capability expresses a deterministic (action, resource, context) triple:
/// - **Action**: What to do (transform, validate, assert, persist, acquire, emit)
/// - **Resource**: The target upon which the action is effected
/// - **Context**: Configuration, constraints, success validation, result shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDeclaration {
    /// Unique identifier (e.g., "http_get", "postgres_upsert", "json_extract").
    pub name: String,

    /// The canonical action this capability performs (e.g., "transform", "validate").
    pub action: String,

    /// Which grammar category this belongs to (e.g., "Acquire", "Transform").
    pub grammar_category: String,

    /// Human-readable description for agent discoverability.
    pub description: String,

    /// JSON Schema: what this capability accepts as input.
    pub input_schema: Value,

    /// JSON Schema: what this capability produces as output.
    pub output_schema: Value,

    /// JSON Schema: configuration parameters for this capability.
    ///
    /// For `transform`: `output` (JSON Schema) + `filter` (jaq expression).
    /// For `validate`: JSON Schema + coercion/failure config.
    /// For `assert`: `filter` (jaq boolean) + `error` message.
    /// For action capabilities: typed envelope with resource, data/params/payload,
    /// constraints, validate_success, result_shape.
    pub config_schema: Value,

    /// Concrete mutation profile (must be compatible with grammar category).
    pub mutation_profile: MutationProfile,

    /// Retry behavior specific to this capability.
    pub retry_profile: RetryProfile,

    /// Tags for capability discovery (e.g., `["http", "rest", "api"]`).
    pub tags: Vec<String>,

    /// Version of this capability declaration.
    pub version: String,
}

/// Retry behavior for a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryProfile {
    /// Whether the capability supports automatic retry.
    pub retriable: bool,

    /// Maximum number of retry attempts.
    pub max_attempts: u32,

    /// Backoff strategy for retries.
    pub backoff: BackoffStrategy,
}

impl Default for RetryProfile {
    fn default() -> Self {
        Self {
            retriable: true,
            max_attempts: 3,
            backoff: BackoffStrategy::Exponential {
                initial_ms: 100,
                multiplier: 2.0,
                max_ms: 10_000,
            },
        }
    }
}

/// Backoff strategy for retry behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BackoffStrategy {
    /// Fixed delay between retries.
    Fixed {
        /// Delay in milliseconds.
        delay_ms: u64,
    },

    /// Exponential backoff with configurable parameters.
    Exponential {
        /// Initial delay in milliseconds.
        initial_ms: u64,
        /// Multiplier applied after each attempt.
        multiplier: f64,
        /// Maximum delay in milliseconds.
        max_ms: u64,
    },

    /// No delay between retries.
    None,
}
