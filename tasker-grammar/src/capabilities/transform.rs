use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, ExecutionContext};

/// Executor for the `transform` capability.
///
/// Pure data transformation: JSON in → jaq filter → JSON out + optional output
/// schema validation. Unifies what were conceptually four capabilities (reshape,
/// compute, evaluate, evaluate_rules) into a single executor powered by jq.
///
/// ## Config shape
///
/// ```json
/// {
///   "filter": "{ field_a: .context.value, field_b: (.deps.step.amount > 100) }",
///   "output": {
///     "type": "object",
///     "required": ["field_a"],
///     "properties": { "field_a": { "type": "number" } }
///   }
/// }
/// ```
///
/// - **`filter`** (required): jaq expression string applied to the composition
///   context envelope.
/// - **`output`** (optional): JSON Schema. When present, the filter result is
///   validated against it; violations produce `CapabilityError::OutputValidation`.
#[derive(Debug)]
pub struct TransformExecutor {
    engine: ExpressionEngine,
}

impl TransformExecutor {
    pub fn new(engine: ExpressionEngine) -> Self {
        Self { engine }
    }
}

impl CapabilityExecutor for TransformExecutor {
    fn execute(
        &self,
        input: &Value,
        config: &Value,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        let filter = config
            .get("filter")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CapabilityError::ConfigValidation(
                    "transform config must contain a 'filter' string".into(),
                )
            })?;

        let result = self
            .engine
            .evaluate(filter, input)
            .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

        if let Some(output_schema) = config.get("output") {
            validate_output(&result, output_schema)?;
        }

        Ok(result)
    }

    fn capability_name(&self) -> &str {
        "transform"
    }
}

fn validate_output(value: &Value, schema: &Value) -> Result<(), CapabilityError> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| CapabilityError::ConfigValidation(format!("invalid output schema: {e}")))?;

    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CapabilityError::OutputValidation(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests;
