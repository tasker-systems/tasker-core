use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Checkpoint data for grammar-composed handlers.
///
/// Stored in the existing `workflow_steps.checkpoint` JSONB column.
/// Used to resume composition execution after failure, skipping
/// already-completed (and checkpointed) mutating invocations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionCheckpoint {
    /// Which capability invocation just completed (0-indexed position
    /// within the composition's `invocations` list).
    pub completed_invocation_index: usize,

    /// Name of the capability that completed.
    pub completed_capability: String,

    /// Output of the completed invocation — used as `.prev` input on resume.
    pub invocation_output: Value,

    /// Accumulated outputs from all completed invocations, indexed by position
    /// within the composition. Supports checkpoint resumption by restoring
    /// the composition context.
    pub all_invocation_outputs: HashMap<usize, Value>,

    /// Whether the completed invocation was a mutation.
    pub was_mutation: bool,
}
