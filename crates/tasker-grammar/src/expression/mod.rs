use std::time::{Duration, Instant};

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Native, RcIter};
use serde_json::Value;

/// Errors that can occur during expression evaluation or compilation.
#[derive(Debug, thiserror::Error)]
pub enum ExpressionError {
    /// The filter string contains syntax errors.
    #[error("syntax error in filter: {details}")]
    SyntaxError { details: String },

    /// Evaluation of the filter produced an error.
    #[error("evaluation error: {details}")]
    EvaluationError { details: String },

    /// Filter evaluation exceeded the configured timeout.
    #[error("evaluation timed out after {elapsed:?} (limit: {limit:?})")]
    Timeout { elapsed: Duration, limit: Duration },

    /// Filter output exceeded the configured size limit.
    #[error("output size {size} bytes exceeds limit of {limit} bytes")]
    OutputTooLarge { size: usize, limit: usize },

    /// Filter produced too many output values.
    #[error("filter produced {count} outputs, exceeding limit of {limit}")]
    TooManyOutputs { count: usize, limit: usize },

    /// Filter produced no output values.
    #[error("filter produced no output")]
    EmptyOutput,
}

/// Configuration for the expression engine.
#[derive(Debug, Clone)]
pub struct ExpressionEngineConfig {
    /// Maximum wall-clock time allowed per filter evaluation.
    ///
    /// **Note:** This timeout is cooperative, not preemptive. It is checked
    /// between jaq iteration steps (i.e., between output values). A single
    /// jaq computation that takes longer than this limit to produce its first
    /// result will not be interrupted mid-iteration.
    pub timeout: Duration,
    /// Maximum serialized JSON size (in bytes) for the output value.
    pub max_output_bytes: usize,
    /// Maximum number of output values allowed from `evaluate_multi`.
    ///
    /// Prevents unbounded memory allocation from filters that produce
    /// excessive output (e.g., `range(1000000000)`). Defaults to 10,000.
    pub max_outputs: usize,
}

impl Default for ExpressionEngineConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(100),
            max_output_bytes: 1_048_576, // 1 MiB
            max_outputs: 10_000,
        }
    }
}

/// Expression engine wrapping jaq-core for jq filter compilation and evaluation.
///
/// Provides sandboxed evaluation with configurable timeout and output size limits.
/// jaq-core is safe by construction (no file/network I/O), so the sandboxing
/// focuses on bounding CPU time and memory via output size caps.
#[derive(Debug)]
pub struct ExpressionEngine {
    config: ExpressionEngineConfig,
}

impl ExpressionEngine {
    /// Create a new expression engine with the given configuration.
    pub fn new(config: ExpressionEngineConfig) -> Self {
        Self { config }
    }

    /// Create a new expression engine with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ExpressionEngineConfig::default())
    }

    /// Validate that a filter string is syntactically correct without evaluating it.
    pub fn validate_syntax(&self, filter: &str) -> Result<(), ExpressionError> {
        self.compile_filter(filter).map(|_| ())
    }

    /// Extract envelope field references from a jaq expression.
    ///
    /// Scans the expression string for path patterns rooted at the four envelope
    /// fields: `.context`, `.deps`, `.prev`, `.step`. Returns deduplicated paths
    /// sorted alphabetically.
    ///
    /// Uses regex-based extraction (jaq-core compiles to an opaque `Filter` type
    /// with no walkable AST). Handles common patterns: `.context.field`,
    /// `.deps.step_name.field`, `.prev.nested.path`. Dynamic patterns like
    /// `.context | keys` are captured as the root reference (`.context`).
    pub fn extract_references(&self, expression: &str) -> Result<Vec<String>, ExpressionError> {
        // First validate syntax so we don't extract from invalid expressions
        self.validate_syntax(expression)?;

        use std::collections::BTreeSet;

        let pattern = regex::Regex::new(r"\.(context|deps|prev|step)(\.[a-zA-Z_][a-zA-Z0-9_]*)*")
            .expect("static regex");

        let refs: BTreeSet<String> = pattern
            .find_iter(expression)
            .map(|m| m.as_str().to_owned())
            .collect();

        Ok(refs.into_iter().collect())
    }

    /// Evaluate a jq filter against an input JSON value.
    ///
    /// Returns the first output value from the filter. If the filter produces
    /// multiple outputs, only the first is returned. Use `evaluate_multi` to
    /// get all outputs.
    ///
    /// **Timeout semantics:** The configured timeout is cooperative — it is
    /// checked after the first result is produced. A filter that takes longer
    /// than the timeout to compute its first (and only) output will not be
    /// interrupted mid-computation.
    pub fn evaluate(&self, filter: &str, input: &Value) -> Result<Value, ExpressionError> {
        let compiled = self.compile_filter(filter)?;
        let start = Instant::now();

        let jaq_input = jaq_json::Val::from(input.clone());
        let inputs = RcIter::new(core::iter::empty());
        let mut results = compiled.run((Ctx::new([], &inputs), jaq_input));

        if let Some(result) = results.next() {
            self.check_timeout(start)?;
            match result {
                Ok(val) => self.val_to_json(&val),
                Err(err) => Err(ExpressionError::EvaluationError {
                    details: format!("{err}"),
                }),
            }
        } else {
            Err(ExpressionError::EmptyOutput)
        }
    }

    /// Evaluate a jq filter and return all output values.
    ///
    /// Returns up to [`ExpressionEngineConfig::max_outputs`] values. If the
    /// filter produces more, a [`ExpressionError::TooManyOutputs`] error is
    /// returned.
    ///
    /// **Timeout semantics:** The configured timeout is cooperative — it is
    /// checked between output values. A single computation step that exceeds
    /// the timeout will not be interrupted mid-iteration.
    pub fn evaluate_multi(
        &self,
        filter: &str,
        input: &Value,
    ) -> Result<Vec<Value>, ExpressionError> {
        let compiled = self.compile_filter(filter)?;
        let start = Instant::now();

        let jaq_input = jaq_json::Val::from(input.clone());
        let inputs = RcIter::new(core::iter::empty());
        let results = compiled.run((Ctx::new([], &inputs), jaq_input));

        let mut outputs = Vec::new();
        for result in results {
            self.check_timeout(start)?;
            if outputs.len() >= self.config.max_outputs {
                return Err(ExpressionError::TooManyOutputs {
                    count: outputs.len() + 1,
                    limit: self.config.max_outputs,
                });
            }
            match result {
                Ok(val) => {
                    let output = self.val_to_json(&val)?;
                    outputs.push(output);
                }
                Err(err) => {
                    return Err(ExpressionError::EvaluationError {
                        details: format!("{err}"),
                    });
                }
            }
        }

        Ok(outputs)
    }

    fn compile_filter(
        &self,
        filter: &str,
    ) -> Result<jaq_core::Filter<Native<jaq_json::Val>>, ExpressionError> {
        let arena = Arena::default();
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let modules = loader
            .load(
                &arena,
                File {
                    path: (),
                    code: filter,
                },
            )
            .map_err(|errs| ExpressionError::SyntaxError {
                details: format_load_errors(&errs),
            })?;

        let compiler = Compiler::default().with_funs(jaq_std::funs().chain(jaq_json::funs()));
        let compiled = compiler
            .compile(modules)
            .map_err(|errs| ExpressionError::SyntaxError {
                details: format_compile_errors(&errs),
            })?;

        Ok(compiled)
    }

    fn check_timeout(&self, start: Instant) -> Result<(), ExpressionError> {
        let elapsed = start.elapsed();
        if elapsed > self.config.timeout {
            return Err(ExpressionError::Timeout {
                elapsed,
                limit: self.config.timeout,
            });
        }
        Ok(())
    }

    fn val_to_json(&self, val: &jaq_json::Val) -> Result<Value, ExpressionError> {
        let json: Value = val.clone().into();
        let serialized =
            serde_json::to_string(&json).map_err(|e| ExpressionError::EvaluationError {
                details: format!("failed to serialize output: {e}"),
            })?;
        if serialized.len() > self.config.max_output_bytes {
            return Err(ExpressionError::OutputTooLarge {
                size: serialized.len(),
                limit: self.config.max_output_bytes,
            });
        }
        Ok(json)
    }
}

fn format_load_errors(errs: &[(File<&str, ()>, jaq_core::load::Error<&str>)]) -> String {
    errs.iter()
        .map(|(_file, e)| format!("{e:?}"))
        .collect::<Vec<_>>()
        .join("; ")
}

type CompileErrors<'a> = [(
    File<&'a str, ()>,
    Vec<(&'a str, jaq_core::compile::Undefined)>,
)];

fn format_compile_errors(errs: &CompileErrors<'_>) -> String {
    errs.iter()
        .flat_map(|(_file, undefs)| {
            undefs
                .iter()
                .map(|(name, undef)| format!("undefined {undef:?}: {name}"))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests;
