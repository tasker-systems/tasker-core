use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::Validate;

use super::categories::{GrammarCategoryKind, MutationProfile};

/// A registered capability in the vocabulary.
///
/// Capabilities are the concrete, composable units that agents discover
/// and assemble into workflows. Each capability belongs to a grammar
/// category and declares its contracts via JSON Schema.
///
/// Every capability expresses a deterministic (action, resource, context) triple:
/// - **Action**: What to do — identified by `grammar_category` (Transform,
///   Validate, Assert, Acquire, Persist, Emit)
/// - **Resource**: The target upon which the action is effected
/// - **Context**: Configuration, constraints, success criteria, result shape
///
/// Input is always the composition context envelope (`.context`, `.deps`,
/// `.prev`, `.step`) — there is no per-capability input schema. Output shape
/// is declared per-invocation in the capability invocation's config (e.g., the
/// `output` field on a `transform` invocation), not on the capability declaration.
///
/// Retry and backoff are workflow-step-level concerns handled by the orchestration
/// layer (see `tasker-shared::models::core::task_template::RetryConfiguration`),
/// not the grammar layer.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CapabilityDeclaration {
    /// Unique identifier (e.g., "http_get", "postgres_upsert", "json_extract").
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    /// Which grammar category this belongs to. In the 6-capability model,
    /// each capability has its own category (1:1), so this also identifies
    /// the canonical action (Transform, Validate, Assert, Acquire, Persist, Emit).
    pub grammar_category: GrammarCategoryKind,

    /// Human-readable description for agent discoverability.
    pub description: String,

    /// JSON Schema: configuration parameters for this capability.
    ///
    /// For `transform`: `output` (JSON Schema contract) + `filter` (jaq expression).
    /// For `validate`: JSON Schema + coercion/failure config.
    /// For `assert`: `filter` (jaq boolean) + `error` message.
    /// For action capabilities (`persist`/`acquire`/`emit`): typed envelope with
    /// resource, data/params/payload, constraints, success_criteria, result_shape.
    pub config_schema: Value,

    /// Concrete mutation profile (must be compatible with grammar category).
    pub mutation_profile: MutationProfile,

    /// Tags for capability discovery (e.g., `["http", "rest", "api"]`).
    pub tags: Vec<String>,

    /// Semantic version of this capability declaration (e.g., "1.0.0").
    #[validate(length(min = 1, max = 64))]
    pub version: String,
}
