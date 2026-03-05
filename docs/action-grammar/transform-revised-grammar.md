# Revised Grammar: The Transform-Centric Capability Model

*March 2026 — Design Refinement from jaq-core Integration*

---

## Motivation

The initial action grammar research identified 9 core capabilities: `validate`, `reshape`, `compute`, `evaluate`, `assert`, `evaluate_rules`, `persist`, `acquire`, `emit`. With the decision to adopt **jaq-core** (Rust-native jq implementation) as the unified expression language, a simplification emerged.

The pure data capabilities (`reshape`, `compute`, `evaluate`, `evaluate_rules`) all perform the same fundamental operation: **take JSON input, apply a transformation, produce JSON output**. The distinctions between them — "reorganize shape" vs. "derive numeric values" vs. "determine booleans" vs. "match rules" — are conventions for how the jq filter is written, not meaningfully different execution models.

Meanwhile, the bespoke configuration schemas invented for each capability (e.g., `compute.config.operations[].select/derive/cast`, `evaluate.config.expressions`, `reshape.config.fields`) duplicate what jq already expresses natively. We were building a custom expression syntax on top of an expression language.

### The Key Insight

**JSON Schema declares what a step promises to produce. jaq filters declare how it produces it.** This separation gives us:

- **Static analyzability**: Tooling validates contract chains by comparing output schemas to downstream input expectations — without running any jq filters
- **Runtime expressiveness**: jq is a mature, well-documented, widely-understood language for JSON transformation
- **Single primitive**: One `transform` capability replaces four, with the semantic distinction becoming documentation guidance rather than separate executor implementations

---

## The 6-Capability Model

| Capability | Purpose | Config Model |
|-----------|---------|-------------|
| **`transform`** | Pure data transformation | `output` (JSON Schema) + `filter` (jaq expression) |
| **`validate`** | Trust boundary gate | JSON Schema + coercion/filtering/failure config |
| **`assert`** | Execution gate | `filter` (jaq boolean) + `error` message |
| **`persist`** | Write to external system | Typed envelope + jaq `data` filter |
| **`acquire`** | Read from external system | Typed envelope + jaq `result_filter` |
| **`emit`** | Fire domain event | Typed envelope + jaq `payload` filter |

### What changed from the 9-capability model

| Previous | Now | Rationale |
|----------|-----|-----------|
| `reshape` | `transform` | Selector-path projection is just a jq object construction filter |
| `compute` | `transform` | Arithmetic/aggregation is just a jq filter with math operators |
| `evaluate` | `transform` | Boolean/selection derivation is just a jq filter returning booleans |
| `evaluate_rules` | `transform` | First-match rule engine is just a jq if-then-elif-else chain |
| `group_by` | `transform` | jq has native `group_by(.field)` |
| `rank` | `transform` | jq has native `sort_by(.field) \| reverse` |

### What stayed the same

| Capability | Why unchanged |
|-----------|--------------|
| `validate` | JSON Schema validation with coercion modes, attribute filtering, and failure mechanics is not a jq concern. This is a distinct execution model (schema conformance checking). |
| `assert` | Execution gating (proceed or fail) is semantically distinct from data production. Assert doesn't produce new data — it gates whether the composition continues. |
| `persist` | Side-effecting write to an external system. Needs typed config for resource targeting, constraints, success validation. jaq handles the data mapping within. |
| `acquire` | Side-effecting read from an external system. Same typed envelope reasoning as persist. |
| `emit` | Side-effecting domain event publication. Maps to Tasker's `DomainEvent` system with typed delivery config. |

---

## The `transform` Capability in Detail

### Structure

```yaml
- capability: transform
  output:
    type: object
    required: [field_a, field_b]
    properties:
      field_a: { type: number }
      field_b: { type: string }
      field_c: { type: boolean }
  filter: |
    {
      field_a: .some_input.value,
      field_b: .other_input.name,
      field_c: (.some_input.value > 100)
    }
```

**`output`**: JSON Schema declaring the shape this step promises to produce. Enables:
- Design-time contract chaining (next step's expected input validated against this output)
- MCP tool discovery (agents can inspect what a composition step produces)
- Runtime validation (optional — executor can validate filter output against declared schema)

**`filter`**: jaq (jq) expression that produces the output data. The filter returns a JSON value matching the `output` schema.

### Input Data Flow

**Terminology note**: In Tasker, a "step" is a `WorkflowStep` — a node in the task DAG. A virtual handler step's composition contains a sequence of **capability invocations** (the individual `transform`, `assert`, `persist`, etc. entries in the `compose:` array). These are not steps — they are the internal execution sequence within a single step's virtual handler.

When a virtual handler step executes, the `CompositionExecutor` (which implements `StepHandler`) receives the full `TaskSequenceStep` via the `StepHandler::call(&self, step: &TaskSequenceStep)` contract. This struct contains:

| Field | Type | Contents |
|-------|------|----------|
| `task.task.context` | `Option<Value>` | The original task input JSON (arbitrary data passed at task creation) |
| `dependency_results` | `HashMap<String, StepExecutionResult>` | All transitive parent step outputs, keyed by step name. Each entry's `.result` field contains the output JSON. |
| `workflow_step.inputs` | `Option<Value>` | Step-specific inputs (if any) |
| `workflow_step.checkpoint` | `Option<Value>` | Checkpoint state from a prior interrupted attempt (for resumption) |
| `workflow_step.attempts` | `Option<i32>` | Current attempt number (for retry-aware logic) |
| `step_definition.handler.initialization` | `HashMap<String, Value>` | Handler-level configuration |
| `step_definition` | `StepDefinition` | The full step definition, including the composition spec |

#### Proposed: Composition Context Envelope

*This is a design proposal. The exact structure will be refined during Phase 1D (TAS-334, CompositionExecutor implementation).*

The `CompositionExecutor` constructs a **composition context** — a `serde_json::Value` — from the `TaskSequenceStep` and passes it as the `input` to each capability invocation. The context evolves as invocations execute:

```json
{
  "context": {
    "cart_items": [ ... ],
    "customer_email": "user@example.com"
  },
  "deps": {
    "validate_cart": { "total": 99.99, "validated_items": [ ... ] },
    "process_payment": { "payment_id": "pay_123", "transaction_id": "txn_456" }
  },
  "step": {
    "name": "create_order",
    "attempts": 1,
    "inputs": null
  },
  "prev": null
}
```

| Field | Source | Mutates between invocations? |
|-------|--------|------------------------------|
| `.context` | `task.task.context` — the original task input data | No |
| `.deps` | `dependency_results` — each entry's `.result` value, keyed by step name | No |
| `.step` | Subset of `workflow_step` — name, attempt count, inputs | No |
| `.prev` | Output of the most recent capability invocation (`null` for the first) | **Yes** — updated after each invocation |

**Design rationale**:

- **`.context`** is the task's input data. jaq filters access it as `.context.cart_items`, `.context.customer_email`, etc.
- **`.deps`** contains only the `.result` JSON from each `StepExecutionResult` — not the full execution metadata (step_uuid, timing, error info). If a capability needs to know whether a dependency succeeded or failed, that's an `assert` gate, not a data transformation concern.
- **`.step`** provides minimal step-level metadata. Most compositions won't reference this, but retry-aware assertions (`if .step.attempts > 1 then ...`) need it.
- **`.prev`** is the threading mechanism. Each capability invocation's output replaces `.prev` for the next invocation. The first invocation sees `.prev` as `null` and reads from `.context` or `.deps` instead.

**Checkpoint resumption**: When a composition resumes from checkpoint, the `CompositionExecutor` restores `.prev` from the checkpointed capability output and skips already-completed invocations. The `.context`, `.deps`, and `.step` fields are reconstructed fresh from the `TaskSequenceStep` (since they come from the database, they reflect current state).

**What this does NOT include**: The full `StepExecutionResult` metadata (timing, error details), the handler initialization config (that's parsed by the CompositionExecutor to get the composition spec, not passed to individual capabilities), or the raw `workflow_step` record. If these prove necessary during implementation, the envelope can be extended.

### Examples: What Each Previous Capability Becomes

#### What was `reshape` (selector-path projection)

```yaml
# Previous:
- capability: reshape
  config:
    fields:
      total: "validate_cart.total"
      payment_id: "process_payment.payment_id"
      validated_items: "validate_cart.validated_items"
  input_mapping: { type: task_context }

# Now:
- capability: transform
  output:
    type: object
    required: [total, payment_id, validated_items]
    properties:
      total: { type: number }
      payment_id: { type: string }
      validated_items: { type: array, items: { type: object } }
  filter: |
    {
      total: .deps.validate_cart.total,
      payment_id: .deps.process_payment.payment_id,
      validated_items: .deps.validate_cart.validated_items
    }
```

#### What was `compute` (arithmetic/aggregation)

```yaml
# Previous:
- capability: compute
  config:
    operations:
      - select: "items[*]"
        derive:
          line_total: "quantity * unit_price"
        cast: decimal(2)
      - select: "$"
        derive:
          subtotal: "sum(items[*].line_total)"
          tax: "subtotal * 0.0875"
        cast: decimal(2)
  input_mapping: { type: previous }

# Now:
- capability: transform
  output:
    type: object
    required: [items, subtotal, tax]
    properties:
      items: { type: array, items: { type: object, properties: { line_total: { type: number } } } }
      subtotal: { type: number }
      tax: { type: number }
  filter: |
    .prev
    | .items |= map(. + {line_total: ((.quantity * .unit_price) * 100 | round / 100)})
    | . + {subtotal: ([.items[].line_total] | add)}
    | . + {tax: ((.subtotal * 0.0875) * 100 | round / 100)}
```

#### What was `evaluate` (boolean/selection derivation)

```yaml
# Previous:
- capability: evaluate
  config:
    expressions:
      free_shipping: "subtotal >= 75.00"
      billing_required: "price > 0"
      health_rating:
        case:
          - when: "health_score >= 80"
            then: "excellent"
          - when: "health_score >= 60"
            then: "good"
          - default: "needs_improvement"
  input_mapping: { type: previous }

# Now:
- capability: transform
  output:
    type: object
    properties:
      free_shipping: { type: boolean }
      billing_required: { type: boolean }
      health_rating: { type: string, enum: [excellent, good, needs_improvement] }
  filter: |
    .prev + {
      free_shipping: (.prev.subtotal >= 75.00),
      billing_required: (.prev.price > 0),
      health_rating: (
        if .prev.health_score >= 80 then "excellent"
        elif .prev.health_score >= 60 then "good"
        else "needs_improvement" end
      )
    }
```

#### What was `evaluate_rules` (first-match rule engine)

```yaml
# Previous:
- capability: evaluate_rules
  config:
    rules:
      - condition: "refund_amount <= 50"
        result: { approval_path: auto_approved }
      - condition: "refund_reason in auto_approve_reasons AND refund_amount <= 500"
        result: { approval_path: auto_approved }
      - condition: "refund_amount > 500"
        result: { approval_path: manager_review }
      - condition: "true"
        result: { approval_path: standard_review }
    first_match: true
  input_mapping: { type: previous }

# Now:
- capability: transform
  output:
    type: object
    required: [approval_path]
    properties:
      approval_path: { type: string, enum: [auto_approved, manager_review, standard_review] }
  filter: |
    .prev
    | if .refund_amount <= 50 then {approval_path: "auto_approved"}
      elif (.refund_reason | IN("defective","wrong_item")) and .refund_amount <= 500 then {approval_path: "auto_approved"}
      elif .refund_amount > 500 then {approval_path: "manager_review"}
      else {approval_path: "standard_review"} end
```

#### What was `group_by` + `rank` (aggregation)

```yaml
# Previous:
- capability: group_by
  config:
    dimensions: [category]
    metrics:
      revenue: { sum: revenue }
      quantity: { sum: quantity }
      transaction_count: { count: "*" }
  input_mapping: { type: previous }

- capability: rank
  config:
    by: revenue
    direction: desc
    output_field: top_category
    limit: 1
  input_mapping: { type: previous }

# Now (single transform):
- capability: transform
  output:
    type: object
    required: [groups, top_category]
    properties:
      groups:
        type: array
        items:
          type: object
          properties:
            category: { type: string }
            revenue: { type: number }
            quantity: { type: number }
            transaction_count: { type: integer }
      top_category:
        type: object
        properties:
          category: { type: string }
          revenue: { type: number }
  filter: |
    .prev.records
    | group_by(.category)
    | map({
        category: .[0].category,
        revenue: ([.[].revenue] | add),
        quantity: ([.[].quantity] | add),
        transaction_count: length
      })
    | {groups: ., top_category: (sort_by(.revenue) | reverse | .[0])}
```

---

## The `assert` Capability

Assert remains separate because it gates execution rather than producing data. It evaluates a jaq boolean expression and either passes through the input unchanged or fails the step.

```yaml
- capability: assert
  filter: '.prev.total == (.prev.subtotal + .prev.tax + .prev.shipping)'
  error: "Order total does not match component sum"
```

**Compound assertions** (what was previously `all`/`any`/`none` quantifiers):

```yaml
- capability: assert
  filter: |
    (.prev.payment_validated and .prev.fraud_passed and .prev.policy_checked)
    and (.prev.manager_approved or .prev.auto_approved)
    and ((.prev.blacklisted or .prev.sanctioned) | not)
  error: "Cannot proceed with refund — prerequisites, approval, or restrictions failed"
```

The jq `and`/`or`/`not` operators replace the bespoke `all`/`any`/`none` quantifier syntax. Named sub-conditions can be expressed as intermediate variables within the jq filter if needed, but in most cases the boolean composition is clear enough inline.

---

## Action Capabilities with jaq Data Mapping

### `persist`

Typed envelope for resource targeting, jaq filter for data mapping:

```yaml
- capability: persist
  config:
    resource:
      type: database
      entity: orders
    constraints:
      unique_key: order_ref
      id_pattern: "ORD-{YYYYMMDD}-{hex}"
    validate_success:
      order_id: { type: string, required: true }
    result_shape: [order_id, order_ref, created_at]
  data: |
    {
      order_ref: .prev.order_ref,
      customer_email: .prev.customer_email,
      items: .prev.validated_items,
      total: .prev.total,
      estimated_delivery: .prev.estimated_delivery
    }
  checkpoint: true
```

### `acquire`

```yaml
- capability: acquire
  config:
    resource:
      type: api
      endpoint: "/api/sales"
      method: GET
    constraints:
      timeout_ms: 5000
    validate_success:
      status: { in: [200] }
    result_shape: [data.sales_records]
  params: |
    {
      source: .context.source,
      date_range_start: .context.date_range_start,
      date_range_end: .context.date_range_end
    }
```

### `emit`

```yaml
- capability: emit
  config:
    event_name: "order.confirmed"
    event_version: "1.0"
    delivery_mode: durable
    condition: success
  payload: |
    {
      order_id: .prev.order_id,
      order_ref: .prev.order_ref,
      customer_email: .prev.customer_email,
      total: .prev.total,
      estimated_delivery: .prev.estimated_delivery
    }
  schema:
    type: object
    required: [order_id, order_ref, customer_email, total]
    properties:
      order_id: { type: string }
      order_ref: { type: string }
      customer_email: { type: string }
      total: { type: number }
```

---

## Contract Chaining: How Static Analysis Works

The `output` schema on `transform` steps enables design-time validation without executing jaq filters:

```yaml
compose:
  # Invocation 1: transform produces {total: number, items: array}
  - capability: transform
    output:
      type: object
      required: [total, items]
      properties:
        total: { type: number }
        items: { type: array }
    filter: |
      {total: .deps.validate_cart.total, items: .deps.validate_cart.validated_items}

  # Invocation 2: assert reads total (number) — validated against invocation 1's output
  - capability: assert
    filter: '.prev.total > 0'
    error: "Order total must be positive"

  # Invocation 3: persist writes total and items — validated against invocation 1's output
  - capability: persist
    config:
      resource: { type: database, entity: orders }
    data: |
      {total: .prev.total, items: .prev.items}
    checkpoint: true
```

The `CompositionValidator` can verify:
1. Invocation 2's filter references `.prev.total` — invocation 1's output declares `total: number` — valid
2. Invocation 3's data filter references `.prev.total` and `.prev.items` — both declared in invocation 1's output — valid
3. If invocation 2 referenced `.prev.discount` — not in invocation 1's output — **validation error at design time**

This is the same contract chaining described in `composition-validation.md`, but now with a cleaner model: JSON Schema output declarations replace the ad-hoc inference that was needed when each capability had its own bespoke config format.

---

## Composition Shape Patterns (Revised)

The three recurring shapes from the case studies simplify:

### Shape 1: Validate → Transform → Persist

```yaml
compose:
  - capability: validate
    config:
      schema: { ... }
      coercion: permissive
      on_failure: fail

  - capability: transform
    output: { ... }
    filter: |
      { ... computed/derived fields ... }

  - capability: persist
    config: { resource: { ... } }
    data: |
      { ... mapped fields ... }
    checkpoint: true
```

### Shape 2: Transform (projection + computation)

```yaml
compose:
  - capability: transform
    output: { ... }
    filter: |
      { ... projected and computed fields ... }
```

Note: What was previously a `reshape` → `compute` chain can often be a single `transform` — jq naturally combines projection and computation in one expression.

### Shape 3: Transform → Emit

```yaml
compose:
  - capability: transform
    output: { ... }
    filter: |
      { ... gathered dependency data ... }

  - capability: emit
    config:
      event_name: "order.confirmed"
      delivery_mode: durable
    payload: |
      { ... payload from prev ... }
```

---

## Relationship to Existing Architecture

### CapabilityExecutor Trait

The trait from `grammar-trait-boundary.md` is unchanged:

```rust
#[async_trait]
pub trait CapabilityExecutor: Send + Sync + fmt::Debug {
    async fn execute(
        &self,
        input: serde_json::Value,
        config: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError>;

    fn capability_name(&self) -> &str;
}
```

The `TransformExecutor` implementation:
1. Receives `input` (the composition context envelope: context + deps + step + prev)
2. Receives `config` containing `output` (JSON Schema) and `filter` (jaq expression string)
3. Compiles and executes the jaq filter against the input
4. Optionally validates the result against the `output` schema
5. Returns the result

### GrammarCategory Mapping

| Grammar Category | Capabilities |
|-----------------|-------------|
| **Acquire** | `acquire` |
| **Transform** | `transform` |
| **Validate** | `validate`, `assert` |
| **Persist** | `persist` |
| **Emit** | `emit` |

### What Stays Outside Grammar Scope

Operations where the (action, resource, context) triple cannot be deterministically expressed remain as traditional domain handlers:

- `fraud_check`, `payment_gateway_charge`, `gateway_refund`
- `inventory_reserve`, `classify_customer`, `generate_credentials`
- `check_policy_window`

**Revised**: Decision point step creation and batch cursor calculation are no longer categorically outside grammar scope. See "Open Design: Decision and Batch Outcome Expression" below.

---

## Open Design: Infrastructure Injection for Action Capabilities

*This section identifies a significant architectural concern that requires its own research spike.*

### The Problem

With domain handlers, infrastructure management follows the "bring Tasker to your code" model. The handler developer owns their database pools, API clients, secrets management, and encryption concerns. Tasker provides the orchestration framework; the handler provides the infrastructure access.

With virtual handlers and grammar-composed action capabilities (`persist`, `acquire`, `emit`), **Tasker becomes the code**. The `CompositionExecutor` is responsible for executing these capabilities, which means the platform must provide:

1. **Database connections**: `persist` and `acquire` with `resource.type: database` need connection pools. Questions:
   - Does each virtual handler step get its own connection, or is there a shared pool?
   - How is connection information configured per-composition? Per-namespace? Per-resource entity?
   - Can multiple compositions share a pool if they target the same database?

2. **API credentials and HTTP clients**: `acquire` with `resource.type: api` needs authenticated HTTP access. Questions:
   - Where are API credentials stored? Tasker's config? An external secrets manager?
   - How are credentials scoped (per-namespace, per-template, per-step)?
   - Does Tasker manage HTTP client lifecycle (connection pooling, TLS, retries)?

3. **Secrets management**: Action capabilities may need secrets (API keys, database passwords, encryption keys) that were previously the handler developer's concern. Questions:
   - Does Tasker integrate with external secrets managers (Vault, AWS Secrets Manager, etc.)?
   - How are secrets referenced in composition configs without embedding them in YAML?
   - What's the secret rotation model?

4. **Data protection**: With domain handlers, encryption of data in transit and at rest was the handler developer's responsibility. Virtual handlers shift some of this responsibility to the platform:
   - Data flowing through `persist` may need encryption at rest
   - Data flowing through `acquire` may need TLS/mTLS
   - Data flowing through `emit` (domain events via PGMQ) may need payload encryption
   - The platform now handles data that it previously never saw in cleartext

### Design Direction (Not Yet Specified)

The likely model is a **resource registry** — a configured set of named resources with connection details, credential references, and security policies. Action capability configs would reference resources by name rather than embedding connection details:

```yaml
# Hypothetical — NOT a finalized design
- capability: persist
  config:
    resource:
      ref: "orders-db"        # Named resource from registry
      entity: orders
    constraints:
      unique_key: order_ref
  data: |
    { ... }
```

The resource registry would be configured at the namespace or deployment level, with secrets resolved at runtime. This follows the twelve-factor app pattern (configuration in the environment, not in code) and parallels how domain handlers already access databases — but managed by the platform rather than by handler code.

This is a substantial body of work that should be scoped as its own research spike after the core grammar primitives (Phase 1) are validated.

---

## Open Design: Decision and Batch Outcome Expression

*This section proposes extending the grammar to express orchestration outcomes, replacing the earlier conclusion that these were "outside grammar scope."*

### The Insight

The earlier case studies concluded that decision point step creation and batch cursor calculation were outside grammar scope because they produce orchestrator protocol types (`DecisionPointOutcome`, `BatchProcessingOutcome`), not domain results. The grammar couldn't produce these types.

**The revised insight**: The grammar doesn't need to produce these types directly. It can express the **outcome shape** — the data that determines what decision to make or how to partition batches — and the `CompositionExecutor` bridge in `tasker-worker` can translate that shape into the formal orchestrator protocol types.

### How It Would Work

**Decision compositions**: A grammar composition produces a JSON result whose `output` schema matches a decision shape. The `CompositionExecutor` recognizes this and translates it into a `DecisionPointOutcome`:

```yaml
# Virtual handler for a decision point step
compose:
  - capability: transform
    output:
      type: object
      required: [route, steps]
      properties:
        route: { type: string }
        steps:
          type: array
          items:
            type: object
            required: [name]
            properties:
              name: { type: string }
              handler: { type: string }
              config: { type: object }
    filter: |
      if .deps.diamond_start.evens >= .deps.diamond_start.odds
      then {route: "even", steps: [{name: "even_batch_analyzer"}]}
      else {route: "odd", steps: [{name: "odd_batch_analyzer"}]}
      end
```

The grammar composition produces a plain JSON object. The `CompositionExecutor` — which knows this is a decision point step (from the `StepDefinition.step_type`) — takes that JSON and constructs the `DecisionPointOutcome::create_steps()` call. The grammar never imports or references Tasker's orchestration types. The bridge layer in `tasker-worker` handles the translation.

**Batch compositions**: Similarly, a batchable step's grammar composition could produce the cursor configuration:

```yaml
# Virtual handler for a batch analyzer step
compose:
  - capability: transform
    output:
      type: object
      required: [batch_size, worker_count, cursors]
      properties:
        batch_size: { type: integer }
        worker_count: { type: integer }
        cursors:
          type: array
          items:
            type: object
            properties:
              start: { type: integer }
              end: { type: integer }
    filter: |
      .context as $ctx
      | ($ctx.dataset_size / $ctx.batch_size | ceil) as $num_batches
      | [limit($num_batches; range(0; $num_batches))]
      | map({
          start: (. * $ctx.batch_size),
          end: ([(. + 1) * $ctx.batch_size, $ctx.dataset_size] | min)
        })
      | {
          batch_size: $ctx.batch_size,
          worker_count: ([length, $ctx.max_workers] | min),
          cursors: .
        }
```

The `CompositionExecutor` translates this into `BatchProcessingOutcome::create_batches()` with the cursor configs. The grammar computes the partitioning; the worker layer bridges it into the orchestrator protocol.

### Architectural Boundary

The grammar system (`tasker-grammar` crate) remains pure — it knows nothing about `DecisionPointOutcome`, `BatchProcessingOutcome`, or any `tasker-shared` orchestration types. It produces `serde_json::Value` matching a declared `output` schema.

The bridge lives in `tasker-worker`, where the `CompositionExecutor` (which implements `StepHandler`) knows the step type from `StepDefinition.step_type` and can translate the grammar's JSON output into the appropriate orchestrator protocol type:

```
tasker-grammar (pure):
  CompositionSpec → jaq filters → serde_json::Value output

tasker-worker (bridge):
  CompositionExecutor::call(step: &TaskSequenceStep)
    → runs composition via tasker-grammar
    → inspects step.step_definition.step_type
    → if Decision: translate output → DecisionPointOutcome
    → if Batchable: translate output → BatchProcessingOutcome
    → if Standard: return output as StepExecutionResult
```

This separation keeps `tasker-grammar` free of orchestration dependencies while enabling the grammar to express the full range of Tasker step types. The translation layer is thin — it maps JSON fields to Rust struct fields — and lives at the crate boundary where `tasker-worker` already depends on both `tasker-grammar` and `tasker-shared`.

### Virtual Handler Wrapper Types

Rather than a single `CompositionExecutor` that inspects `step_type` and branches, the more natural model is a **family of wrapper `StepHandler` implementations** in `tasker-worker`. Each wrapper owns the orchestration protocol mechanics for its step type and delegates the pure data transformation to a grammar composition:

```
tasker-worker virtual handler wrappers:
├── CompositionHandler
│     Standard step: run composition, return result as StepExecutionResult
│
├── DecisionCompositionHandler
│     Decision step: run composition, translate JSON output → DecisionPointOutcome
│
├── BatchAnalyzerCompositionHandler
│     Batchable step: run composition, translate JSON output → BatchProcessingOutcome
│
└── BatchWorkerCompositionHandler
      Batch worker step: cursor loop + checkpoint_yield + per-chunk composition
```

Each wrapper implements `StepHandler` and is registered via the normal handler resolution path. The `HandlerDispatchService` treats them identically to domain handlers — it calls `handler.call(step)` and gets back a `StepExecutionResult` (or a `DecisionPointOutcome`/`BatchProcessingOutcome` for the protocol-aware wrappers).

### Batch Worker Compositions: Wrapper Around Grammar

The earlier conclusion that batch *worker* checkpoint yields were "outside grammar scope" was correct about the composition model — sequential capability invocations cannot express `while cursor < end` loops. But the insight is that the **wrapper provides the loop, and the composition provides the per-chunk body**:

```
BatchWorkerCompositionHandler::call(step):
  1. Read checkpoint (cursor position, accumulated results)
  2. Loop from cursor to end:
     a. Extract chunk from input data (cursor range)
     b. Build composition context with chunk as input
     c. Run grammar composition (transform/persist per chunk)
     d. Accumulate results
     e. If chunk threshold reached: checkpoint_yield(cursor, accumulated)
  3. Return batch worker success with metrics
```

The grammar composition that runs at step (c) is a normal composition — it receives a chunk of data and does transform/persist work. It doesn't know it's inside a batch loop. The wrapper manages:
- Cursor iteration and chunk extraction
- Checkpoint save/restore via `CheckpointService`
- `checkpoint_yield()` calls at configurable chunk boundaries
- Accumulated result tracking across chunks
- The final `StepExecutionResult` with batch metrics

This means the per-chunk logic (which is often just "transform this subset of records and persist the results") becomes grammar-composable, while the batch lifecycle (which is orchestration machinery) stays in Rust wrapper code.

A batch worker's composition spec might look like:

```yaml
# The per-chunk composition — called once per chunk by the wrapper
compose:
  - capability: transform
    output:
      type: object
      required: [processed_records]
      properties:
        processed_records:
          type: array
          items: { type: object }
        processed_count: { type: integer }
    filter: |
      .chunk
      | map(. + {processed: true, processed_at: now | todate})
      | {processed_records: ., processed_count: length}

  - capability: persist
    config:
      resource: { ref: "analytics-db", entity: processed_records }
      constraints: { batch_insert: true }
    data: |
      .prev.processed_records
    checkpoint: true
```

The wrapper feeds `.chunk` into the composition context for each iteration. The composition transforms and persists each chunk. The wrapper handles everything else.

### Impact on Case Studies

This insight means several handlers previously marked as "outside grammar scope" in the case studies become potential composition candidates:

- `RoutingDecisionHandler` — threshold-based routing can be a `transform` with jq `if-elif-else`, wrapped by `DecisionCompositionHandler`
- `DatasetAnalyzerHandler` — cursor partitioning can be a `transform` with jq math, wrapped by `BatchAnalyzerCompositionHandler`
- `BatchWorkerHandler` — per-chunk processing can be a `transform` + `persist` composition, wrapped by `BatchWorkerCompositionHandler`
- `ResultsAggregatorHandler` (WithBatches path) — aggregation across batch results is already expressible as a standard composition

What remains genuinely outside grammar scope:
- The wrapper implementations themselves (Rust code in `tasker-worker`)
- Domain-specific batch workers where per-chunk logic is opaque (e.g., ML model inference per batch)
- Orchestration protocol type construction (always in the wrapper, never in the grammar)

---

*This document captures the design refinement from the jaq-core integration decision. It should be read alongside `actions-traits-and-capabilities.md` for the grammar architecture, `grammar-trait-boundary.md` for the trait design, and the case study documents for concrete handler-to-composition mappings.*
