# MCP Development

## Overview

`tasker-mcp` is an MCP (Model Context Protocol) server that exposes Tasker's developer tooling and profile management to LLM agents. It uses stdio transport via the `rmcp` crate. Tier 1 tools delegate to `tasker-sdk`; Tier 2+ tools use `tasker-client` for server connectivity via `ProfileManager`.

## Architecture

```
tasker-mcp (MCP server, stdio transport)
  ├── tasker-sdk (shared SDK library)
  │    ├── codegen (types + handlers + tests for 4 languages)
  │    ├── template_parser (YAML → TaskTemplate)
  │    ├── template_validator (structural + cycle checks)
  │    ├── template_generator (spec → YAML)
  │    ├── schema_inspector (field-level analysis)
  │    ├── schema_comparator (producer/consumer compatibility)
  │    └── schema_diff (temporal diff between versions)
  └── tasker-client (profile management + server connectivity)
       └── ProfileManager (multi-profile sessions, health probing)
```

### Modes

- **Offline** (`--offline` or `TaskerMcpServer::new()`): Tier 1 tools only, no profiles loaded
- **Connected** (default): Loads profiles from `tasker-client.toml`, probes health at startup

### CLI Flags

```bash
tasker-mcp                    # Connected mode, loads profiles
tasker-mcp --offline          # Offline mode, Tier 1 only
tasker-mcp --profile staging  # Set initial active profile
```

## 23 Tools

### Tier 1 — Offline Developer Tools (7)

| Tool | Module | What It Does |
|------|--------|-------------|
| `template_generate` | `template_generator` | Structured spec → valid template YAML |
| `template_validate` | `template_validator` | Structural correctness, cycle detection |
| `template_inspect` | `schema_inspector` + DAG analysis | Execution order, root/leaf steps |
| `handler_generate` | `codegen::scaffold` / `codegen` | Types + handlers + tests (py/rb/ts/rs) |
| `schema_inspect` | `codegen::schema` | Field-level result_schema details |
| `schema_compare` | `schema_comparator` | Producer/consumer compatibility check |
| `schema_diff` | `schema_diff` | Temporal diff between template versions |

### Profile Management (1)

| Tool | Module | What It Does |
|------|--------|-------------|
| `connection_status` | `ProfileManager` | List profiles with health, refresh probes |

### Tier 2 — Connected Read-Only Tools (15)

All accept optional `profile` parameter to target a specific environment.

**Task & Step Inspection**

| Tool | What It Does |
|------|-------------|
| `task_list` | List tasks with namespace/status filtering |
| `task_inspect` | Task details + step breakdown |
| `step_inspect` | Step details including results, timing, retry info |
| `step_audit` | SOC2-compliant audit trail for a step |

**DLQ Investigation**

| Tool | What It Does |
|------|-------------|
| `dlq_list` | List DLQ entries with resolution status filtering |
| `dlq_inspect` | Detailed DLQ entry with error context and snapshots |
| `dlq_stats` | DLQ statistics aggregated by reason code |
| `dlq_queue` | Prioritized investigation queue ranked by severity |
| `staleness_check` | Task staleness monitoring with health annotations |

**Analytics**

| Tool | What It Does |
|------|-------------|
| `analytics_performance` | System-wide performance metrics |
| `analytics_bottlenecks` | Slow steps and bottleneck identification |

**System**

| Tool | What It Does |
|------|-------------|
| `system_health` | Detailed component health (DB, queues, circuit breakers) |
| `system_config` | Orchestration config (secrets redacted) |

**Remote Templates**

| Tool | What It Does |
|------|-------------|
| `template_list_remote` | List templates registered on the server |
| `template_inspect_remote` | Template details from the server |

## Key Files

| File | Purpose |
|------|---------|
| `tasker-mcp/src/server.rs` | ServerHandler impl, tool methods, proc macro routing |
| `tasker-mcp/src/tools/params.rs` | Parameter and response structs (schemars + serde) |
| `tasker-mcp/src/lib.rs` | Library target for integration test imports |
| `tasker-mcp/src/main.rs` | Binary entry point (clap CLI + stdio transport) |
| `tasker-mcp/tests/mcp_protocol_test.rs` | Protocol-level integration tests (all 23 tools) |
| `tests/mcp_tests.rs` | Connected integration tests entry point (requires running services) |
| `tests/mcp/harness.rs` | `McpTestHarness` — duplex MCP + IntegrationTestManager for seeding |
| `tests/mcp/` | 4 persona test files: task_inspection, system_monitoring, dlq_investigation, analytics |
| `tasker-client/src/profile_manager.rs` | ProfileManager, health types, multi-profile sessions |
| `.mcp.json.example` | Example client config (copy to `.mcp.json`) |

## Test Commands

```bash
# All tests (unit + integration)
cargo test --all-features -p tasker-mcp

# Unit tests only
cargo test --all-features -p tasker-mcp --lib

# Integration tests only (MCP protocol round-trip)
cargo test --all-features -p tasker-mcp --test mcp_protocol_test

# Connected integration tests (requires running services)
cargo test --features test-services --test mcp_tests -- --nocapture
# Or via cargo-make:
cd tasker-mcp && cargo make test-mcp-connected

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

### Server Struct (Interior Mutability)

`TaskerMcpServer` holds `Arc<tokio::sync::RwLock<ProfileManager>>` because rmcp's `ServerHandler` requires `Clone`. Use `self.profile_manager.read().await` for reads and `self.profile_manager.write().await` for mutations (e.g., health probing). Profile selection is stateless — each connected tool accepts an optional `profile` parameter.

### Scaffold Mode

`handler_generate` defaults to scaffold mode (`scaffold: true`), which generates handlers that import the generated type models. Non-scaffold mode generates independent files without import wiring.

## Persona Skills

Operational skill files for context-aware tool recommendations live in `docs/skills/mcp/`:

| Skill | Persona | Tool Sequence |
|-------|---------|--------------|
| `task-debugging.md` | Software Engineer | connection_status → task_list → task_inspect → step_inspect → step_audit |
| `system-monitoring.md` | SRE / Platform | connection_status → system_health → system_config → staleness_check → analytics |
| `dlq-triage.md` | Technical Ops | dlq_stats → dlq_queue → dlq_list → dlq_inspect → task/step cross-ref |
| `performance-analysis.md` | Analytics | analytics_performance → analytics_bottlenecks → task_list with filters |

## Test Fixtures

| Fixture | Steps | Shape |
|---------|-------|-------|
| `codegen_test_template.yaml` | 5 | Linear with fan-out |
| `content_publishing_template.yaml` | 7 | Double diamond |
