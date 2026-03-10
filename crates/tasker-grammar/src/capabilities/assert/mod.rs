use serde::Deserialize;
use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::types::{
    CapabilityError, CompositionEnvelope, ExecutionContext, TypedCapabilityExecutor,
};

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
/// # use tasker_grammar::types::{CapabilityExecutor, CompositionEnvelope, ExecutionContext};
/// # use tasker_grammar::capabilities::assert::AssertExecutor;
/// let exec = AssertExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"total": 100, "subtotal": 90, "tax": 10}
/// });
/// let envelope = CompositionEnvelope::new(&input);
/// let config = json!({
///     "filter": ".prev.total == (.prev.subtotal + .prev.tax)",
///     "error": "Totals do not balance"
/// });
/// let result = exec.execute(&envelope, &config, &ctx).unwrap();
/// assert_eq!(result["total"], json!(100));
/// ```
///
/// **Named conditions with quantifier:**
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::ExpressionEngine;
/// # use tasker_grammar::types::{CapabilityExecutor, CompositionEnvelope, ExecutionContext};
/// # use tasker_grammar::capabilities::assert::AssertExecutor;
/// let exec = AssertExecutor::new(ExpressionEngine::with_defaults());
/// let ctx = ExecutionContext { step_name: "s".into(), attempt: 1, checkpoint_state: None };
///
/// let input = json!({
///     "context": {}, "deps": {}, "step": {},
///     "prev": {"total": 100, "items": [1, 2, 3]}
/// });
/// let envelope = CompositionEnvelope::new(&input);
/// let config = json!({
///     "conditions": [
///         {"name": "positive_total", "expression": ".prev.total > 0"},
///         {"name": "has_items", "expression": ".prev.items | length > 0"}
///     ],
///     "quantifier": "all"
/// });
/// let result = exec.execute(&envelope, &config, &ctx).unwrap();
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

// ---------------------------------------------------------------------------
// Typed config structs
// ---------------------------------------------------------------------------

/// Typed configuration for the `assert` capability.
///
/// Supports two forms via optional fields:
/// - **Simple**: `filter` (required) + `error` (optional)
/// - **Conditions**: `conditions` (required) + `quantifier` (optional)
///
/// Both forms support `dependency_precedent` for conditional skip.
/// Exactly one of `filter` or `conditions` must be present.
#[derive(Debug, Deserialize)]
pub struct AssertConfig {
    /// Simple form: jaq boolean expression.
    pub filter: Option<String>,

    /// Error message for simple form (default: "assertion failed").
    #[serde(default = "default_error_message")]
    pub error: String,

    /// Named conditions form: array of conditions.
    pub conditions: Option<Vec<Condition>>,

    /// Quantifier for named conditions (default: `All`).
    #[serde(default)]
    pub quantifier: Quantifier,

    /// Skip assertion if the named dependency is missing/null.
    pub dependency_precedent: Option<String>,
}

fn default_error_message() -> String {
    "assertion failed".into()
}

/// A single named assertion condition.
#[derive(Debug, Deserialize)]
pub struct Condition {
    /// Human-readable name for error reporting.
    #[serde(default = "default_condition_name")]
    pub name: String,
    /// jaq boolean expression to evaluate.
    pub expression: String,
}

fn default_condition_name() -> String {
    "unnamed".into()
}

/// Quantifier for combining multiple condition results.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quantifier {
    /// Every condition must evaluate to `true`.
    #[default]
    All,
    /// At least one condition must evaluate to `true`.
    Any,
    /// Every condition must evaluate to `false` (all must fail).
    None,
}

impl std::fmt::Display for Quantifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Any => write!(f, "any"),
            Self::None => write!(f, "none"),
        }
    }
}

// ---------------------------------------------------------------------------
// TypedCapabilityExecutor impl
// ---------------------------------------------------------------------------

impl TypedCapabilityExecutor for AssertExecutor {
    type Config = AssertConfig;

    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &AssertConfig,
        _context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        // Check dependency_precedent: skip assertion if the named dep is missing/null
        if let Some(dep_name) = &config.dependency_precedent {
            let dep = envelope.dep(dep_name);
            if dep.is_null() {
                return Ok(envelope.resolve_target().clone());
            }
        }

        match (&config.filter, &config.conditions) {
            (Some(filter), None) => self.execute_simple(filter, &config.error, envelope),
            (None, Some(conditions)) => {
                self.execute_conditions(conditions, config.quantifier, envelope)
            }
            (Some(_), Some(_)) => Err(CapabilityError::ConfigValidation(
                "assert config must contain either 'filter' or 'conditions', not both".into(),
            )),
            (None, None) => Err(CapabilityError::ConfigValidation(
                "assert config must contain either 'filter' (simple form) or 'conditions' (named conditions form)".into(),
            )),
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
        filter: &str,
        error_msg: &str,
        envelope: &CompositionEnvelope<'_>,
    ) -> Result<Value, CapabilityError> {
        let result = self
            .engine
            .evaluate(filter, envelope.raw())
            .map_err(|e| CapabilityError::ExpressionEvaluation(e.to_string()))?;

        if is_truthy(&result) {
            Ok(envelope.resolve_target().clone())
        } else {
            Err(CapabilityError::Execution(format!(
                "assertion failed: {error_msg} (filter: {})",
                truncate_expression(filter)
            )))
        }
    }

    /// Named conditions form: `conditions` array with `quantifier`.
    fn execute_conditions(
        &self,
        conditions: &[Condition],
        quantifier: Quantifier,
        envelope: &CompositionEnvelope<'_>,
    ) -> Result<Value, CapabilityError> {
        if conditions.is_empty() {
            return Err(CapabilityError::ConfigValidation(
                "assert config 'conditions' must not be empty".into(),
            ));
        }

        // Evaluate each condition
        let mut results: Vec<ConditionResult> = Vec::with_capacity(conditions.len());

        for condition in conditions {
            let eval_result = self
                .engine
                .evaluate(&condition.expression, envelope.raw())
                .map_err(|e| {
                    CapabilityError::ExpressionEvaluation(format!(
                        "condition '{}': {e}",
                        condition.name
                    ))
                })?;

            results.push(ConditionResult {
                name: &condition.name,
                expression: &condition.expression,
                passed: is_truthy(&eval_result),
            });
        }

        // Apply quantifier
        let assertion_passed = match quantifier {
            Quantifier::All => results.iter().all(|r| r.passed),
            Quantifier::Any => results.iter().any(|r| r.passed),
            Quantifier::None => results.iter().all(|r| !r.passed),
        };

        if assertion_passed {
            Ok(envelope.resolve_target().clone())
        } else {
            let failed: Vec<&ConditionResult> = match quantifier {
                Quantifier::All | Quantifier::Any => results.iter().filter(|r| !r.passed).collect(),
                Quantifier::None => results.iter().filter(|r| r.passed).collect(),
            };

            let details: Vec<String> = failed
                .iter()
                .map(|r| {
                    format!(
                        "'{}' (expression: {})",
                        r.name,
                        truncate_expression(r.expression)
                    )
                })
                .collect();

            Err(CapabilityError::Execution(format!(
                "assertion failed: {quantifier} quantifier not satisfied. Failed conditions: {}",
                details.join(", ")
            )))
        }
    }
}

/// Maximum length for expression text in error messages.
///
/// Prevents business logic leakage through overly verbose error messages.
const MAX_ERROR_EXPRESSION_LEN: usize = 200;

/// Truncate an expression string for safe inclusion in error messages.
fn truncate_expression(expr: &str) -> String {
    if expr.len() <= MAX_ERROR_EXPRESSION_LEN {
        expr.to_owned()
    } else {
        // Find a valid UTF-8 boundary near the limit
        let end = expr
            .char_indices()
            .take_while(|(i, _)| *i <= MAX_ERROR_EXPRESSION_LEN)
            .last()
            .map_or(0, |(i, c)| i + c.len_utf8());
        format!("{}...", &expr[..end])
    }
}

/// Determine whether a jaq result value is "truthy" for assertion purposes.
///
/// Follows jq truthiness: `false` and `null` are falsy, everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

struct ConditionResult<'a> {
    name: &'a str,
    expression: &'a str,
    passed: bool,
}

#[cfg(test)]
mod tests;
