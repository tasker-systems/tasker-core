use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

/// Executor for the `assert` capability — the composable execution gate in the
/// action grammar.
///
/// `assert` evaluates named conditions (jaq boolean expressions) against the
/// composition context envelope and halts composition execution if assertions
/// fail. Unlike `transform` (which produces output values), `assert` either
/// passes silently — returning `.prev` unchanged — or stops the composition
/// with a structured error listing which conditions failed.
///
/// ## Config shapes
///
/// ### Simple form (single expression)
///
/// ```yaml
/// capability: assert
/// config:
///   filter: ".prev.total == (.prev.subtotal + .prev.tax)"
///   error: "Order total does not match component sum"
/// ```
///
/// ### Named conditions form
///
/// ```yaml
/// capability: assert
/// config:
///   conditions:
///     - name: "totals_balance"
///       expression: ".total == (.subtotal + .tax)"
///     - name: "has_items"
///       expression: ".items | length > 0"
///   quantifier: all    # all | any | none
/// ```
///
/// ## Quantifiers
///
/// - **`all`** (default): Every condition must evaluate to `true`.
/// - **`any`**: At least one condition must evaluate to `true`.
/// - **`none`**: Every condition must evaluate to `false` (all must fail).
///
/// ## Dependency precedent
///
/// When `dependency_precedent` is specified, the assertion is conditionally
/// skipped if the named dependency step did not produce a result (its entry
/// in `.deps` is missing or null). This enables patterns like "only assert
/// payment totals if the payment step actually succeeded."
///
/// ## Composition context envelope
///
/// Assert conditions evaluate jaq expressions against the full composition
/// context envelope (`.context`, `.deps`, `.prev`, `.step`). On success,
/// the executor returns `.prev` unchanged (or `.context` if `.prev` is null),
/// preserving the data flow for downstream capabilities.
///
/// ## Examples
///
/// **Simple assertion passes:**
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::assert::AssertExecutor;
/// let exec = AssertExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"total": 100, "subtotal": 90, "tax": 10}
/// });
/// let config = json!({
///     "filter": ".prev.total == (.prev.subtotal + .prev.tax)",
///     "error": "Totals do not balance"
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result["total"], json!(100));
/// ```
///
/// **Named conditions with quantifier:**
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, ExecutionContext};
/// # use tasker_grammar::capabilities::assert::AssertExecutor;
/// let exec = AssertExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"total": 100, "items": [1, 2, 3]}
/// });
/// let config = json!({
///     "conditions": [
///         {"name": "positive_total", "expression": ".prev.total > 0"},
///         {"name": "has_items", "expression": ".prev.items | length > 0"}
///     ],
///     "quantifier": "all"
/// });
/// let result = exec.execute(&input, &config, &ctx).unwrap();
/// assert_eq!(result["total"], json!(100));
/// ```
#[derive(Debug)]
pub struct AssertExecutor {
    engine: ExpressionEngine,
}

impl AssertExecutor {
    pub fn new(engine: ExpressionEngine) -> Self {
        Self { engine }
    }
}

impl CapabilityExecutor for AssertExecutor {
    fn execute(
        &self,
        input: &Value,
        config: &Value,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        let envelope = CompositionEnvelope::new(input);

        // Check dependency_precedent: skip assertion if the named dep is missing/null
        if let Some(dep_name) = config.get("dependency_precedent").and_then(Value::as_str) {
            let dep = envelope.dep(dep_name);
            if dep.is_null() {
                // Prior step didn't produce a result — skip this assertion
                return Ok(envelope.resolve_target().clone());
            }
        }

        // Determine config form: simple (filter+error) or named conditions
        if config.get("filter").is_some() {
            self.execute_simple(config, &envelope)
        } else if config.get("conditions").is_some() {
            self.execute_conditions(config, &envelope)
        } else {
            Err(CapabilityError::ConfigValidation(
                "assert config must contain either 'filter' (simple form) or 'conditions' (named conditions form)".into(),
            ))
        }
    }

    fn capability_name(&self) -> &str {
        "assert"
    }
}

impl AssertExecutor {
    /// Simple form: single `filter` expression with `error` message.
    fn execute_simple(
        &self,
        config: &Value,
        envelope: &CompositionEnvelope<'_>,
    ) -> Result<Value, CapabilityError> {
        let filter = config
            .get("filter")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CapabilityError::ConfigValidation("assert config 'filter' must be a string".into())
            })?;

        let error_msg = config
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("assertion failed");

        let result = self
            .engine
            .evaluate(filter, envelope.raw())
            .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

        if is_truthy(&result) {
            Ok(envelope.resolve_target().clone())
        } else {
            Err(CapabilityError::Execution(format!(
                "assertion failed: {error_msg} (filter: {filter})"
            )))
        }
    }

    /// Named conditions form: `conditions` array with `quantifier`.
    fn execute_conditions(
        &self,
        config: &Value,
        envelope: &CompositionEnvelope<'_>,
    ) -> Result<Value, CapabilityError> {
        let conditions = config
            .get("conditions")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CapabilityError::ConfigValidation(
                    "assert config 'conditions' must be an array".into(),
                )
            })?;

        if conditions.is_empty() {
            return Err(CapabilityError::ConfigValidation(
                "assert config 'conditions' must not be empty".into(),
            ));
        }

        let quantifier = config
            .get("quantifier")
            .and_then(Value::as_str)
            .unwrap_or("all");

        // Evaluate each condition
        let mut results: Vec<ConditionResult> = Vec::with_capacity(conditions.len());

        for (i, condition) in conditions.iter().enumerate() {
            let name = condition
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unnamed");

            let expression = condition
                .get("expression")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CapabilityError::ConfigValidation(format!(
                        "condition at index {i} ('{name}') must have an 'expression' string"
                    ))
                })?;

            let eval_result = self
                .engine
                .evaluate(expression, envelope.raw())
                .map_err(|e| {
                    CapabilityError::ExpressionEvaluation(format!("condition '{name}': {e}"))
                })?;

            results.push(ConditionResult {
                name: name.to_string(),
                expression: expression.to_string(),
                passed: is_truthy(&eval_result),
            });
        }

        // Apply quantifier
        let assertion_passed = match quantifier {
            "all" => results.iter().all(|r| r.passed),
            "any" => results.iter().any(|r| r.passed),
            "none" => results.iter().all(|r| !r.passed),
            other => {
                return Err(CapabilityError::ConfigValidation(format!(
                    "unknown quantifier '{other}': expected 'all', 'any', or 'none'"
                )));
            }
        };

        if assertion_passed {
            Ok(envelope.resolve_target().clone())
        } else {
            let failed: Vec<&ConditionResult> = match quantifier {
                "all" => results.iter().filter(|r| !r.passed).collect(),
                "any" => results.iter().filter(|r| !r.passed).collect(),
                "none" => results.iter().filter(|r| r.passed).collect(),
                _ => unreachable!(),
            };

            let details: Vec<String> = failed
                .iter()
                .map(|r| format!("'{}' (expression: {})", r.name, r.expression))
                .collect();

            Err(CapabilityError::Execution(format!(
                "assertion failed: {quantifier} quantifier not satisfied. Failed conditions: {}",
                details.join(", ")
            )))
        }
    }
}

/// Determine whether a jaq result value is "truthy" for assertion purposes.
///
/// Follows jq truthiness: `false` and `null` are falsy, everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

struct ConditionResult {
    name: String,
    expression: String,
    passed: bool,
}

#[cfg(test)]
mod tests;
