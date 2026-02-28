# MCP Server Setup Guide

This guide covers installing and configuring `tasker-mcp`, the Model Context Protocol server for Tasker developer tooling.

## Installation

### From Source (Development)

```bash
# Build
cargo build -p tasker-mcp --all-features

# Install to ~/.cargo/bin
cargo install --path tasker-mcp
```

### From crates.io

```bash
cargo install tasker-mcp
```

### Docker

```bash
docker run --rm -i ghcr.io/tasker-systems/tasker-mcp:latest
```

## Client Configuration

### Claude Code

Copy the example config at the repo root:

```bash
cp .mcp.json.example .mcp.json
```

Or create `.mcp.json` in your project directory:

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

For user-scope (all projects), place in `~/.claude/.mcp.json`.

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

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

### Cursor / VS Code / Windsurf

These editors support MCP via their settings. Add the server configuration in the MCP settings section, pointing to the `tasker-mcp` binary.

### mcphost (Ollama / Local Models)

Create `mcphost.json`:

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

Run with a local model:

```bash
mcphost --config mcphost.json --model ollama:qwen2.5-coder:14b
```

## Available Tools

| Tool | Description |
|------|-------------|
| `template_generate` | Generate task template YAML from a structured spec |
| `template_validate` | Validate template for structural correctness and cycles |
| `template_inspect` | Inspect DAG structure, execution order, root/leaf steps |
| `handler_generate` | Generate typed handlers, models, and tests |
| `schema_inspect` | Inspect result_schema field details and consumer relationships |
| `schema_compare` | Compare producer/consumer schema compatibility |

## Troubleshooting

### Server doesn't start

The MCP server uses stdio transport — it reads from stdin and writes to stdout, with logs on stderr.

```bash
# Test manually (should start without error, then wait for input)
tasker-mcp
```

### Enable debug logging

```bash
RUST_LOG=tasker_mcp=debug tasker-mcp
```

### Tools not appearing

1. Verify the binary is on your PATH: `which tasker-mcp`
2. Check the `.mcp.json` file is in the correct location
3. Restart your MCP client after configuration changes
4. Check stderr output for initialization errors

### Timeout errors

MCP tool calls are synchronous — all 6 tools run locally against in-memory data (no network calls). If you see timeouts, the issue is likely transport-level. Check that the stdio pipe is connected properly.

### Common parameter errors

- **YAML parse errors**: The `template_yaml` parameter expects raw YAML text, not a file path. When passing YAML in JSON, ensure proper escaping of newlines.
- **Invalid language**: `handler_generate` accepts: `python`, `ruby`, `typescript`, `rust`.
- **Step not found**: `schema_compare` requires both `producer_step` and `consumer_step` to exist in the template.
