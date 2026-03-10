# tasker-grammar

Action grammar engine for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Enables "virtual handler" workflow steps whose behavior is defined declaratively as a composition of typed capabilities, using jq expressions (via [jaq-core](https://github.com/01mf02/jaq)) as the unified expression language.

## Overview

`tasker-grammar` is a standalone, pure data-transformation crate with no dependencies on `tasker-worker`, `tasker-orchestration`, or any database/messaging infrastructure. All operations — expression evaluation, capability execution, composition validation — operate on `serde_json::Value` inputs and produce `Value` outputs, testable with `cargo test` against pure data.

The crate replaces imperative handler code with declarative compositions authored in TaskTemplate YAML. Instead of writing a handler in Rust/Python/Ruby/TypeScript for every workflow step, template authors describe behavior as a sequence of typed capabilities with jq filters for data transformation.

## Architecture

```
TaskTemplate YAML
       │
       ▼
┌─────────────────────┐
│  ExpressionEngine   │  ◄── jq filter compilation & sandboxed evaluation
│  (jaq-core)         │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│  Capability          │  ◄── transform, validate, assert, persist, acquire, emit
│  Executors           │      (each uses ExpressionEngine for data shaping)
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│  CompositionExecutor │  ◄── chains capabilities, threads context envelope
└─────────────────────┘
```

## Capabilities

The grammar defines six typed capabilities that compose into workflow step behavior:

| Capability | Purpose | Expression Role |
|------------|---------|-----------------|
| `transform` | Pure data transformation | jq filter reshapes/computes output |
| `validate` | JSON Schema trust boundary gate | Schema validation, no jq |
| `assert` | Boolean execution gate | jq filter must return `true` |
| `persist` | Write to external system | jq filter shapes the data envelope |
| `acquire` | Read from external system | jq filter shapes the result |
| `emit` | Fire domain events | jq filter constructs the event payload |

## Expression Engine

The `ExpressionEngine` wraps jaq-core with sandboxing for safe evaluation of jq filters:

```rust,ignore
use tasker_grammar::{ExpressionEngine, ExpressionEngineConfig};
use serde_json::json;

let engine = ExpressionEngine::with_defaults();

// Evaluate a jq filter against JSON input
let input = json!({"items": [{"price": 10, "qty": 2}, {"price": 5, "qty": 3}]});
let result = engine.evaluate(
    "[.items[] | .price * .qty] | add",
    &input,
)?;
assert_eq!(result, json!(35));

// Validate filter syntax without evaluating
engine.validate_syntax(".foo | .bar")?;

// Multi-output evaluation
let results = engine.evaluate_multi(".items[].price", &input)?;
assert_eq!(results, vec![json!(10), json!(5)]);
```

### Composition Context Envelope

Filters operate on a standard context envelope that threads data between capabilities:

```json
{
  "context": { /* task input data */ },
  "deps":    { /* dependency step results */ },
  "prev":    { /* previous capability output */ },
  "step":    { /* step metadata */ }
}
```

### Sandboxing

| Concern | Mechanism | Default |
|---------|-----------|---------|
| Execution timeout | Wall-clock bound per evaluation | 100ms |
| Output size limit | Serialized JSON byte cap | 1 MiB |
| No I/O | jaq-core has no file/network access | Safe by construction |
| Error propagation | Structured `ExpressionError` variants | Always |

### Supported Expression Patterns

| Pattern | Example |
|---------|---------|
| Path traversal | `.items[].price`, `.customer.address.city` |
| Field projection | `{total: .subtotal + .tax, items: .line_items}` |
| Arithmetic | `.items \| map(.price * .quantity) \| add` |
| Boolean expressions | `.amount > 1000 and .status == "pending"` |
| String construction | `"Order \(.order_id) confirmed"` |
| Conditional | `if .tier == "gold" then "priority" else "standard" end` |
| Collection ops | `group_by(.dept)`, `sort`, `unique`, `select(. > 3)` |

## Configuration

```rust,ignore
use std::time::Duration;
use tasker_grammar::ExpressionEngineConfig;

let config = ExpressionEngineConfig {
    timeout: Duration::from_millis(200),    // per-filter evaluation limit
    max_output_bytes: 2_097_152,            // 2 MiB output cap
};
```

## Workflow Examples

The crate ships with three end-to-end workflow compositions that demonstrate real-world usage patterns. Each is available as a programmatic fixture (via `tasker_grammar::fixtures`) and as a YAML reference in `tests/fixtures/workflows/`. Integration tests in `tests/workflow_integration.rs` exercise every scenario listed below.

### E-commerce Order Processing

**Pipeline:** validate → transform (line items) → transform (totals) → transform (routing) → persist → emit

Processes a shopping cart through validation, computes line-item totals, calculates subtotal/tax/shipping, applies business routing rules (priority, warehouse, fraud review), persists the confirmed order, and emits an `order.confirmed` event.

**Capabilities exercised:** validate, transform (×3), persist, emit
**Checkpoints:** persist (index 4), emit (index 5)

```rust,ignore
use tasker_grammar::fixtures::{self, WorkflowFixture};

let WorkflowFixture { spec, input, acquire_fixtures } =
    fixtures::ecommerce_order_processing();

assert_eq!(spec.invocations.len(), 6);
assert_eq!(spec.invocations[0].capability, "validate");
assert_eq!(spec.invocations[4].capability, "persist");
assert!(spec.invocations[4].checkpoint); // persist is a checkpoint
```

### Payment Reconciliation

**Pipeline:** acquire (external txns) → validate (schema) → transform (matching) → transform (discrepancies) → assert (balance) → persist (report)

Acquires settled transactions from a payment gateway, validates the data schema, matches external transactions against internal records by reference, computes per-transaction variance, asserts that total variance and unmatched count are within configurable thresholds, and persists the reconciliation report.

**Capabilities exercised:** acquire, validate, transform (×2), assert, persist
**Checkpoints:** persist (index 5)

```rust,ignore
let WorkflowFixture { spec, input, acquire_fixtures } =
    fixtures::payment_reconciliation();

// Fixture data includes 4 external transactions, 3 internal records
// One external transaction has no internal match → unmatched_count = 1
// One matched pair has a $0.50 variance
assert_eq!(acquire_fixtures["transactions"].len(), 4);
```

### Customer Onboarding

**Pipeline:** acquire (CRM profile) → validate (completeness) → transform (tier classification) → transform (reshape/enrich) → persist (upsert) → emit (welcome event)

Acquires a customer profile from CRM, validates required fields (id, email, name), classifies the customer into a loyalty tier (bronze/silver/gold/platinum) based on purchase history, reshapes the profile with tier benefits, persists the enriched record as an upsert, and emits a `customer.onboarded` event.

**Capabilities exercised:** acquire, validate, transform (×2), persist, emit
**Checkpoints:** persist (index 4), emit (index 5)

```rust,ignore
let WorkflowFixture { spec, input, acquire_fixtures } =
    fixtures::customer_onboarding();

// Tier classification: $7500 total + 25 purchases → "gold" tier
// Gold benefits: 15% discount, free shipping, 2× loyalty multiplier
```

### Test Coverage

The integration test suite (`tests/workflow_integration.rs`) covers 30 scenarios across these workflows:

| Category | Tests | What's verified |
|----------|-------|-----------------|
| Execution correctness | 3 | Full pipeline produces expected outputs |
| Intermediate outputs | 3 | Each capability stage produces correct data |
| Cross-step references | 3 | `.prev` / `.context` threading works across capabilities |
| Checkpoint creation | 3 | Checkpoints capture correct index and accumulated state |
| Checkpoint resume | 3 | `resume()` from checkpoint skips completed work |
| Validation passes | 3 | `CompositionValidator` accepts all three specs |
| Negative cases | 4 | Validation failure, assertion failure, empty items, missing fields |
| Bulk operations | 3 | All fixtures load, execute, and validate together |
| Executor errors | 5 | Invalid filters, missing entities, unknown capabilities |

Run with:

```bash
cargo test --package tasker-grammar --test workflow_integration
```

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| *(none yet)* | All functionality is unconditionally included | — |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `jaq-core` | jq filter compilation and evaluation engine |
| `jaq-std` | jq standard library functions (`map`, `select`, `group_by`, etc.) |
| `jaq-json` | JSON value type with `serde_json::Value` conversion |
| `serde_json` | JSON value representation |
| `thiserror` | Structured error types |

## Documentation

| Topic | Document |
|-------|----------|
| Foundational model | `docs/action-grammar/actions-traits-and-capabilities.md` |
| Revised capability model | `docs/action-grammar/transform-revised-grammar.md` |
| Trait boundaries | `docs/action-grammar/grammar-trait-boundary.md` |
| Implementation phases | `docs/action-grammar/implementation-phases.md` |
| Composition validation | `docs/action-grammar/composition-validation.md` |
| Worker integration | `docs/action-grammar/virtual-handler-dispatch.md` |

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md).
