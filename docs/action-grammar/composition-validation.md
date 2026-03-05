# Composition Validation: JSON Schema Contract Chaining

*How compositions are validated at assembly time using JSON Schema contracts*

*March 2026 — Research Spike*

---

## Overview

When an agent or developer composes a virtual handler from the capability vocabulary, the composition must be validated before it can be executed. This validation happens at **assembly time** — when the `GrammarActionResolver` resolves a `grammar:` callable — not at compile time and not at execution time.

The validator checks that capabilities exist, configurations are valid, contracts chain correctly, mutations are checkpointed, and the outcome converges. It reuses the JSON Schema compatibility logic already proven in `tasker-sdk`'s `schema_comparator`.

---

## What Gets Validated

### 1. Capability Existence

Every capability referenced in the composition must exist in the vocabulary.

```yaml
steps:
  - capability: acquire           # ✓ exists
  - capability: transform         # ✓ exists
  - capability: quantum_teleport  # ✗ not in vocabulary
```

**Finding**: `Error — capability 'quantum_teleport' not found in vocabulary`

This is the simplest check and the first one performed. If a capability doesn't exist, downstream checks are skipped for that step.

### 2. Configuration Validity

Each step's `config` must validate against the capability's declared `config_schema`.

```yaml
# Capability "acquire" declares config_schema:
#   required: [resource]
#   properties:
#     resource: { type: string }              # The target resource
#     constraints: { type: object }           # Resource-specific constraints
#     validate_success: { type: object }      # How to verify acquisition
#     result_shape: { type: object }          # Expected output structure

steps:
  - capability: acquire
    config:
      resource: "https://api.example.com/records"
      constraints:
        timeout_ms: 5000
        method: GET           # ✓ valid

  - capability: acquire
    config:
      resource: "https://api.example.com/records"
      constraints:
        method: POST          # ✗ acquire is non-mutating; POST implies mutation
                               # (a mutating HTTP call belongs in a persist capability)
```

**Finding**: `Error — step 1 config: acquire capability does not permit mutating methods`

Standard JSON Schema validation. We can use the `jsonschema` crate for this — it's already a transitive dependency via our existing schema validation work.

### 3. Contract Chaining

The output schema of step N must be **compatible** with the input schema of step N+1. This is the core of composition validation.

#### What "compatible" means

We use **structural compatibility** — the same model as `schema_comparator`:

- Every **required** field in the consumer's input schema must exist in the producer's output schema
- Shared fields must have **compatible types** (same type, or producer type is a subtype)
- **Extra fields** in the producer's output are permitted (the consumer ignores them)
- **Optional fields** in the consumer's input are permitted to be absent from the producer's output

This is deliberately not exact-match. It's closer to structural subtyping: "does the producer provide everything the consumer needs?" Extra data flows through; missing optional data is fine; missing required data or type mismatches are errors.

#### Worked Example

```yaml
# Capability "acquire" output_schema (result_shape):
#   type: object
#   required: [status_code, body, headers]
#   properties:
#     status_code: { type: integer }
#     body: { type: object }           # raw JSON body
#     headers: { type: object }
#     elapsed_ms: { type: integer }

# Capability "transform" declares output schema per-step:
#   The output JSON Schema is declared inline on each transform step,
#   enabling static contract chaining without running jaq filters.

# Capability "validate" input_schema:
#   type: object
#   required: [data]
#   properties:
#     data: {}                          # the data to validate
#     schema_ref: { type: string }     # injected from config, not from input

steps:
  - capability: acquire
    config:
      resource: "https://api.example.com/records"
      constraints: { timeout_ms: 5000 }
    # output: { status_code, body, headers, elapsed_ms? }

  - capability: transform
    output:
      type: object
      required: [records]
      properties:
        records: { type: array }
    filter: |
      {records: .prev.body.data.records}
    # The output schema declares what the jaq filter produces.
    # Contract chaining uses this schema, not the filter itself.
    # output: { records }

  - capability: validate
    config:
      schema: "record_v2"
      on_failure: partition
    # The composition context envelope provides .prev automatically.
    # validate reads .prev.records — but its input_schema expects { data }.
    # ✗ 'data' is required but .prev only has { records }
```

**Finding**: `Error — step 1→2 contract mismatch: consumer requires field 'data' but producer output does not contain it`

#### Resolving the mismatch: jaq filter or output naming

The issue above is that `transform` outputs `records` but `validate` expects `data`. With the composition context envelope model (see `transform-revised-grammar.md`), the resolution is straightforward — either rename the output field in the transform step to match what validate expects, or use a jaq filter within validate's configuration to map the field:

```yaml
  # Option 1: Name the transform output to match validate's expectation
  - capability: transform
    output:
      type: object
      required: [data]
      properties:
        data: { type: array }
    filter: |
      {data: .prev.body.data.records}

  # Option 2: Validate reads from .prev using its own field convention
  - capability: validate
    config:
      schema: "record_v2"
      on_failure: partition
      data_path: ".prev.records"    # capability-level config for input resolution
```

With jaq filters accessing the composition context envelope (`.context`, `.deps`, `.prev`, `.step`) directly, explicit `InputMapping` field mappings are less necessary — the jaq filter itself handles data selection and field renaming inline. The `Mapped` variant from the original design is superseded by jaq's native path traversal within the composition context.

### 4. Checkpoint Coverage

Every mutating capability must be marked as a checkpoint boundary:

```yaml
steps:
  - capability: acquire
    config:
      resource: "https://api.example.com/data"
    checkpoint: false           # ✓ non-mutating, checkpoint optional

  - capability: transform
    output:
      type: object
      required: [records]
      properties:
        records: { type: array }
    filter: |
      {records: .prev.body.data}
    checkpoint: false           # ✓ non-mutating

  - capability: persist
    config:
      resource: "records_table"
      data: "$.previous.records"
      constraints: { conflict_key: "external_id" }
    checkpoint: false           # ✗ mutating capability without checkpoint!
```

**Finding**: `Error — step 2: mutating capability 'persist' (grammar: Persist) must be a checkpoint boundary`

The validator looks up each capability's `mutation_profile` (inherited from its grammar category or overridden in the declaration) and enforces that mutating capabilities have `checkpoint: true`.

Non-mutating capabilities *may* set `checkpoint: true` as an optimization — useful for expensive computations whose results are worth preserving on retry — but it's not required.

### 5. Outcome Convergence

The composition's final step output must be compatible with the declared outcome schema:

```yaml
outcome:
  description: "Validated records from external API"
  output_schema:
    type: object
    required: [valid_records, invalid_count]
    properties:
      valid_records: { type: array, items: { type: object } }
      invalid_count: { type: integer }

steps:
  # ... steps that end with validate, which outputs:
  #   { valid: [...], invalid: [...], total_checked: int }
```

**Finding**: `Error — outcome mismatch: declared outcome requires 'valid_records' but final step produces 'valid'`

This catches the common case where the composition's internal naming doesn't match the declared external contract. The fix is either renaming the outcome schema fields to match, or adding a final `transform` step that maps the output fields to the declared outcome names.

### 6. Grammar-Specific Composition Constraints

Each grammar category can declare additional composition constraints via `composition_constraints()`. For example:

- **Validate**: A `validate` that produces partitioned results (valid/invalid) should have its partition-awareness reflected in downstream input mappings
- **Assert**: An `assert` with `on_failure: halt` terminates the composition — no steps should follow it unless the failure mode allows continuation
- **Emit**: An `emit` capability is typically a terminal step — it fires a domain event and should not have downstream steps that depend on its output for data flow

These are warnings by default (not errors) — they represent best practices, not hard rules. Organizations can configure them as errors if they want stricter enforcement.

### 7. Input Mapping Resolution

> **Note**: With the composition context envelope model (see `transform-revised-grammar.md`), each capability invocation receives the full context (`.context`, `.deps`, `.prev`, `.step`) and jaq filters handle data selection directly. The explicit `InputMapping` enum is superseded for runtime data threading, but the validator still checks that jaq filter references resolve to available data sources. The checks below apply to the composition context model:

- `.prev` references: Valid if there is a preceding invocation (not valid for the first invocation — `.prev` is `null`)
- `.deps.<step_name>` references: Valid if the named step exists in the task's dependency graph
- `.context` references: Valid (the path is checked at execution time against actual task context)
- `.step` references: Always valid (step metadata is always available)

```yaml
steps:
  - capability: acquire         # index 0
  - capability: transform       # index 1
    output: { type: object, required: [records], properties: { records: { type: array } } }
    filter: '{records: .prev.body.data}'     # ✓ .prev is acquire's output
  - capability: validate        # index 2
    config:
      schema: "record_v2"
    # validator checks: .prev references transform's declared output — valid
```

---

## The `Mapped` Input Mapping (Superseded by Composition Context Envelope)

> **Note**: The `InputMapping` enum described below was part of the original 9-capability design. With the adoption of jaq-core and the composition context envelope model (see `transform-revised-grammar.md`), explicit input mapping is largely superseded. Each capability invocation receives the full composition context (`.context`, `.deps`, `.prev`, `.step`) and jaq filters handle field selection, renaming, and cross-step references inline. The `InputMapping` type may still exist as a lightweight hint for the `CompositionValidator`'s contract chaining analysis, but the runtime data threading is handled entirely by the jaq filter operating on the composition context.

The original design revealed a need beyond simple input mapping types when capability input/output field names don't align. The explicit `Mapped` variant addressed this:

```rust
/// How a composition step receives its input.
/// NOTE: With the composition context envelope model, jaq filters access
/// .context, .deps, .prev, and .step directly. This enum is retained for
/// validator hints but is no longer the primary data-threading mechanism.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputMapping {
    /// Input is the previous step's full output (default for linear chains)
    Previous,

    /// Input is a specific earlier step's full output, by index
    StepOutput { step_index: usize },

    /// Input comes from task context / step_inputs
    TaskContext { path: String },

    /// Input is composed from multiple sources via explicit field mapping
    Mapped { fields: HashMap<String, String> },

    /// Input is a deep merge of multiple sources
    Merged { sources: Vec<InputMapping> },
}
```

With jaq-core, the equivalent of `Mapped` field references is expressed directly in the jaq filter:

- `".prev.extracted"` — previous invocation's `extracted` field (was `"$.previous.extracted"`)
- `".deps.step_name.body.data"` — a dependency step's nested field (was `"$.steps[0].body.data"`)
- `".context.threshold"` — task context field (was `"$.context.step_inputs.threshold"`)

The validator can still analyze jaq filter expressions to determine which fields from the composition context are referenced, enabling contract chaining validation without the explicit mapping enum.

---

## Validation Report Format

The validator produces a structured report compatible with the existing `ValidationFinding` pattern from `tasker-sdk`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CompositionValidationReport {
    /// Whether the composition is valid (no Error-level findings)
    pub valid: bool,

    /// All findings, ordered by step index
    pub findings: Vec<CompositionFinding>,

    /// Summary statistics
    pub step_count: usize,
    pub mutation_count: usize,
    pub checkpoint_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompositionFinding {
    /// Error, Warning, or Info
    pub severity: Severity,

    /// Machine-readable code (e.g., "MISSING_CAPABILITY", "CONTRACT_MISMATCH",
    /// "CHECKPOINT_REQUIRED", "OUTCOME_MISMATCH")
    pub code: String,

    /// Which composition step this applies to (None for composition-level findings)
    pub step_index: Option<usize>,

    /// Human-readable message
    pub message: String,

    /// For contract mismatches: the specific fields involved
    pub field_path: Option<String>,

    /// Whether this finding is actionable by an agent
    /// (can the agent fix this by adjusting the composition?)
    pub actionable: bool,
}
```

This format is designed for both human readability (via the `message` field) and agent consumption (via structured `code`, `step_index`, and `field_path` fields). An agent iterating on a composition can parse the findings, identify what's wrong, and adjust — the MCP `composition_validate` tool returns this report directly.

---

## Reuse from `schema_comparator`

The contract chaining logic is fundamentally the same as what `schema_comparator` already does for producer/consumer schema comparison. The core algorithm:

1. Walk the consumer's required properties
2. For each, check if the producer's output has a matching property
3. If present, check type compatibility (same type, or compatible via JSON Schema type hierarchy)
4. If absent and required, emit `MISSING_REQUIRED_FIELD` (breaking)
5. If absent and optional, emit `MISSING_OPTIONAL_FIELD` (non-breaking)
6. Recurse into nested objects

The composition validator calls this same logic for each step transition, with the addition of:
- **Input mapping resolution** (resolving `Mapped` field sources before comparison)
- **Config injection** (some fields come from config, not from the previous step's output)
- **Multi-source merging** (for `Merged` input mappings, the combined output of all sources is the effective producer schema)

This means the composition validator can *depend on* the existing `schema_comparator` module rather than reimplementing schema comparison. The `compare_schemas` function takes producer and consumer schemas and returns a `ComparisonReport` — exactly what's needed for each step transition in a composition.

---

## Worked Example: Full Composition

> **Updated**: This example uses the 6-capability model with jaq filters and the composition context envelope. See `transform-revised-grammar.md` for the full revised grammar specification.

A realistic composition that an agent might create for "fetch records from an API, validate them, and persist the valid ones":

```yaml
name: fetch_validate_persist_records
outcome:
  description: "Fetch records from external API, validate against schema, persist valid records"
  output_schema:
    type: object
    required: [persisted_count, invalid_count]
    properties:
      persisted_count: { type: integer }
      invalid_count: { type: integer }

steps:
  - capability: acquire
    config:
      resource:
        type: api
        endpoint: "${context.api_endpoint}"
        method: GET
      constraints:
        headers:
          Authorization: "Bearer ${context.api_token}"
        timeout_ms: 10000
      result_shape:
        type: object
        required: [body]
    checkpoint: false

  - capability: transform
    output:
      type: object
      required: [records]
      properties:
        records: { type: array, items: { type: object } }
    filter: |
      {records: .prev.body.data.records}
    checkpoint: false

  - capability: validate
    config:
      schema: "${context.record_schema}"
      on_failure: partition
    # validate reads .prev.records from the transform step's output
    checkpoint: false

  - capability: persist
    config:
      resource:
        type: database
        entity: "${context.target_table}"
      constraints:
        conflict_key: "external_id"
        idempotency_key: "${context.correlation_id}"
      validate_success:
        min_rows: 1
      result_shape: [persisted_count]
    data: |
      .prev.valid
    checkpoint: true    # Required: mutating capability

  - capability: transform
    output:
      type: object
      required: [persisted_count, invalid_count]
      properties:
        persisted_count: { type: integer }
        invalid_count: { type: integer }
    filter: |
      {
        persisted_count: .prev.persisted_count,
        invalid_count: (.prev.invalid // [] | length)
      }
    checkpoint: false

mixins: [with_retry, with_observability]
```

### Validation trace for this composition:

| Check | Step | Result |
|-------|------|--------|
| Capability existence | 0-4 | All 5 invocations use capabilities from the 6-capability vocabulary |
| Config validity | 0 | `resource`, `constraints`, `result_shape` match acquire config_schema |
| Config validity | 1 | `output` is valid JSON Schema, `filter` is valid jaq expression |
| Config validity | 2 | `schema`, `on_failure` match validate config_schema |
| Config validity | 3 | `resource`, `constraints`, `validate_success`, `result_shape` match persist config_schema; `data` is valid jaq |
| Config validity | 4 | `output` is valid JSON Schema, `filter` is valid jaq expression |
| Contract chain | 0→1 | acquire outputs `{body, ...}` — transform filter reads `.prev.body` — compatible |
| Contract chain | 1→2 | transform declares output `{records}` — validate reads `.prev.records` — compatible |
| Contract chain | 2→3 | validate outputs `{valid, invalid, total_checked}` — persist data reads `.prev.valid` — compatible |
| Contract chain | 3→4 | persist outputs `{persisted_count}` — transform filter reads `.prev.persisted_count` — compatible |
| Checkpoint coverage | 3 | persist is Persist/Mutating, checkpoint=true — valid |
| Outcome convergence | 4→outcome | transform declares output `{persisted_count, invalid_count}` — matches declared outcome — compatible |
| Grammar constraints | all | No category-specific constraint violations |

**Result**: Valid composition. 5 invocations, 1 mutation, 1 checkpoint.

---

## Performance Considerations

Composition validation runs at resolution time — when the `GrammarActionResolver` processes a `grammar:` callable. For inline compositions, this happens on every task execution. For named compositions, validation can be cached.

**Expected cost per validation**:
- Capability lookups: O(n) hash map lookups where n = number of steps (microseconds)
- Config validation: O(n) JSON Schema validations (low milliseconds)
- Contract chaining: O(n) schema comparisons, each walking the property tree (low milliseconds)
- Total: well under 10ms for a typical 3-7 step composition

**Caching strategy**: Named compositions can be validated once at registration time and cached. Inline compositions should be cached by content hash if the same template is executed repeatedly. The cache key is the SHA-256 of the serialized `CompositionSpec`.

---

## Open Questions (from trait boundary proposal, refined)

### Retry semantics: step-level vs. composition-step-level

The Tasker step-level retry (from `RetryConfiguration` on `StepDefinition`) retries the *entire composition* from the last checkpoint. Should individual composition steps have their own retry behavior?

**Proposal**: Yes, but limited. Non-mutating capability invocations (`acquire`, `transform`, `validate`, `assert`) can retry internally based on their capability's `retry_profile` — this handles transient failures, rate limits, etc. Mutating invocations (`persist`, `emit`) do NOT retry internally — failure at a mutation boundary triggers the step-level retry, which resumes from the checkpoint. This keeps the checkpoint model clean: checkpoints are the resume points, and the step-level retry is the only mechanism that crosses checkpoint boundaries.

### Conditional steps within compositions

Should a composition support "if condition X from step N's output, skip step M"?

**Proposal**: Not in the initial design. Conditional logic within a composition adds significant complexity to validation (branches in the contract chain, multiple possible output schemas). If a workflow needs conditional branching, that's what Tasker's step-level decision patterns are for — the composition produces its output, and separate steps use `transform` capabilities (with boolean/conditional jaq filters) to route to different downstream steps. Keep compositions as linear chains toward a singular outcome.

If real-world usage reveals this is too limiting, conditional steps can be added later as a composition feature without changing the trait boundary or the grammar categories.

### JSON Schema compatibility model

**Proposal**: Use the same structural compatibility model as `schema_comparator` — required field presence, type compatibility, recursive nested checking. This is already implemented, tested, and proven. The `compare_schemas` function can be called directly by the composition validator.

One addition needed: **type coercion rules**. JSON Schema's type hierarchy (`integer` is compatible with `number`, for example) should be respected in contract chaining. The `schema_comparator` may need a small extension for this, but the model is the same.

---

*This proposal should be read alongside `actions-traits-and-capabilities.md` for the trait design and architectural rationale, `checkpoint-generalization.md` for how compositions checkpoint during execution, and `transform-revised-grammar.md` for the finalized 6-capability model with jaq-core expressions and the composition context envelope.*
