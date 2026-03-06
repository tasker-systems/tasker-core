use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, ExecutionContext};

/// Executor for the `transform` capability — the unified pure data transformation
/// primitive in the action grammar.
///
/// `transform` replaces four conceptual capabilities with a single jaq-powered
/// executor. The semantic distinction between these intents becomes a documentation
/// convention rather than separate implementations:
///
/// | Intent | jaq pattern | Example |
/// |--------|-------------|---------|
/// | **Projection** (reshape) | Object construction, field selection | `{name: .context.name}` |
/// | **Computation** (compute) | Math, aggregation, string building | `.items \| map(.price) \| add` |
/// | **Evaluation** (evaluate) | Booleans, classification values | `.amount > 1000` |
/// | **Rule matching** (evaluate_rules) | `if-elif-else` chains, all-match arrays | `if .tier == "gold" then ... end` |
///
/// ## Config shape
///
/// - **`filter`** (required): jaq expression string applied to the composition
///   context envelope.
/// - **`output`** (optional): JSON Schema. When present, the filter result is
///   validated against it; violations produce [`CapabilityError::OutputValidation`].
///
/// ## Composition context envelope
///
/// The filter operates on the composition context envelope with four fields:
///
/// - **`.context`** — task-level input data (immutable across invocations)
/// - **`.deps`** — dependency step results keyed by step name (immutable)
/// - **`.step`** — step metadata: name, attempt count, inputs (immutable)
/// - **`.prev`** — output of the previous capability invocation (`null` for first)
///
/// ## Examples
///
/// **Projection** — select and rename fields from dependency outputs:
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::transform::TransformExecutor;
/// let exec = TransformExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {"customer_email": "alice@example.com"},
///     "deps": {"validate_cart": {"total": 99.99}},
///     "step": {}, "prev": null
/// });
/// let config = json!({
///     "filter": "{email: .context.customer_email, total: .deps.validate_cart.total}",
///     "output": {
///         "type": "object",
///         "required": ["email", "total"],
///         "properties": {
///             "email": {"type": "string"},
///             "total": {"type": "number"}
///         }
///     }
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result, json!({"email": "alice@example.com", "total": 99.99}));
/// ```
///
/// **Computation** — derive new values with arithmetic and aggregation:
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::transform::TransformExecutor;
/// let exec = TransformExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {"tax_rate": 0.08},
///     "deps": {}, "step": {},
///     "prev": {"items": [{"price": 10, "qty": 2}, {"price": 25, "qty": 1}]}
/// });
/// let config = json!({
///     "filter": "(.prev.items | map(.price * .qty) | add) as $sub | {subtotal: $sub, tax: ($sub * .context.tax_rate), total: ($sub + $sub * .context.tax_rate)}"
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result["subtotal"], json!(45));
/// assert_eq!(result["total"], json!(48.6));
/// ```
///
/// **Evaluation** — produce boolean or classification values:
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::transform::TransformExecutor;
/// let exec = TransformExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({"context": {}, "deps": {}, "step": {}, "prev": {"total": 1500}});
/// let config = json!({
///     "filter": "{high_value: (.prev.total > 1000), tier: (if .prev.total > 1000 then \"gold\" else \"standard\" end)}"
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result["high_value"], json!(true));
/// assert_eq!(result["tier"], json!("gold"));
/// ```
///
/// **Rule matching** — first-match routing via if-elif-else:
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::transform::TransformExecutor;
/// let exec = TransformExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({"context": {}, "deps": {}, "step": {}, "prev": {"amount": 750}});
/// let config = json!({
///     "filter": "if .prev.amount > 1000 then {queue: \"vip\"} elif .prev.amount > 500 then {queue: \"priority\"} else {queue: \"standard\"} end"
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result["queue"], json!("priority"));
/// ```
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
        .map(|e| format_validation_error(&e))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CapabilityError::OutputValidation(errors.join("; ")))
    }
}

/// Format a JSON Schema validation error without leaking instance data.
///
/// The default `Display` impl for `ValidationError` embeds the actual failing
/// value (e.g. `"alice@secret.com" is not of type "integer"`), which risks
/// leaking task context, step results, or PII into logs and error responses.
///
/// This formatter emits only structural information: the instance path (which
/// field failed) and the constraint that was violated, never the value itself.
fn format_validation_error(e: &jsonschema::ValidationError<'_>) -> String {
    use jsonschema::error::ValidationErrorKind;

    let path = e.instance_path.to_string();
    let at = if path.is_empty() {
        String::new()
    } else {
        format!("at {path}: ")
    };

    let constraint = match &e.kind {
        ValidationErrorKind::Type { kind } => format!("expected type {kind:?}"),
        ValidationErrorKind::Required { property } => {
            format!("missing required property {property}")
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            format!("unexpected properties: {}", unexpected.join(", "))
        }
        ValidationErrorKind::MinLength { limit } => {
            format!("string length below minimum of {limit}")
        }
        ValidationErrorKind::MaxLength { limit } => {
            format!("string length exceeds maximum of {limit}")
        }
        ValidationErrorKind::Minimum { limit } => {
            format!("value below minimum of {limit}")
        }
        ValidationErrorKind::Maximum { limit } => {
            format!("value exceeds maximum of {limit}")
        }
        ValidationErrorKind::ExclusiveMinimum { limit } => {
            format!("value at or below exclusive minimum of {limit}")
        }
        ValidationErrorKind::ExclusiveMaximum { limit } => {
            format!("value at or above exclusive maximum of {limit}")
        }
        ValidationErrorKind::MinItems { limit } => {
            format!("array has fewer than {limit} items")
        }
        ValidationErrorKind::MaxItems { limit } => {
            format!("array has more than {limit} items")
        }
        ValidationErrorKind::MinProperties { limit } => {
            format!("object has fewer than {limit} properties")
        }
        ValidationErrorKind::MaxProperties { limit } => {
            format!("object has more than {limit} properties")
        }
        ValidationErrorKind::Pattern { pattern } => {
            format!("value does not match pattern {pattern:?}")
        }
        ValidationErrorKind::MultipleOf { multiple_of } => {
            format!("value is not a multiple of {multiple_of}")
        }
        ValidationErrorKind::Enum { options } => {
            format!("value not in enum {options}")
        }
        ValidationErrorKind::Constant { expected_value } => {
            format!("value does not match const {expected_value}")
        }
        ValidationErrorKind::Format { format } => {
            format!("value does not match format {format:?}")
        }
        ValidationErrorKind::UniqueItems => "array contains duplicate items".into(),
        ValidationErrorKind::FalseSchema => "value rejected by false schema".into(),
        ValidationErrorKind::Not { .. } => "value matched a negated schema".into(),
        ValidationErrorKind::AnyOf => "value does not match any 'anyOf' schemas".into(),
        ValidationErrorKind::OneOfNotValid => "value does not match any 'oneOf' schema".into(),
        ValidationErrorKind::OneOfMultipleValid => "value matches multiple 'oneOf' schemas".into(),
        ValidationErrorKind::Contains => "array does not contain a required element".into(),
        // Fallback: use Debug which is less likely to embed raw values than Display
        _ => format!("{:?}", e.kind),
    };

    format!("{at}{constraint}")
}

#[cfg(test)]
mod tests;
