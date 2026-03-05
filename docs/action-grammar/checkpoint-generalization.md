# Checkpoint Generalization: From Batch Workers to Composition Steps

*Extending the TAS-125 checkpoint model for grammar-composed virtual handlers*

*March 2026 — Research Spike*

---

## Current State

Tasker has two checkpoint mechanisms, both designed for batch processing but with different scopes:

### Legacy: `CheckpointProgress` (TAS-64)

Stored in `StepExecutionResult.results.metadata.context` — piggybacked on the step's error result. The handler fails, includes checkpoint progress in the failure context, and the retry system reads it back on re-execution.

```rust
pub struct CheckpointProgress {
    pub checkpoint_progress: u64,    // Resume position
    pub processed_before_failure: u64,
    pub resumed_from: u64,
}
```

This mechanism is **failure-driven** — the step must fail for the checkpoint to persist. It's also **integer-only** (a positional cursor) and **has no accumulated results**.

### Modern: `CheckpointRecord` (TAS-125)

Stored in a dedicated `workflow_steps.checkpoint` JSONB column. The handler yields a checkpoint *during execution* without failing. The step stays `InProgress` and is re-dispatched within the worker process.

```rust
pub struct CheckpointRecord {
    pub cursor: serde_json::Value,          // Any JSON value
    pub items_processed: u64,
    pub timestamp: DateTime<Utc>,
    pub accumulated_results: Option<serde_json::Value>,
    pub history: Vec<CheckpointHistoryEntry>,
}
```

This mechanism is **yield-driven** — the handler decides when to checkpoint. The cursor is **opaque JSON** (integers, strings, complex pagination tokens). It supports **accumulated results** for carrying forward partial aggregations. History is appended atomically in SQL.

### What's Already General

The TAS-125 `CheckpointRecord` system is remarkably well-suited for grammar composition checkpointing:

| Feature | Batch Use | Composition Use |
|---------|-----------|-----------------|
| Opaque JSON cursor | Batch position (integer, pagination token) | Composition step index + last output |
| Accumulated results | Partial aggregations across batch items | Step outputs for input mapping resolution |
| History | Record of batch yield points | Record of completed composition steps |
| Atomic SQL append | Prevents race conditions on yields | Same — sequential step completion |
| Handler-driven timing | Handler decides when to yield | Composition executor yields after each mutating step |
| Step stays InProgress | No state transition on yield | Same — the composition is still executing |

The infrastructure is already there. What's needed is:

1. A checkpoint data format specific to composition execution
2. A way for the composition executor (not the FFI dispatch channel) to access `CheckpointService`
3. Resume logic that restores composition state from a checkpoint

---

## Composition Checkpoint Design

### Checkpoint Format

```rust
/// Checkpoint data for a grammar-composed handler execution.
/// Stored in `workflow_steps.checkpoint` using the existing JSONB column.
///
/// This uses the existing CheckpointRecord infrastructure (TAS-125)
/// with a composition-specific cursor format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionCheckpoint {
    /// Which composition step just completed (0-indexed)
    pub completed_step_index: usize,

    /// Name of the capability that completed
    pub completed_capability: String,

    /// Output of the completed step — used as input for the next step on resume
    pub step_output: serde_json::Value,

    /// Outputs from all completed steps, indexed by step position.
    /// Needed for InputMapping::StepOutput and InputMapping::Merged
    /// that reference earlier (non-previous) steps.
    pub all_step_outputs: HashMap<usize, serde_json::Value>,

    /// Whether the completed step was a mutation
    pub was_mutation: bool,
}
```

This maps onto `CheckpointRecord` naturally:

| `CheckpointRecord` field | Composition value |
|--------------------------|-------------------|
| `cursor` | Serialized `CompositionCheckpoint` |
| `items_processed` | `completed_step_index + 1` |
| `accumulated_results` | `all_step_outputs` (the running state) |
| `timestamp` | Set by `CheckpointService` |
| `history` | Appended atomically by SQL — one entry per checkpoint |

### When to Checkpoint

The composition executor checkpoints after completing any step that has `checkpoint: true` in the `CompositionSpec`. The composition validation (from `composition-validation.md`) ensures that:

- All mutating capabilities have `checkpoint: true` (mandatory)
- Non-mutating capabilities may have `checkpoint: true` (optional, for expensive computations)

In practice, a typical 5-step composition with one mutation checkpoints once — after the mutation. A composition with two mutations checkpoints twice. A composition with no mutations doesn't checkpoint at all (all steps are idempotent and safe to re-execute).

### Checkpoint Flow

```
Composition Executor receives step (InProgress state)
  │
  ├─ Load existing checkpoint from workflow_steps.checkpoint
  │   ├─ None → start from step 0
  │   └─ Some(checkpoint) → resume from step (completed_step_index + 1)
  │
  ├─ For each composition step (from start_index..steps.len()):
  │   │
  │   ├─ Resolve input via composition context envelope
  │   │   ├─ .prev  → last step's output (or checkpoint.step_output on resume)
  │   │   ├─ .context → task-level input data (from step_inputs)
  │   │   ├─ .deps → dependency step results (keyed by step name)
  │   │   └─ .step → step metadata
  │   │   (Note: replaces the earlier InputMapping enum — see transform-revised-grammar.md)
  │   │
  │   ├─ Execute capability via CapabilityExecutor
  │   │   ├─ Success → store output
  │   │   └─ Failure → return error (step-level retry handles it)
  │   │
  │   ├─ If step.checkpoint == true:
  │   │   └─ Persist CompositionCheckpoint via CheckpointService
  │   │
  │   └─ Continue to next step
  │
  ├─ All steps complete → validate output against outcome schema
  │
  └─ Return StepExecutionResult::success(final_output)
```

### Resume After Failure

When a composition step fails and the step is retried (via Tasker's standard retry mechanism):

1. The composition executor loads the checkpoint from `workflow_steps.checkpoint`
2. If a checkpoint exists, it reads `completed_step_index` and `all_step_outputs`
3. Execution resumes from `completed_step_index + 1`
4. The `step_output` field provides the input for the next step (for `Previous` mapping)
5. The `all_step_outputs` map provides inputs for `StepOutput` and `Merged` mappings

**What gets re-executed on retry:**

| Scenario | What happens |
|----------|-------------|
| Failure before first checkpoint | Entire composition re-executes (all steps so far were non-mutating and idempotent) |
| Failure after checkpoint at step 3 | Steps 0-3 skipped, resume from step 4 with checkpointed state |
| Failure at a mutating step (which is a checkpoint) | The mutation either completed (checkpointed) or didn't. If checkpointed, skip it. If not, the mutation's idempotency key prevents duplicate execution. |
| Failure after the last mutation, in a non-mutating trailing step | Resume from after the last checkpoint. The trailing non-mutating steps re-execute (they're idempotent). |

The critical safety property: **no mutation is executed twice** — either it checkpointed (skip on resume) or it didn't complete (re-execute with idempotency key).

---

## Integration with CheckpointService

### Current Access Path

Today, `CheckpointService` is accessed through the `FfiDispatchChannel` — batch workers call `checkpoint_yield()` on the dispatch channel, which calls `checkpoint_service.persist_checkpoint()` internally. This path is specific to the FFI pull-based worker model.

### Composition Access Path

Grammar-composed handlers execute through the standard `StepHandler::call()` path (via the `GrammarActionResolver` → `ResolvedHandler` → `StepHandler` bridge). They need direct access to `CheckpointService` without going through the FFI dispatch channel.

The cleanest approach: provide `CheckpointService` through the `ExecutionContext` that the composition executor uses:

```rust
/// Context available during composition execution.
/// Provided by the GrammarActionResolver when constructing the handler.
pub struct CompositionExecutionContext {
    pub step_uuid: Uuid,
    pub correlation_id: String,
    pub step_inputs: serde_json::Value,

    /// Direct access to checkpoint persistence.
    /// The composition executor calls this after each checkpoint step.
    pub checkpoint_service: Arc<CheckpointService>,
}
```

The `GrammarActionResolver` receives `CheckpointService` at construction time (dependency injection, same as how `FfiDispatchChannel` receives it today) and passes it through to the composition executor.

### No New Database Schema

The existing `workflow_steps.checkpoint` JSONB column is fully sufficient. The `CompositionCheckpoint` struct serializes to JSON and fits in the same column that batch workers use. The existing indexes (`idx_workflow_steps_checkpoint_exists` and `idx_workflow_steps_checkpoint_cursor`) remain useful.

The `CheckpointService` API (`persist_checkpoint`, `get_checkpoint`, `clear_checkpoint`) is also sufficient — no new methods needed. The service operates on opaque `CheckpointYieldData`, and the composition executor maps its `CompositionCheckpoint` to that format.

### Mapping CompositionCheckpoint to CheckpointYieldData

```rust
impl CompositionCheckpoint {
    /// Convert to the format expected by CheckpointService
    fn to_yield_data(&self, step_uuid: Uuid) -> CheckpointYieldData {
        CheckpointYieldData {
            step_uuid,
            cursor: serde_json::to_value(self)
                .expect("CompositionCheckpoint is always serializable"),
            items_processed: (self.completed_step_index + 1) as u64,
            accumulated_results: Some(
                serde_json::to_value(&self.all_step_outputs)
                    .expect("step outputs are always serializable")
            ),
        }
    }

    /// Restore from a CheckpointRecord loaded by CheckpointService
    fn from_record(record: &CheckpointRecord) -> Result<Self, CheckpointError> {
        serde_json::from_value(record.cursor.clone())
            .map_err(|e| CheckpointError::DeserializationError(e.to_string()))
    }
}
```

---

## The Composition Executor

The composition executor is the component that actually runs a validated composition. It lives behind the `StepHandler` trait so it integrates with the existing dispatch pipeline:

```rust
/// Executes a validated grammar composition as a StepHandler.
///
/// Created by GrammarActionResolver for each grammar: callable resolution.
/// Wraps a ValidatedComposition with access to the capability vocabulary
/// and checkpoint service.
pub struct CompositionExecutor {
    /// The validated composition to execute
    composition: ValidatedComposition,

    /// Capability executors, looked up by name
    executor_registry: Arc<ExecutorRegistry>,

    /// For checkpoint persistence
    checkpoint_service: Arc<CheckpointService>,
}

#[async_trait]
impl StepHandler for CompositionExecutor {
    async fn call(&self, step: &TaskSequenceStep) -> TaskerResult<StepExecutionResult> {
        let step_uuid = step.workflow_step.workflow_step_uuid;

        // Load existing checkpoint (resume case)
        let resume_state = self.checkpoint_service
            .get_checkpoint(step_uuid).await?
            .and_then(|record| CompositionCheckpoint::from_record(&record).ok());

        let start_index = resume_state.as_ref()
            .map(|cp| cp.completed_step_index + 1)
            .unwrap_or(0);

        let mut step_outputs: HashMap<usize, serde_json::Value> = resume_state
            .as_ref()
            .map(|cp| cp.all_step_outputs.clone())
            .unwrap_or_default();

        let mut last_output: Option<serde_json::Value> = resume_state
            .as_ref()
            .map(|cp| cp.step_output.clone());

        // Execute composition steps
        for (i, comp_step) in self.composition.steps.iter().enumerate() {
            if i < start_index {
                continue; // Skip checkpointed steps
            }

            // Resolve input
            let input = self.resolve_input(
                &comp_step.input_mapping,
                last_output.as_ref(),
                &step_outputs,
                step,
            )?;

            // Get the executor for this capability
            let executor = self.executor_registry
                .get(&comp_step.capability)
                .ok_or_else(|| /* capability not found error */)?;

            // Execute the capability
            let output = executor
                .execute(input, comp_step.config.clone(), &self.build_context(step))
                .await
                .map_err(|e| /* map to TaskerError */)?;

            // Store output
            step_outputs.insert(i, output.clone());
            last_output = Some(output);

            // Checkpoint if required
            if comp_step.checkpoint {
                let checkpoint = CompositionCheckpoint {
                    completed_step_index: i,
                    completed_capability: comp_step.capability.clone(),
                    step_output: last_output.clone().unwrap_or_default(),
                    all_step_outputs: step_outputs.clone(),
                    was_mutation: self.is_mutating(&comp_step.capability),
                };

                self.checkpoint_service
                    .persist_checkpoint(step_uuid, &checkpoint.to_yield_data(step_uuid))
                    .await?;
            }
        }

        // Build final result from last output
        let final_output = last_output.unwrap_or(serde_json::Value::Null);

        Ok(StepExecutionResult::success(
            serde_json::to_string(&final_output)?,
            Some(final_output),
        ))
    }

    fn name(&self) -> &str {
        self.composition.name.as_deref().unwrap_or("grammar:inline")
    }
}
```

This is a sketch — the actual implementation would handle errors more carefully, apply mixin behaviors (retry, observability), and validate the final output against the outcome schema. But the structure is correct: load checkpoint → skip completed steps → execute remaining → checkpoint at boundaries → return result.

---

## Comparison: Batch Checkpoints vs. Composition Checkpoints

| Aspect | Batch Workers | Composition Steps |
|--------|---------------|-------------------|
| **What's being iterated** | Items in a dataset | Steps in a capability chain |
| **Cursor semantics** | Position in dataset (integer, pagination token) | Index of last completed step |
| **Accumulated results** | Partial aggregation of item processing | Map of all step outputs (for input mapping) |
| **Who decides when to checkpoint** | Handler code (`items_per_checkpoint`) | Composition spec (`checkpoint: true` on step) |
| **Checkpoint frequency** | Every N items (handler-driven) | After each mutating step (spec-driven) |
| **Resume behavior** | Re-execute from cursor position | Skip completed steps, execute from next |
| **Step state during checkpoint** | InProgress (re-dispatched in worker) | InProgress (no re-dispatch needed — sequential execution) |
| **Access to CheckpointService** | Via FfiDispatchChannel | Via CompositionExecutionContext |
| **Storage** | `workflow_steps.checkpoint` JSONB | Same column, same format |

The key difference: batch workers checkpoint *within* a single logical operation (iterating items), while compositions checkpoint *between* distinct operations (sequential capability executions). But the persistence mechanism, storage format, and resume semantics are the same.

---

## What's Needed to Implement This

### Already exists (no changes needed)

- `workflow_steps.checkpoint` JSONB column with indexes
- `CheckpointService` with `persist_checkpoint`, `get_checkpoint`, `clear_checkpoint`
- `CheckpointRecord` and `CheckpointYieldData` types
- Atomic SQL history append
- Step state machine — no special handling needed for grammar-composed steps

### New code needed

1. **`CompositionCheckpoint` type** — the composition-specific cursor format (small, straightforward)
2. **`CompositionExecutor`** — the `StepHandler` implementation that runs compositions with checkpoint support
3. **`CheckpointService` access in `GrammarActionResolver`** — dependency injection at resolver construction
4. **Conversion methods** — `CompositionCheckpoint ↔ CheckpointYieldData` mapping

### What does NOT need to change

- Database schema — existing column is sufficient
- `CheckpointService` API — existing methods are sufficient
- Step state machine — grammar-composed steps use `Standard` type (same as instantiated batch workers)
- Retry semantics — Tasker's step-level retry handles composition retry naturally
- Worker architecture — composition executor runs as a normal `StepHandler`

---

## Edge Cases

### Checkpoint with no mutations

A composition with zero mutations never checkpoints (no mutating steps to mark as checkpoint boundaries). If it fails, it re-executes entirely. This is correct — all steps are non-mutating and idempotent, so full re-execution is safe and semantically identical to partial re-execution.

A developer can opt into checkpointing expensive non-mutating steps by setting `checkpoint: true`, but this is an optimization, not a correctness requirement.

### Multiple mutations in sequence

A composition with two mutations (both checkpointed) handles sequential failure correctly:

1. Steps 0-2 (non-mutating) execute
2. Step 3 (Persist, checkpoint) completes and checkpoints
3. Step 4 (another Persist, checkpoint) fails
4. On retry: steps 0-3 are skipped (checkpointed), step 4 re-executes with idempotency key
5. Steps 5+ continue

### Checkpoint storage growth

Each checkpoint overwrites the previous one (same SQL column). The history array grows by one entry per checkpoint. For a typical composition with 1-3 checkpoints, history size is negligible. Even a pathological composition with many checkpoints would produce a history of perhaps a few KB — well within JSONB column limits.

### Concurrent execution

Composition steps execute sequentially within a single `StepHandler::call()` invocation. There is no concurrent access to the checkpoint for a given step. The atomic SQL append pattern from TAS-125 provides additional safety, but sequential execution means it's not strictly needed for compositions (unlike batch workers where concurrent yields could theoretically overlap).

---

*This proposal should be read alongside `actions-traits-and-capabilities.md` for how the CompositionExecutor integrates with the resolver chain, `composition-validation.md` for how compositions are validated before execution, and `transform-revised-grammar.md` for the current 6-capability model with jaq-core expression language and composition context envelope.*
