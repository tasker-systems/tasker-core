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
}

/// A single step within a composition.
///
/// Each step invokes a capability from the vocabulary. The `config` field
/// is capability-specific:
/// - `transform`: `output` (JSON Schema) + `filter` (jaq expression)
/// - `assert`: `filter` (jaq boolean) + `error` message
/// - `validate`: JSON Schema + coercion/failure config
/// - `persist`/`acquire`/`emit`: typed envelope with resource, data/params/payload,
///   constraints, validate_success, result_shape
///
/// Input is always the composition context envelope. jaq filters access
/// `.context`, `.deps.{step_name}`, `.prev`, and `.step` directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionStep {
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
