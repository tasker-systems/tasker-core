//! Executor for the `emit` capability — domain event publication
//! through the `OperationProvider` interface.
//!
//! The emit executor owns the full action pipeline:
//! config parse → condition eval → payload expression eval → get_emittable
//! via OperationProvider → emit() call → result construction.
//!
//! ## Config shape
//!
//! ```yaml
//! capability: emit
//! config:
//!   event_name: "order.confirmed"
//!   event_version: "1.0"
//!   delivery_mode: async       # async | sync (default: async)
//!   resource: "event-bus"      # optional: resource ref for OperationProvider lookup
//!   condition:
//!     expression: ".prev.payment_status == \"captured\""
//!   payload:
//!     expression: "{order_id: .prev.order_id, total: .prev.total, customer_id: .deps.customer_profile.id}"
//!   metadata:
//!     correlation_id:
//!       expression: ".context.request_id"
//!     idempotency_key:
//!       expression: "\"order-confirmed-\" + (.prev.order_id | tostring)"
//!   validate_success:
//!     expression: ".confirmed"
//!   result_shape:
//!     expression: "{event_id: .data.message_id, event_name: .event_name}"
//! ```
//!
//! ## Domain events, not notifications
//!
//! The `emit` capability fires **domain events** — structural signals that other
//! parts of the system react to. This maps to Tasker's existing `DomainEvent`
//! system. This is NOT email/notification dispatch.
//!
//! ## Architecture
//!
//! The executor calls `context.operations.get_emittable(resource_ref)` to obtain
//! an `EmittableResource` trait object. In tests, this is backed by
//! `InMemoryOperations` with zero I/O. In production (Phase 3+), it's backed by
//! a runtime adapter wrapping a secure handle.
//!
//! ## Emit result envelope
//!
//! After the emit operation completes, the executor constructs an **emit result
//! envelope** — a structured JSON value passed to `validate_success` and
//! `result_shape` jaq expressions:
//!
//! ```json
//! {
//!   "data": { "message_id": "..." },   // Backend confirmation data
//!   "confirmed": true,                  // Whether delivery was confirmed
//!   "event_name": "order.confirmed",    // Echo of the event name
//!   "event_version": "1.0"             // Echo of the event version
//! }
//! ```
//!
//! When **no `result_shape`** is provided, the executor returns the `data` field
//! directly. When `result_shape` is provided, its expression receives the full
//! envelope.
//!
//! ## Conditional emission
//!
//! When a `condition` expression is provided, it is evaluated against the
//! composition envelope. If it evaluates to a falsy value (`false` or `null`),
//! the event is **not emitted** and the executor returns a skip marker:
//!
//! ```json
//! { "emitted": false, "reason": "condition not met" }
//! ```

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::operations::{EmitMetadata, OperationProvider};
use crate::types::{
    CapabilityError, CompositionEnvelope, ExecutionContext, TypedCapabilityExecutor,
};

/// Executor for the `emit` capability.
///
/// Evaluates jaq expressions to construct event payloads, optionally checks
/// a condition, then calls through `OperationProvider` to publish the event.
pub struct EmitExecutor {
    engine: ExpressionEngine,
    operations: Arc<dyn OperationProvider>,
}

impl std::fmt::Debug for EmitExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmitExecutor")
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

impl EmitExecutor {
    /// Create a new `EmitExecutor` with the given expression engine and
    /// operation provider.
    pub fn new(engine: ExpressionEngine, operations: Arc<dyn OperationProvider>) -> Self {
        Self { engine, operations }
    }
}

// ---------------------------------------------------------------------------
// Typed config structs
// ---------------------------------------------------------------------------

/// Typed configuration for the `emit` capability.
#[derive(Debug, Deserialize)]
pub struct EmitConfig {
    /// Domain event identifier (e.g., "order.confirmed").
    pub event_name: String,

    /// Schema version for event consumers (e.g., "1.0").
    #[serde(default)]
    pub event_version: Option<String>,

    /// Delivery mode: "async" (fire-and-forget) or "sync" (wait for ack).
    /// Defaults to "async".
    #[serde(default)]
    pub delivery_mode: DeliveryMode,

    /// Optional resource reference for OperationProvider lookup.
    /// Defaults to "events" if not provided.
    pub resource: Option<String>,

    /// Optional condition expression evaluated against the composition envelope.
    /// If falsy, the event is not emitted and a skip marker is returned.
    pub condition: Option<ExpressionField>,

    /// jaq expression constructing the event payload from the composition envelope.
    pub payload: ExpressionField,

    /// Optional metadata expressions for correlation and idempotency.
    pub metadata: Option<MetadataConfig>,

    /// Expression to verify the emission succeeded, evaluated against the
    /// emit result envelope. Must evaluate to a truthy value.
    pub validate_success: Option<ExpressionField>,

    /// Expression to extract/reshape the result for downstream capabilities,
    /// evaluated against the emit result envelope.
    pub result_shape: Option<ExpressionField>,
}

/// Delivery mode for event emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryMode {
    /// Fire-and-forget via messaging (default).
    #[default]
    Async,
    /// Wait for acknowledgment from the target.
    Sync,
}

/// A jaq expression field wrapper.
#[derive(Debug, Deserialize)]
pub struct ExpressionField {
    /// The jaq expression string.
    pub expression: String,
}

/// Metadata configuration with optional jaq expressions.
#[derive(Debug, Deserialize)]
pub struct MetadataConfig {
    /// Expression producing the correlation ID.
    pub correlation_id: Option<ExpressionField>,
    /// Expression producing the idempotency key.
    pub idempotency_key: Option<ExpressionField>,
}

// ---------------------------------------------------------------------------
// TypedCapabilityExecutor impl
// ---------------------------------------------------------------------------

impl TypedCapabilityExecutor for EmitExecutor {
    type Config = EmitConfig;

    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &EmitConfig,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        // 1. Check condition (if provided) — skip emission if falsy
        if let Some(ref condition) = config.condition {
            let condition_result = self
                .engine
                .evaluate(&condition.expression, envelope.raw())
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

            if !is_truthy(&condition_result) {
                return Ok(serde_json::json!({
                    "emitted": false,
                    "reason": "condition not met"
                }));
            }
        }

        // 2. Evaluate the payload expression against the composition envelope
        let payload = self
            .engine
            .evaluate(&config.payload.expression, envelope.raw())
            .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

        // 3. Build EmitMetadata from config expressions
        let metadata = self.build_metadata(config, envelope)?;

        // 4. Get the emittable resource and call emit
        let resource_ref = config.resource.as_deref().unwrap_or("events").to_string();

        let emit_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let emittable = self
                    .operations
                    .get_emittable(&resource_ref)
                    .await
                    .map_err(|e| CapabilityError::ResourceNotFound(e.to_string()))?;

                emittable
                    .emit(&config.event_name, payload, &metadata)
                    .await
                    .map_err(|e| CapabilityError::Execution(e.to_string()))
            })
        })?;

        // 5. Build the emit result envelope for validate_success and result_shape.
        //
        // Fields:
        //   .data          — backend confirmation data (message_id, timestamp, etc.)
        //   .confirmed     — boolean: whether delivery was confirmed
        //   .event_name    — echo of the event name for downstream reference
        //   .event_version — echo of the event version (null if not specified)
        let raw_result = serde_json::json!({
            "data": emit_result.data,
            "confirmed": emit_result.confirmed,
            "event_name": config.event_name,
            "event_version": config.event_version,
        });

        // 6. Validate success if expression is provided
        if let Some(ref validate_success) = config.validate_success {
            let validation = self
                .engine
                .evaluate(&validate_success.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

            if !is_truthy(&validation) {
                return Err(CapabilityError::Execution(format!(
                    "emit validate_success failed: expression '{}' evaluated to falsy value",
                    validate_success.expression
                )));
            }
        }

        // 7. Apply result_shape if provided, otherwise return the confirmation data
        if let Some(ref result_shape) = config.result_shape {
            self.engine
                .evaluate(&result_shape.expression, &raw_result)
                .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))
        } else {
            Ok(emit_result.data)
        }
    }

    fn capability_name(&self) -> &str {
        "emit"
    }
}

impl EmitExecutor {
    /// Build `EmitMetadata` from config, evaluating any jaq expressions.
    fn build_metadata(
        &self,
        config: &EmitConfig,
        envelope: &CompositionEnvelope<'_>,
    ) -> Result<EmitMetadata, CapabilityError> {
        let mut metadata = EmitMetadata::default();

        if let Some(ref meta_config) = config.metadata {
            if let Some(ref corr) = meta_config.correlation_id {
                let val = self
                    .engine
                    .evaluate(&corr.expression, envelope.raw())
                    .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;
                metadata.correlation_id = val.as_str().map(String::from);
            }

            if let Some(ref idem) = meta_config.idempotency_key {
                let val = self
                    .engine
                    .evaluate(&idem.expression, envelope.raw())
                    .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;
                metadata.idempotency_key = val.as_str().map(String::from);
            }
        }

        Ok(metadata)
    }
}

/// Determine whether a value is "truthy" for validation purposes.
/// Follows jq truthiness: `false` and `null` are falsy, everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

#[cfg(test)]
mod tests;
