//! Standalone composition executor.
//!
//! The [`CompositionExecutor`] chains capability executions according to a
//! [`CompositionSpec`](crate::types), threading the composition context envelope
//! (`.context`, `.deps`, `.prev`, `.step`) through each step.
//!
//! This executor is a pure data transformation — it knows nothing about workers,
//! queues, handlers, or orchestration. The worker integration layer wraps this
//! executor as a `StepHandler`.
//!
//! # Execution model
//!
//! 1. Build the initial envelope from task input (`.context`), dependency results
//!    (`.deps`), and step metadata (`.step`). `.prev` starts as `null`.
//! 2. For each capability invocation in the composition:
//!    a. Resolve the step's input — the full envelope is passed to the capability
//!    executor, which uses jaq expressions to select fields from `.context`,
//!    `.deps`, `.prev`, and `.step`.
//!    b. Look up the capability executor by name.
//!    c. Execute the capability with the envelope and invocation config.
//!    d. Store the invocation's output in the accumulated context (for cross-step
//!    references via `.deps.invocations.{index}`).
//!    e. Update `.prev` to this invocation's output.
//!    f. If the invocation has `checkpoint: true`, record a [`CompositionCheckpoint`].
//! 3. Return the final invocation's output as the composition result.
//!
//! # Resume from checkpoint
//!
//! Given a [`CompositionCheckpoint`] (which records the last completed invocation
//! index and accumulated outputs), the executor restores the envelope state and
//! resumes execution from the next invocation. Already-completed invocations are
//! skipped entirely.
//!
//! # Error model
//!
//! A capability failure at any invocation produces a [`CompositionError::InvocationFailure`]
//! wrapping the underlying [`CapabilityError`] with the invocation index and
//! capability name for structured error reporting.
//!
//! **Ticket**: TAS-334

use std::collections::HashMap;
use std::fmt;

use serde_json::Value;

use crate::types::{
    CapabilityExecutor, CompositionCheckpoint, CompositionEnvelope, CompositionError,
    CompositionSpec, ExecutionContext,
};

/// Result of a composition execution, including the output value and any
/// checkpoints emitted during execution.
#[derive(Debug, Clone)]
pub struct CompositionResult {
    /// The final output value produced by the last invocation.
    pub output: Value,

    /// Checkpoints emitted during execution, in order. Each checkpoint records
    /// the state after a checkpoint-marked invocation completes.
    pub checkpoints: Vec<CompositionCheckpoint>,
}

/// Standalone composition executor that chains capability executions.
///
/// The executor holds a registry of capability executors indexed by name.
/// It is constructed once and reused across multiple composition executions.
///
/// # Usage
///
/// ```
/// use std::collections::HashMap;
/// use serde_json::json;
/// use tasker_grammar::ExpressionEngine;
/// use tasker_grammar::capabilities::transform::TransformExecutor;
/// use tasker_grammar::types::{
///     CapabilityExecutor, CompositionSpec, CapabilityInvocation, OutcomeDeclaration,
/// };
/// use tasker_grammar::executor::{CompositionExecutor, CompositionInput};
///
/// // Build executor with registered capabilities
/// let engine = ExpressionEngine::with_defaults();
/// let executor = CompositionExecutor::builder()
///     .register("transform", TransformExecutor::new(engine))
///     .build();
///
/// // Define a simple composition
/// let spec = CompositionSpec {
///     name: Some("example".to_owned()),
///     outcome: OutcomeDeclaration {
///         description: "Double the input value".to_owned(),
///         output_schema: json!({"type": "object"}),
///     },
///     invocations: vec![
///         CapabilityInvocation {
///             capability: "transform".to_owned(),
///             config: json!({
///                 "filter": "{doubled: (.context.value * 2)}",
///                 "output": {"type": "object"}
///             }),
///             checkpoint: false,
///         },
///     ],
/// };
///
/// // Execute
/// let input = CompositionInput {
///     context: json!({"value": 21}),
///     deps: json!({}),
///     step: json!({"name": "test_step"}),
/// };
/// let result = executor.execute(&spec, input.clone(), "test_step", 1).unwrap();
/// assert_eq!(result.output, json!({"doubled": 42}));
/// ```
pub struct CompositionExecutor {
    /// Registered capability executors, keyed by capability name.
    capabilities: HashMap<String, Box<dyn CapabilityExecutor>>,
}

impl fmt::Debug for CompositionExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompositionExecutor")
            .field(
                "capabilities",
                &self.capabilities.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Input data for a composition execution.
///
/// These three fields populate the composition envelope's immutable sections.
/// `.prev` is managed by the executor and starts as `null`.
#[derive(Debug, Clone)]
pub struct CompositionInput {
    /// Task-level input data — populates `.context` in the envelope.
    pub context: Value,

    /// Dependency step results — populates `.deps` in the envelope.
    pub deps: Value,

    /// Step metadata — populates `.step` in the envelope.
    pub step: Value,
}

/// Builder for constructing a [`CompositionExecutor`] with registered capabilities.
#[derive(Debug, Default)]
pub struct CompositionExecutorBuilder {
    capabilities: HashMap<String, Box<dyn CapabilityExecutor>>,
}

impl CompositionExecutorBuilder {
    /// Register a capability executor under its declared name.
    ///
    /// The name is taken from [`CapabilityExecutor::capability_name`].
    pub fn register(mut self, name: &str, executor: impl CapabilityExecutor + 'static) -> Self {
        self.capabilities
            .insert(name.to_owned(), Box::new(executor));
        self
    }

    /// Build the executor with all registered capabilities.
    pub fn build(self) -> CompositionExecutor {
        CompositionExecutor {
            capabilities: self.capabilities,
        }
    }
}

impl CompositionExecutor {
    /// Create a builder for constructing a `CompositionExecutor`.
    pub fn builder() -> CompositionExecutorBuilder {
        CompositionExecutorBuilder::default()
    }

    /// Execute a composition spec against the given input.
    ///
    /// Returns the final invocation's output and any checkpoints emitted.
    /// An empty composition (zero invocations) returns the context input unchanged.
    pub fn execute(
        &self,
        spec: &CompositionSpec,
        input: CompositionInput,
        step_name: &str,
        attempt: u32,
    ) -> Result<CompositionResult, CompositionError> {
        // Empty composition: return context unchanged
        if spec.invocations.is_empty() {
            return Ok(CompositionResult {
                output: input.context,
                checkpoints: Vec::new(),
            });
        }

        let accumulated_outputs: HashMap<usize, Value> = HashMap::new();

        self.execute_from(
            spec,
            &input,
            step_name,
            attempt,
            0,           // start from first invocation
            Value::Null, // no previous output
            accumulated_outputs,
        )
    }

    /// Resume a composition from a checkpoint.
    ///
    /// Restores the envelope state from the checkpoint's accumulated outputs
    /// and resumes execution from the invocation after the checkpoint.
    pub fn resume(
        &self,
        spec: &CompositionSpec,
        checkpoint: &CompositionCheckpoint,
        input: &CompositionInput,
        step_name: &str,
        attempt: u32,
    ) -> Result<CompositionResult, CompositionError> {
        let resume_index = checkpoint.completed_invocation_index + 1;

        // If the checkpoint is at the last invocation, the composition is already done
        if resume_index >= spec.invocations.len() {
            return Ok(CompositionResult {
                output: checkpoint.invocation_output.clone(),
                checkpoints: Vec::new(),
            });
        }

        // Restore accumulated outputs from checkpoint
        let accumulated_outputs = checkpoint.all_invocation_outputs.clone();
        let prev = checkpoint.invocation_output.clone();

        self.execute_from(
            spec,
            input,
            step_name,
            attempt,
            resume_index,
            prev,
            accumulated_outputs,
        )
    }

    /// Core execution loop shared by `execute` and `resume`.
    fn execute_from(
        &self,
        spec: &CompositionSpec,
        input: &CompositionInput,
        step_name: &str,
        attempt: u32,
        start_index: usize,
        initial_prev: Value,
        mut accumulated_outputs: HashMap<usize, Value>,
    ) -> Result<CompositionResult, CompositionError> {
        let mut prev = initial_prev;
        let mut checkpoints = Vec::new();

        for (idx, invocation) in spec.invocations.iter().enumerate().skip(start_index) {
            // Build the envelope for this invocation
            let envelope_value = build_envelope(
                &input.context,
                &input.deps,
                &input.step,
                &prev,
                &accumulated_outputs,
            );
            let envelope = CompositionEnvelope::new(&envelope_value);

            // Look up the capability executor
            let executor = self
                .capabilities
                .get(&invocation.capability)
                .ok_or_else(|| {
                    CompositionError::Validation(format!(
                        "capability '{}' not found in executor registry",
                        invocation.capability
                    ))
                })?;

            // Build the execution context for this invocation
            let exec_context = ExecutionContext {
                step_name: step_name.to_owned(),
                attempt,
                checkpoint_state: None,
            };

            // Execute the capability
            let output = executor
                .execute(&envelope, &invocation.config, &exec_context)
                .map_err(|cause| CompositionError::InvocationFailure {
                    invocation_index: idx,
                    capability: invocation.capability.clone(),
                    cause,
                })?;

            // Store output in accumulated context
            accumulated_outputs.insert(idx, output.clone());

            // Update prev for the next invocation
            prev = output;

            // Emit checkpoint if marked
            if invocation.checkpoint {
                checkpoints.push(CompositionCheckpoint {
                    completed_invocation_index: idx,
                    completed_capability: invocation.capability.clone(),
                    invocation_output: prev.clone(),
                    all_invocation_outputs: accumulated_outputs.clone(),
                    was_mutation: is_mutating_capability(&invocation.capability),
                });
            }
        }

        Ok(CompositionResult {
            output: prev,
            checkpoints,
        })
    }

    /// List the names of all registered capabilities.
    pub fn registered_capabilities(&self) -> Vec<&str> {
        self.capabilities.keys().map(String::as_str).collect()
    }

    /// Check whether a capability is registered.
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.contains_key(name)
    }
}

/// Build the composition envelope value for a single invocation.
///
/// The envelope contains:
/// - `.context` — task-level input (immutable)
/// - `.deps` — dependency step results (immutable), with an additional
///   `.invocations` sub-key containing accumulated invocation outputs
/// - `.step` — step metadata (immutable)
/// - `.prev` — output of the previous invocation (or null for first)
fn build_envelope(
    context: &Value,
    deps: &Value,
    step: &Value,
    prev: &Value,
    accumulated: &HashMap<usize, Value>,
) -> Value {
    // Build invocation outputs map for cross-step references
    let invocation_outputs: serde_json::Map<String, Value> = accumulated
        .iter()
        .map(|(idx, output)| (idx.to_string(), output.clone()))
        .collect();

    // Merge invocation outputs into deps under "invocations" key
    let mut deps_with_invocations = match deps {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    if !invocation_outputs.is_empty() {
        deps_with_invocations.insert("invocations".to_owned(), Value::Object(invocation_outputs));
    }

    serde_json::json!({
        "context": context,
        "deps": deps_with_invocations,
        "step": step,
        "prev": prev,
    })
}

/// Determine if a capability name corresponds to a mutating capability.
///
/// The canonical mutating capabilities are `persist` and `emit`.
/// This is a heuristic for checkpoint metadata — the authoritative mutation
/// profile lives in `GrammarCategory::mutation_profile`.
fn is_mutating_capability(name: &str) -> bool {
    matches!(name, "persist" | "emit")
}

#[cfg(test)]
mod tests;
