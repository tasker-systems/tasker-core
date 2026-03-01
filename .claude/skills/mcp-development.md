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

## 29 Tools

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

### Tier 3 — Write Tools (6)

All accept optional `profile` parameter to target a specific environment. Writes use preview → confirm workflow.

**Task Management**

| Tool | What It Does |
|------|-------------|
| `task_submit` | Submit a task for execution with confirmation |
| `task_cancel` | Cancel a task and all pending steps with confirmation |

**Step Resolution**

| Tool | What It Does |
|------|-------------|
| `step_retry` | Reset a failed step for retry with confirmation |
| `step_resolve` | Mark a step as manually resolved with confirmation |
| `step_complete` | Manually complete a step with result data and confirmation |

**DLQ Management**

| Tool | What It Does |
|------|-------------|
| `dlq_update` | Update DLQ entry investigation status with confirmation |

## Key Files

| File | Purpose |
|------|---------|
| `tasker-mcp/src/server.rs` | ServerHandler impl, thin tool routing stubs, client resolution |
| `tasker-mcp/src/tools/mod.rs` | Module declarations, re-exports `error_json` and `params::*` |
| `tasker-mcp/src/tools/developer.rs` | Tier 1 offline tool logic (7 pure functions + unit tests) |
| `tasker-mcp/src/tools/connected.rs` | Tier 2 read-only tool logic (15 async functions) |
| `tasker-mcp/src/tools/write.rs` | Tier 3 write tool logic (6 async functions, confirmation pattern) |
| `tasker-mcp/src/tools/helpers.rs` | `error_json()`, `topological_sort()` |
| `tasker-mcp/src/tools/params.rs` | Parameter and response structs (schemars + serde) |
| `tasker-mcp/src/lib.rs` | Library target for integration test imports |
| `tasker-mcp/src/main.rs` | Binary entry point (clap CLI + stdio transport) |
| `tasker-mcp/tests/mcp_protocol_test.rs` | Protocol-level integration tests (all 29 tools) |
| `tests/mcp_tests.rs` | Connected integration tests entry point (requires running services) |
| `tests/mcp/harness.rs` | `McpTestHarness` — duplex MCP + IntegrationTestManager for seeding |
| `tests/mcp/` | 5 persona test files: task_inspection, system_monitoring, dlq_investigation, analytics, write_tools |
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
3. Add business logic function in the appropriate tier module:
   - `tools/developer.rs` — Tier 1 offline (pure `fn`, no client)
   - `tools/connected.rs` — Tier 2 read-only (`async fn`, takes `&UnifiedOrchestrationClient`)
   - `tools/write.rs` — Tier 3 write (`async fn`, takes client + profile_name, uses `ConfirmationPhase`)
4. Add thin routing stub to `server.rs` `#[tool_router]` impl with `#[tool(...)]` attribute
5. Add unit test in the tier module (developer.rs for pure functions)
6. Add protocol integration test in `tests/mcp_protocol_test.rs`
7. For connected tools: add integration test in `tests/mcp/` (requires running services)

### Tool Parameter Schema

Parameter structs use `schemars::JsonSchema` for automatic MCP tool schema generation. Every field must have a `#[schemars(description = "...")]` attribute — this is what LLM agents see.

### Error Handling

All tools return `String`. Errors are returned as JSON: `{"error": "code", "message": "detail", "valid": false}`. The `error_json()` helper in `tools/helpers.rs` produces this format.

Write tools use `handle_api_error()` from `tasker-sdk::operational::confirmation` which detects HTTP 403 responses and returns structured `permission_denied` errors with tool name, required permission, and actionable hints.

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
