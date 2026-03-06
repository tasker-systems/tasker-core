use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A composed virtual handler — a chain of capability invocations toward a singular outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSpec {
    /// Optional name for registered compositions.
    pub name: Option<String>,

    /// The declared singular outcome.
    pub outcome: OutcomeDeclaration,

    /// Ordered sequence of capability invocations.
    ///
    /// Each entry is a single invocation of a capability from the vocabulary.
    /// These are NOT workflow steps — they are the internal execution sequence
    /// within a single step's virtual handler.
    pub invocations: Vec<CapabilityInvocation>,
}

/// A single capability invocation within a composition.
///
/// Each invocation targets a capability from the vocabulary. The `config` field
/// is capability-specific:
/// - `transform`: `output` (JSON Schema) + `filter` (jaq expression)
/// - `validate`: JSON Schema + coercion/failure config
/// - `assert`: `filter` (jaq boolean) + `error` message
/// - `persist`/`acquire`/`emit`: typed envelope with resource, data/params/payload,
///   constraints, success_criteria, result_shape
///
/// Input is always the composition context envelope. jaq filters access
/// `.context`, `.deps.{step_name}`, `.prev`, and `.step` directly.
///
/// **Terminology note**: These are capability invocations, not workflow steps.
/// In Tasker, "step" refers to `WorkflowStep` — a node in the task DAG.
/// A composition is the internal execution sequence within a single step's
/// virtual handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInvocation {
    /// Which capability to invoke (must exist in vocabulary).
    pub capability: String,

    /// Configuration for this invocation.
    pub config: Value,

    /// Whether this is a checkpoint boundary.
    ///
    /// Required for mutating capabilities. Optional for non-mutating
    /// (useful for expensive computations worth preserving).
    #[serde(default)]
    pub checkpoint: bool,
}

/// The declared outcome of a composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeDeclaration {
    /// Human-readable description of what this composition achieves.
    pub description: String,

    /// JSON Schema for what the composition produces.
    pub output_schema: Value,
}
