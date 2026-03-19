use serde::{Deserialize, Serialize};

/// Severity level for validation findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocks composition acceptance.
    Error,
    /// Indicates a potential issue but does not block.
    Warning,
    /// Informational only.
    Info,
}

/// A single finding from validation of a capability declaration or composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Severity of this finding.
    pub severity: Severity,

    /// Machine-readable code (e.g., "MISSING_CAPABILITY", "CONTRACT_MISMATCH").
    pub code: String,

    /// Which capability invocation produced this finding (0-indexed position
    /// within the composition's `invocations` list, if applicable).
    pub invocation_index: Option<usize>,

    /// Human-readable description of the finding.
    pub message: String,

    /// JSON path within the config or schema that triggered this finding.
    pub field_path: Option<String>,
}

/// A constraint that a grammar category imposes on compositions.
///
/// For example, a "Persist" category might require that it's preceded
/// by a Validate invocation, or a domain-specific category might impose
/// ordering constraints on subsequent invocations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionConstraint {
    /// Machine-readable constraint identifier.
    pub code: String,

    /// Human-readable description of the constraint.
    pub description: String,

    /// Severity when this constraint is violated.
    pub severity: Severity,
}
