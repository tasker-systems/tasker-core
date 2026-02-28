# MCP Development

## Overview

`tasker-mcp` is an MCP (Model Context Protocol) server that exposes Tasker's developer tooling to LLM agents. It uses stdio transport via the `rmcp` crate and delegates all logic to `tasker-tooling`.

## Architecture

```
tasker-mcp (MCP server, stdio transport)
  └── tasker-tooling (shared dev tooling library)
       ├── codegen (types + handlers + tests for 4 languages)
       ├── template_parser (YAML → TaskTemplate)
       ├── template_validator (structural + cycle checks)
       ├── template_generator (spec → YAML)
       ├── schema_inspector (field-level analysis)
       ├── schema_comparator (producer/consumer compatibility)
       └── schema_diff (temporal diff between versions)
```

## 7 Tools

| Tool | Module | What It Does |
|------|--------|-------------|
| `template_generate` | `template_generator` | Structured spec → valid template YAML |
| `template_validate` | `template_validator` | Structural correctness, cycle detection |
| `template_inspect` | `schema_inspector` + DAG analysis | Execution order, root/leaf steps |
| `handler_generate` | `codegen::scaffold` / `codegen` | Types + handlers + tests (py/rb/ts/rs) |
| `schema_inspect` | `codegen::schema` | Field-level result_schema details |
| `schema_compare` | `schema_comparator` | Producer/consumer compatibility check |
| `schema_diff` | `schema_diff` | Temporal diff between template versions |

## Key Files

| File | Purpose |
|------|---------|
| `tasker-mcp/src/server.rs` | ServerHandler impl, tool methods, proc macro routing |
| `tasker-mcp/src/tools/params.rs` | Parameter and response structs (schemars + serde) |
| `tasker-mcp/src/lib.rs` | Library target for integration test imports |
| `tasker-mcp/src/main.rs` | Binary entry point (stdio transport) |
| `tasker-mcp/tests/mcp_protocol_test.rs` | Protocol-level integration tests (all 7 tools) |
| `.mcp.json.example` | Example client config (copy to `.mcp.json`) |

## Test Commands

```bash
# All tests (unit + integration)
cargo test --all-features -p tasker-mcp

# Unit tests only
cargo test --all-features -p tasker-mcp --lib

# Integration tests only (MCP protocol round-trip)
cargo test --all-features -p tasker-mcp --test mcp_protocol_test

# Clippy
cargo clippy --all-targets --all-features -p tasker-mcp
```

## Development Patterns

### Adding a New Tool

1. Add parameter struct to `tasker-mcp/src/tools/params.rs` (derive `Deserialize + JsonSchema`)
2. Add response struct if needed (derive `Serialize`)
3. Add tool method to `impl TaskerMcpServer` in `server.rs` with `#[tool(...)]` attribute
4. Add unit test in `server.rs` `mod tests`
5. Add protocol integration test in `tests/mcp_protocol_test.rs`

### Tool Parameter Schema

Parameter structs use `schemars::JsonSchema` for automatic MCP tool schema generation. Every field must have a `#[schemars(description = "...")]` attribute — this is what LLM agents see.

### Error Handling

All tools return `String`. Errors are returned as JSON: `{"error": "code", "message": "detail", "valid": false}`. The `error_json()` helper in `server.rs` produces this format.

### Scaffold Mode

`handler_generate` defaults to scaffold mode (`scaffold: true`), which generates handlers that import the generated type models. Non-scaffold mode generates independent files without import wiring.

## Test Fixtures

| Fixture | Steps | Shape |
|---------|-------|-------|
| `codegen_test_template.yaml` | 5 | Linear with fan-out |
| `content_publishing_template.yaml` | 7 | Double diamond |
