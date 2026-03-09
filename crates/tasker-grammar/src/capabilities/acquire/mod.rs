//! Executor for the `acquire` capability — declarative read operations
//! against resource targets through the `OperationProvider` interface.
//!
//! The acquire executor owns the full action pipeline:
//! config parse → jaq expression eval → get_acquirable via OperationProvider →
//! acquire() call → validate_success → result_shape.
//!
//! ## Config shape
//!
//! ```yaml
//! capability: acquire
//! config:
//!   resource:
//!     ref: "orders-db"
//!     entity: orders
//!   select:
//!     columns: ["id", "customer_id", "total", "status", "created_at"]
//!     include:
//!       customer:
//!         entity: customers
//!         foreign_key: ["customer_id"]
//!         references: ["id"]
//!         columns: ["name", "email"]
//!   filter:
//!     status:
//!       eq: "pending"
//!     created_at:
//!       gte: "2026-01-01"
//!   params:
//!     expression: "{customer_id: .context.customer_id}"
//!   constraints:
//!     limit: 100
//!     offset: 0
//!     timeout_ms: 5000
//!   validate_success:
//!     expression: ".total_count > 0"
//!   result_shape:
//!     expression: "{name: (.data[0]).name, tier: (.data[0]).tier}"
//! ```
//!
//! ## Declarative filter operators
//!
//! A fixed set of operators for static filtering:
//! `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`, `is_null`, `like`.
//!
//! Static filters are declared in `filter`. Dynamic values come through
//! `params.expression` — a jaq expression evaluated against the composition
//! envelope that produces a JSON object merged into the query parameters.
//!
//! ## Architecture
//!
//! The executor calls `context.operations.get_acquirable(resource_ref)` to obtain
//! an `AcquirableResource` trait object. In tests, this is backed by
//! `InMemoryOperations` with zero I/O. In production (Phase 3+), it's backed by
//! a runtime adapter wrapping a secure handle.
//!
//! ## Acquire result envelope
//!
//! After the acquire operation completes, the executor constructs an **acquire
//! result envelope** — a structured JSON value that is passed as input to both
//! the `validate_success` and `result_shape` jaq expressions. This envelope has
//! a fixed schema:
//!
//! ```json
//! {
//!   "data": [ ... ],        // The records returned by the acquire operation
//!   "total_count": 42,      // Total records available (for pagination), null if unknown
//!   "row_count": 10         // Number of records in this response (integer, always present)
//! }
//! ```
//!
//! | Field | Type | Description |
//! |-------|------|-------------|
//! | `data` | `array` | The records returned by `AcquirableResource::acquire()`. |
//! | `total_count` | `integer \| null` | Total count of matching records across all pages. Null if the backend doesn't support total counts. |
//! | `row_count` | `integer` | Count of records in the current response (length of `data`). Always present. |
//!
//! When **no `result_shape`** is provided, the executor returns `data` directly
//! (not the full envelope). When `result_shape` is provided, its expression
//! receives the full envelope and can select from any field:
//!
//! ```yaml
//! result_shape:
//!   expression: "{records: .data, page_info: {count: .row_count, total: .total_count}}"
//! ```
//!
//! See `docs/composition-architecture/operation-shape-constraints.md` for design
//! constraints on what acquire does and does not do.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::operations::{AcquireConstraints, OperationProvider};
use crate::types::{
    CapabilityError, CompositionEnvelope, ExecutionContext, TypedCapabilityExecutor,
};

/// Executor for the `acquire` capability.
///
/// Evaluates jaq expressions to construct query parameters, calls through
/// `OperationProvider` to execute the read, then optionally validates the
/// result and shapes the output.
pub struct AcquireExecutor {
    engine: ExpressionEngine,
    operations: Arc<dyn OperationProvider>,
}

impl std::fmt::Debug for AcquireExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcquireExecutor")
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

impl AcquireExecutor {
    /// Create a new `AcquireExecutor` with the given expression engine and
    /// operation provider.
    pub fn new(engine: ExpressionEngine, operations: Arc<dyn OperationProvider>) -> Self {
        Self { engine, operations }
    }
}

// ---------------------------------------------------------------------------
// Typed config structs
// ---------------------------------------------------------------------------

/// Typed configuration for the `acquire` capability.
#[derive(Debug, Deserialize)]
pub struct AcquireConfig {
    /// Target resource — either a string ("customer_profile") or structured ref.
    pub resource: ResourceTarget,

    /// Column selection and optional relationship includes.
    pub select: Option<SelectConfig>,

    /// Static declarative filters using the fixed operator set.
    pub filter: Option<Value>,

    /// jaq expression constructing dynamic query parameters, evaluated against
    /// the composition envelope.
    pub params: Option<ExpressionField>,

    /// Operational constraints (limit, offset, timeout, retry).
    pub constraints: Option<ConstraintConfig>,

    /// Expression to verify the acquisition succeeded, evaluated against the
    /// acquire result envelope. Must evaluate to a truthy value.
    pub validate_success: Option<ExpressionField>,

    /// Expression to extract/reshape the result for downstream capabilities,
    /// evaluated against the acquire result envelope.
    pub result_shape: Option<ExpressionField>,
}

/// Target resource — supports both simple string and structured forms.
///
/// Simple: `resource: "customer_profile"`
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

/// Column selection and optional relationship includes.
#[derive(Debug, Deserialize)]
pub struct SelectConfig {
    /// Columns to select from the primary entity.
    pub columns: Option<Vec<String>>,

    /// Related entities to include (one level of joins/subqueries).
    pub include: Option<std::collections::HashMap<String, IncludeRelationship>>,
}

/// A relationship to include in the acquire result.
#[derive(Debug, Deserialize)]
pub struct IncludeRelationship {
    /// Related entity name.
    pub entity: String,
    /// Foreign key column(s) defining the relationship.
    pub foreign_key: Vec<String>,
    /// Column(s) on the parent that the FK references.
    pub references: Vec<String>,
    /// Columns to select from the related entity.
    pub columns: Option<Vec<String>>,
}

/// A jaq expression field wrapper.
#[derive(Debug, Deserialize)]
pub struct ExpressionField {
    /// The jaq expression string.
    pub expression: String,
}

/// Constraint configuration for acquire operations.
#[derive(Debug, Deserialize)]
pub struct ConstraintConfig {
    /// Maximum number of records to return.
    pub limit: Option<u64>,
    /// Pagination offset.
    pub offset: Option<u64>,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Retry policy.
    pub retry: Option<RetryConfig>,
}

/// Retry policy configuration.
#[derive(Debug, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: Option<u32>,
    /// Backoff interval in milliseconds.
    pub backoff_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// TypedCapabilityExecutor impl
// ---------------------------------------------------------------------------

impl TypedCapabilityExecutor for AcquireExecutor {
    type Config = AcquireConfig;

    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &AcquireConfig,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        // 1. Validate config consistency
        self.validate_config_consistency(config)?;

        // 2. Evaluate the params expression against the composition envelope
        let params = if let Some(ref params_config) = config.params {
            self.engine
                .evaluate(&params_config.expression, envelope.raw())
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?
        } else {
            Value::Object(serde_json::Map::new())
        };

        // 3. Build AcquireConstraints from config
        let constraints = self.build_constraints(config);

        // 4. Get the acquirable resource and call acquire
        let entity = config.resource.entity().to_string();
        let resource_ref = config.resource.resource_ref().to_string();
        let acquire_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let acquirable = self
                    .operations
                    .get_acquirable(&resource_ref)
                    .await
                    .map_err(|e| CapabilityError::ResourceNotFound(e.to_string()))?;

                acquirable
                    .acquire(&entity, params, &constraints)
                    .await
                    .map_err(|e| CapabilityError::Execution(e.to_string()))
            })
        })?;

        // 5. Build the acquire result envelope for validate_success and result_shape.
        //
        // This envelope is the input to both jaq expressions. Its schema is documented
        // in the module-level docs under "Acquire result envelope". Any changes to this
        // structure are a contract change — update the module docs and all tests that
        // reference these fields.
        //
        // Fields:
        //   .data         — array of records returned by AcquirableResource::acquire()
        //   .total_count  — nullable total count for pagination
        //   .row_count    — integer count of records in this response
        let row_count = if let Value::Array(ref arr) = acquire_result.data {
            arr.len() as u64
        } else {
            1 // Non-array data counts as 1 record
        };

        let raw_result = serde_json::json!({
            "data": acquire_result.data,
            "total_count": acquire_result.total_count,
            "row_count": row_count,
        });

        // 6. Validate success if expression is provided
        if let Some(ref validate_success) = config.validate_success {
            let validation = self
                .engine
                .evaluate(&validate_success.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

            if !is_truthy(&validation) {
                return Err(CapabilityError::Execution(format!(
                    "acquire validate_success failed: expression '{}' evaluated to falsy value",
                    validate_success.expression
                )));
            }
        }

        // 7. Apply result_shape if provided, otherwise return the raw data
        if let Some(ref result_shape) = config.result_shape {
            self.engine
                .evaluate(&result_shape.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))
        } else {
            Ok(acquire_result.data)
        }
    }

    fn capability_name(&self) -> &str {
        "acquire"
    }
}

impl AcquireExecutor {
    /// Validate that the config is internally consistent.
    fn validate_config_consistency(&self, config: &AcquireConfig) -> Result<(), CapabilityError> {
        // Validate include relationship declarations
        if let Some(ref select) = config.select {
            if let Some(ref includes) = select.include {
                for (name, rel) in includes {
                    if rel.foreign_key.len() != rel.references.len() {
                        return Err(CapabilityError::ConfigValidation(format!(
                            "include relationship '{}': foreign_key and references must have the same length",
                            name
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Build `AcquireConstraints` from the config.
    fn build_constraints(&self, config: &AcquireConfig) -> AcquireConstraints {
        let mut constraints = AcquireConstraints::default();

        if let Some(ref constraint_config) = config.constraints {
            constraints.limit = constraint_config.limit;
            constraints.offset = constraint_config.offset;
            constraints.timeout_ms = constraint_config.timeout_ms;
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
