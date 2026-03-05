# Case Study: Capability Vocabulary from Handler Patterns

*Extracting a core vocabulary from recurring operations across four languages*

*March 2026 — Research Spike (revised for transform-centric 6-capability model)*

---

## Approach

The grammar proposals in `workflow-patterns.md` and `advanced-patterns.md` generated capability names organically — each handler's internal logic was decomposed into capabilities without a predefined vocabulary. This case study collects those capabilities, identifies patterns, and proposes an initial core vocabulary for the grammar system.

The original analysis identified 9 core capabilities. After adopting **jaq-core** (Rust-native jq implementation) as the unified expression language, the pure data capabilities (`reshape`, `compute`, `evaluate`, `evaluate_rules`) collapsed into a single **`transform`** primitive — see `transform-revised-grammar.md` for the full design rationale. This document reflects the refined **6-capability model**.

---

## Vocabulary Extraction

### The (Action, Resource, Context) Triple

Every grammar-composable capability must express a deterministic triple:

1. **Action** — what operation to perform (transform, validate, assert, persist, acquire, emit)
2. **Resource** — the target upon which the action is effected (a data shape, a database entity, an API endpoint, a domain event topic)
3. **Context** — configuration, prior action context (step results, checkpoint caches, task request inputs), constraints, success validation criteria, and result shape expectations

Capabilities that cannot express this triple — opaque domain operations like `fraud_check`, `payment_gateway_charge`, or `inventory_reserve` — are **not grammar-composable**. They remain as traditional Tasker domain handlers. The grammar system doesn't attempt to represent logic it cannot deterministically decompose.

### Capability Vocabulary (Refined)

From the 27 handlers analyzed across contrib workflows and test fixtures, the following **core capabilities** emerged. Each shares the grammar typology — the capability name IS the action, and the configuration surface expresses (resource, context):

| Capability | Grammar Affinity | Action | (Resource, Context) Surface |
|-----------|-----------------|--------|---------------------------|
| `transform` | All | Derive/reshape/compute data | `output` (JSON Schema) + `filter` (jaq expression) |
| `validate` | Validate | Verify conformance | Schema, coercion, filtering, failure mechanics |
| `assert` | All | Gate execution | `filter` (jaq boolean) + `error` message |
| `persist` | Persist | Write data to target | Typed envelope + jaq `data` filter |
| `acquire` | Acquire | Read data from source | Typed envelope + jaq `params` filter |
| `emit` | Emit | Fire domain event | Typed envelope + jaq `payload` filter |

**What changed from the 9-capability model**: The pure data capabilities `reshape`, `compute`, `evaluate`, and `evaluate_rules` are all subsumed by `transform` — they perform the same fundamental operation (take JSON input, apply a transformation, produce JSON output) and the distinctions between them are conventions for how the jq filter is written, not meaningfully different execution models. Additionally, `group_by` and `rank` are native jq operations (`group_by(.field)`, `sort_by(.field) | reverse`). See `transform-revised-grammar.md` for the detailed before/after mapping.

**What also changed**: `db_insert`, `db_update`, `create_ledger_entries`, `set_reconciliation` are now configurations of `persist`. `lookup_config`, `lookup_purchase`, `http_fetch` are now configurations of `acquire`. `send_email` and `render_template` are eliminated — `emit` fires domain events (not notifications), and content construction/delivery are downstream consumer concerns. These were all **domain outcome descriptions** — YAML-ized descriptions of a thing, not deterministic statements of (action, resource, context).

**What stays outside grammar scope**: `fraud_check`, `payment_gateway_charge`, `gateway_refund`, `inventory_reserve`, `check_policy_window`, `classify_customer`, `generate_credentials` — these are opaque domain operations that should remain as **traditional domain handlers**. We cannot reliably build virtual handler compositions that express their internal logic.

---

## Core Vocabulary (Tier 1)

Capabilities that appeared 3+ times across different workflows, representing general-purpose operations:

### jaq-core Expression Language

The grammar system uses **jaq-core** — a Rust-native jq implementation — as its single unified expression language. This is not a hypothetical choice: jq is a mature, well-documented, widely-understood language for JSON transformation, and jaq-core operates on `serde_json::Value` directly with zero-copy integration.

One expression language powers ALL evaluability across the system. The same jq syntax handles:

**Path traversal and field projection** (what was `reshape`):

```jq
{
  total: .deps.validate_cart.total,
  payment_id: .deps.process_payment.payment_id,
  validated_items: .deps.validate_cart.validated_items
}
```

**Arithmetic and aggregation** (what was `compute`):

```jq
.prev
| .items |= map(. + {line_total: ((.quantity * .unit_price) * 100 | round / 100)})
| . + {subtotal: ([.items[].line_total] | add)}
| . + {tax: ((.subtotal * 0.0875) * 100 | round / 100)}
```

**Boolean expressions** (what was `evaluate`):

```jq
.prev + {
  free_shipping: (.prev.subtotal >= 75.00),
  billing_required: (.prev.price > 0)
}
```

**Conditional selection** (what was `evaluate` with `case` clauses):

```jq
if .prev.health_score >= 80 then "excellent"
elif .prev.health_score >= 60 then "good"
else "needs_improvement" end
```

**First-match rule engine** (what was `evaluate_rules`):

```jq
if .refund_amount <= 50 then {approval_path: "auto_approved"}
elif (.refund_reason | IN("defective","wrong_item")) and .refund_amount <= 500 then {approval_path: "auto_approved"}
elif .refund_amount > 500 then {approval_path: "manager_review"}
else {approval_path: "standard_review"} end
```

**Grouping and ranking** (what was `group_by` and `rank`):

```jq
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

**String construction**:

```jq
"Order \(.order_id) confirmed for \(.customer_email)"
```

**Coalesce/alternative**:

```jq
.primary_email // .fallback_email // "unknown@example.com"
```

**Boolean composition** (used in `assert` filters):

```jq
(.payment_validated and .fraud_passed and .policy_checked)
and (.manager_approved or .auto_approved)
and ((.blacklisted or .sanctioned) | not)
```

**Sandboxing**: jaq-core is safe by construction — it has no I/O capabilities. Runtime sandboxing adds execution timeout (~100ms default) and output size limits. This means jq filters in compositions cannot make network calls, read files, or perform any side effects. Side effects are the exclusive domain of the action capabilities (`persist`, `acquire`, `emit`).

The key principle: **evaluability is a first-class concern with a single standard**, not something embedded ad-hoc into each capability.

### Input Data Convention

Every capability invocation receives the same **composition context envelope** — a `serde_json::Value` constructed by the `CompositionExecutor` from the `TaskSequenceStep`:

```json
{
  "context": {
    "cart_items": [ "..." ],
    "customer_email": "user@example.com"
  },
  "deps": {
    "validate_cart": { "total": 99.99, "validated_items": [ "..." ] },
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

This convention replaces the previous `input_mapping` field. Instead of declaring `input_mapping: { type: task_context }` or `input_mapping: { type: previous }`, jq filters simply reference `.context`, `.deps.{step_name}`, or `.prev` directly. The composition context is always the full envelope — the filter decides what to read from it.

### `transform`

**Type**: Pure function (no side effects)
**Input**: Composition context envelope (`.context`, `.deps`, `.step`, `.prev`)
**Output**: New JSON value matching the declared `output` schema
**Config**: `output` (JSON Schema) + `filter` (jaq expression)

The `transform` capability is the primary data manipulation primitive. It subsumes what were previously `reshape` (selector-path projection), `compute` (arithmetic/aggregation), `evaluate` (boolean/selection derivation), `evaluate_rules` (first-match rule engine), `group_by`, and `rank`. The distinction between these operations is now a naming/documentation convention rather than a type-system distinction — "a transform that produces booleans" vs. "a transform that produces numbers" are both just transforms with different jq filters.

**Structure**:

```yaml
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

**`output`**: JSON Schema declaring the shape this step promises to produce. Enables:
- **Design-time contract chaining**: Tooling validates chains by comparing output schemas to downstream input expectations — without running any jq filters
- **MCP tool discovery**: Agents can inspect what a composition step produces
- **Runtime validation**: Optional — the executor can validate filter output against the declared schema

**`filter`**: jaq (jq) expression that produces the output data. The filter receives the full composition context envelope and returns a JSON value matching the `output` schema.

**How it subsumes previous capabilities**:

| Previous capability | Transform pattern | jq technique |
|--------------------|--------------------|--------------|
| `reshape` (field projection) | Object construction | `{field: .deps.step_name.value}` |
| `compute` (arithmetic) | Math operators | `.quantity * .unit_price`, `[.items[].amount] \| add` |
| `evaluate` (booleans) | Boolean expressions | `.subtotal >= 75.00`, `if-then-elif-else-end` |
| `evaluate_rules` (rule engine) | Conditional chain | `if cond then result elif cond then result else default end` |
| `group_by` (aggregation) | Native jq | `group_by(.field) \| map({...})` |
| `rank` (sorting) | Native jq | `sort_by(.field) \| reverse` |
| `merge_dependencies` (multi-source) | Object construction | `{a: .deps.step_a.val, b: .deps.step_b.val}` |

**What this unification means**: A single `transform` can combine projection, computation, boolean evaluation, and conditional logic in one jq expression. What was previously a `reshape` followed by a `compute` followed by an `evaluate` can often become a single transform:

```yaml
# Previously three capabilities:
# 1. reshape: project fields from deps
# 2. compute: calculate subtotal, tax, total
# 3. evaluate: determine free_shipping, billing_required

# Now one transform:
- capability: transform
  output:
    type: object
    required: [subtotal, tax, total, free_shipping]
    properties:
      subtotal: { type: number }
      tax: { type: number }
      total: { type: number }
      free_shipping: { type: boolean }
  filter: |
    .deps.validate_cart as $cart
    | ($cart.validated_items | map(. + {line_total: ((.quantity * .unit_price) * 100 | round / 100)})) as $items
    | ([$items[].line_total] | add) as $subtotal
    | ($subtotal * 0.0875 * 100 | round / 100) as $tax
    | {
        items: $items,
        subtotal: $subtotal,
        tax: $tax,
        total: ($subtotal + $tax),
        free_shipping: ($subtotal >= 75.00),
        billing_required: ($cart.price > 0)
      }
```

**When to split into multiple transforms**: While a single transform *can* do everything, splitting remains valuable when:
- Different parts of the computation need to be independently testable
- An `assert` gate should check intermediate results before further processing
- The jq filter would become unwieldy (readability matters for maintenance)
- Multiple downstream capabilities need the intermediate output via `.prev`

**Contract chaining benefit**: The `output` schema on each transform enables design-time validation. The `CompositionValidator` can verify that invocation N's filter references to `.prev.field` are satisfied by invocation N-1's declared output schema — without running any jq filters. This is the same contract chaining described in `composition-validation.md`, but with a cleaner model: JSON Schema output declarations replace the ad-hoc inference that was needed when each capability had its own bespoke config format.

**Implementation complexity**: Medium. Requires compiling and executing jaq filters against `serde_json::Value` input, with optional output schema validation. The jaq-core crate handles the expression evaluation; the capability executor handles envelope construction, filter compilation (with caching), and schema validation.

### `validate`

**Type**: Boundary gate (configurable pass-through, coercion, or failure)
**Input**: JSON object from an untrusted or unmanaged source (API response, file read, data connector, upstream dependency with an evolving contract)
**Output**: Validated (and optionally coerced/filtered) JSON object, or error
**Config**: `schema` (field definitions), `coercion` (type flexibility), `unknown_fields` (attribute filtering), `on_failure` (failure mechanics)

```yaml
capability: validate
config:
  schema:
    refund_amount: { type: number, required: true, min: 0.01, max: 10000 }
    refund_reason: { type: string, required: true, enum: [defective, wrong_item, changed_mind] }
    customer_email: { type: string, required: true, pattern: "^[^@]+@[^@]+$" }
    metadata: { type: object, required: false }

  coercion: permissive        # permissive | strict | none
  unknown_fields: drop        # drop | reject | passthrough
  on_failure: fail            # fail | collect | best_effort
```

**Appears in**: Cart validation, payment eligibility, user registration, refund request — every handler that receives data from an external boundary.

**Architectural position**: `validate` operates at the **trust boundary** — the point where untrusted external data enters the composition's internal data flow. This is fundamentally different from `transform`, which operates on data already "inside" the composition. `transform` assumes its inputs are well-typed and structurally sound; `validate` is what makes that assumption safe.

**Three concerns**:

1. **Schema conformance** — does the data match the expected shape? Required fields present, types correct, constraints satisfied (min/max, pattern, enum, array length). This is the core assertion: does reality match expectation?

2. **Type coercion** — when types don't match exactly, can we safely convert? This is explicitly NOT transform (not deriving new values). It's about normalizing data at the boundary so downstream capabilities can trust their type assumptions.
   - `coercion: permissive` — attempt safe conversions: `"123"` -> `123`, `"true"` -> `true`, `123` -> `123.0`, `"2026-03-04"` -> date. Fail on incompatible conversions (e.g., `"hello"` -> number).
   - `coercion: strict` — types must match exactly, no conversion.
   - `coercion: none` — type checking disabled; only check presence and constraints.

3. **Attribute filtering** — what happens with fields not in the schema? This resembles `transform`'s projection superficially, but the intent is defensive. Transform says "I want to derive fields X, Y, Z." Validate filtering says "I expect fields X, Y, Z — and I don't trust anything else."
   - `unknown_fields: drop` — silently remove fields not in schema (defensive filtering at the boundary).
   - `unknown_fields: reject` — fail if any unknown fields present (strict contract enforcement).
   - `unknown_fields: passthrough` — validate known fields, pass unknowns through unchanged.

**Failure mechanics** — `on_failure` is what makes `validate` architecturally distinct. It's the only core capability whose **primary purpose is to decide whether execution should continue**:
   - `on_failure: fail` — first violation causes step failure (`PermanentError`). Strictest mode.
   - `on_failure: collect` — collect all violations, return them as a structured error list. Useful for user-facing validation where you want to report all problems at once.
   - `on_failure: best_effort` — coerce and filter where possible, fail only when required fields are missing or values are fundamentally incompatible. Most permissive mode.

**Relationship to `assert`**: An assertion evaluates a jq boolean expression as an execution gate: "is the total positive? do all items have valid SKUs?" Validation is a schema-level check on data entering from outside: "does this JSON object conform to the contract I expect?" Assertions guard against logic errors in the composition flow; validation guards against contract violations at system boundaries. Both can fail the step, but for different reasons and at different points in the pipeline.

**Relationship to `transform`**: Both can produce a reduced set of fields in the output. But transform is intentional derivation ("produce these fields from these inputs"), while validate's `unknown_fields: drop` is defensive filtering ("reject what I don't expect"). In a typical composition, `validate` at the boundary ensures shape conformance, then `transform` projects and derives exactly the fields needed for the next capability:

```yaml
compose:
  - capability: validate          # Trust boundary: assert shape, coerce types, filter unknowns
    config:
      schema: { ... }
      coercion: permissive
      unknown_fields: drop
      on_failure: fail

  - capability: transform         # Derive exactly what downstream needs
    output:
      type: object
      required: [amount, reason]
      properties:
        amount: { type: number }
        reason: { type: string }
    filter: |
      {
        amount: .prev.refund_amount,
        reason: .prev.refund_reason
      }
```

**Implementation complexity**: Medium. Schema validation can delegate to the `jsonschema` crate. Type coercion requires a safe conversion layer. Attribute filtering is straightforward (set difference on field names). The `on_failure` modes affect error propagation strategy.

### `assert`

**Type**: Guard (returns input unchanged or halts execution)
**Input**: Composition context envelope (reads `.prev`, `.context`, `.deps` as needed)
**Output**: Same input (pass-through) or error
**Config**: `filter` (jaq boolean expression) + `error` (message string)

The `assert` capability evaluates a jq boolean expression and either passes through the input unchanged or fails the step. It does not produce new data — it gates whether the composition continues.

**Simple assertion** (single precondition):

```yaml
- capability: assert
  filter: '.prev.payment_status == "completed"'
  error: "Payment must be validated before processing"
```

**Compound assertion** (multiple conditions):

```yaml
- capability: assert
  filter: |
    (.prev.payment_validated and .prev.fraud_passed and .prev.policy_checked)
    and (.prev.manager_approved or .prev.auto_approved)
    and ((.prev.blacklisted or .prev.sanctioned) | not)
  error: "Cannot proceed with refund — prerequisites, approval, or restrictions failed"
```

The jq `and`/`or`/`not` operators replace the bespoke `all`/`any`/`none` quantifier syntax from the previous design. Named sub-conditions can be expressed as intermediate jq variables if needed, but in most cases the boolean composition is clear enough inline:

```yaml
# If naming sub-conditions aids readability:
- capability: assert
  filter: |
    ((.prev.payment_validated and .prev.fraud_passed and .prev.policy_checked) as $prerequisites
    | (.prev.manager_approved or .prev.auto_approved) as $approved
    | ((.prev.blacklisted or .prev.sanctioned) | not) as $not_restricted
    | $prerequisites and $approved and $not_restricted)
  error: "Cannot proceed with refund"
```

**Relationship to `validate`**: Both can fail the step, but for different reasons. `validate` checks schema conformance at the trust boundary (does this JSON match the contract?). `assert` checks logical conditions within the composition flow (are the prerequisites met?). Validate is structural; assert is semantic.

**Relationship to `transform`**: A transform can produce boolean fields that a downstream assert consumes. But unlike the previous model where `evaluate` produced booleans and `assert` consumed them as a mandatory two-step pattern, the jq-based assert can evaluate any boolean expression directly — there is no need for a preceding transform step unless the boolean computation is complex enough to warrant separate declaration and testing.

**Appears in**: Gateway refund (verify eligibility), record update (verify refund), ticket update (verify delegation), refund policy (verify request validated)

**Implementation complexity**: Low. Compile and execute a jaq boolean filter. If the result is truthy, pass through the input unchanged. If falsy, return a `CapabilityError::Permanent` with the `error` message.

---

## Action Capabilities: `persist`, `acquire`, `emit`

The grammar categories (Acquire, Transform, Validate, Persist, Emit) describe **abstract flow shapes** — what kinds of action compositions can cohere. The corresponding capabilities (`acquire`, `persist`, `emit`) are **concrete invocations** of those same action types, expressing the (action, resource, context) triple with a typed Rust envelope and JSON Schema-flexible configuration.

All three action capabilities use **jaq filters** for data mapping — the same expression language used by `transform` and `assert`. Where the previous design used selector-path expressions like `"$.field"`, the new design uses jq expressions that read from the composition context envelope.

### `persist`

**Type**: Write (mutating, typically checkpointed)
**Input**: Composition context envelope
**Output**: JSON object (success confirmation with result shape)
**Config**: `resource` (target), `constraints`, `validate_success`, `result_shape`
**Data mapping**: `data` (jaq filter producing the object to persist)

The `persist` capability subsumes what were previously `db_insert`, `db_update`, `create_ledger_entries`, `set_reconciliation`, and similar domain-specific write operations. The action is always "write data to a resource target" — domain specificity comes from configuration, not capability naming.

```yaml
# Previously: capability: db_insert + entity: orders
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

```yaml
# Previously: capability: create_ledger_entries
- capability: persist
  config:
    resource:
      type: database
      entity: ledger_entries
    constraints:
      fiscal_period_format: "{year}-{month}"
      idempotency_key: refund_id
    validate_success:
      record_id: { type: string, required: true }
    result_shape: [record_id, journal_id, fiscal_period, created_at]
  data: |
    {
      entries: [
        {type: "debit", account: "refunds_payable", amount: .prev.refund_amount},
        {type: "credit", account: "accounts_receivable", amount: .prev.refund_amount}
      ],
      refund_id: .prev.refund_id
    }
  checkpoint: true
```

```yaml
# Previously: capability: set_reconciliation (was just setting a flag)
- capability: persist
  config:
    resource:
      type: database
      entity: reconciliation_status
    validate_success:
      status: { equals: pending }
    result_shape: [reconciliation_id, status]
  data: |
    {
      status: "pending",
      refund_id: .prev.refund_id,
      journal_id: .prev.journal_id
    }
```

**Config surface** (typed Rust envelope, JSON Schema-flexible values):

| Field | Type | Purpose |
|-------|------|---------|
| `resource` | `{ type, entity, ... }` | Where to persist — database table, API endpoint, queue, etc. |
| `data` | jaq filter | What to persist — jq expression over composition context producing the write payload |
| `constraints` | `{ ... }` | Rules governing the write — idempotency, uniqueness, format rules |
| `validate_success` | `{ field: schema, ... }` | How to confirm success — expected shape of the write result |
| `result_shape` | `[fields...]` | What the next capability sees — projection of the write confirmation |

**Implementation complexity**: Medium. The `resource.type` discriminator selects a backend (database adapter, API client, queue producer). The Rust type provides the envelope structure; the JSON Schema within each field is runtime-flexible. Different resource types will have different valid configurations — a database persist needs `entity`, an API persist needs `endpoint`.

**Appears in**: create_order, update_inventory, create_user_account, update_payment_records, set_reconciliation, setup_billing, update_ticket_status

### `acquire`

**Type**: Read (non-mutating)
**Input**: Composition context envelope
**Output**: JSON object (acquired data in result shape)
**Config**: `resource` (source), `constraints`, `validate_success`, `result_shape`
**Param mapping**: `params` (jaq filter producing the query parameters)

The `acquire` capability subsumes `http_fetch`, `lookup_config`, `lookup_purchase`, and similar read operations. The action is always "read data from a resource source."

```yaml
# Previously: capability: http_fetch
- capability: acquire
  config:
    resource:
      type: api
      endpoint: "/api/sales"
      method: GET
    constraints:
      timeout_ms: 5000
      cache_ttl: 300
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

```yaml
# Previously: capability: lookup_config
- capability: acquire
  config:
    resource:
      type: config
      table: rate_plans
      key_field: plan_code
    constraints:
      required: true
    result_shape: [monthly_rate, billing_cycle, features]
  params: |
    {
      plan_code: .prev.plan_code
    }
```

**Config surface**:

| Field | Type | Purpose |
|-------|------|---------|
| `resource` | `{ type, endpoint/table/key, ... }` | Where to read from — API, config store, database, cache |
| `params` | jaq filter | Query parameters — jq expression over composition context |
| `constraints` | `{ timeout_ms, cache_ttl, required, ... }` | Rules governing the read — timeouts, caching, required vs optional |
| `validate_success` | `{ field: schema, ... }` | How to confirm the read succeeded |
| `result_shape` | `[fields...]` | What the next capability sees — projection of the acquired data |

**Appears in**: extract_sales_data, lookup_config (rate plans, billing), lookup_purchase, all data source fetch operations

### `emit`

**Type**: Domain event publication (side-effecting)
**Input**: Composition context envelope
**Output**: JSON object (event publication confirmation)
**Config**: `event_name`, `event_version`, `delivery_mode`, `condition`, optional `schema`
**Payload mapping**: `payload` (jaq filter producing the event payload)

The `emit` capability is the **action grammar representation of firing a domain event**. Tasker already has a mature domain event system (`DomainEvent`, `DomainEventPublisher`, three delivery modes, PGMQ persistence, JSON Schema validation). The `emit` capability maps directly onto this existing infrastructure.

`emit` does NOT try to send emails, push notifications, or interact with external services. It fires a domain event with a payload. What happens downstream — whether some other service reads from the `{namespace}_domain_events` PGMQ queue and sends an email, updates a CRM, or triggers a webhook — is entirely outside the grammar system's concern.

```yaml
# Refund notification — fires a domain event, not an email
- capability: emit
  config:
    event_name: "refund.processed"
    event_version: "1.0"
    delivery_mode: durable
    condition: success
  payload: |
    {
      refund_id: .prev.refund_id,
      amount_refunded: .prev.amount_refunded,
      customer_email: .prev.customer_email,
      estimated_arrival: .prev.estimated_arrival,
      correlation_id: .prev.correlation_id
    }
  schema:
    type: object
    required: [refund_id, amount_refunded, customer_email]
    properties:
      refund_id: { type: string }
      amount_refunded: { type: number }
      customer_email: { type: string }
      estimated_arrival: { type: string }
```

```yaml
# Order confirmation — fires a domain event for downstream consumers
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
      estimated_delivery: .prev.estimated_delivery,
      items: .prev.validated_items
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

**Config surface**:

| Field | Type | Purpose |
|-------|------|---------|
| `event_name` | string | Dot-notation event name (e.g., `refund.processed`, `order.confirmed`) |
| `event_version` | string | Event schema version for consumer compatibility |
| `delivery_mode` | `durable \| fast \| broadcast` | Maps to Tasker's `EventDeliveryMode` — PGMQ persistence, in-memory, or both |
| `condition` | `success \| failure \| always` | Maps to Tasker's `PublicationCondition` — when to fire |
| `payload` | jaq filter | jq expression projecting data from composition context into event payload |
| `schema` | JSON Schema | Optional payload validation — same as `publishes_events.schema` in task templates |

**Relationship to Tasker's domain event system**: `emit` maps 1:1 onto the existing `DomainEvent` infrastructure. The `event_name` becomes `DomainEvent.event_name`, the `payload` filter output is assembled into `DomainEventPayload.payload`, `delivery_mode` selects the `EventDeliveryMode`, and `schema` provides the same JSON Schema validation that `publishes_events` declarations already support. The composition executor constructs and publishes the `DomainEvent` through the existing `DomainEventPublisher` -> `EventRouter` pipeline.

**What `emit` is NOT**: It is not an email client, notification dispatcher, or API integration. Template rendering, marketing content, and external service calls are concerns for **downstream consumers** of the domain event. The grammar system's responsibility ends at firing the event with the right payload shape. This aligns with Tasker's existing design where domain events are "declarative of what HAS happened" — and consumers decide what to do about it.

**Appears in**: notify_customer, send_confirmation, send_welcome_sequence (all reframed as domain event publications)

### What Stays Outside Grammar Scope

Operations that cannot be expressed as (action, resource, context) remain as **traditional domain handlers**. The grammar system doesn't attempt to compose what it cannot deterministically decompose:

| Operation | Why Not Grammar-Composable |
|-----------|--------------------------|
| `fraud_check` | Proprietary algorithm, model-specific scoring, data-specific features |
| `payment_gateway_charge` | Provider-specific API, error codes, retry semantics, PCI compliance |
| `gateway_refund` | Same as charge — provider-specific interaction |
| `inventory_reserve` | Warehouse topology, distributed reservation protocols |
| `check_policy_window` | Organization-specific policy rules, jurisdiction-dependent |
| `classify_customer` | Organization-specific classification logic |
| `generate_credentials` | Security-sensitive — token generation, key management |

These are handlers where the "how" is irreducibly domain-specific. An agent or composition executor cannot reliably parameterize them through a generic (action, resource, context) surface. They should be implemented as domain handler code — the same domain handlers that exist today.

Operations that produce **orchestration protocol types** — decision point outcomes, batch processing outcomes — were previously considered outside grammar scope but are now composable via virtual handler wrappers. The grammar composition produces the routing or partitioning logic as plain JSON; the wrapper in `tasker-worker` translates the JSON output to the formal protocol type (`DecisionPointOutcome`, `BatchProcessingOutcome`). The grammar system itself (`tasker-grammar` crate) remains pure — no orchestration types leak in. See `transform-revised-grammar.md`, section "Open Design: Decision and Batch Outcome Expression" for the full design.

---

## Vocabulary Architecture

### Grammar-Capability Typology

The grammar categories (Acquire, Transform, Validate, Persist, Emit) and the capability vocabulary share a **typology**: both describe action types, but at different levels of abstraction.

**Grammar** is the abstract structural layer. It describes:
- What flows can cohere (a Validate grammar naturally begins a composition, Emit naturally ends one)
- Where checkpoints belong (persist and emit are typically checkpointed; transform is not)
- Ordering constraints (nothing has been validated or acquired to emit at the top of a composition)

**Capabilities** are concrete invocations within a grammar composition. Each is an (action, resource, context) triple where:
- The **Rust types** describe the capability-config-shape (the envelope: `resource`, `constraints`, `validate_success`, `result_shape`)
- The **JSON Schema-flexible contents** within those typed fields are runtime-variable (what the fields in `data` should be, what `timeout_ms` value to use, etc.)

```
Grammar Layer (abstract flow structure):
  Acquire -> Transform -> Validate -> Persist -> Emit

Capability Layer (concrete action invocations):
  Core Data Operations (pure, no side effects):
  |-- transform       -- jaq-powered data derivation (output schema + filter)
  |-- validate        -- boundary gate (schema, coercion, filtering, failure)
  +-- assert          -- composable execution gate (jaq boolean filter)

  Action Capabilities (side-effecting, typed envelope + jaq data mapping):
  |-- acquire         -- read data from resource source
  |-- persist         -- write data to resource target
  +-- emit            -- fire domain event (maps to Tasker's DomainEvent system)

Virtual Handler Wrappers (protocol bridge in tasker-worker):
  |-- CompositionHandler              -- standard step: composition → StepExecutionResult
  |-- DecisionCompositionHandler      -- decision step: composition → DecisionPointOutcome
  |-- BatchAnalyzerCompositionHandler -- batch analyzer: composition → BatchProcessingOutcome
  +-- BatchWorkerCompositionHandler   -- batch worker: loop + checkpoint + per-chunk composition

Domain Handlers (outside grammar scope):
  |-- fraud_check, payment_gateway_charge, gateway_refund
  |-- inventory_reserve, classify_customer, generate_credentials
  +-- ... any operation where (action, resource, context) cannot be deterministically expressed
```

### Grammar-Category Mapping

| Grammar Category | Capabilities | Typical Shape |
|-----------------|-------------|---------------|
| **Acquire** | `acquire` | acquire -> transform |
| **Transform** | `transform` | transform (single or chained) |
| **Validate** | `validate`, `assert` | validate at boundary, assert at gates |
| **Persist** | `persist` | transform -> persist |
| **Emit** | `emit` | transform -> emit |

The virtual handler wrapper pattern extends this mapping: compositions can now produce outputs that the wrapper translates to decision point outcomes or batch processing outcomes. The grammar categories themselves are unchanged — a decision step's composition still uses the same Acquire/Transform/Validate/Persist/Emit capabilities — but the wrapper bridges the composition's JSON output to the orchestration protocol type required by the step's role.

### What This Means for `CapabilityExecutor`

The trait from `grammar-trait-boundary.md` serves the core capabilities:

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

For core data operations (`transform`, `validate`, `assert`), the `config` parameter is fully specified by the jaq filter/JSON Schema and the capability's typed configuration. For action capabilities (`persist`, `acquire`, `emit`), the `config` parameter contains a typed envelope (`resource`, `constraints`, `validate_success`, `result_shape`) with JSON Schema-flexible contents — the Rust type enforces the envelope structure, while the values within are runtime-variable.

Operations that fall outside this model — opaque domain logic — remain as traditional domain handlers. The grammar system provides the composition framework for what it can express; domain handler code provides the implementation for what it cannot.

---

## Cross-Language Observations

### Handler Pattern -> Capability Pattern Mapping

From examining all four language implementations:

| Handler DSL Pattern | Capability Equivalent |
|--------------------|----------------------|
| `@depends_on(cart=("validate_cart", CartType))` | `.deps.validate_cart` in jaq filter + composition resolves deps |
| `@inputs(EcommerceOrderInput)` | `validate` capability with schema (validated at registration) |
| `return svc.validate_cart_items(...)` | Capability `execute()` delegates to implementation |
| `PermanentError("...")` | `CapabilityError::Permanent { message }` |
| `RetryableError("...")` | `CapabilityError::Retryable { message }` |
| `context.get_input_or('key', default)` | `.context.key // default` in jaq filter |

### What's Different in Capability Executors

1. **No dependency declarations** — the composition executor handles input resolution
2. **No step lifecycle** — capabilities don't know about workflow steps, retries, or state machines
3. **Simpler error model** — just Retryable vs. Permanent, no DLQ or backoff config
4. **Schema-first** — input/output schemas are part of the trait, not decorators
5. **Config-driven** — behavior varies by config, not by code branching

### The Service Layer Pattern Persists

All four languages delegate handler logic to service functions. The same pattern applies to capabilities:

```
Today:
  Domain Handler DSL -> Service Function -> External Calls

With grammars:
  Virtual Handler (Composition Spec) -> Capability Executor -> Service Function -> External Calls
```

The service layer is unchanged. Capabilities wrap services the same way handlers do. The difference is that capabilities declare their contracts (schemas) and the composition system validates compatibility before execution.

---

## Composition Shape Patterns

From the grammar proposals, three composition shapes emerged repeatedly:

### Shape 1: Validate -> Transform -> Persist

```yaml
compose:
  - capability: validate           # Boundary gate: assert shape, coerce, filter
    config:
      schema: { ... }
      coercion: permissive
      on_failure: fail

  - capability: transform          # Derive fields, calculate values
    output: { ... }
    filter: |
      { ... computed/derived fields ... }

  - capability: persist            # Write record to resource target
    config:
      resource: { ... }
    data: |
      { ... mapped fields ... }
    checkpoint: true
```

**Appears in**: create_user_account, update_inventory, create_order, update_payment_records, update_ticket_status

### Shape 2: Transform (projection + computation)

```yaml
compose:
  - capability: transform          # Project, compute, evaluate — all in one
    output: { ... }
    filter: |
      { ... projected and computed fields ... }
```

Note: What was previously a `reshape` -> `compute` chain can often be a single `transform` — jq naturally combines projection, arithmetic, boolean evaluation, and conditional logic in one expression.

**Appears in**: All extract handlers, all transform handlers, aggregate_metrics

### Shape 3: Transform -> Emit

```yaml
compose:
  - capability: transform          # Gather and derive data from dependencies
    output: { ... }
    filter: |
      { ... gathered dependency data ... }

  - capability: emit               # Fire domain event with payload
    config:
      event_name: "order.confirmed"
      delivery_mode: durable
    payload: |
      { ... payload from prev ... }
```

**Appears in**: send_confirmation, notify_customer, send_welcome_sequence (all reframed as domain event publications)

### What Shapes Tell Us

These three shapes map to three grammar categories:
- **Persist**: validate -> transform -> persist (to resource target)
- **Transform**: transform (derive and aggregate — single or chained)
- **Emit**: transform -> emit (domain event with derived payload)

Each grammar category has a characteristic composition shape. This isn't enforced — a Persist grammar could have any number of capabilities in any order — but the patterns suggest that grammar categories naturally gravitate toward specific shapes. The grammar layer provides structural insight: `emit` cannot appear at the top of a composition (nothing has been acquired or validated to emit), and `validate` naturally gravitates toward the boundary where untrusted data enters.

Note how the Emit shape is the simplest — `transform` gathers the data from upstream dependencies, and `emit` fires the domain event with that data as the payload. Content construction (email templates, notification formatting) is NOT the grammar's concern — that belongs to downstream consumers of the domain event. Future tooling (template generators, MCP tools) could use these shapes as defaults when scaffolding compositions.

---

## Open Questions from the Case Studies

### 1. Shared Expression Language -- RESOLVED

**Decision**: jaq-core (Rust-native jq implementation). See `transform-revised-grammar.md` for the full design rationale and `implementation-phases.md` TAS-321 for the implementation ticket.

jq provides all the capabilities that were previously listed as requirements: arithmetic, aggregation, comparisons, boolean combinators, conditional selection, path traversal, and string construction. The bespoke expression syntax (selector-paths, `select()`, `case()`, `decimal(N)`) is replaced entirely by standard jq syntax. One mature, well-documented language replaces what would have been a custom DSL.

### 2. Per-Record vs. Whole-Input -- RESOLVED

jq naturally handles both modes. Per-record operations use `map()` or array iteration (`[.items[] | ...]`), while whole-input operations work on the root value directly. There is no need for a `select: "items[*]"` vs. `select: "$"` mode distinction — the jq filter itself determines the scope of operation.

### 3. Conditional Logic Within Compositions -- RESOLVED

jq's `if-then-elif-else-end` handles all three levels of conditional logic that were identified:

1. **Field-level**: Produce boolean fields within a `transform` filter (`.subtotal >= 75.00`)
2. **Value-level**: Choose between values in a `transform` filter (`if .free_shipping then 0 else 9.99 end`)
3. **Execution-level**: Gate continuation with `assert` (`filter: '.prev.total > 0'`)

The previously discussed `when:` clause for capability-level conditional skipping remains a potential future addition, but the three levels above cover the patterns observed in the case studies. If capability-level skipping is needed, it would use the same jq expression language.

### 4. Evaluate and Compute Composition -- RESOLVED

The question of "should evaluate and compute be separate steps (Option A) or combined (Option B)?" dissolves with the transform-centric model. There is just one `transform` with a jq filter that does whatever combination of boolean, arithmetic, and conditional logic is needed. The author chooses whether to use one transform or several based on readability, testability, and whether intermediate `assert` gates are needed — not because the type system forces a separation.

### 5. Decision and Batch Steps in Grammar Scope -- RESOLVED

Decision point outcomes and batch processing outcomes were previously considered outside grammar scope because they require orchestration protocol types that the grammar system should not depend on. The virtual handler wrapper pattern in `transform-revised-grammar.md` resolves this: compositions produce plain JSON; wrapper types in `tasker-worker` (`DecisionCompositionHandler`, `BatchAnalyzerCompositionHandler`, `BatchWorkerCompositionHandler`) translate the JSON output to the formal protocol type. The grammar crate stays pure — no orchestration types — while the expressible surface of grammar compositions extends to cover decision routing and batch partitioning logic.

---

*This case study synthesizes findings from `workflow-patterns.md` and `advanced-patterns.md` into a vocabulary proposal. It should be read alongside `transform-revised-grammar.md` for the jaq-core design refinement, `grammar-trait-boundary.md` for the trait design, `composition-validation.md` for contract chaining between capabilities, and `virtual-handler-dispatch.md` for how grammar-composed virtual handlers execute within the worker infrastructure.*
