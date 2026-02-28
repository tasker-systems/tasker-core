# Using tasker-mcp with Local Models

This guide covers using tasker-mcp with locally-hosted LLMs via Ollama and mcphost.

## Setup

### Prerequisites

- [Ollama](https://ollama.ai) installed and running
- [mcphost](https://github.com/mark3labs/mcphost) — Go CLI that bridges Ollama to MCP servers

```bash
# Install Ollama (macOS)
brew install ollama

# Pull a recommended model
ollama pull qwen2.5-coder:14b

# Install mcphost
go install github.com/mark3labs/mcphost@latest
```

### Configuration

Create `mcphost.json` in your project directory:

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

### Run

```bash
mcphost --config mcphost.json --model ollama:qwen2.5-coder:14b
```

## Recommended Models

| Model | Size | VRAM | Tool-Calling Quality |
|-------|------|------|---------------------|
| `qwen2.5-coder:32b` | ~20GB | ~24GB | Best — reliable tool selection and parameter construction |
| `qwen2.5-coder:14b` | ~9GB | ~12GB | Good — handles most tools, may need retries for complex YAML |
| `qwen2.5-coder:7b` | ~4.5GB | ~6GB | Fair — struggles with multi-line YAML-in-JSON parameters |

Larger models handle the YAML-as-string parameter format more reliably. If you see malformed tool calls, try a larger model before adjusting prompts.

## Known Limitations

### YAML-in-JSON parameter passing

The biggest challenge for local models is constructing multi-line YAML as a JSON string parameter. The `template_yaml` parameter requires raw YAML text embedded in a JSON string with escaped newlines.

**Workaround**: Use `template_generate` to create YAML from structured parameters (name, namespace, steps array) instead of asking the model to write raw YAML. The structured input is more reliable for smaller models.

### Tool selection accuracy

Smaller models may:
- Choose `template_inspect` when `template_validate` is more appropriate
- Forget to pass required parameters like `template_yaml`
- Confuse `schema_inspect` and `schema_compare`

The server's `instructions` field provides a suggested workflow order. Models that respect the `instructions` field perform better.

### Response interpretation

Local models may not always correctly interpret the JSON response from tools. If the model misreads a result, rephrasing your question or asking it to read specific fields helps.

## Prompt Patterns That Help

1. **Be specific about which tool to use**: "Use the `template_validate` tool to check this template" works better than "validate my template"
2. **Break complex tasks into steps**: Instead of "create and validate a template", do "first generate a template" then "now validate it"
3. **Reference the workflow**: "Following the workflow: template_generate → template_validate → handler_generate"
4. **Provide context for parameters**: "Pass the YAML content from the previous response as the `template_yaml` parameter"
