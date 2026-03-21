use std::collections::HashMap;

use serde_json::Value;

use crate::types::{GrammarCategoryKind, ValidationFinding};

/// Complete trace of data flow through a composition.
#[derive(Debug, Clone)]
pub struct ExplanationTrace {
    /// Composition name (if declared).
    pub name: Option<String>,
    /// Declared outcome description and output schema.
    pub outcome: OutcomeSummary,
    /// Per-invocation trace entries, in execution order.
    pub invocations: Vec<InvocationTrace>,
    /// Validation findings (errors/warnings) from the underlying validator.
    pub validation: Vec<ValidationFinding>,
    /// Whether simulation was performed (sample data provided).
    pub simulated: bool,
}

/// Summary of the declared outcome.
#[derive(Debug, Clone)]
pub struct OutcomeSummary {
    /// Human-readable description of what the composition achieves.
    pub description: String,
    /// JSON Schema for the composition's output.
    pub output_schema: Value,
}

/// Trace for a single capability invocation.
#[derive(Debug, Clone)]
pub struct InvocationTrace {
    /// Position in the invocation chain (0-based).
    pub index: usize,
    /// Capability name.
    pub capability: String,
    /// Grammar category.
    pub category: GrammarCategoryKind,
    /// Whether this is a checkpoint boundary.
    pub checkpoint: bool,
    /// Whether this capability is mutating.
    pub is_mutating: bool,
    /// Envelope fields available at this invocation.
    pub envelope_available: EnvelopeSnapshot,
    /// Jaq expressions found in config and which envelope paths they reference.
    pub expressions: Vec<ExpressionReference>,
    /// Declared output schema (if any — transforms declare this).
    pub output_schema: Option<Value>,
    /// Simulated output value (when sample data provided).
    pub simulated_output: Option<Value>,
    /// For side-effecting capabilities: whether a mock output was provided.
    pub mock_output_used: bool,
}

/// What's available in the envelope at a given point in the chain.
#[derive(Debug, Clone)]
pub struct EnvelopeSnapshot {
    /// Always true — task-level input.
    pub context: bool,
    /// Always true — dependency step results.
    pub deps: bool,
    /// Always true — step metadata.
    pub step: bool,
    /// Whether .prev is non-null at this point.
    pub has_prev: bool,
    /// Description of what .prev contains (e.g., "output of invocation 0 (transform)").
    pub prev_source: Option<String>,
    /// Schema of .prev if known (from prior invocation's output schema).
    pub prev_schema: Option<Value>,
}

/// A jaq expression found in an invocation's config.
#[derive(Debug, Clone)]
pub struct ExpressionReference {
    /// Config field path (e.g., "filter", "data.expression").
    pub field_path: String,
    /// The raw expression string.
    pub expression: String,
    /// Envelope paths referenced (e.g., [".context.order_id", ".prev.total"]).
    pub referenced_paths: Vec<String>,
    /// Simulated result value (when sample data provided).
    pub simulated_result: Option<Value>,
}

/// Sample data for simulated evaluation.
#[derive(Debug, Clone)]
pub struct SimulationInput {
    /// Sample task-level input — populates .context
    pub context: Value,
    /// Sample dependency results — populates .deps
    pub deps: Value,
    /// Sample step metadata — populates .step
    pub step: Value,
    /// Mock outputs for side-effecting invocations, keyed by invocation index.
    /// Used as .prev for the next invocation when the capability can't be
    /// evaluated purely (persist, acquire, emit).
    pub mock_outputs: HashMap<usize, Value>,
}
