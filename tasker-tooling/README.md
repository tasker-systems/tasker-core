# tasker-tooling

Shared developer tooling library for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Provides code generation, template parsing, schema inspection, and a runtime template engine consumed by both `tasker-ctl` and `tasker-mcp`.

## Modules

| Module | Description |
|--------|-------------|
| `codegen` | Schema-driven code generation for Python, Ruby, TypeScript, and Rust |
| `template_parser` | Parse `TaskTemplate` definitions from YAML with structured errors |
| `schema_inspector` | Inspect `result_schema` contracts across template steps |
| `template_engine` | Tera runtime rendering with custom case-conversion filters |

## Code Generation

Generate typed handler stubs from task template `result_schema` definitions:

```rust,ignore
use tasker_tooling::codegen::{generate_handlers, generate_types, TargetLanguage};
use tasker_tooling::template_parser::parse_template;

let template = parse_template(Path::new("workflow.yaml"))?;
let types = generate_types(&template, TargetLanguage::Python)?;
let handlers = generate_handlers(&template, None, TargetLanguage::Python)?;
```

Supported languages: Python, Ruby, TypeScript, Rust.

## Schema Inspection

Summarize schema presence across a template's steps:

```rust,ignore
use tasker_tooling::schema_inspector::inspect;

let report = inspect(&template);
for step in &report.steps {
    println!("{}: has_schema={}, properties={:?}", step.name, step.has_result_schema, step.property_count);
}
```

## License

MIT License — see [LICENSE](LICENSE) for details.
