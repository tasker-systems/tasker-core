//! Executor for the `persist` capability — structured write operations
//! against resource targets through the `OperationProvider` interface.
//!
//! The persist executor owns the full action pipeline:
//! config parse → jaq expression eval → get_persistable via OperationProvider →
//! persist() call → validate_success → result_shape.
//!
//! ## Config shape
//!
//! ```yaml
//! capability: persist
//! config:
//!   resource:
//!     ref: "orders-db"
//!     entity: orders
//!   mode: upsert              # insert | update | upsert | delete
//!   data:
//!     expression: "{id: .prev.order_id, total: .prev.computed_total}"
//!   identity:
//!     primary_key: ["id"]
//!   constraints:
//!     on_conflict: update      # update | skip | reject
//!   relationships:
//!     line_items:
//!       entity: order_line_items
//!       foreign_key: ["order_id"]
//!       references: ["id"]
//!       mode: insert
//!   validate_success:
//!     expression: ".affected_rows > 0"
//!   result_shape:
//!     expression: "{persisted_id: .id, timestamp: .created_at}"
//! ```
//!
//! ## Operation modes
//!
//! | Mode | SQL Equivalent | Semantics |
//! |------|---------------|-----------|
//! | `insert` | INSERT | Create new record(s). Fail on conflict. |
//! | `update` | UPDATE ... WHERE | Modify existing record(s) by PK. |
//! | `upsert` | INSERT ... ON CONFLICT | Create or update. Requires conflict key. |
//! | `delete` | DELETE ... WHERE | Remove record(s) by PK. |
//!
//! ## Architecture
//!
//! The executor calls `context.operations.get_persistable(resource_ref)` to obtain
//! a `PersistableResource` trait object. In tests, this is backed by
//! `InMemoryOperations` with zero I/O. In production (Phase 3+), it's backed by
//! a runtime adapter wrapping a secure handle.
//!
//! See `docs/composition-architecture/operation-shape-constraints.md` for design
//! constraints on what persist does and does not do.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::operations::{ConflictStrategy, OperationProvider, PersistConstraints};
use crate::types::{
    CapabilityError, CompositionEnvelope, ExecutionContext, TypedCapabilityExecutor,
};

/// Executor for the `persist` capability.
///
/// Evaluates jaq expressions to construct data, calls through `OperationProvider`
/// to execute the write, then optionally validates the result and shapes the output.
pub struct PersistExecutor {
    engine: ExpressionEngine,
    operations: Arc<dyn OperationProvider>,
}

impl std::fmt::Debug for PersistExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistExecutor")
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

impl PersistExecutor {
    /// Create a new `PersistExecutor` with the given expression engine and
    /// operation provider.
    pub fn new(engine: ExpressionEngine, operations: Arc<dyn OperationProvider>) -> Self {
        Self { engine, operations }
    }
}

// ---------------------------------------------------------------------------
// Typed config structs
// ---------------------------------------------------------------------------

/// Typed configuration for the `persist` capability.
#[derive(Debug, Deserialize)]
pub struct PersistConfig {
    /// Target resource — either a string ("orders") or structured ref.
    pub resource: ResourceTarget,

    /// Operation mode (default: insert).
    #[serde(default)]
    pub mode: PersistMode,

    /// jaq expression constructing the data to persist, evaluated against the
    /// composition envelope.
    pub data: ExpressionField,

    /// Identity declarations for targeting existing records.
    pub identity: Option<IdentityDeclaration>,

    /// Operational constraints (conflict resolution, idempotency).
    pub constraints: Option<ConstraintConfig>,

    /// One level of nested relationship declarations.
    pub relationships: Option<std::collections::HashMap<String, RelationshipDeclaration>>,

    /// Expression to verify the operation succeeded, evaluated against the
    /// persist result. Must evaluate to a truthy value.
    pub validate_success: Option<ExpressionField>,

    /// Expression to extract/reshape the result for downstream capabilities,
    /// evaluated against the persist result data.
    pub result_shape: Option<ExpressionField>,
}

/// Target resource — supports both simple string and structured forms.
///
/// Simple: `resource: "orders"`
/// Structured: `resource: { ref: "orders-db", entity: "orders" }`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ResourceTarget {
    /// Simple string form — used as both ref and entity.
    Simple(String),
    /// Structured form with explicit ref and entity.
    Structured {
        /// Resource reference name (used to look up the resource via OperationProvider).
        #[serde(rename = "ref")]
        resource_ref: String,
        /// Entity name (table, API endpoint, etc.).
        entity: String,
    },
}

impl ResourceTarget {
    /// The resource reference for looking up via OperationProvider.
    fn resource_ref(&self) -> &str {
        match self {
            Self::Simple(s) => s,
            Self::Structured { resource_ref, .. } => resource_ref,
        }
    }

    /// The entity name (table, API endpoint, etc.).
    fn entity(&self) -> &str {
        match self {
            Self::Simple(s) => s,
            Self::Structured { entity, .. } => entity,
        }
    }
}

/// The four persist operation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersistMode {
    /// Create new record(s). Fail on conflict.
    #[default]
    Insert,
    /// Modify existing record(s) identified by PK.
    Update,
    /// Create or update. Requires conflict key declaration.
    Upsert,
    /// Remove record(s) identified by PK.
    Delete,
}

/// A jaq expression field wrapper.
#[derive(Debug, Deserialize)]
pub struct ExpressionField {
    /// The jaq expression string.
    pub expression: String,
}

/// Identity declaration for targeting existing records.
#[derive(Debug, Deserialize)]
pub struct IdentityDeclaration {
    /// Primary key column(s) for record identification.
    pub primary_key: Vec<String>,
}

/// Constraint configuration for persist operations.
#[derive(Debug, Deserialize)]
pub struct ConstraintConfig {
    /// Conflict resolution strategy for upsert.
    pub on_conflict: Option<ConflictStrategy>,
    /// Keys for upsert conflict resolution.
    pub upsert_key: Option<Vec<String>>,
    /// Idempotency key for at-most-once semantics.
    pub idempotency_key: Option<String>,
}

/// Nested relationship declaration (one level only).
#[derive(Debug, Deserialize)]
pub struct RelationshipDeclaration {
    /// Child entity name.
    pub entity: String,
    /// Foreign key column(s) on the child entity.
    pub foreign_key: Vec<String>,
    /// Column(s) on the parent that the FK references.
    pub references: Vec<String>,
    /// Operation mode for the nested records.
    #[serde(default)]
    pub mode: PersistMode,
}

// ---------------------------------------------------------------------------
// TypedCapabilityExecutor impl
// ---------------------------------------------------------------------------

impl TypedCapabilityExecutor for PersistExecutor {
    type Config = PersistConfig;

    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &PersistConfig,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        // 1. Validate config consistency
        self.validate_mode_requirements(config)?;

        // 2. Evaluate the data expression against the composition envelope
        let data = self
            .engine
            .evaluate(&config.data.expression, envelope.raw())
            .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

        // 3. Build PersistConstraints from config
        let constraints = self.build_constraints(config);

        // 4. Get the persistable resource and call persist
        let entity = config.resource.entity().to_string();
        let resource_ref = config.resource.resource_ref().to_string();
        let persist_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let persistable = self
                    .operations
                    .get_persistable(&resource_ref)
                    .await
                    .map_err(|e| CapabilityError::ResourceNotFound(e.to_string()))?;

                persistable
                    .persist(&entity, data, &constraints)
                    .await
                    .map_err(|e| CapabilityError::Execution(e.to_string()))
            })
        })?;

        // 5. Build the raw result value for validation and shaping
        let raw_result = serde_json::json!({
            "data": persist_result.data,
            "affected_rows": persist_result.affected_count.unwrap_or(0),
            "affected_count": persist_result.affected_count,
        });

        // 6. Validate success if expression is provided
        if let Some(ref validate_success) = config.validate_success {
            let validation = self
                .engine
                .evaluate(&validate_success.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

            if !is_truthy(&validation) {
                return Err(CapabilityError::Execution(format!(
                    "persist validate_success failed: expression '{}' evaluated to falsy value",
                    validate_success.expression
                )));
            }
        }

        // 7. Apply result_shape if provided, otherwise return the raw result data
        if let Some(ref result_shape) = config.result_shape {
            self.engine
                .evaluate(&result_shape.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))
        } else {
            Ok(persist_result.data)
        }
    }

    fn capability_name(&self) -> &str {
        "persist"
    }
}

impl PersistExecutor {
    /// Validate that the config is consistent with the declared mode.
    fn validate_mode_requirements(&self, config: &PersistConfig) -> Result<(), CapabilityError> {
        match config.mode {
            PersistMode::Update | PersistMode::Delete => {
                if config.identity.is_none() {
                    return Err(CapabilityError::ConfigValidation(format!(
                        "persist mode '{:?}' requires an identity declaration with primary_key",
                        config.mode
                    )));
                }
            }
            PersistMode::Upsert => {
                if config.identity.is_none() {
                    return Err(CapabilityError::ConfigValidation(
                        "persist mode 'upsert' requires an identity declaration with primary_key"
                            .into(),
                    ));
                }
            }
            PersistMode::Insert => {
                // PK is optional for inserts (DB may auto-generate)
            }
        }

        // Validate relationship declarations
        if let Some(ref relationships) = config.relationships {
            for (name, rel) in relationships {
                if rel.foreign_key.len() != rel.references.len() {
                    return Err(CapabilityError::ConfigValidation(format!(
                        "relationship '{}': foreign_key and references must have the same length",
                        name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Build `PersistConstraints` from the config.
    fn build_constraints(&self, config: &PersistConfig) -> PersistConstraints {
        let mut constraints = PersistConstraints::default();

        if let Some(ref constraint_config) = config.constraints {
            constraints.on_conflict = constraint_config.on_conflict.clone();
            constraints.upsert_key = constraint_config.upsert_key.clone();
            constraints.idempotency_key = constraint_config.idempotency_key.clone();
        }

        // For upsert mode, fall back to identity.primary_key if no explicit upsert_key
        if config.mode == PersistMode::Upsert && constraints.upsert_key.is_none() {
            if let Some(ref identity) = config.identity {
                constraints.upsert_key = Some(identity.primary_key.clone());
            }
        }

        constraints
    }
}

/// Determine whether a value is "truthy" for validation purposes.
/// Follows jq truthiness: `false` and `null` are falsy, everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

#[cfg(test)]
mod tests;
