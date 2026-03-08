# tasker-sdk

Shared SDK library for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Provides code generation, template parsing, schema inspection, operational tooling, and a runtime template engine consumed by `tasker-ctl`, `tasker-mcp`, and future integrations.

## Modules

### Developer Tooling

| Module | Description |
|--------|-------------|
| `codegen` | Schema-driven code generation for Python, Ruby, TypeScript, and Rust |
| `template_parser` | Parse `TaskTemplate` definitions from YAML with structured errors |
| `template_validator` | Structural validation, cycle detection, best-practice checks |
| `template_generator` | Generate template YAML from structured specs |
| `schema_inspector` | Inspect `result_schema` contracts across template steps |
| `schema_comparator` | Compare producer/consumer schema compatibility |
| `schema_diff` | Detect field-level changes between template versions |
| `template_engine` | Tera runtime rendering with custom case-conversion filters |

### Operational Tooling

| Module | Description |
|--------|-------------|
| `operational::client_factory` | Transport-agnostic client construction from `ClientConfig` |
| `operational::enums` | DLQ resolution status and task status parsing |
| `operational::responses` | Shared response DTOs (`TaskSummary`, `StepSummary`, `DlqSummary`, etc.) |

## Code Generation

Generate typed handler stubs from task template `result_schema` definitions:

```rust,ignore
use tasker_sdk::codegen::{generate_handlers, generate_types, TargetLanguage};
use tasker_sdk::template_parser::parse_template;

let template = parse_template(Path::new("workflow.yaml"))?;
let types = generate_types(&template, TargetLanguage::Python, None)?;
let handlers = generate_handlers(&template, TargetLanguage::Python, None)?;
```

Supported languages: Python, Ruby, TypeScript, Rust.

## Operational Client Factory

Build connected clients for MCP tools or CLI commands:

```rust,ignore
use tasker_sdk::operational::client_factory::{build_orchestration_client, ClientConfig};

let config: ClientConfig = /* from ProfileManager */;
let client = build_orchestration_client(&config).await?;
```

## License

MIT License — see [LICENSE](LICENSE) for details.
