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

/// A category of action in the grammar.
///
/// Grammar categories define the *kind* of action (Acquire, Transform, Validate,
/// Persist, Emit) and declare what properties actions of this kind guarantee.
/// This is the extension point for organizations that need domain-specific
/// action categories.
///
/// Object-safe: suitable for `dyn` dispatch and build-from-source extensibility.
pub trait GrammarCategory: Send + Sync + fmt::Debug {
    /// Unique name of this grammar category (e.g., "Acquire", "Transform").
    fn name(&self) -> &str;

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
// Built-in grammar categories
// ---------------------------------------------------------------------------

/// Pure data transformation via jaq (jq) filters.
#[derive(Debug)]
pub struct TransformCategory;

impl GrammarCategory for TransformCategory {
    fn name(&self) -> &str {
        "Transform"
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

/// Schema validation and execution gating.
///
/// Covers both `validate` (JSON Schema trust boundary) and `assert`
/// (jaq boolean gate) capabilities.
#[derive(Debug)]
pub struct ValidateCategory;

impl GrammarCategory for ValidateCategory {
    fn name(&self) -> &str {
        "Validate"
    }

    fn description(&self) -> &str {
        "Assert invariants, validate schemas, gate execution"
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
            "description": "Config varies by capability: validate uses schema + coercion, assert uses filter + error"
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
#[derive(Debug)]
pub struct AcquireCategory;

impl GrammarCategory for AcquireCategory {
    fn name(&self) -> &str {
        "Acquire"
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
                "resource": { "type": "object" },
                "params": { "type": "string", "description": "jaq expression for parameters" },
                "constraints": { "type": "object" },
                "validate_success": { "type": "object" },
                "result_shape": { "type": "array" }
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
#[derive(Debug)]
pub struct PersistCategory;

impl GrammarCategory for PersistCategory {
    fn name(&self) -> &str {
        "Persist"
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
                "resource": { "type": "object" },
                "data": { "type": "string", "description": "jaq expression for data to persist" },
                "constraints": { "type": "object" },
                "validate_success": { "type": "object" },
                "result_shape": { "type": "array" }
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

/// Send notifications or events.
#[derive(Debug)]
pub struct EmitCategory;

impl GrammarCategory for EmitCategory {
    fn name(&self) -> &str {
        "Emit"
    }

    fn description(&self) -> &str {
        "Send notifications or events"
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
