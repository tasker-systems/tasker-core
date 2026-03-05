# Action Grammar Implementation Phases

*Phased roadmap from research spike to production-ready grammar compositions*

*March 2026 — Implementation Planning*

---

## Guiding Principle

**Build confidence from the bottom up.** Grammar compositions must be independently buildable, independently testable, and independently validatable before they participate in worker dispatch, queue routing, or orchestration changes. Each phase produces artifacts that are useful and demonstrable on their own — no phase exists only as scaffolding for the next.

---

## Phase 1: Grammar Primitives & Independent Testing

*Goal: Implement the 6 core capabilities (transform, validate, assert, persist, acquire, emit), prove they compose correctly, and validate the jaq-core expression language — all without touching the worker lifecycle.*

This phase produces a standalone `tasker-grammar` crate — a new workspace member with no dependencies on `tasker-worker`, `tasker-orchestration`, or database/messaging infrastructure. The grammar system is a distinct responsibility: expression evaluation, capability execution, composition validation, and composition execution are all pure data transformations that operate on `serde_json::Value` inputs and produce `Value` outputs. The crate can be tested with `cargo test` against pure data — no database, no messaging, no workers.

`tasker-grammar` depends on:
- `jaq-core` / `jaq-std` (expression engine)
- `serde` / `serde_json` (serialization)
- `jsonschema` (JSON Schema validation)
- Avoid `tasker-shared` for shared types (e.g., `StepExecutionError`). Define grammar-specific error types to avoid coupling. Other crates can transform error types to crate-specific types at the boundary.

`tasker-worker` depends on `tasker-grammar` to access the `CompositionExecutor` and wrap it as a `StepHandler`. `tasker-sdk` depends on `tasker-grammar` to access the `CompositionValidator` for template validation tooling. The grammar crate itself knows nothing about workers, queues, handlers, or orchestration.

```
tasker-grammar (new)          ← pure data transformation library
  ├── types/                  ← GrammarCategory, CapabilityDeclaration, CompositionSpec
  ├── expression/             ← ExpressionEngine (jaq-core wrapper, sandboxing)
  ├── capabilities/           ← 6 capability executors (transform, validate, assert, persist, acquire, emit)
  ├── validation/             ← CompositionValidator (contract chaining, schema checks)
  └── executor/               ← CompositionExecutor (standalone, not StepHandler)

tasker-worker                 ← depends on tasker-grammar
  └── composition_handler.rs  ← Wraps CompositionExecutor as StepHandler
                                 (adds CheckpointService, TaskSequenceStep mapping)

tasker-sdk                    ← depends on tasker-grammar
  └── composition_validate.rs ← Exposes CompositionValidator for ctl/mcp tooling
```

### 1A: Expression Language Integration — jaq-core (Sequential — blocks 1B)

**Decision (finalized)**: Use `jaq-core` as the unified expression language for all grammar capabilities. This decision is confirmed — see `transform-revised-grammar.md` for how jaq-core integration led to the 6-capability model.

jq syntax is widely adopted, well-understood, and provides a single consistent language for path traversal, data transformation, arithmetic, boolean logic, and aggregation. Rather than stitching together separate libraries for path selection (JSONPath), computation (evalexpr), and boolean evaluation (CEL), jq covers all three concerns natively. This eliminates the cognitive overhead of switching between expression syntaxes across capabilities and gives template authors one language to learn.

**Why jq (via jaq-core)**:

| Concern | jq Capability | Example |
|---------|--------------|---------|
| Path traversal (`transform`) | Native | `.items[].price`, `.customer.address.city` |
| Field projection (`transform`) | Native | `{total: .subtotal + .tax, items: .line_items}` |
| Arithmetic (`transform`) | Native | `.items | map(.price * .quantity) | add` |
| Aggregation (`transform`) | Native | `[.records[] | .amount] | add`, `length` |
| String construction (`transform`) | Native | `"Order \(.order_id) confirmed"` |
| Boolean expressions (`transform`) | Native | `.amount > 1000 and .status == "pending"` |
| Conditional selection (`transform`) | Native | `if .tier == "gold" then "priority" else "standard" end` |
| Assertions (`assert`) | Native | `.total == (.subtotal + .tax)`, `.items | length > 0` |
| Rule matching (`transform`) | Native | First-match via jq if-elif-else chains |
| Payload construction (`emit`, `persist`) | Native | `{event_name: "order.confirmed", payload: {id: .order_id}}` |
| Cross-step references | Convention | `.deps.validate_cart.total`, `.prev.subtotal`, `.context.customer_id` |

**jaq-core specifics**:
- Pure Rust implementation (no C dependencies)
- MIT licensed, actively maintained
- Implements core jq language (filters, pipes, conditionals, try-catch, reduce, functions)
- Operates on `serde_json::Value` directly — zero-copy integration with our existing JSON infrastructure
- Streaming evaluation model — bounded memory for large inputs

**The "overkill" concern and why it doesn't matter**:

jq is more expressive than strictly necessary for any single capability. With the 6-capability model (see `transform-revised-grammar.md`), the `transform` capability intentionally embraces this expressiveness — a single `transform` step can project fields, compute derived values, evaluate conditions, and match rules, all in one jaq filter. The grammar categories (Transform, Validate, Persist, etc.) provide structural guidance; the expression language provides power. Composition authors can choose whether to use one large `transform` or multiple focused ones — either way, the composition validates against the declared `output` schemas and produces the correct result.

This is the same principle as any general-purpose language: you *can* write SQL in a single expression, but good practice separates concerns. Our MCP tooling and documentation guide authors toward clean separation; the grammar system doesn't need to enforce it.

**Sandboxing requirements**:

jq filters can be complex. For grammar evaluation, we need:
- **Execution timeout**: Bounded wall-clock time per filter evaluation (configurable, default ~100ms)
- **Output size limit**: Cap output `Value` size to prevent memory exhaustion from recursive expansion
- **No I/O**: jq in `jaq-core` has no file/network access — it operates on in-memory `Value` only (safe by construction)
- **Error propagation**: `jaq-core` produces structured errors on malformed filters — surfaceable in validation tooling

**Cross-step data referencing convention — Composition Context Envelope**:

When a capability expression references data from other steps, the composition executor assembles a **composition context envelope** (see `transform-revised-grammar.md` for the full specification):

```json
{
  "context": { /* task.task.context — the original task input data */ },
  "deps": {
    "validate_cart": { "total": 99.99, "validated_items": [ ... ] },
    "process_payment": { "payment_id": "pay_123" }
  },
  "step": { "name": "create_order", "attempts": 1, "inputs": null },
  "prev": null
}
```

- `.context` — the original task input data (immutable across invocations)
- `.deps` — dependency step results keyed by step name (immutable across invocations)
- `.step` — step metadata: name, attempt count, inputs (immutable across invocations)
- `.prev` — output of the most recent capability invocation (`null` for the first; **updated after each invocation**)

Expressions reference this context naturally: `.context.customer_id`, `.deps.validate_cart.total`, `.prev.subtotal`. The composition context envelope replaces the earlier `InputMapping` enum as the primary data-threading mechanism — jaq filters access whatever they need directly from the envelope.

**Deliverables**:
- Integrate `jaq-core` as a workspace dependency
- Build a thin `ExpressionEngine` wrapper that compiles jq filters, evaluates against `Value`, and enforces timeout/size limits
- Unit tests exercising each capability's expression patterns (path traversal, arithmetic, boolean, aggregation, string interpolation)
- Error message quality validation (malformed filters produce actionable diagnostics)

**Ticket**: `TAS-xxx: Integrate jaq-core expression engine with sandboxing and capability-pattern tests`

### 1B: Core Type Definitions (Sequential — blocks 1C, 1D)

Define the foundational types that all subsequent work depends on. These are data structures only — no execution logic yet.

```
GrammarCategory (trait)
  ├── name() → &str
  ├── description() → &str
  ├── mutation_profile() → MutationProfile
  ├── idempotency() → IdempotencyGuarantee
  ├── requires_checkpointing() → bool
  ├── config_schema() → Value (JSON Schema)
  ├── validate_capability_declaration() → Result<(), Vec<Finding>>
  └── composition_constraints() → CompositionConstraints

CapabilityDeclaration (data, serializable)
  ├── name: String
  ├── action: String
  ├── grammar_category: String
  ├── config_schema: Value (JSON Schema for capability config)
  ├── input_schema: Value (JSON Schema for expected input)
  └── output_schema: Value (JSON Schema for produced output)

CompositionSpec (data, serializable)
  ├── grammar: String (category name)
  ├── compose: Vec<CompositionStep>
  ├── outcome_schema: Option<Value>
  └── checkpoint_markers: Vec<usize>

CompositionStep
  ├── capability: String
  ├── config: Value              # capability-specific config (e.g., resource for persist)
  ├── output: Option<Value>      # JSON Schema declaring output shape (required for transform)
  ├── filter: Option<String>     # jaq expression (required for transform, assert)
  └── checkpoint: bool

# NOTE: InputMapping is superseded by the composition context envelope.
# Each capability invocation receives the full context (.context, .deps, .prev, .step)
# and jaq filters handle data selection directly. InputMapping may be retained as a
# lightweight validator hint but is no longer the primary data-threading mechanism.
# See transform-revised-grammar.md for details.

CapabilityExecutor (trait)
  ├── name() → &str
  ├── execute(input: &Value, config: &Value, ctx: &ExecutionContext) → Result<Value>
  └── validate_config(config: &Value) → Result<(), Vec<Finding>>
```

These types live in `tasker-grammar` and have zero runtime dependencies — pure data + trait definitions.

**Tickets**:
- `TAS-xxx: Scaffold tasker-grammar crate with workspace integration`
- `TAS-xxx: Define grammar core types (GrammarCategory, CapabilityDeclaration, CompositionSpec, CapabilityExecutor)`

### 1C: Capability Executor Implementations (Parallelizable after 1B)

Implement each of the 6 capability executors independently. Each executor is a pure function: `(input: Value, config: Value) → Result<Value>`. No database, no messaging, no worker context.

> **Updated**: The original 9-capability model has been consolidated to 6 capabilities. `reshape`, `compute`, `evaluate`, and `evaluate_rules` are replaced by a single `transform` capability that uses jaq filters with JSON Schema output declarations. See `transform-revised-grammar.md` for the rationale.

These can be developed in parallel by multiple contributors once the types from 1B and the `jaq-core` expression engine from 1A are in place.

**Data capabilities** (pure, no side effects):

| Ticket | Capability | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | `transform` | Medium | jaq filter execution + JSON Schema output validation. Replaces reshape, compute, evaluate, evaluate_rules. |
| `TAS-xxx` | `validate` | Medium | JSON Schema validation, coercion modes, partition (valid/invalid) |
| `TAS-xxx` | `assert` | Medium | jaq boolean filter evaluation; gates execution (proceed or fail) |

**Action capabilities** (side-effecting — but in this phase, tested with mocks/stubs):

| Ticket | Capability | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | `persist` | High | Resource abstraction layer + jaq data filter. Phase 1 tests with in-memory stub. |
| `TAS-xxx` | `acquire` | High | Resource abstraction layer + jaq result filter. Phase 1 tests with fixture data. |
| `TAS-xxx` | `emit` | Medium | Domain event construction + jaq payload filter. Phase 1 tests event shape only, no actual publishing. |

Each capability ticket includes:
- Executor implementation
- Config schema definition (JSON Schema)
- Input/output schema definitions
- Unit tests with representative data
- Error handling (malformed config, type mismatches, expression errors)

### 1D: Composition Engine (Sequential after 1B, parallel with 1C)

The composition engine chains capabilities together, threading output → input with schema validation at each boundary.

**Composition Validator** — Validates a `CompositionSpec` is well-formed without executing it:
- Capability existence check (all referenced capabilities are registered)
- Config validation (each step's config validates against capability's config_schema)
- Contract chaining (`output` schemas on `transform` steps enable static analysis — the validator checks that each invocation's declared output schema is compatible with the next invocation's expected input, without executing jaq filters)
- Checkpoint coverage (mutating capabilities have checkpoint markers)
- Outcome convergence (final invocation's output schema compatible with declared outcome_schema)
- jaq filter syntax validation (filters parse correctly)

Reuses existing `schema_comparator` from `tasker-sdk`.

**Composition Executor** — Executes a validated `CompositionSpec` against input data:
- Iterates capabilities in sequence
- Resolves input mapping for each step
- Calls capability executor
- Stores step output for downstream reference
- Produces final result

In Phase 1, the executor runs standalone within `tasker-grammar` (not as a `StepHandler`). It takes `(spec: CompositionSpec, input: Value) → Result<Value>`. The `StepHandler` wrapper in `tasker-worker` comes in Phase 3 — that's where worker-specific concerns (checkpoint service, task sequence step mapping, step execution result construction) are added.

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | `CompositionValidator` | High | JSON Schema contract chaining, input mapping validation |
| `TAS-xxx` | `CompositionExecutor` (standalone) | High | Capability chaining, step context threading, error propagation |

### 1E: Real-World Workflow Modeling (Parallel with 1C/1D)

Identify and model 3 rich real-world workflows that exercise the full grammar vocabulary. These become the acceptance test suite — if the grammar can express these workflows correctly, we have confidence in viability.

Each workflow should:
- Have 6-10 steps mixing virtual handlers and domain handlers
- Exercise at least 4 of the 6 capabilities
- Include at least one `acquire` (external data), one `persist` (write), and one `emit` (event)
- Represent a genuinely useful business process (not a toy example)
- Include steps that clearly CANNOT be grammar-composed (domain handlers) to validate the boundary

**Candidate workflows** (extend beyond existing case study examples):

| Workflow | Domain | Grammar Steps | Domain Handler Steps | Key Capabilities Exercised |
|----------|--------|--------------|---------------------|--------------------------|
| **Invoice reconciliation** | Finance | validate invoices, transform line items + compute totals + evaluate matching rules, assert balance, persist reconciliation record, emit reconciliation event | fetch invoices from AP system (acquire), match against PO system (domain — fuzzy matching logic), flag exceptions (domain — org-specific rules) | validate, transform, assert, persist, emit, acquire |
| **Customer onboarding pipeline** | SaaS/CRM | validate application data, transform to internal format + evaluate eligibility + compute risk score inputs, persist customer record, emit welcome event | credit check (domain — third-party API), compliance screening (domain — regulatory rules), provision account (domain — platform-specific) | validate, transform, persist, emit |
| **Content moderation pipeline** | Platform | validate submission metadata, transform for analysis + evaluate auto-approve rules + compute moderation scores, assert policy compliance, persist moderation decision, emit moderation event | classify content (domain — ML model inference), detect prohibited content (domain — specialized detection), human review routing (domain — org-specific escalation) | validate, transform, assert, persist, emit |

Each workflow produces:
- A complete `TaskTemplate` YAML with mixed virtual/domain handler steps
- A dependency DAG showing step ordering
- Test fixtures (input data, expected intermediate results, expected final output)
- A narrative explaining why each step is virtual vs. domain handler
- Identification of which capabilities are exercised and how they chain

**Ticket**: `TAS-xxx: Model 3 real-world workflows for grammar acceptance testing`

This ticket is research-heavy and can run in parallel with capability implementation. Its output directly feeds Phase 2 validation tooling and Phase 1 integration tests.

### 1F: Composition Integration Tests (Sequential after 1C, 1D, 1E)

With capabilities implemented and workflows modeled, build end-to-end composition tests that execute full workflow compositions against fixture data.

These tests validate:
- Multi-step chaining works correctly (validate → transform → persist)
- Composition context envelope threads correctly across invocations (.context, .deps, .prev, .step)
- Error propagation works (failure at step 3 of 5 produces clear diagnostics)
- Checkpoint markers are respected (mutating steps produce checkpoint data)
- The 3 modeled workflows execute correctly against their fixture data

No database, no workers, no messaging. Pure composition execution against test data.

**Ticket**: `TAS-xxx: End-to-end composition integration tests for 3 modeled workflows`

### Phase 1 Dependency Graph

```
1A (Expression Language Research)
 │
 ↓
1B (Core Type Definitions)
 │
 ├──→ 1C (Capability Executors) ──→ 1F (Integration Tests)
 │         [6 tickets, parallelizable]        ↑
 │                                            │
 ├──→ 1D (Composition Engine) ───────────────→│
 │         [validator + executor]              │
 │                                            │
 └──→ 1E (Workflow Modeling) ────────────────→│
           [3 workflows, parallel with 1C/1D]
```

**Phase 1 Total**: ~12-14 tickets
**Parallelism**: After 1A and 1B complete, up to 8 tickets can proceed in parallel (6 capabilities + validator + executor, with workflow modeling running alongside)

---

## Phase 2: Validation Tooling & Confidence Building

*Goal: Build template-level validation tools that check grammar compositions for correctness, compatibility, and coherence — integrated into tasker-ctl and tasker-mcp.*

Phase 2 produces developer-facing tooling. When a TaskTemplate author writes a `composition:` block, the tooling tells them immediately whether it's well-formed, whether the capabilities chain correctly, and whether the result shapes are compatible.

### 2A: Template Composition Validator (Sequential after Phase 1)

Extend `tasker-sdk`'s template validation to understand `composition:` blocks.

Today, `template_validate` checks:
- YAML structure
- Step dependencies form a valid DAG
- Handler callable format is valid per language
- `result_schema` is valid JSON Schema

Add composition-aware checks:
- `composition:` block parses to valid `CompositionSpec`
- All referenced capabilities exist in the vocabulary
- Each capability's config validates against its config_schema
- Contract chaining validates across capability boundaries
- Checkpoint markers cover all mutating capabilities
- `result_schema` on the step is compatible with the composition's outcome
- jaq filter syntax is valid (all `filter` expressions parse correctly)
- jaq filter context references resolve (`.deps.<name>` references existing dependency steps, `.prev` is valid for non-first invocations)

This is the **formal correctness** check — syntactic and structural validity.

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | Composition-aware template validator | High | Extends existing tasker-sdk validation |
| `TAS-xxx` | jaq filter syntax validation | Medium | Validates all jaq filter expressions parse correctly |
| `TAS-xxx` | Composition context reference validation | Medium | Validates .prev, .deps, .context references resolve against available data |

### 2B: Capability Compatibility Checker (Parallel with 2A)

Beyond structural correctness, validate that compositions are **behaviorally compatible** with the capability implementations.

- Validate that `persist` config references a resource type the system knows about (database entity names, API endpoints)
- Validate that `acquire` config references accessible data sources
- Validate that `emit` config produces events compatible with registered domain event schemas
- Validate that `transform` jaq filters reference fields that exist in the composition context or declared output schemas of prior invocations
- Validate that `assert` jaq filters reference fields that exist in the composition context

This is the **semantic compatibility** check — does the composition make sense given the system's actual capabilities?

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | Capability compatibility checker | High | Requires capability registry introspection |
| `TAS-xxx` | jaq filter field resolution checker | Medium | Validates jaq filter field references against available schemas |

### 2C: MCP & CLI Tool Integration (Parallel with 2A/2B)

Expose the validation capabilities through existing developer tooling.

**MCP tools** (extend the existing 29-tool set):

| Tool | Tier | Description |
|------|------|-------------|
| `grammar_list` | Offline | List available grammar categories with descriptions |
| `capability_search` | Offline | Search capabilities by name, category, or action type |
| `capability_inspect` | Offline | Show capability's config_schema, input_schema, output_schema |
| `composition_validate` | Offline | Validate a CompositionSpec for correctness and compatibility |
| `composition_explain` | Offline | Trace data flow through a composition, showing schema at each boundary |
| `vocabulary_document` | Offline | Generate human-readable documentation for registered vocabulary |

**tasker-ctl commands** (parity with MCP tools):

```
tasker-ctl grammar list
tasker-ctl grammar inspect <category>
tasker-ctl capability search <query>
tasker-ctl capability inspect <name>
tasker-ctl composition validate <template.yaml> [--step <name>]
tasker-ctl composition explain <template.yaml> [--step <name>]
```

All of these are **offline tools** — they work without a running Tasker instance, operating on the compiled-in vocabulary and local template files.

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | MCP grammar/capability discovery tools | Medium | 6 new offline tools |
| `TAS-xxx` | tasker-ctl grammar/composition commands | Medium | CLI parity with MCP tools |
| `TAS-xxx` | `composition_explain` trace output | Medium | Visual data flow tracing |

### 2D: Confidence Validation Against Modeled Workflows (Sequential after 2A/2B)

Run the Phase 1 modeled workflows through the Phase 2 validation tooling:

- All 3 workflows' composition blocks pass `composition_validate`
- `composition_explain` produces clear, accurate data flow traces
- Intentionally broken compositions produce actionable error messages
- Edge cases (optional fields, type coercion, cross-step references) are handled

This is the **acceptance gate** for Phase 2 — if the tooling can validate the 3 workflows correctly and reject malformed variants with clear errors, we have confidence to proceed to worker integration.

**Ticket**: `TAS-xxx: Validate 3 modeled workflows through composition tooling pipeline`

### Phase 2 Dependency Graph

```
Phase 1 (complete)
 │
 ├──→ 2A (Template Validator) ──────→ 2D (Confidence Validation)
 │                                         ↑
 ├──→ 2B (Compatibility Checker) ─────────→│
 │                                         │
 └──→ 2C (MCP & CLI Tools) ──────────────→│
```

**Phase 2 Total**: ~8-9 tickets
**Parallelism**: 2A, 2B, and 2C can proceed in parallel after Phase 1 completes

---

## Phase 3: Worker Registry-Resolver Integration

*Goal: Integrate grammar compositions into the worker dispatch lifecycle so that virtual handler steps execute through the standard worker infrastructure.*

This phase makes compositions executable as real workflow steps — claimed from queues, dispatched through `HandlerDispatchService`, producing `StepExecutionResult`, feeding into the orchestrator's dependency resolution.

### 3A: StepDefinition Extension (Sequential — blocks 3B, 3C)

Add `composition: Option<CompositionSpec>` to `StepDefinition`. This is the field that `HandlerDispatchService` checks to route between composition executor and resolver chain.

- Extend `StepDefinition` struct
- Update `StepDefinition` serialization/deserialization
- Update TaskTemplate YAML parsing to populate the field
- Update sqlx queries that hydrate `StepDefinition` (if any)
- Update existing tests to handle the new field (optional, defaults to `None`)

**Ticket**: `TAS-xxx: Add composition field to StepDefinition`

### 3B: CompositionExecutor as StepHandler (Sequential after 3A)

Wrap `tasker-grammar`'s standalone `CompositionExecutor` in a `StepHandler` implementation (this bridge lives in `tasker-worker`, not `tasker-grammar`):

- Receives `TaskSequenceStep` (task context, dependency results, step definition with composition spec)
- Extracts `CompositionSpec` from step definition
- Maps `TaskSequenceStep` data into the composition's initial input (resolving `TaskContext` and `StepOutput` input mappings against `dependency_results`)
- Executes the composition chain
- Produces `StepExecutionResult` with result conforming to `result_schema`
- Integrates with `CheckpointService` for resumable compositions (the `CompositionCheckpoint` format from Phase 1)

**Ticket**: `TAS-xxx: Implement CompositionExecutor as StepHandler with checkpoint integration`

### 3C: GrammarActionResolver Integration (Sequential after 3A)

Integrate the grammar resolver into the existing `ResolverChain`:

- `GrammarActionResolver` at priority 15 (before ExplicitMapping at 10, after nothing)
- Checks `step_definition.composition.is_some()`
- Returns the built-in `CompositionExecutor` as the resolved handler
- No changes to existing resolver chain implementations (Ruby, Python, TypeScript, Rust)

Alternatively, the routing decision can happen in `HandlerDispatchService` directly (before the resolver chain is consulted), which is simpler and avoids touching the resolver chain at all. The research document favors this approach.

**Ticket**: `TAS-xxx: Integrate composition routing in HandlerDispatchService`

### 3D: End-to-End Worker Tests (Sequential after 3B, 3C)

Test virtual handler steps executing through the full worker lifecycle:

- TaskTemplate with mixed virtual/domain handler steps
- Virtual handler steps claimed and executed by workers
- `StepExecutionResult` produced correctly
- Dependency resolution works (domain handler step depends on virtual handler step's output)
- Checkpoint/resume works for multi-step compositions
- Error handling (composition failure produces proper step error state)

These tests require database and messaging (feature-gated `test-messaging` or `test-services`).

**Ticket**: `TAS-xxx: End-to-end worker tests for virtual handler step execution`

### Phase 3 Dependency Graph

```
Phase 2 (complete, or at least 2A/2B)
 │
 ↓
3A (StepDefinition Extension)
 │
 ├──→ 3B (CompositionExecutor as StepHandler)──→ 3D (E2E Worker Tests)
 │                                                    ↑
 └──→ 3C (Resolver/Dispatch Integration) ────────────→│
```

**Phase 3 Total**: ~4 tickets
**Parallelism**: 3B and 3C can proceed in parallel after 3A

---

## Phase 4: Composition Queues & Orchestration

*Goal: Implement cross-namespace composition queues, the composition-only worker binary, and the orchestration routing changes that enable horizontal scaling of virtual handler throughput.*

### 4A: Composition Queue Routing (Sequential after 3C)

Implement the routing decision in `StepEnqueuerActor`:

- Parse `composition_queue` field from TaskTemplate (default: `true`)
- Route virtual handler steps to `worker_composition_queue_N` when template allows
- Fall back to namespace queue when template disables composition queues
- Shard selection (consistent hash on step name, configurable)

**Routing authority: template only.** The orchestrator never consults worker configuration, worker registration tables, or any worker-reported state. This preserves the existing architectural boundary where the orchestrator manages step lifecycle and workers manage step execution. The template is the sole routing authority:

- `composition_queue: true` (default) → composition queue
- `composition_queue: false` → namespace queue (tenancy/compliance/isolation boundary)

The "what if nobody is listening?" scenario is identical to namespace queues: if no workers poll `worker_ecommerce_queue`, steps sit until a worker comes online. Same answer for composition queues. This is a deployment problem, not a routing problem.

**Why no worker registration table**: The orchestration system has no concept of "workers" — it transforms task requests into step DAGs and routes steps to namespace queues. A `worker_registrations` table would inject worker-awareness into a system that has no mechanism to use that information for routing decisions (the orchestrator routes to queues, not to workers). Separately, if worker boot were to inject its `composition_queues` preference into the task template's persisted JSON in `named_tasks.configuration`, boot-order would indeterminately mutate routing behavior across all subsequent tasks — a state leakage problem with no error signal. Neither approach is necessary when the template is the sole routing authority.

Also:
- Worker boot subscribes to composition queue shards (configurable count, based on worker's own `composition_queues` config)
- Worker `composition_queues = true` (default) means the worker subscribes to composition queues — this is a self-regarding operational decision about what work the worker accepts, not a routing input
- Worker `composition_queues = false` opts the worker out of the shared pool (for resource reservation or dedicated domain processing)

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | Composition queue routing in StepEnqueuerActor | Medium | Template-only routing logic, no worker feasibility check |
| `TAS-xxx` | Worker composition queue subscription | Medium | Boot-time subscription based on worker config |
| `TAS-xxx` | TaskTemplate composition_queue field | Low | YAML field + parsing |

### 4B: Composition-Only Worker Binary (Parallel with 4A)

Create the `workers/composition` crate — the **deployment prerequisite** for action grammar compositions:

- Thin binary: bootstrap + `CompositionOnlyRegistry` + signal handling
- `CompositionOnlyRegistry` implements `StepHandlerRegistry` (returns `CompositionExecutor` for composition steps, `None` otherwise)
- Docker image: same `wolfi-base` pattern as orchestration
- Configuration: `namespaces = []`, `composition_queues = true`

At least one composition-only worker must be deployed for any Tasker ecosystem using action grammar compositions. This is the deployment-level guarantee that composition queue steps will be processed — equivalent to requiring at least one domain worker per namespace.

| Ticket | Component | Complexity | Notes |
|--------|-----------|------------|-------|
| `TAS-xxx` | `workers/composition` crate and binary | Medium | Thin wrapper around tasker-worker |
| `TAS-xxx` | Composition worker Dockerfile | Low | Follow orchestration.prod.Dockerfile pattern |

### 4C: Integration & Scaling Tests (Sequential after 4A, 4B)

- Composition-only workers claim and execute virtual handler steps from composition queues
- Domain workers continue to claim domain handler steps from namespace queues
- Domain workers with `composition_queues = true` also claim composition queue steps (additive capacity)
- Mixed-step TaskTemplates execute correctly with steps on different queues
- Template `composition_queue: false` routes virtual handler steps to namespace queue
- Load test: burst of virtual handler steps distributed across composition worker pool

**Ticket**: `TAS-xxx: Composition queue integration and scaling tests`

### Phase 4 Dependency Graph

```
Phase 3 (3C complete)
 │
 ↓
 ├──→ 4A (Queue Routing) ──────────────→ 4C (Integration Tests)
 │                                            ↑
 └──→ 4B (Composition Worker Binary) ───────→│
```

**Phase 4 Total**: ~5 tickets
**Parallelism**: 4A and 4B can proceed in parallel after Phase 3

---

## Cross-Phase Dependencies

```
Phase 1: Grammar Primitives & Independent Testing
  │  ~12-14 tickets, high internal parallelism after 1A+1B
  │
  ↓
Phase 2: Validation Tooling & Confidence Building
  │  ~8-9 tickets, high internal parallelism
  │  (confidence gate: 3 workflows validate correctly)
  │
  ↓
Phase 3: Worker Registry-Resolver Integration
  │  ~4 tickets, mostly sequential
  │
  ↓
Phase 4: Composition Queues & Orchestration
     ~5 tickets, high parallelism (4A and 4B parallel)
```

**Total estimated tickets**: ~29-32

### What Can Be Parallelized Across Phases

| Work Item | Can Start During | Why |
|-----------|-----------------|-----|
| Phase 2 MCP tools (2C) | Late Phase 1 | Can stub capability registry while executors are in progress |
| Workflow modeling (1E) | Phase 1A | Research task, no code dependency |
| Expression language research (1A) | Immediately | Blocks everything else, should start first |

### What Must Be Sequential

| Dependency | Reason |
|-----------|--------|
| 1A → 1B → 1C | jaq-core integration provides the expression engine that type definitions reference and all 6 executor implementations depend on |
| 1D → 2A | Composition validator logic feeds template-level validation |
| Phase 2 confidence gate → Phase 3 | Worker integration should only proceed with validated grammar viability |
| 3A → 3B/3C | StepDefinition extension must land before dispatch routing |
| 3C → 4A | Queue routing needs dispatch integration in place first |

---

## Phase 0 Closure (Parallel with Phase 1)

The Phase 0 completion assessment identifies two gaps:

1. **2-3 end-to-end examples** (description → template → handlers → tests)
2. **Formalized patterns document** (recurring workflow shapes mapped to grammar primitives)

Both can be addressed alongside Phase 1 work:
- The patterns document emerges naturally from 1E (workflow modeling)
- End-to-end examples can use the existing MCP + tasker-ctl tooling to demonstrate the full pipeline

| Ticket | Description | Parallel With |
|--------|-------------|--------------|
| `TAS-xxx` | Create 2-3 end-to-end MCP workflow examples with tests | Phase 1 (any) |
| `TAS-xxx` | Formalize workflow patterns document from research observations | Phase 1E |

---

## Risk Factors

### jaq-core Integration (Phase 1A)

The expression language decision is made: `jaq-core` (jq syntax). The remaining risk is integration-level:
- **Sandboxing adequacy**: jq filters can recurse or expand data. The `ExpressionEngine` wrapper must enforce timeout and output size limits. Mitigation: implement and test sandboxing boundaries early, before capability executors depend on it.
- **Error message quality**: Template authors need actionable diagnostics when jq filters are malformed. Mitigation: build error-message-quality tests as part of 1A, not as an afterthought.
- **Filter complexity in practice**: Real workflow compositions may produce complex jq expressions that are hard to read. Mitigation: `composition_explain` tooling (Phase 2) shows data flow at each boundary, making individual filter behavior inspectable.

### JSON Schema Compatibility Model (Phase 1D / 2A)

JSON Schema contract chaining is a **design-time concern**, not a runtime concern. The Phase 2 validation tooling checks that producer output schemas are compatible with consumer input schemas across capability boundaries — this runs in `composition_validate` and again on template load/persist to `named_tasks.configuration`. The structural subtyping model (producer provides superset, consumer requires subset) has edge cases:
- `integer` vs `number` compatibility
- `additionalProperties: false` in consumer schema
- Nullable fields (`type: ["string", "null"]`)
- Array item schema compatibility

These edge cases affect the quality of design-time validation diagnostics, not runtime correctness. At runtime, data flows between capabilities without schema checks — the composition is a closed system after design-time validation confirms coherence.

**Mitigation**: Build the compatibility checker incrementally, starting with the strict cases that the 3 modeled workflows exercise, then expanding coverage as edge cases emerge from real usage.

### Runtime Validation Model (Phase 1C / 3B)

An important clarification on when JSON Schema validation runs at runtime: **only when explicitly authored as a `validate` capability step**. The `validate` capability exists for third-party data integration boundaries — API responses, file reads, SFDC connectors, or any external data source where the shape is not under our control. This is an intentional, authored step in the composition, not implicit infrastructure.

Within a composition's internal data flow, capabilities trust the contracts validated at design time. The data exchange between capabilities is a closed system — we control the grammar components, the expression engine, and the data threading. If a capability somehow produces unexpected output (a bug in the executor, an expression that evaluates to an unexpected type), that will produce a runtime error that gets logged and fails the step through normal step failure mechanics. There is no need for per-boundary schema validation during execution.

This means a 5-step composition does **not** incur 15 schema validations at runtime. It incurs zero implicit schema checks — only the explicit `validate` steps the author placed at ingestion boundaries. The performance concern as previously framed does not apply.

**Remaining performance consideration**: The `validate` capability itself should compile schemas once (at composition load time or first execution) rather than re-parsing on every invocation. This is straightforward — `jsonschema` supports compiled validators.

---

## Success Criteria by Phase

| Phase | Gate | Measurement |
|-------|------|-------------|
| **1** | All 6 capability executors pass unit tests; 3 workflow compositions execute correctly against fixture data | `cargo make test-no-infra` passes all grammar tests |
| **2** | Template validation catches known-bad compositions with actionable errors; 3 workflow templates validate cleanly | `tasker-ctl composition validate` exits 0 for valid, non-zero with clear messages for invalid |
| **3** | Virtual handler steps execute through worker lifecycle; dependency resolution works across virtual/domain handler boundaries | `cargo make test-rust-e2e` passes with mixed-handler TaskTemplates |
| **4** | Composition-only workers claim and execute virtual handler steps from composition queues; routing precedence works correctly | `cargo make test-rust-cluster` passes with composition queue routing |

---

*This document should be read alongside the research documents it references: `actions-traits-and-capabilities.md` for the foundational architecture, `grammar-trait-boundary.md` for trait design, `composition-validation.md` for validation mechanics, `checkpoint-generalization.md` for the checkpoint model, `virtual-handler-dispatch.md` for worker integration and composition queues, and the case studies in `case-studies/` for the grammar proposals that ground this plan in real handler analysis.*
