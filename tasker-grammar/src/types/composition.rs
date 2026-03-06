use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A composed virtual handler — a chain of capabilities toward a singular outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSpec {
    /// Optional name for registered compositions.
    pub name: Option<String>,

    /// The declared singular outcome.
    pub outcome: OutcomeDeclaration,

    /// Ordered sequence of capability invocations.
    pub steps: Vec<CompositionStep>,

    /// Cross-cutting concern mixins (retry, observability, timeout, etc.).
    #[serde(default)]
    pub mixins: Vec<String>,
}

/// A single step within a composition.
///
/// Each step invokes a capability from the vocabulary. The `config` field
/// is capability-specific:
/// - `transform`: `output` (JSON Schema) + `filter` (jaq expression)
/// - `validate`: JSON Schema + coercion/failure config
/// - `assert`: `filter` (jaq boolean) + `error` message
/// - `persist`/`acquire`/`emit`: typed envelope with resource, data/params/payload,
///   constraints, validate_success, result_shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionStep {
    /// Which capability to invoke (must exist in vocabulary).
    pub capability: String,

    /// Configuration for this invocation.
    pub config: Value,

    /// How this step's input is resolved.
    ///
    /// **Note**: In the revised 6-capability model, explicit input mapping is
    /// superseded by the composition context envelope. jaq filters access
    /// `.context`, `.deps.{step_name}`, `.prev`, and `.step` directly.
    /// This field is retained for backward compatibility.
    #[serde(default)]
    pub input_mapping: InputMapping,

    /// Whether this is a checkpoint boundary.
    ///
    /// Required for mutating capabilities. Optional for non-mutating
    /// (useful for expensive computations worth preserving).
    #[serde(default)]
    pub checkpoint: bool,
}

/// How a composition step receives its input.
///
/// **Superseded** by the composition context envelope in the revised model.
/// jaq filters access the full context directly:
/// - `.context` — task input data
/// - `.deps.{step_name}` — dependency step results
/// - `.prev` — previous capability invocation output
/// - `.step` — step metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputMapping {
    /// Input comes from the previous step's output (default for linear chains).
    #[default]
    Previous,

    /// Input comes from a specific earlier step's output, by index.
    StepOutput { step_index: usize },

    /// Input comes from task context / step_inputs.
    TaskContext { path: String },

    /// Input is composed from multiple sources.
    Merged { sources: Vec<InputMapping> },
}

/// The declared outcome of a composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeDeclaration {
    /// Human-readable description of what this composition achieves.
    pub description: String,

    /// JSON Schema for what the composition produces.
    pub output_schema: Value,
}
