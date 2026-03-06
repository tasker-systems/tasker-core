use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::declaration::CapabilityDeclaration;
use super::validation::{CompositionConstraint, ValidationFinding};

/// How a grammar category relates to external state mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MutationProfile {
    /// Never mutates external state. Safe to re-execute freely.
    NonMutating,

    /// Mutates external state. Requires checkpoint tracking in compositions
    /// with multiple mutations.
    Mutating {
        /// Whether this category supports idempotency keys to prevent
        /// duplicate mutations on retry.
        supports_idempotency_key: bool,
    },

    /// Mutation behavior depends on configuration. The capability declaration
    /// must specify its concrete mutation profile.
    ConfigDependent,
}

/// How a grammar category relates to idempotency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdempotencyProfile {
    /// Inherently idempotent — safe to re-execute with same inputs,
    /// produces same outputs.
    Inherent,

    /// Idempotent when provided with an idempotency key. The system
    /// generates or accepts a key and ensures at-most-once execution.
    WithKey,

    /// The capability must declare its own idempotency strategy.
    CapabilityDefined,
}

/// The finite set of grammar categories — one per capability in the
/// 6-capability model.
///
/// Each variant corresponds to a capability with a distinct execution model:
///
/// - **Transform**: Pure data transformation via jaq filter + output schema.
/// - **Validate**: Trust boundary gate — JSON Schema conformance with coercion
///   modes, attribute filtering, and failure mechanics. Not a jaq concern.
/// - **Assert**: Execution gate — jaq boolean filter that passes or fails.
///   Produces no new data; `.prev` passes through unchanged on success.
/// - **Acquire**: Side-effecting read from an external system via typed
///   resource envelope.
/// - **Persist**: Side-effecting write to an external system via typed
///   resource envelope.
/// - **Emit**: Side-effecting domain event publication via typed envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrammarCategoryKind {
    /// Pure data transformation via jaq (jq) filters.
    Transform,

    /// Trust boundary gate — JSON Schema validation with coercion modes,
    /// attribute filtering, and failure mechanics.
    Validate,

    /// Execution gate — jaq boolean filter that gates whether the
    /// composition continues. Produces no new data.
    Assert,

    /// Fetch data from external sources.
    Acquire,

    /// Write state to external systems.
    Persist,

    /// Send notifications or events.
    Emit,
}

impl fmt::Display for GrammarCategoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transform => write!(f, "Transform"),
            Self::Validate => write!(f, "Validate"),
            Self::Assert => write!(f, "Assert"),
            Self::Acquire => write!(f, "Acquire"),
            Self::Persist => write!(f, "Persist"),
            Self::Emit => write!(f, "Emit"),
        }
    }
}

/// A category of action in the grammar.
///
/// Grammar categories define the *kind* of action and declare what properties
/// actions of this kind guarantee. Each of the 6 capabilities has its own
/// category with a distinct execution model.
///
/// Object-safe: suitable for `dyn` dispatch and build-from-source extensibility.
pub trait GrammarCategory: Send + Sync + fmt::Debug {
    /// Unique name of this grammar category (e.g., "Acquire", "Transform").
    fn name(&self) -> &str;

    /// The enum variant for this category.
    fn kind(&self) -> GrammarCategoryKind;

    /// Human-readable description for agent discoverability.
    fn description(&self) -> &str;

    /// The mutation profile of this category.
    fn mutation_profile(&self) -> MutationProfile;

    /// Whether actions of this category are inherently idempotent,
    /// or require explicit idempotency strategies.
    fn idempotency(&self) -> IdempotencyProfile;

    /// Whether capabilities in this category require checkpoint support
    /// when used in compositions with multiple mutations.
    fn requires_checkpointing(&self) -> bool;

    /// JSON Schema for configuration that capabilities in this category accept.
    fn config_schema(&self) -> Value;

    /// Validate that a capability declaration is compatible with this category's
    /// constraints. Called when a capability is registered against this category.
    fn validate_capability_declaration(
        &self,
        declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding>;

    /// Additional composition rules specific to this category.
    fn composition_constraints(&self) -> Vec<CompositionConstraint> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in grammar categories — one per capability
// ---------------------------------------------------------------------------

/// Pure data transformation via jaq (jq) filters.
///
/// Config model: `output` (JSON Schema declaring output shape) + `filter`
/// (jaq expression producing the output). The single primitive that replaced
/// reshape, compute, evaluate, and evaluate_rules.
#[derive(Debug)]
pub struct TransformCategory;

impl GrammarCategory for TransformCategory {
    fn name(&self) -> &str {
        "Transform"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Transform
    }

    fn description(&self) -> &str {
        "Pure data transformation via jaq (jq) filters"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::NonMutating
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::Inherent
    }

    fn requires_checkpointing(&self) -> bool {
        false
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "output": { "type": "object", "description": "JSON Schema declaring output shape" },
                "filter": { "type": "string", "description": "jaq expression producing the output" }
            },
            "required": ["output", "filter"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}

/// Trust boundary gate — JSON Schema validation with coercion modes,
/// attribute filtering, and failure mechanics.
///
/// Validate sits at the boundary where external or untrusted data enters
/// a composition. It checks conformance against a JSON Schema, optionally
/// coerces types (e.g. `"1.00"` → `1.0`, string dates → date types),
/// filters attributes, and declares failure behavior.
///
/// This is a schema engine concern, not a jaq concern. The execution model
/// is fundamentally different from `assert` (which evaluates a jaq boolean).
#[derive(Debug)]
pub struct ValidateCategory;

impl GrammarCategory for ValidateCategory {
    fn name(&self) -> &str {
        "Validate"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Validate
    }

    fn description(&self) -> &str {
        "Trust boundary gate — JSON Schema conformance with coercion and failure mechanics"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::NonMutating
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::Inherent
    }

    fn requires_checkpointing(&self) -> bool {
        false
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "schema": { "type": "object", "description": "JSON Schema to validate against" },
                "coercion": {
                    "type": "string",
                    "enum": ["strict", "permissive"],
                    "description": "Whether to coerce types (string-to-number, string-to-date, etc.)"
                },
                "on_failure": {
                    "type": "string",
                    "enum": ["fail", "filter", "default"],
                    "description": "Behavior when validation fails"
                }
            },
            "required": ["schema"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}

/// Execution gate — jaq boolean filter that gates whether the composition
/// continues.
///
/// Assert evaluates a jaq expression that must produce a boolean. On `true`,
/// the composition continues and `.prev` passes through unchanged. On `false`,
/// the step fails with the declared error message.
///
/// Assert produces no new data — it is semantically distinct from `transform`
/// (which produces data) and from `validate` (which does schema conformance
/// checking, not jaq evaluation).
#[derive(Debug)]
pub struct AssertCategory;

impl GrammarCategory for AssertCategory {
    fn name(&self) -> &str {
        "Assert"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Assert
    }

    fn description(&self) -> &str {
        "Execution gate — jaq boolean filter that passes or fails the step"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::NonMutating
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::Inherent
    }

    fn requires_checkpointing(&self) -> bool {
        false
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "jaq boolean expression" },
                "error": { "type": "string", "description": "Error message when assertion fails" }
            },
            "required": ["filter", "error"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}

/// Fetch data from external sources.
///
/// Typed resource envelope for targeting (API endpoint, database query, etc.)
/// with jaq `params` filter for parameter mapping and `result_filter` for
/// shaping the response.
#[derive(Debug)]
pub struct AcquireCategory;

impl GrammarCategory for AcquireCategory {
    fn name(&self) -> &str {
        "Acquire"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Acquire
    }

    fn description(&self) -> &str {
        "Fetch data from external sources"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::NonMutating
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::Inherent
    }

    fn requires_checkpointing(&self) -> bool {
        false
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "resource": { "type": "object", "description": "Typed resource targeting (type, endpoint, method, etc.)" },
                "params": { "type": "string", "description": "jaq expression producing parameters for the request" },
                "constraints": { "type": "object", "description": "Operational constraints (timeout_ms, retries, etc.)" },
                "success_criteria": {
                    "type": "object",
                    "description": "Criteria for determining whether the external operation succeeded (e.g., { status: { in: [200] } })"
                },
                "result_shape": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Field paths to include from the external response (e.g., [\"data.sales_records\"]). Unlike transform's 'output' (a full JSON Schema contract), this is a field-path filter on the external system's response."
                }
            },
            "required": ["resource"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}

/// Write state to external systems.
///
/// Typed resource envelope for targeting with jaq `data` filter for mapping
/// the data to persist. Supports constraints (unique keys, ID patterns),
/// success criteria, and result shape declarations.
#[derive(Debug)]
pub struct PersistCategory;

impl GrammarCategory for PersistCategory {
    fn name(&self) -> &str {
        "Persist"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Persist
    }

    fn description(&self) -> &str {
        "Write state to external systems"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::Mutating {
            supports_idempotency_key: true,
        }
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::WithKey
    }

    fn requires_checkpointing(&self) -> bool {
        true
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "resource": { "type": "object", "description": "Typed resource targeting (type, entity, etc.)" },
                "data": { "type": "string", "description": "jaq expression producing the data to persist" },
                "constraints": { "type": "object", "description": "Operational constraints (unique_key, id_pattern, batch_insert, etc.)" },
                "success_criteria": {
                    "type": "object",
                    "description": "Criteria for determining whether the persist succeeded (e.g., { order_id: { type: string, required: true } })"
                },
                "result_shape": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Field paths to include from the operation's response (e.g., [\"order_id\", \"order_ref\", \"created_at\"]). Unlike transform's 'output' (a full JSON Schema contract), this is a field-path filter on the external system's response."
                }
            },
            "required": ["resource", "data"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}

/// Fire domain events.
///
/// Typed envelope for event configuration (name, version, delivery mode)
/// with jaq `payload` filter for mapping event data. Optionally declares
/// a JSON Schema for the event payload and a jaq boolean `condition` for
/// conditional emission.
#[derive(Debug)]
pub struct EmitCategory;

impl GrammarCategory for EmitCategory {
    fn name(&self) -> &str {
        "Emit"
    }

    fn kind(&self) -> GrammarCategoryKind {
        GrammarCategoryKind::Emit
    }

    fn description(&self) -> &str {
        "Fire domain events"
    }

    fn mutation_profile(&self) -> MutationProfile {
        MutationProfile::Mutating {
            supports_idempotency_key: true,
        }
    }

    fn idempotency(&self) -> IdempotencyProfile {
        IdempotencyProfile::WithKey
    }

    fn requires_checkpointing(&self) -> bool {
        true
    }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "event_name": { "type": "string" },
                "event_version": { "type": "string" },
                "delivery_mode": { "type": "string", "enum": ["durable", "fast"] },
                "payload": { "type": "string", "description": "jaq expression for event payload" },
                "schema": { "type": "object", "description": "JSON Schema for event payload" },
                "condition": { "type": "string", "description": "jaq boolean expression for conditional emission" }
            },
            "required": ["event_name", "payload"]
        })
    }

    fn validate_capability_declaration(
        &self,
        _declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding> {
        Vec::new()
    }
}
