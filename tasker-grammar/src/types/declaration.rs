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
///
/// Retry and backoff are step-level concerns handled by the orchestration layer
/// (see `tasker-shared::models::core::task_template::RetryConfiguration`), not
/// the grammar layer. Capabilities declare their mutation and idempotency
/// profiles so the orchestration layer can make informed retry decisions.
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

    /// Tags for capability discovery (e.g., `["http", "rest", "api"]`).
    pub tags: Vec<String>,

    /// Version of this capability declaration.
    pub version: String,
}
