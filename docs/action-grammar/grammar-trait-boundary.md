# Grammar Trait Boundary Design

*Proposal for the `GrammarCategory` trait system and its integration with Tasker's existing infrastructure*

*March 2026 — Research Spike*

---

## Design Principles

1. **Extend, don't rewrite.** The handler dispatch system (`ResolverChain` → `StepHandlerResolver` → `ResolvedHandler` → `StepHandler`) already provides the extensibility we need. Grammar actions integrate through this chain, not alongside it.

2. **Object-safe by default.** The grammar trait must support `dyn` dispatch for build-from-source extensibility. Follow the `CircuitBreakerBehavior` and `ResolvedHandler` patterns — no generic methods, no RPIT, no `Self` in non-receiver position.

3. **JSON Schema as the contract language.** Compile-time Rust generics are powerful but close the system. JSON Schema contracts are open, agent-discoverable, and already proven in our `schema_comparator` and `schema_diff` work.

4. **Checkpoint-native.** Interior mutations use the existing `CheckpointService` and `checkpoint` JSONB column. No new persistence infrastructure needed.

5. **Public traits, build-from-source extensibility.** Organizations extend the grammar by depending on Tasker's public crates, implementing traits, and building their own binary — the same model as custom handler registration today. No dynamic library loading infrastructure needed. The traits are the contract; good documentation is the extension mechanism.

---

## Integration with Existing Handler Dispatch

The cleanest integration point is the `ResolverChain`. A grammar-composed handler enters the system through the same dispatch pipeline as any other handler:

```
Template YAML
  └─ StepDefinition.handler.callable = "grammar:http_acquire_validate"
                                        ─────── ─────────────────────
                                        prefix   composition name

ResolverChain
  ├─ ExplicitMappingResolver (priority 10)   → no match
  ├─ GrammarActionResolver (priority 15)     → MATCH on "grammar:" prefix
  │   └─ Looks up composition spec from registry
  │   └─ Validates composition (contract chaining, checkpoint coverage)
  │   └─ Returns Arc<dyn ResolvedHandler> wrapping the composition
  ├─ [custom domain resolvers] (priority 20-99)
  └─ ClassConstantResolver (priority 100)    → fallback
```

This means:
- **No changes to `StepDefinition`** — `handler.callable` already accepts arbitrary strings
- **No changes to the step state machine** — grammar-composed steps go through the same 10-state lifecycle
- **No changes to `HandlerDispatchService`** — it dispatches `Arc<dyn StepHandler>` regardless of provenance
- **Virtual and domain handlers coexist in the same template** — the resolver chain handles both

### The `GrammarActionResolver`

```rust
/// Resolves grammar-composed handlers from the capability vocabulary.
/// Registered in the ResolverChain at priority 15 (after explicit mappings,
/// before domain resolvers and class constant fallback).
pub struct GrammarActionResolver {
    /// The composition registry — maps composition names to validated specs
    compositions: Arc<RwLock<HashMap<String, ValidatedComposition>>>,

    /// The capability vocabulary — registered capabilities by name
    vocabulary: Arc<CapabilityVocabulary>,

    /// The grammar registry — registered grammar categories
    grammars: Arc<GrammarRegistry>,
}

#[async_trait]
impl StepHandlerResolver for GrammarActionResolver {
    fn can_resolve(&self, definition: &HandlerDefinition) -> bool {
        definition.callable.starts_with("grammar:")
    }

    async fn resolve(
        &self,
        definition: &HandlerDefinition,
        context: &ResolutionContext,
    ) -> Option<Arc<dyn ResolvedHandler>> {
        let composition_name = definition.callable.strip_prefix("grammar:")?;

        // Look up pre-validated composition, or validate inline composition
        // from handler.initialization if present
        let composition = self.resolve_composition(composition_name, definition)?;

        Some(Arc::new(GrammarResolvedHandler {
            composition,
            vocabulary: Arc::clone(&self.vocabulary),
        }))
    }

    fn resolver_name(&self) -> &str { "grammar_action" }
    fn priority(&self) -> u32 { 15 }

    fn registered_callables(&self) -> Vec<String> {
        self.compositions.read().ok()
            .map(|c| c.keys().map(|k| format!("grammar:{k}")).collect())
            .unwrap_or_default()
    }
}
```

### Inline Compositions

Named compositions are pre-registered and looked up by name. But agents also need to compose ad-hoc. For inline compositions, the composition spec is provided in `handler.initialization`:

```yaml
steps:
  - name: custom_analysis
    handler:
      callable: "grammar:inline"
      initialization:
        composition:
          outcome:
            description: "Validated analysis results"
            output_schema: { type: object, required: [results], properties: { results: { type: array } } }
          steps:
            - capability: acquire
              config:
                resource:
                  type: api
                  endpoint: "${step_inputs.endpoint}"
                  method: GET
                validate_success:
                  status: { in: [200] }
                result_shape: [data.records]
            - capability: transform
              output:
                type: object
                required: [results]
                properties:
                  results: { type: array, items: { type: object } }
              filter: |
                .prev.data.records
                | map(select(.active))
                | {results: .}
            - capability: validate
              config:
                schema: { $ref: "record_v2" }
                on_failure: fail
          mixins: [with_retry, with_observability]
```

The resolver detects `grammar:inline`, extracts the composition spec from `initialization`, validates it, and constructs the handler. This is the primary path for agent-composed workflows. See [`transform-revised-grammar.md`](transform-revised-grammar.md) for the full revised capability model with jaq filters and composition context envelope.

---

## The Grammar Layer

### `GrammarCategory` Trait

The grammar category declares what *kind* of action this is and what properties it guarantees. This is the trait that extension authors implement when adding domain-specific grammar categories.

```rust
/// A category of action in the grammar.
///
/// Grammar categories define the *kind* of action (Acquire, Transform, Validate,
/// Persist, Emit) and declare what properties actions of this kind guarantee.
/// This is the extension point for organizations that need domain-specific
/// action categories.
///
/// Object-safe: suitable for dyn dispatch and build-from-source extensibility.
pub trait GrammarCategory: Send + Sync + fmt::Debug {
    /// Unique name of this grammar category (e.g., "Acquire", "Transform",
    /// "Validate", "Persist", "Emit"). Must be stable — capability
    /// registrations reference this name.
    fn name(&self) -> &str;

    /// Human-readable description for agent discoverability.
    fn description(&self) -> &str;

    /// The mutation profile of this category.
    fn mutation_profile(&self) -> MutationProfile;

    /// Whether actions of this category are inherently idempotent,
    /// or require explicit idempotency strategies.
    fn idempotency(&self) -> IdempotencyProfile;

    /// Whether capabilities in this category require checkpoint support
    /// when used in compositions with multiple mutations.
    fn requires_checkpointing(&self) -> bool;

    /// JSON Schema for configuration that capabilities in this category accept.
    /// Used to validate capability registrations.
    fn config_schema(&self) -> &serde_json::Value;

    /// Validate that a capability declaration is compatible with this category's
    /// constraints. Called when a capability is registered against this category.
    fn validate_capability_declaration(
        &self,
        declaration: &CapabilityDeclaration,
    ) -> Vec<ValidationFinding>;

    /// Optional: additional composition rules specific to this category.
    /// For example, a "Persist" category might require that it's preceded
    /// by a Validate step, or an organization-defined category might impose
    /// ordering constraints on subsequent steps.
    fn composition_constraints(&self) -> Vec<CompositionConstraint> {
        Vec::new()
    }
}
```

### `MutationProfile` and `IdempotencyProfile`

```rust
/// How a grammar category relates to external state mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationProfile {
    /// Never mutates external state. Safe to re-execute freely.
    /// Examples: Acquire, Transform, Validate
    NonMutating,

    /// Mutates external state. Requires checkpoint tracking in compositions
    /// with multiple mutations. The mutation contributes to the step's
    /// singular outcome.
    Mutating {
        /// Whether this category supports idempotency keys to prevent
        /// duplicate mutations on retry.
        supports_idempotency_key: bool,
    },

    /// Mutation behavior depends on configuration. The capability declaration
    /// must specify its concrete mutation profile.
    /// Example: an organization-defined category where mutation depends on config
    ConfigDependent,
}

/// How a grammar category relates to idempotency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdempotencyProfile {
    /// Inherently idempotent — safe to re-execute with same inputs,
    /// produces same outputs. No special handling needed.
    Inherent,

    /// Idempotent when provided with an idempotency key. The system
    /// generates or accepts a key and ensures at-most-once execution.
    WithKey,

    /// The capability must declare its own idempotency strategy.
    /// Used for categories where idempotency depends on the specific
    /// external system being interacted with.
    CapabilityDefined,
}
```

### Core Grammar Categories

The system ships with these built-in categories. Each is a struct implementing `GrammarCategory`:

| Category | Mutation | Idempotency | Checkpoint | Purpose |
|----------|----------|-------------|------------|---------|
| `Acquire` | NonMutating | Inherent | No | Fetch data from external sources |
| `Transform` | NonMutating | Inherent | No | Pure data transformation via jaq (jq) filters |
| `Validate` | NonMutating | Inherent | No | Assert invariants, validate schemas, gate execution |
| `Persist` | Mutating | WithKey | Yes | Write state to external systems |
| `Emit` | Mutating | WithKey | Yes | Send notifications or events |

These five grammar categories map to the 6 core capabilities (see [`transform-revised-grammar.md`](transform-revised-grammar.md) for the full revised model):

- **Acquire**: `acquire` capability (side-effecting, fetches external data)
- **Transform**: `transform` capability (pure data transformation — `output` JSON Schema + `filter` jaq expression)
- **Validate**: `validate`, `assert` capabilities (schema validation and execution gating)
- **Persist**: `persist` capability (side-effecting, writes to external systems)
- **Emit**: `emit` capability (side-effecting, sends events/notifications)

The grammar categories are the abstract structural layer; capabilities are concrete invocations with typed Rust envelopes and JSON Schema-flexible contents. They share a typology but operate at different abstraction levels.

Organizations extend this by implementing the trait. A financial services company might add `Reconcile`. A data pipeline company might add `Enrich`. An ML platform might add `Infer`.

### `GrammarRegistry`

```rust
/// Registry of known grammar categories.
/// Populated at startup from built-in categories and plugin-provided categories.
pub struct GrammarRegistry {
    categories: RwLock<HashMap<String, Arc<dyn GrammarCategory>>>,
}

impl GrammarRegistry {
    /// Register a grammar category. Called at startup for built-ins,
    /// and by custom binaries for build-from-source extensions.
    pub fn register(&self, category: Arc<dyn GrammarCategory>) -> Result<(), RegistrationError>;

    /// Look up a grammar category by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn GrammarCategory>>;

    /// List all registered categories (for agent discoverability).
    pub fn list(&self) -> Vec<Arc<dyn GrammarCategory>>;
}
```

---

## The Vocabulary Layer

### `CapabilityDeclaration`

A capability is a concrete, registered implementation within a grammar category:

```rust
/// A registered capability in the vocabulary.
///
/// Capabilities are the concrete, composable units that agents discover
/// and assemble into workflows. Each capability belongs to a grammar
/// category and declares its contracts via JSON Schema.
///
/// Every capability expresses a deterministic (action, resource, context) triple:
/// - **Action**: What to do (one of the 6 core capabilities: transform, validate,
///   assert, persist, acquire, emit)
/// - **Resource**: The target upon which the action is effected
/// - **Context**: Configuration, constraints, success validation, result shape
///
/// The `transform` capability uses `output` (JSON Schema) + `filter` (jaq expression)
/// for pure data transformation. `validate` uses JSON Schema + coercion/failure config.
/// `assert` uses a jaq boolean filter + error message. Action capabilities (persist,
/// acquire, emit) use a typed Rust envelope with JSON Schema-flexible contents for
/// resource, data, constraints, validate_success, and result_shape.
///
/// See `transform-revised-grammar.md` for the rationale behind the 6-capability model
/// (which replaced the earlier 9-capability model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDeclaration {
    /// Unique identifier (e.g., "http_get", "postgres_upsert", "json_extract")
    pub name: String,

    /// The canonical action this capability performs (e.g., "acquire", "persist",
    /// "validate", "transform", "assert", "emit")
    pub action: String,

    /// Which grammar category this belongs to (e.g., "Acquire", "Persist")
    pub grammar_category: String,

    /// Human-readable description for agent discoverability
    pub description: String,

    /// JSON Schema: what this capability accepts as input
    pub input_schema: serde_json::Value,

    /// JSON Schema: what this capability produces as output
    pub output_schema: serde_json::Value,

    /// JSON Schema: configuration parameters for this capability.
    /// For `transform`: config contains `output` (JSON Schema declaring the
    /// promised output shape) and `filter` (jaq expression producing the output).
    /// For `validate`: JSON Schema + coercion/filtering/failure config.
    /// For `assert`: `filter` (jaq boolean expression) + `error` message.
    /// For action capabilities (persist, acquire, emit): typed envelope with
    /// resource, data/params/payload (jaq filters), constraints, validate_success,
    /// result_shape.
    pub config_schema: serde_json::Value,

    /// Concrete mutation profile (must be compatible with grammar category).
    /// For ConfigDependent categories, this resolves the ambiguity.
    pub mutation_profile: MutationProfile,

    /// Retry behavior specific to this capability
    pub retry_profile: RetryProfile,

    /// Tags for capability discovery (e.g., ["http", "rest", "api"])
    pub tags: Vec<String>,

    /// Version of this capability declaration
    pub version: String,
}
```

### `CapabilityVocabulary`

```rust
/// The discoverable vocabulary of capabilities.
///
/// This is the primary interface for agents composing workflows.
/// MCP tools expose this vocabulary for search and inspection.
pub struct CapabilityVocabulary {
    capabilities: RwLock<HashMap<String, CapabilityDeclaration>>,
    grammar_registry: Arc<GrammarRegistry>,
}

impl CapabilityVocabulary {
    /// Register a capability. Validates against its grammar category's constraints.
    pub fn register(
        &self,
        declaration: CapabilityDeclaration,
    ) -> Result<(), RegistrationError>;

    /// Look up a capability by name.
    pub fn get(&self, name: &str) -> Option<CapabilityDeclaration>;

    /// Search capabilities by grammar category.
    pub fn by_category(&self, category: &str) -> Vec<CapabilityDeclaration>;

    /// Search capabilities by tags.
    pub fn by_tags(&self, tags: &[&str]) -> Vec<CapabilityDeclaration>;

    /// List all capabilities (for agent discoverability).
    pub fn list(&self) -> Vec<CapabilityDeclaration>;

    /// Get the full vocabulary as a JSON document suitable for
    /// inclusion in LLM context or MCP tool responses.
    pub fn to_discovery_document(&self) -> serde_json::Value;
}
```

### `CapabilityExecutor` Trait

The bridge between a capability declaration and actual execution:

```rust
/// Executes a capability against concrete inputs.
///
/// Separate from CapabilityDeclaration because the declaration is
/// data (serializable, discoverable) while the executor is behavior
/// (may hold connections, state, configuration).
///
/// The executor receives the (action, resource, context) triple as
/// resolved by the composition executor: input carries the resource
/// data, config carries the context (constraints, result_shape, etc.),
/// and the action is implicit in the capability identity.
#[async_trait]
pub trait CapabilityExecutor: Send + Sync + fmt::Debug {
    /// Execute this capability with the given input and config.
    /// For action capabilities, config contains the typed envelope
    /// (resource, data, constraints, validate_success, result_shape).
    /// Returns the output conforming to the capability's output_schema.
    async fn execute(
        &self,
        input: serde_json::Value,
        config: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError>;

    /// The capability name this executor handles.
    fn capability_name(&self) -> &str;
}

/// Context available during capability execution.
///
/// The virtual handler boundary: input is StepContext (represented here
/// as step_uuid, correlation_id, step_config), output must conform to
/// the result_schema declared in the StepDefinition.
pub struct ExecutionContext {
    /// Step UUID for checkpoint tracking
    pub step_uuid: Uuid,

    /// Task correlation ID
    pub correlation_id: String,

    /// Checkpoint service for persisting intermediate state
    pub checkpoint: Arc<CheckpointService>,

    /// The composition-level checkpoint state (for resume)
    pub checkpoint_state: Option<CheckpointRecord>,

    /// Configuration from the step's initialization params (StepContext)
    pub step_config: serde_json::Value,
}
```

---

## Composition Model

A virtual handler is ONLY a composition of action grammar primitives. It cannot invoke domain handler logic. The composition boundary is:
- **Input boundary**: StepContext (the step's inputs, configuration, and correlation context)
- **Output boundary**: result_schema declared in the StepDefinition

TaskTemplate DAGs freely mix virtual handler steps (grammar-composed) and domain handler steps (traditional callables) in the same workflow.

### `CompositionSpec`

```rust
/// A composed virtual handler — a chain of capabilities toward a singular outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSpec {
    /// Optional name for registered compositions
    pub name: Option<String>,

    /// The declared singular outcome
    pub outcome: OutcomeDeclaration,

    /// Ordered sequence of capability invocations
    pub steps: Vec<CompositionStep>,

    /// Cross-cutting concerns (retry, observability, timeout, etc.)
    pub mixins: Vec<String>,
}

/// A single step within a composition.
///
/// Each composition step expresses the (action, resource, context) triple
/// from the capability vocabulary. For `transform`, config contains `output`
/// (JSON Schema) + `filter` (jaq expression). For `assert`, config contains
/// `filter` (jaq boolean) + `error` message. For `validate`, config contains
/// JSON Schema + coercion/failure settings. For action capabilities (persist,
/// acquire, emit), the config contains a typed envelope with resource,
/// data/params/payload (jaq filters), constraints, validate_success, and
/// result_shape fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionStep {
    /// Which capability to invoke (must exist in vocabulary)
    pub capability: String,

    /// Configuration for this invocation.
    /// For `transform`: `output` (JSON Schema) + `filter` (jaq expression).
    /// For `assert`: `filter` (jaq boolean) + `error` message.
    /// For `validate`: JSON Schema + coercion/failure config.
    /// For action capabilities: typed envelope with resource, data/params/payload
    /// (jaq filters), constraints, validate_success, result_shape.
    pub config: serde_json::Value,

    /// How this step's input is resolved.
    ///
    /// **Revised model**: In the 6-capability model (see `transform-revised-grammar.md`),
    /// explicit input mapping is superseded by the **composition context envelope**.
    /// jaq filters access `.context` (task input), `.deps.{step_name}` (dependency
    /// results), `.prev` (previous capability output), and `.step` (step metadata)
    /// directly, making the `InputMapping` enum unnecessary. This field is retained
    /// for backward compatibility with the original design sketch.
    pub input_mapping: InputMapping,

    /// Whether this is a checkpoint boundary.
    /// Required for mutating capabilities. Optional for non-mutating
    /// (useful for expensive computations worth preserving).
    #[serde(default)]
    pub checkpoint: bool,
}

/// How a composition step receives its input.
///
/// **Note**: This enum has been superseded by the **composition context envelope**
/// model in `transform-revised-grammar.md`. In the revised model, jaq filters
/// access the full composition context directly:
/// - `.context` — task input data (replaces `TaskContext`)
/// - `.deps.{step_name}` — dependency step results (replaces `StepOutput`)
/// - `.prev` — previous capability invocation output (replaces `Previous`)
/// - `.step` — step metadata (name, attempts, inputs)
///
/// This makes explicit input mapping unnecessary — the jaq filter itself
/// determines what data it reads. The `Merged` variant is also superseded,
/// since a jaq filter can freely reference `.context`, `.deps`, and `.prev`
/// in a single expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputMapping {
    /// Input comes from the previous step's output (default for linear chains)
    Previous,

    /// Input comes from a specific earlier step's output, by index
    StepOutput { step_index: usize },

    /// Input comes from task context / step_inputs
    TaskContext { path: String },

    /// Input is composed from multiple sources
    Merged { sources: Vec<InputMapping> },
}

/// The declared outcome of a composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeDeclaration {
    /// Human-readable description of what this composition achieves
    pub description: String,

    /// JSON Schema for what the composition produces
    pub output_schema: serde_json::Value,
}
```

### Composition Validation

```rust
/// Validates a composition spec against the vocabulary and grammar rules.
pub struct CompositionValidator {
    vocabulary: Arc<CapabilityVocabulary>,
    grammar_registry: Arc<GrammarRegistry>,
}

impl CompositionValidator {
    /// Validate a composition, returning all findings.
    /// A composition with any Error-level findings is rejected.
    pub fn validate(&self, spec: &CompositionSpec) -> CompositionValidationReport {
        let mut findings = Vec::new();

        // 1. Capability existence
        for (i, step) in spec.steps.iter().enumerate() {
            if self.vocabulary.get(&step.capability).is_none() {
                findings.push(finding(Error, format!(
                    "Step {i}: capability '{}' not found in vocabulary",
                    step.capability
                )));
            }
        }

        // 2. Configuration validity
        // Each step's config is validated against its capability's config_schema

        // 3. Contract chaining
        // Output schema of step N must be compatible with input schema of step N+1.
        // In the revised model (transform-revised-grammar.md), `transform` steps
        // declare an `output` JSON Schema. The validator checks that `.prev`
        // references in downstream jaq filters are compatible with the declared
        // output schema of the preceding step. This replaces the ad-hoc inference
        // that was needed when each capability had its own bespoke config format.
        // Uses the same JSON Schema compatibility logic as schema_comparator.

        // 4. Checkpoint coverage
        // Every mutating capability must be a checkpoint boundary
        for (i, step) in spec.steps.iter().enumerate() {
            if let Some(cap) = self.vocabulary.get(&step.capability) {
                if matches!(cap.mutation_profile, MutationProfile::Mutating { .. })
                    && !step.checkpoint
                {
                    findings.push(finding(Error, format!(
                        "Step {i}: mutating capability '{}' must be a checkpoint boundary",
                        step.capability
                    )));
                }
            }
        }

        // 5. Outcome convergence
        // Final step's output must be compatible with declared outcome schema

        // 6. Grammar-specific composition constraints
        // Each category's composition_constraints() are checked

        // 7. Input mapping resolution
        // Every input_mapping resolves to an available data source.
        // In the revised model, this becomes: validate that jaq filter references
        // to .context, .deps.{name}, and .prev resolve to declared data sources.

        CompositionValidationReport { findings, valid: !has_errors(&findings) }
    }
}
```

---

## Checkpoint Integration

Grammar-composed steps use the existing `CheckpointService` with a composition-aware checkpoint format:

```rust
/// Checkpoint data for grammar-composed handlers.
/// Stored in the existing workflow_steps.checkpoint JSONB column.
/// See checkpoint-generalization.md for the full checkpoint design.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompositionCheckpoint {
    /// Which composition step just completed (0-indexed)
    pub completed_step_index: usize,

    /// Name of the capability that completed
    pub completed_capability: String,

    /// Output of the completed step (input for the next step on resume)
    pub step_output: serde_json::Value,

    /// Accumulated outputs from all completed steps, indexed by step position.
    /// In the original model, needed for InputMapping::StepOutput and
    /// InputMapping::Merged. In the revised model (transform-revised-grammar.md),
    /// this supports checkpoint resumption — restoring `.prev` from the last
    /// completed capability invocation's output.
    pub all_step_outputs: HashMap<usize, serde_json::Value>,

    /// Whether the completed step was a mutation
    pub was_mutation: bool,
}
```

On resume after failure:
1. The `CompositionExecutor` loads the checkpoint
2. Skips steps 0..=`completed_step_index`
3. Feeds `step_output` as input to step `completed_step_index + 1`
4. Uses `all_step_outputs` if needed for non-linear data references
5. Continues execution from there

This means non-mutating steps before a mutation are re-executed on retry (they're idempotent, so this is safe), while mutating steps that checkpointed are skipped. The checkpoint boundary is the resume point.

---

## MCP Integration

The vocabulary and grammar registry expose naturally through MCP tools, extending the existing Tier 1 developer tools:

| Tool | Purpose |
|------|---------|
| `grammar_list` | List registered grammar categories with their properties |
| `capability_search` | Search capabilities by category, tags, or free text |
| `capability_inspect` | Full details of a capability: schemas, mutation profile, retry behavior |
| `composition_validate` | Validate a composition spec before workflow submission |
| `composition_suggest` | Given an outcome description, suggest capabilities that could compose toward it |
| `vocabulary_document` | Full vocabulary as a structured document for LLM context |

These are offline tools (Tier 1) — they operate against the local grammar and vocabulary registries. An agent composing a workflow queries these tools to discover capabilities, validate compositions, and iterate toward a valid workflow.

---

## Extensibility Model: Build from Source

Organizations extend the grammar and vocabulary by depending on Tasker's public crates, implementing the traits, and building their own binary. This follows the same model as custom handler registration today — no dynamic library loading needed.

### How it works

1. **Depend on `tasker-shared`** (or a future `tasker-grammar` crate) for the public traits
2. **Implement `GrammarCategory`** for domain-specific action categories
3. **Implement `CapabilityExecutor`** for domain-specific capabilities
4. **Register at startup** alongside standard handler registrations

```rust
// In a custom worker binary's main.rs:
fn register_extensions(
    grammar_registry: &GrammarRegistry,
    vocabulary: &CapabilityVocabulary,
    executor_registry: &ExecutorRegistry,
) {
    // Register a domain-specific grammar category
    grammar_registry.register(Arc::new(ReconcileCategory::new()))
        .expect("failed to register Reconcile category");

    // Register capabilities against that category
    vocabulary.register(CapabilityDeclaration {
        name: "ledger_reconcile".into(),
        grammar_category: "Reconcile".into(),
        description: "Reconcile ledger entries against bank feed".into(),
        // ... schemas, profiles, etc.
    }).expect("failed to register capability");

    // Register the executor
    executor_registry.register(Arc::new(LedgerReconcileExecutor::new()));
}
```

### Why not dynamic library loading

- **No stable ABI problem.** Rust's ABI is not stable across versions. Building from source means the compiler handles compatibility naturally.
- **No runtime loading failure modes.** Missing `.so`, ABI mismatch, symbol resolution failure — none of these exist when everything is compiled together.
- **Standard Rust tooling.** `cargo build`, `cargo test`, dependency management via `Cargo.toml`. No custom plugin discovery infrastructure.
- **The traits are the contract.** Well-documented public traits are a more idiomatic Rust extension mechanism than dynamic loading.
- **The existing pattern works.** Custom Rust workers already build from source against `tasker-worker` and register handlers at startup. Grammar extensions follow the same model.

The only capability this foregoes is hot-loading — adding grammar categories without rebuilding. But deploying a new binary is standard practice for any change that affects execution behavior, and the rebuild is incremental (only the custom crate and its binary recompile).

Dynamic library loading could be added later if demand warrants it — the trait boundary is object-safe and supports `dyn` dispatch — but it's not a first-class concern for the initial design.

---

## What This Does NOT Change

- **Step state machine**: Grammar-composed steps use the same 10-state lifecycle
- **Handler dispatch pipeline**: `ResolverChain` → `StepHandlerResolver` → `ResolvedHandler` → `StepHandler`
- **Checkpoint storage**: Existing `workflow_steps.checkpoint` JSONB column
- **Template format**: `StepDefinition.handler.callable` already accepts arbitrary strings
- **Worker architecture**: Dispatch channel, completion channel, semaphore-bounded parallelism
- **FFI workers**: Continue working exactly as they do — they're domain handlers, not grammar-composed
- **Batch processing**: `Batchable` and `BatchWorker` step types are orthogonal to grammar composition
- **Domain handlers**: Operations where the (action, resource, context) triple cannot be deterministically expressed remain as traditional domain handlers — the grammar does not attempt to subsume them

---

## Open Questions

1. **~~Should compositions support conditional steps?~~** Resolved. jaq handles field/value-level conditionals via `if-elif-else` within `transform` filters. Execution gating (proceed or fail the composition) is handled by `assert` capability steps with jaq boolean filters. See [`transform-revised-grammar.md`](transform-revised-grammar.md).

2. **~~How should the `Merged` input mapping work?~~** Superseded. The composition context envelope provides `.context`, `.deps.{step_name}`, `.prev`, and `.step` — jaq filters access all of these directly in a single expression, eliminating the need for explicit input mapping. See [`transform-revised-grammar.md`](transform-revised-grammar.md).

3. **Should the `GrammarActionResolver` cache validated compositions?** Inline compositions (`grammar:inline`) require validation on every resolution. Caching valid compositions by content hash would improve performance for repeated template executions.

4. **What does the executor registration look like?** Capability declarations are data; executors are behavior. How are they paired? A separate `ExecutorRegistry` that maps capability names to executors? Or bundled in capability registration?

5. **How do compositions interact with Tasker's retry semantics?** The step-level retry (from `RetryConfiguration`) retries the entire composition. Should composition steps have their own retry behavior (via the capability's `retry_profile`), or is step-level retry sufficient?

6. **What's the right JSON Schema compatibility model?** Full structural subtyping? Subset checking? The `schema_comparator` already implements one model — should compositions use the same logic, or do we need something different?

---

*This proposal should be read alongside `actions-traits-and-capabilities.md` for the architectural rationale, `transform-revised-grammar.md` for the 6-capability model with jaq filters and composition context envelope, `composition-validation.md` for JSON Schema contract chaining mechanics, `checkpoint-generalization.md` for the checkpoint integration design, `virtual-handler-dispatch.md` for how the composition executor integrates with the worker dispatch infrastructure, and `phase-0-completion-assessment.md` for the current state of the system.*
