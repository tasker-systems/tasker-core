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
