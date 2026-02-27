# tasker-mcp

MCP (Model Context Protocol) server exposing [Tasker](https://github.com/tasker-systems/tasker-core) developer tooling to LLM agents and AI-assisted development tools.

## Installation

```bash
# From crates.io
cargo install tasker-mcp

# From source
cargo install --path tasker-mcp

# Docker
docker run --rm -i ghcr.io/tasker-systems/tasker-mcp:latest
```

Pre-built binaries for Linux and macOS are available on the [GitHub Releases](https://github.com/tasker-systems/tasker-core/releases) page (tagged `tasker-mcp-v*`).

## Quick Start

Copy `.mcp.json.example` to `.mcp.json` (gitignored) to register the server with Claude Code:

```bash
cp .mcp.json.example .mcp.json
```

Or configure manually in your MCP client:

```json
{
  "mcpServers": {
    "tasker": {
      "command": "tasker-mcp",
      "env": { "RUST_LOG": "tasker_mcp=info" }
    }
  }
}
```

Verify the server starts: `tasker-mcp` will listen on stdio and log to stderr.

## Tools

| Tool | Description | Key Parameters |
|------|-------------|----------------|
| `template_generate` | Generate task template YAML from a structured spec | `name`, `namespace`, `steps[]` with `outputs[]` |
| `template_validate` | Validate template for structural correctness and cycles | `template_yaml` |
| `template_inspect` | Inspect DAG structure, execution order, root/leaf steps | `template_yaml` |
| `handler_generate` | Generate typed handlers, models, and tests | `template_yaml`, `language`, `scaffold` |
| `schema_inspect` | Inspect result_schema field details and consumer relationships | `template_yaml`, `step_filter` |
| `schema_compare` | Compare producer/consumer schema compatibility | `template_yaml`, `producer_step`, `consumer_step` |

### Canonical Workflow

```
template_generate → template_validate → handler_generate
```

1. **Generate**: Describe your workflow steps and output fields → get valid template YAML
2. **Validate**: Check structural correctness, dependency cycles, best-practice warnings
3. **Generate handlers**: Produce typed models + handler scaffolds + test files for your language

When debugging schema contracts between steps:

```
template_inspect → schema_inspect → schema_compare
```

### Supported Languages

`handler_generate` supports: **python**, **ruby**, **typescript**, **rust**

By default, scaffold mode is enabled — handlers import generated type models and use typed return values.

## Architecture

- **Transport**: stdio (standard MCP transport for CLI tools)
- **Protocol**: MCP 2025-03-26 via [`rmcp`](https://crates.io/crates/rmcp)
- **Tooling**: All tools delegate to [`tasker-tooling`](../tasker-tooling/) for codegen, validation, and schema analysis
- **Runtime**: Tokio async with tracing to stderr

## Dependencies

- `tasker-tooling` — Shared developer tooling (codegen, template parsing, schema inspection)
- `tasker-shared` — Core types (`TaskTemplate`, `StepDefinition`)
- `rmcp` — MCP protocol implementation (server mode, stdio transport)
- `tokio` — Async runtime

## License

MIT License — see [LICENSE](LICENSE) for details.
