# tasker-mcp

MCP (Model Context Protocol) server exposing [Tasker](https://github.com/tasker-systems/tasker-core) developer tooling to LLM agents and AI-assisted development tools.

## Overview

`tasker-mcp` implements an MCP server using the `rmcp` crate that makes Tasker's code generation, template parsing, and schema inspection capabilities available over the Model Context Protocol. This enables LLM agents (such as Claude) to generate handler scaffolds, inspect task templates, and analyze schema contracts as part of AI-assisted workflow development.

## Status

This crate is currently a **scaffold** (TAS-304). The MCP server responds to the `initialize` handshake with Tasker server info. Tool implementations will be added in TAS-305.

## Usage

The MCP server uses stdio transport:

```bash
# Run directly
cargo run -p tasker-mcp

# Or install and run
cargo install --path tasker-mcp
tasker-mcp
```

Configure as an MCP server in your client (e.g., Claude Desktop or Claude Code):

```json
{
  "mcpServers": {
    "tasker": {
      "command": "tasker-mcp"
    }
  }
}
```

## Dependencies

- `tasker-tooling` — Shared developer tooling (codegen, template parsing, schema inspection)
- `rmcp` — MCP protocol implementation (server mode, stdio transport)
- `tokio` — Async runtime

## License

MIT License — see [LICENSE](LICENSE) for details.
