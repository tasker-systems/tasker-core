# Case Study: Capability Vocabulary from Handler Patterns

*Extracting a core vocabulary from recurring operations across four languages*

*March 2026 — Research Spike*

---

## Approach

The grammar proposals in `workflow-patterns.md` and `advanced-patterns.md` generated capability names organically — each handler's internal logic was decomposed into capabilities without a predefined vocabulary. This case study collects those capabilities, identifies patterns, and proposes an initial core vocabulary for the grammar system.

---

## Vocabulary Extraction

### The (Action, Resource, Context) Triple

Every grammar-composable capability must express a deterministic triple:

1. **Action** — what operation to perform (validate, reshape, compute, evaluate, assert, evaluate_rules, persist, acquire, emit)
2. **Resource** — the target upon which the action is effected (a data shape, a database entity, an API endpoint, a domain event topic)
3. **Context** — configuration, prior action context (step results, checkpoint caches, task request inputs), constraints, success validation criteria, and result shape expectations

Capabilities that cannot express this triple — opaque domain operations like `fraud_check`, `payment_gateway_charge`, or `inventory_reserve` — are **not grammar-composable**. They remain as traditional Tasker domain handlers. The grammar system doesn't attempt to represent logic it cannot deterministically decompose.

### Capability Vocabulary (Refined)

From the 27 handlers analyzed across contrib workflows and test fixtures, the following **core capabilities** emerged. Each shares the grammar typology — the capability name IS the action, and the configuration surface expresses (resource, context):

| Capability | Grammar Affinity | Action | (Resource, Context) Surface |
|-----------|-----------------|--------|---------------------------|
| `validate` | Validate | Verify conformance | Schema, coercion, filtering, failure mechanics |
| `reshape` | All | Reorganize data shape | Selector-paths over data from any source(s) |
| `compute` | All | Derive numeric/data values | Expressions, aggregation, numeric casting |
| `evaluate` | All | Determine booleans/selections | Boolean expressions, case/switch selection |
| `assert` | All | Gate execution | Named conditions, set-logic quantifiers, precedent |
| `evaluate_rules` | Validate, Transform | Match conditions to results | Rules, match semantics (first/all) |
| `persist` | Persist | Write data to target | Resource target, data, constraints, success validation, result shape |
| `acquire` | Acquire | Read data from source | Resource source, constraints, result shape |
| `emit` | Emit | Fire domain event | Event name, version, delivery mode, condition, payload, schema |

**What changed**: `db_insert`, `db_update`, `create_ledger_entries`, `set_reconciliation` are now configurations of `persist`. `lookup_config`, `lookup_purchase`, `http_fetch` are now configurations of `acquire`. `send_email` and `render_template` are eliminated — `emit` fires domain events (not notifications), and content construction/delivery are downstream consumer concerns. These were all **domain outcome descriptions** — YAML-ized descriptions of a thing, not deterministic statements of (action, resource, context).

**What stays outside grammar scope**: `fraud_check`, `payment_gateway_charge`, `gateway_refund`, `inventory_reserve`, `check_policy_window`, `classify_customer`, `generate_credentials` — these are opaque domain operations that should remain as **traditional domain handlers**. We cannot reliably build virtual handler compositions that express their internal logic.

---

## Core Vocabulary (Tier 1)

Capabilities that appeared 3+ times across different workflows, representing general-purpose operations:

### Shared Expression Language

Before describing individual capabilities, note that `compute` and `evaluate` share a **single expression language** — neither capability "owns" conditional logic or arithmetic. The expression language is infrastructure that both capabilities leverage for their distinct purposes.

The expression language provides:
- **Selector-path access**: `items[*].field`, `$.field`, `groups[*].revenue`
- **Arithmetic**: `+`, `-`, `*`, `/`, `^` with operator precedence
- **Aggregation functions**: `sum()`, `count()`, `avg()`, `min()`, `max()`
- **Comparison operators**: `>`, `>=`, `<`, `<=`, `==`, `!=`, `in`
- **Boolean combinators**: `AND`, `OR`, `NOT`
- **Date functions**: `date_add()`, `now()`
- **Utility functions**: `clamp()`, `has()`, `generate_id()`
- **Numeric casting**: `decimal(N)`, `integer`, `float`

This shared language means that the same expression `subtotal >= 75.00` can:
- Guide a **decision point step** in a workflow DAG (routing expression)
- Determine a **boolean field** in a return value (`{ free_shipping: true }` via `evaluate`)
- Inform a **derived value** in a compute operation (by referencing evaluable fields)

The selector-path subset of the expression language also powers the `reshape` capability — pure data projection/reorganization without expression evaluation. All three core data capabilities (`reshape`, `evaluate`, `compute`) share this foundation, differing only in what they do after selection.

The key principle: **evaluability is a first-class concern with a single standard**, not something embedded ad-hoc into each capability.

### `reshape`

**Type**: Pure function (no side effects)
**Input**: JSON object(s) — may span task context, dependency outputs, or previous capability output
**Output**: New JSON object with projected/reorganized fields
**Config**: `fields` map of output_name → selector-path expression
**Scope**: Data projection and reorganization — selector-paths only, no arithmetic or boolean evaluation

```yaml
capability: reshape
config:
  fields:
    customer_email: "$.email"
    order_total: "validate_cart.total"
    payment_ref: "process_payment.transaction_id"
    item_skus: "validate_cart.validated_items[*].sku"
    items: "validate_cart.validated_items[*].{sku, name, quantity, unit_price}"
```

**Appears in**: Every handler that pulls fields from upstream dependencies, every convergence point that assembles data from multiple sources, every notification handler that gathers context before rendering — 14+ occurrences across all grammar categories.

**What `reshape` subsumes**:
- `extract_field` (single-source field extraction) — reshape with one source
- `merge_dependencies` (multi-source assembly) — reshape with multiple source paths
- `resolve_recipient` (fallback from multiple sources) — reshape with ordered selector-paths
- `build_summary` (conditional field inclusion) — reshape with selector-paths that may resolve to null
- `deep_merge` (overlay two objects) — reshape where later fields override earlier ones

All of these are variations on the same operation: use selector-paths to project data from one or more sources into a new shape. The distinction between "extract from one source" and "merge from many sources" is artificial — the selector-path notation already knows how to reach into any source.

**Relationship to `compute` and `evaluate`**: `reshape` uses the selector-path subset of the shared expression language. It does NOT apply arithmetic expressions (that's `compute`) or boolean/selection expressions (that's `evaluate`). The three capabilities form a spectrum:

| Capability | Uses selector-paths | Applies expressions | Concern |
|-----------|-------------------|-------------------|---------|
| `reshape` | yes | no | Reorganize data shape |
| `evaluate` | yes | boolean/selection | Determine boolean/selection fields |
| `compute` | yes | arithmetic/aggregation | Derive numeric/data fields |

This means extraction and transformation are not separate concerns but positions on a continuum. Pure extraction is `reshape`. Extraction with boolean determinations is `reshape` → `evaluate`. Extraction with derived calculations is `reshape` → `compute`. Full transformation is `reshape` → `evaluate` → `compute`. The shared expression language infrastructure serves all three.

**Implementation complexity**: Low. Pure selector-path resolution — the simplest use of the expression language. No evaluation, no arithmetic, just path traversal and projection.

### `compute`

**Type**: Pure function (no side effects)
**Input**: JSON object with source fields (may include boolean fields from upstream `evaluate`)
**Output**: Same object enriched with computed/derived fields
**Config**: `operations` array, each with `select` (selector-path), `derive` (expression map), optional `cast` (numeric type)
**Scope**: Arithmetic, aggregation, data derivation — NOT boolean/conditional evaluation

```yaml
capability: compute
config:
  operations:
    - select: "items[*]"           # Per-record: enrich each item
      derive:
        line_total: "quantity * unit_price"
      cast: decimal(2)
    - select: "$"                  # Whole-input: aggregate across items
      derive:
        subtotal: "sum(items[*].line_total)"
        tax: "subtotal * 0.0875"
        total: "subtotal + tax + shipping"
      cast: decimal(2)
```

**Appears in**: Cart validation (totals), payment (fees), analytics (derived metrics), math handlers (square/multiply), delivery estimates, refund percentages — 12 occurrences across all grammar categories.

**Three concerns**:
1. **Selector-path** — which values to operate on (JSONPath-like: `items[*]`, `$`, `groups[*].revenue`)
2. **Expression** — what arithmetic/aggregation to perform (`sum()`, `count()`, `*`, `/`, `date_add()`)
3. **Cast** — what numeric type the result should be (`decimal(2)`, `integer`, `float`)

**What `compute` does NOT do**: Boolean evaluation and conditional branching. Where earlier drafts had `if(subtotal >= 75, 0, 9.99)` inside `compute`, this conditional logic belongs in an `evaluate` capability that produces boolean/selection fields. `compute` then references those fields. This separation avoids mixing arithmetic transformation with boolean evaluation — two distinct concerns that happen to share an expression language.

This unified model absorbs what were initially separate capabilities: `compute_derived`, `compute_line_totals`, `compute_order_totals`, `compute_health_score`, `compute_delivery_estimate`, `compute_totals`. All are variations on: select target → apply expression → cast result.

**Implementation complexity**: Medium. Requires a safe expression evaluator with selector-path support, numeric precision, and type casting. Simpler than previous drafts because conditional logic is delegated to `evaluate`.

### `evaluate`

**Type**: Pure function (no side effects)
**Input**: JSON object with fields to evaluate
**Output**: Same object enriched with boolean/selection fields
**Config**: `expressions` map of field_name → expression (boolean or selection)

```yaml
capability: evaluate
config:
  expressions:
    free_shipping: "subtotal >= 75.00"
    high_value_order: "subtotal > 500"
    billing_required: "price > 0"
    health_rating:
      case:
        - when: "health_score >= 80"
          then: "excellent"
        - when: "health_score >= 60"
          then: "good"
        - when: "health_score >= 40"
          then: "fair"
        - default: "needs_improvement"
```

**Appears in**: Shipping determination, billing conditionals, health scoring classification, approval routing, notification channel selection, tier-based policy evaluation, fraud threshold evaluation — pervasive across all grammar categories.

**What `evaluate` provides**:
1. **Boolean fields**: `{ free_shipping: true, high_value: false }` — expressions that resolve to true/false
2. **Selection fields**: `{ health_rating: "excellent", billing_status: "active" }` — case/switch expressions that resolve to a value from a set of options
3. **Evaluable state**: The output is data in an "evaluable" state — downstream capabilities can reference these fields without re-implementing the evaluation logic

**Relationship to `evaluate_rules`**: The earlier `evaluate_rules` capability (first-match rule engine with condition/result pairs) is a specific composition pattern using `evaluate`. Rule evaluation is: evaluate N conditions → select first match → return associated result. The `evaluate` capability is the primitive; rule engines are compositions of it.

**Relationship to `compute`**: Both use the shared expression language. `evaluate` produces boolean/selection values; `compute` produces numeric/data values. When a `compute` operation needs conditional logic (e.g., shipping = 0 or 9.99), it composes with an upstream `evaluate` that determines the condition, then references the evaluated field:

```yaml
# evaluate determines the boolean
- capability: evaluate
  config:
    expressions:
      free_shipping: "subtotal >= 75.00"

# compute references the evaluated field
- capability: compute
  config:
    operations:
      - select: "$"
        derive:
          shipping: "select(free_shipping, 0, 9.99)"
        cast: decimal(2)
```

Here, `select(field, if_true, if_false)` is a value-selection function in the expression language — it selects between two values based on an already-evaluated boolean. The boolean evaluation itself happened in `evaluate`. This is distinct from `if()` which would embed the evaluation inline: `select()` references an evaluable field, `if()` performs the evaluation. The former separates concerns; the latter mixes them.

**Relationship to workflow decision points**: The same expression language that powers `evaluate` within a composition can also power decision point routing in the workflow DAG. A decision step's routing conditions (`amount >= 5000 → dual_approval`) use the same expression syntax as an `evaluate` capability's boolean fields. This means one expression engine serves all evaluability needs — from field-level booleans inside a handler to DAG-level routing across the workflow.

**Implementation complexity**: Medium. Shares the expression evaluator with `compute`. The evaluate-specific concern is producing typed output (boolean vs. selection) and ensuring downstream capabilities can reference evaluated fields by name.

### ~~`merge_dependencies`~~ → subsumed by `reshape`

The earlier `merge_dependencies` capability (combine outputs from multiple upstream steps) is now subsumed by `reshape`. What was:

```yaml
capability: merge_dependencies
config:
  sources:
    cart: { dependency: validate_cart, fields: [total, validated_items] }
    payment: { dependency: process_payment, fields: [payment_id, transaction_id] }
```

Becomes:

```yaml
capability: reshape
config:
  fields:
    total: "validate_cart.total"
    validated_items: "validate_cart.validated_items"
    payment_id: "process_payment.payment_id"
    transaction_id: "process_payment.transaction_id"
```

Same result, but using the general-purpose `reshape` capability with selector-paths that reach into different dependency outputs. No special-purpose capability needed.

**Implementation note**: `reshape` needs access to dependency results when selector-paths reference upstream step names. The composition executor resolves workflow-level dependencies at composition entry, making them available as resolvable paths. For composition-internal steps, `input_mapping` handles step-to-step data flow as before.

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

**Architectural position**: `validate` operates at the **trust boundary** — the point where untrusted external data enters the composition's internal data flow. This is fundamentally different from `reshape`, `evaluate`, and `compute`, which all operate on data already "inside" the composition. Those capabilities assume their inputs are well-typed and structurally sound; `validate` is what makes that assumption safe.

**Three concerns**:

1. **Schema conformance** — does the data match the expected shape? Required fields present, types correct, constraints satisfied (min/max, pattern, enum, array length). This is the core assertion: does reality match expectation?

2. **Type coercion** — when types don't match exactly, can we safely convert? This is explicitly NOT reshape (not selecting different fields) and NOT compute (not deriving new values). It's about normalizing data at the boundary so downstream capabilities can trust their type assumptions.
   - `coercion: permissive` — attempt safe conversions: `"123"` → `123`, `"true"` → `true`, `123` → `123.0`, `"2026-03-04"` → date. Fail on incompatible conversions (e.g., `"hello"` → number).
   - `coercion: strict` — types must match exactly, no conversion.
   - `coercion: none` — type checking disabled; only check presence and constraints.

3. **Attribute filtering** — what happens with fields not in the schema? This resembles `reshape`'s projection superficially, but the intent is defensive. Reshape says "I want fields X, Y, Z." Validate filtering says "I expect fields X, Y, Z — and I don't trust anything else."
   - `unknown_fields: drop` — silently remove fields not in schema (defensive filtering at the boundary).
   - `unknown_fields: reject` — fail if any unknown fields present (strict contract enforcement).
   - `unknown_fields: passthrough` — validate known fields, pass unknowns through unchanged.

**Failure mechanics** — `on_failure` is what makes `validate` architecturally distinct. It's the only core capability whose **primary purpose is to decide whether execution should continue**:
   - `on_failure: fail` — first violation causes step failure (`PermanentError`). Strictest mode.
   - `on_failure: collect` — collect all violations, return them as a structured error list. Useful for user-facing validation where you want to report all problems at once.
   - `on_failure: best_effort` — coerce and filter where possible, fail only when required fields are missing or values are fundamentally incompatible. Most permissive mode.

**Relationship to `assert`**: An assertion composes boolean conditions (produced by `evaluate`) into execution gates: "are prerequisites_met AND has_approval AND not_restricted?" Validation is a schema-level check on data entering from outside: "does this JSON object conform to the contract I expect?" Assertions guard against logic errors in the composition flow (composing evaluated booleans with set-logic quantifiers); validation guards against contract violations at system boundaries (schema conformance). Both can fail the step, but for different reasons and at different points in the pipeline.

**Relationship to `reshape`**: Both can reduce the set of fields in the output. But reshape is intentional projection ("give me these fields"), while validate's `unknown_fields: drop` is defensive filtering ("reject what I don't expect"). In a typical composition, `validate` at the boundary ensures shape conformance, then `reshape` projects exactly the fields needed for the next capability:

```yaml
compose:
  - capability: validate          # Trust boundary: assert shape, coerce types, filter unknowns
    config:
      schema: { ... }
      coercion: permissive
      unknown_fields: drop
      on_failure: fail

  - capability: reshape           # Projection: select exactly what downstream needs
    config:
      fields:
        amount: "$.refund_amount"
        reason: "$.refund_reason"
```

**Implementation complexity**: Medium. Schema validation can delegate to the `jsonschema` crate. Type coercion requires a safe conversion layer. Attribute filtering is straightforward (set difference on field names). The `on_failure` modes affect error propagation strategy.

### `evaluate_rules`

**Type**: Rule engine (no side effects) — a composition pattern built on `evaluate`
**Input**: JSON object with fields to evaluate against rules
**Output**: Result from first matching rule (or all matching rules)
**Config**: `rules` array with `condition` and `result` per rule, plus `first_match` flag

```yaml
capability: evaluate_rules
config:
  rules:
    - condition: "refund_amount <= 50"
      result: { approval_path: auto_approved }
    - condition: "refund_reason in auto_approve_reasons AND amount <= 500"
      result: { approval_path: auto_approved }
    - condition: "true"
      result: { approval_path: standard_review }
  first_match: true
```

**Appears in**: Refund policy evaluation, analytics insights, notification channel selection, approval routing

**Relationship to `evaluate`**: `evaluate_rules` uses the same expression language as `evaluate` for its condition expressions. The difference is structural: `evaluate` produces named boolean/selection fields on the data, while `evaluate_rules` is a first-match (or all-match) rule engine that maps conditions to result objects. You can think of `evaluate_rules` as "evaluate N conditions, select the first/all that match, and return the associated result." If the evaluate primitive is the atom, evaluate_rules is a molecule.

**Implementation complexity**: Medium. Shares the expression evaluator with `evaluate` and `compute`. The rule-engine-specific concern is the match semantics (first-match vs. all-match) and result mapping.

### `assert`

**Type**: Guard (returns input unchanged or halts execution)
**Input**: JSON object (typically output of a preceding `evaluate` capability)
**Output**: Same object (pass-through) or error
**Config**: Named `conditions` using set-logic quantifiers (`all`, `any`, `none`) over boolean fields, with optional `on_failure` behavior

The `assert` capability consumes boolean fields — typically produced by a preceding `evaluate` step — and composes them using set-logic quantifiers. Conditions are **named** and **ordered by dependency precedent**, so later conditions can reference the truth of earlier named conditions. This enables building complex logical assertions incrementally through composition rather than requiring deeply nested boolean expressions.

**Simple assertion** (single precondition):

```yaml
- capability: evaluate
  config:
    expressions:
      payment_validated: "payment_status == 'completed'"
  input_mapping: { type: previous }

- capability: assert
  config:
    conditions:
      - name: payment_ready
        all: [payment_validated]
        error: "Payment must be validated before processing"
    on_failure: fail
  input_mapping: { type: previous }
```

**Compound assertion** (multiple levels with dependency precedent):

```yaml
- capability: evaluate
  config:
    expressions:
      payment_validated: "payment_status == 'completed'"
      fraud_passed: "fraud_score < 85"
      policy_checked: "policy_result != 'rejected'"
      manager_approved: "approval_type == 'manager'"
      auto_approved: "approval_type == 'auto'"
      blacklisted: "customer_flags contains 'blacklist'"
      sanctioned: "customer_flags contains 'sanctions'"
  input_mapping: { type: previous }

- capability: assert
  config:
    conditions:
      # Level 1: direct boolean references from evaluate output
      - name: prerequisites_met
        all: [payment_validated, fraud_passed, policy_checked]
        error: "Payment prerequisites not met: {failed}"

      - name: has_approval
        any: [manager_approved, auto_approved]
        error: "No approval path satisfied"

      - name: not_restricted
        none: [blacklisted, sanctioned]
        error: "Customer is restricted: {matched}"

      # Level 2: compound assertions referencing Level 1 names
      - name: can_proceed
        all: [prerequisites_met, has_approval, not_restricted]
        error: "Cannot proceed with refund"

    on_failure: fail
  input_mapping: { type: previous }
```

**Quantifier semantics**:
- `all`: Every listed field/condition must be true (logical AND)
- `any`: At least one listed field/condition must be true (logical OR)
- `none`: No listed field/condition may be true (logical NOR — equivalent to `all` over negations)

**Relationship to `evaluate`**: `assert` consumes `evaluate` output. Where `evaluate` produces boolean fields on the data, `assert` composes those booleans into execution gates. The separation allows the same evaluated booleans to be used by both `assert` (should we proceed?) and `compute` (what values depend on these conditions?).

**Relationship to `evaluate_rules`**: Both compose boolean logic, but serve different purposes. `assert` gates execution (proceed or fail), while `evaluate_rules` maps conditions to output objects (first-match routing). An `evaluate_rules` produces a result; an `assert` guards a boundary.

**Appears in**: Gateway refund (verify eligibility), record update (verify refund), ticket update (verify delegation), refund policy (verify request validated)

**Implementation complexity**: Low-medium. Requires sequential condition evaluation with name resolution, set-logic quantification, and error message interpolation.

---

## Action Capabilities: `persist`, `acquire`, `emit`

The grammar categories (Acquire, Transform, Validate, Persist, Emit) describe **abstract flow shapes** — what kinds of action compositions can cohere. The corresponding capabilities (`acquire`, `persist`, `emit`) are **concrete invocations** of those same action types, expressing the (action, resource, context) triple with a typed Rust envelope and JSON Schema-flexible configuration.

### `persist`

**Type**: Write (mutating, typically checkpointed)
**Input**: JSON object (data to persist)
**Output**: JSON object (success confirmation with result shape)
**Config**: `resource` (target), `data` (what to write), `constraints`, `validate_success`, `result_shape`

The `persist` capability subsumes what were previously `db_insert`, `db_update`, `create_ledger_entries`, `set_reconciliation`, and similar domain-specific write operations. The action is always "write data to a resource target" — domain specificity comes from configuration, not capability naming.

```yaml
# Previously: capability: db_insert + entity: orders
- capability: persist
  config:
    resource:
      type: database
      entity: orders
    data:
      order_ref: "$.order_ref"
      customer_email: "$.customer_email"
      items: "$.validated_items"
      total: "$.total"
    constraints:
      unique_key: order_ref
    validate_success:
      order_id: { type: string, required: true }
    result_shape:
      fields: [order_id, order_ref, created_at]
  checkpoint: true
```

```yaml
# Previously: capability: create_ledger_entries
- capability: persist
  config:
    resource:
      type: database
      entity: ledger_entries
    data:
      entries:
        - { type: debit, account: refunds_payable, amount: "$.refund_amount" }
        - { type: credit, account: accounts_receivable, amount: "$.refund_amount" }
    constraints:
      fiscal_period_format: "{year}-{month}"
      idempotency_key: "$.refund_id"
    validate_success:
      record_id: { type: string, required: true }
    result_shape:
      fields: [record_id, journal_id, fiscal_period, created_at]
  checkpoint: true
```

```yaml
# Previously: capability: set_reconciliation (was just setting a flag)
- capability: persist
  config:
    resource:
      type: database
      entity: reconciliation_status
    data:
      status: pending
      refund_id: "$.refund_id"
      journal_id: "$.journal_id"
    validate_success:
      status: { equals: pending }
    result_shape:
      fields: [reconciliation_id, status]
```

**Config surface** (typed Rust envelope, JSON Schema-flexible values):

| Field | Type | Purpose |
|-------|------|---------|
| `resource` | `{ type, entity, ... }` | Where to persist — database table, API endpoint, queue, etc. |
| `data` | `{ field: expression, ... }` | What to persist — selector-path expressions over composition data |
| `constraints` | `{ ... }` | Rules governing the write — idempotency, uniqueness, format rules |
| `validate_success` | `{ field: schema, ... }` | How to confirm success — expected shape of the write result |
| `result_shape` | `{ fields: [...] }` | What the next capability sees — projection of the write confirmation |

**Implementation complexity**: Medium. The `resource.type` discriminator selects a backend (database adapter, API client, queue producer). The Rust type provides the envelope structure; the JSON Schema within each field is runtime-flexible. Different resource types will have different valid configurations — a database persist needs `entity`, an API persist needs `endpoint`.

**Appears in**: create_order, update_inventory, create_user_account, update_payment_records, set_reconciliation, setup_billing, update_ticket_status

### `acquire`

**Type**: Read (non-mutating)
**Input**: JSON object (query parameters, keys)
**Output**: JSON object (acquired data in result shape)
**Config**: `resource` (source), `constraints`, `validate_success`, `result_shape`

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
    result_shape:
      fields: [data.sales_records]
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
    result_shape:
      fields: [monthly_rate, billing_cycle, features]
```

**Config surface**:

| Field | Type | Purpose |
|-------|------|---------|
| `resource` | `{ type, endpoint/table/key, ... }` | Where to read from — API, config store, database, cache |
| `constraints` | `{ timeout_ms, cache_ttl, required, ... }` | Rules governing the read — timeouts, caching, required vs optional |
| `validate_success` | `{ field: schema, ... }` | How to confirm the read succeeded |
| `result_shape` | `{ fields: [...] }` | What the next capability sees — projection of the acquired data |

**Appears in**: extract_sales_data, lookup_config (rate plans, billing), lookup_purchase, all data source fetch operations

### `emit`

**Type**: Domain event publication (side-effecting)
**Input**: JSON object (composition context data)
**Output**: JSON object (event publication confirmation)
**Config**: `event_name`, `event_version`, `delivery_mode`, `condition`, `payload` (selector-paths), optional `schema`

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
    payload:
      refund_id: "$.refund_id"
      amount_refunded: "$.amount_refunded"
      customer_email: "$.customer_email"
      estimated_arrival: "$.estimated_arrival"
      correlation_id: "$.correlation_id"
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
    payload:
      order_id: "$.order_id"
      order_ref: "$.order_ref"
      customer_email: "$.customer_email"
      total: "$.total"
      estimated_delivery: "$.estimated_delivery"
      items: "$.validated_items"
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
| `payload` | `{ field: expression }` | Selector-path expressions projecting data from composition context into event payload |
| `schema` | JSON Schema | Optional payload validation — same as `publishes_events.schema` in task templates |

**Relationship to Tasker's domain event system**: `emit` maps 1:1 onto the existing `DomainEvent` infrastructure. The `event_name` becomes `DomainEvent.event_name`, `payload` fields are assembled into `DomainEventPayload.payload`, `delivery_mode` selects the `EventDeliveryMode`, and `schema` provides the same JSON Schema validation that `publishes_events` declarations already support. The composition executor constructs and publishes the `DomainEvent` through the existing `DomainEventPublisher` → `EventRouter` pipeline.

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

~~Previously listed but now subsumed:~~
- ~~`compute_totals`~~ → `compute`
- ~~`deep_merge`~~ → `reshape`
- ~~`build_summary`~~ → `reshape`
- ~~`conditional_compute`~~ → `evaluate` + `compute` composition

---

## Vocabulary Architecture

### Grammar–Capability Typology

The grammar categories (Acquire, Transform, Validate, Persist, Emit) and the capability vocabulary share a **typology**: both describe action types, but at different levels of abstraction.

**Grammar** is the abstract structural layer. It describes:
- What flows can cohere (a Validate grammar naturally begins a composition, Emit naturally ends one)
- Where checkpoints belong (persist and emit are typically checkpointed; reshape and evaluate are not)
- Ordering constraints (nothing has been validated or acquired to emit at the top of a composition)

**Capabilities** are concrete invocations within a grammar composition. Each is an (action, resource, context) triple where:
- The **Rust types** describe the capability-config-shape (the envelope: `resource`, `constraints`, `validate_success`, `result_shape`)
- The **JSON Schema-flexible contents** within those typed fields are runtime-variable (what the fields in `data` should be, what `timeout_ms` value to use, etc.)

```
Grammar Layer (abstract flow structure):
  Acquire → Transform → Validate → Persist → Emit

Capability Layer (concrete action invocations):
  Core Data Operations (pure, no side effects):
  ├── validate        — boundary gate (schema, coercion, filtering, failure)
  ├── reshape         — selector-path projection/reorganization
  ├── compute         — arithmetic/aggregation derivation
  ├── evaluate        — boolean/selection field derivation
  ├── assert          — composable execution gate (set-logic quantifiers)
  └── evaluate_rules  — first-match/all-match rule engine

  Action Capabilities (side-effecting, typed envelope + JSON Schema config):
  ├── acquire         — read data from resource source
  ├── persist         — write data to resource target
  └── emit            — fire domain event (maps to Tasker's DomainEvent system)

Domain Handlers (outside grammar scope):
  ├── fraud_check, payment_gateway_charge, gateway_refund
  ├── inventory_reserve, classify_customer, generate_credentials
  └── ... any operation where (action, resource, context) cannot be deterministically expressed
```

### What This Means for `CapabilityExecutor`

The trait from `grammar-trait-boundary.md` serves the core capabilities:

```rust
pub trait CapabilityExecutor: Send + Sync {
    fn name(&self) -> &str;
    fn input_schema(&self) -> &serde_json::Value;
    fn output_schema(&self) -> &serde_json::Value;
    fn is_mutating(&self) -> bool;
    async fn execute(
        &self,
        input: serde_json::Value,
        config: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError>;
}
```

For core data operations (`reshape`, `compute`, `evaluate`, `validate`, `assert`, `evaluate_rules`), the `config` parameter is fully specified by the expression language and the capability's typed configuration. For action capabilities (`persist`, `acquire`, `emit`), the `config` parameter contains a typed envelope (`resource`, `constraints`, `validate_success`, `result_shape`) with JSON Schema-flexible contents — the Rust type enforces the envelope structure, while the values within are runtime-variable.

Operations that fall outside this model — opaque domain logic — remain as traditional domain handlers. The grammar system provides the composition framework for what it can express; domain handler code provides the implementation for what it cannot.

---

## Cross-Language Observations

### Handler Pattern → Capability Pattern Mapping

From examining all four language implementations:

| Handler DSL Pattern | Capability Equivalent |
|--------------------|----------------------|
| `@depends_on(cart=("validate_cart", CartType))` | `input_mapping: { type: task_context }` + composition resolves deps |
| `@inputs(EcommerceOrderInput)` | Capability `input_schema` (validated at registration) |
| `return svc.validate_cart_items(...)` | Capability `execute()` delegates to implementation |
| `PermanentError("...")` | `CapabilityError::Permanent { message }` |
| `RetryableError("...")` | `CapabilityError::Retryable { message }` |
| `context.get_input_or('key', default)` | `config` parameter with defaults |

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
  Domain Handler DSL → Service Function → External Calls

With grammars:
  Virtual Handler (Composition Spec) → Capability Executor → Service Function → External Calls
```

The service layer is unchanged. Capabilities wrap services the same way handlers do. The difference is that capabilities declare their contracts (schemas) and the composition system validates compatibility before execution.

---

## Composition Shape Patterns

From the grammar proposals, three composition shapes emerged repeatedly:

### Shape 1: Validate → Compute → Persist

```yaml
compose:
  - capability: validate           # Boundary gate: assert shape, coerce, filter
  - capability: compute            # Calculate results
  - capability: persist            # Write record to resource target
    checkpoint: true
```

**Appears in**: create_user_account, update_inventory, create_order, update_payment_records, update_ticket_status

### Shape 2: Reshape → Compute

```yaml
compose:
  - capability: reshape            # Project from source(s)
  - capability: compute            # Derive, aggregate, transform
```

**Appears in**: All extract handlers, all transform handlers, aggregate_metrics

### Shape 3: Reshape → Emit

```yaml
compose:
  - capability: reshape            # Gather and project data from dependencies
  - capability: emit               # Fire domain event with payload
```

**Appears in**: send_confirmation, notify_customer, send_welcome_sequence (all reframed as domain event publications)

### What Shapes Tell Us

These three shapes map to three grammar categories:
- **Persist**: validate → compute → persist (to resource target)
- **Transform**: reshape → compute (derive and aggregate)
- **Emit**: reshape → emit (domain event with projected payload)

Each grammar category has a characteristic composition shape. This isn't enforced — a Persist grammar could have any number of capabilities in any order — but the patterns suggest that grammar categories naturally gravitate toward specific shapes. The grammar layer provides structural insight: `emit` cannot appear at the top of a composition (nothing has been acquired or validated to emit), and `validate` naturally gravitates toward the boundary where untrusted data enters.

Note how the Emit shape is the simplest — `reshape` gathers the data from upstream dependencies, and `emit` fires the domain event with that data as the payload. Content construction (email templates, notification formatting) is NOT the grammar's concern — that belongs to downstream consumers of the domain event. Future tooling (template generators, MCP tools) could use these shapes as defaults when scaffolding compositions.

---

## Open Questions from the Case Studies

### 1. Shared Expression Language

The expression language is now the **single most critical design decision** — it serves all evaluability across the system. It's not owned by any one capability; `compute`, `evaluate`, `evaluate_rules`, `assert`, and workflow-level decision point routing all share it.

The language must support:
- **Arithmetic**: `+`, `-`, `*`, `/`, `^` with operator precedence
- **Aggregation functions**: `sum()`, `count()`, `avg()`, `min()`, `max()`
- **Comparisons**: `>`, `>=`, `<`, `<=`, `==`, `!=`, `in`
- **Boolean combinators**: `AND`, `OR`, `NOT`
- **Value selection**: `select(bool_field, if_true, if_false)` — references an already-evaluated boolean
- **Case mapping**: `case(when: expr, then: value, ...)` — multi-branch selection
- **Date functions**: `date_add()`, `now()`
- **Utility functions**: `clamp()`, `has()`, `generate_id()`
- **Selector-path access**: `items[*].field`, `$.field`, `groups[*].revenue`
- **Numeric casting**: `decimal(N)`, `integer`, `float`

**Key design constraint**: The language must clearly separate **evaluation** (producing booleans/selections) from **computation** (producing numeric/data values). The `select()` function is the bridge — it takes a boolean field (produced by `evaluate`) and returns one of two values (consumed by `compute`). This is different from `if()` which embeds the evaluation inline.

Options:
- **CEL (Common Expression Language)**: Google's expression language, designed for config evaluation. Has selector support, type safety, and is widely adopted. Strong candidate — natively supports the evaluation/computation distinction.
- **jq-like**: JSON-native expression language with built-in path semantics. Natural fit for JSON data but unfamiliar syntax.
- **Custom DSL**: Purpose-built for Tasker. Full control, maintenance burden. Could start minimal.
- **JSONata**: JSON query and transformation language. Powerful but complex.

The expression language choice is the highest-priority vocabulary design decision. It unifies evaluability across the entire system — from field-level booleans inside a handler to DAG-level routing across the workflow.

### 2. ~~`merge_dependencies` as Primitive vs. Capability~~ → Resolved by `reshape`

The introduction of `reshape` resolves this question. What was `merge_dependencies` (7 occurrences) is now just `reshape` with selector-paths that reference multiple upstream dependencies. No special primitive needed — `reshape` is a standard vocabulary capability that uses the selector-path infrastructure to project data from any source(s) into a new shape. The composition executor makes dependency outputs available as resolvable paths at composition entry.

### 3. Per-Record vs. Whole-Input in `compute`

The `compute` capability's `select` field already handles this: `select: "items[*]"` operates per-record (map), while `select: "$"` operates on the whole input (reduce). This is cleaner than a separate `mode` flag because the selector-path *is* the mode — if you select an array element pattern, you get per-record; if you select the root, you get whole-input. Multiple operations in one `compute` invocation can mix both modes.

### 4. Conditional Logic Within Compositions

With the `evaluate`/`compute` separation, conditional logic operates at three levels:

1. **Field-level** (evaluate): Produce boolean fields (`{ free_shipping: true }`)
2. **Value-level** (compute + select): Choose between values based on evaluated booleans (`select(free_shipping, 0, 9.99)`)
3. **Capability-level** (composition `when` clause): Skip an entire capability based on a condition

```yaml
compose:
  - capability: emit
    config:
      event_name: "billing.trial_ending"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
      payload:
        trial_end: "$.billing.trial_end"
        customer_id: "$.customer_id"
    when: "billing.trial_end != null"  # Skip this capability entirely if no trial
```

The `when` clause uses the same shared expression language as `evaluate` and `compute`. This consistency means one expression engine serves all three levels of conditional logic. The composition executor evaluates `when` clauses before invoking capabilities, using the same evaluator that powers `evaluate` capabilities and `compute`'s `select()` references.

### 5. `evaluate` and `compute` Composition Patterns

When a handler needs both boolean determinations and numeric derivations, how should evaluate and compute compose?

**Option A: Explicit sequencing** — evaluate as a separate capability step:
```yaml
compose:
  - capability: evaluate
    config:
      expressions:
        free_shipping: "subtotal >= 75.00"
        billing_required: "price > 0"
  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            shipping: "select(free_shipping, 0, 9.99)"
            billing_status: "select(billing_required, 'active', 'skipped')"
```

**Option B: Implicit evaluation** — compute references expressions that are auto-evaluated:
```yaml
compose:
  - capability: compute
    config:
      evaluable:
        free_shipping: "subtotal >= 75.00"
      operations:
        - select: "$"
          derive:
            shipping: "select(free_shipping, 0, 9.99)"
```

Option A keeps the capabilities fully separate (cleaner conceptually). Option B reduces composition verbosity when evaluate and compute are always paired. The recommendation leans toward **Option A** — explicit sequencing makes the data flow visible and keeps each capability's concern singular. When evaluate is a separate step, its output is available to *any* downstream capability, not just the next compute.

---

*This case study synthesizes findings from `workflow-patterns.md` and `advanced-patterns.md` into a vocabulary proposal. It should be read alongside `grammar-trait-boundary.md` for the trait design, `composition-validation.md` for contract chaining between capabilities, and `virtual-handler-dispatch.md` for how grammar-composed virtual handlers execute within the worker infrastructure.*
