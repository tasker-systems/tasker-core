use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Checkpoint data for grammar-composed handlers.
///
/// Stored in the existing `workflow_steps.checkpoint` JSONB column.
/// Used to resume composition execution after failure, skipping
/// already-completed (and checkpointed) mutating steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionCheckpoint {
    /// Which composition step just completed (0-indexed).
    pub completed_step_index: usize,

    /// Name of the capability that completed.
    pub completed_capability: String,

    /// Output of the completed step — used as `.prev` input on resume.
    pub step_output: Value,

    /// Accumulated outputs from all completed steps, indexed by step position.
    /// Supports checkpoint resumption by restoring the composition context.
    pub all_step_outputs: HashMap<usize, Value>,

    /// Whether the completed step was a mutation.
    pub was_mutation: bool,
}
