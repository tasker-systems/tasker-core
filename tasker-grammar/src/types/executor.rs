use std::fmt;

use serde_json::Value;

use super::envelope::CompositionEnvelope;
use super::error::CapabilityError;

/// Executes a capability against concrete inputs.
///
/// Separate from [`CapabilityDeclaration`](super::CapabilityDeclaration) because
/// the declaration is data (serializable, discoverable) while the executor is
/// behavior (may hold connections, state, configuration).
///
/// The executor receives the typed [`CompositionEnvelope`] and capability-specific
/// config as `config`. The action is implicit in the capability identity.
pub trait CapabilityExecutor: Send + Sync + fmt::Debug {
    /// Execute this capability with the given envelope and config.
    ///
    /// - `envelope`: The typed composition context envelope.
    /// - `config`: Capability-specific configuration (`serde_json::Value`).
    /// - `context`: Execution metadata (step identity, checkpoint state).
    ///
    /// Returns the output conforming to the capability's `output_schema`.
    fn execute(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &Value,
        context: &ExecutionContext,
    ) -> Result<Value, CapabilityError>;

    /// The capability name this executor handles.
    fn capability_name(&self) -> &str;
}

/// Context available during capability execution.
///
/// Provides step identity and checkpoint state for the executor.
/// Deliberately lightweight — no database handles, no messaging connections.
/// The composition executor (TAS-334) wraps this with runtime services.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Step name for correlation.
    pub step_name: String,

    /// Attempt number (1-indexed).
    pub attempt: u32,

    /// Existing checkpoint state if resuming after failure.
    pub checkpoint_state: Option<super::CompositionCheckpoint>,
}
