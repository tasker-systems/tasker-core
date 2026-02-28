# Tasker Core Guides

This directory contains practical how-to guides for working with Tasker Core.

## Getting Started

| Document | Description |
|----------|-------------|
| [Quick Start](./quick-start.md) | Get running in 5 minutes |
| [Use Cases and Patterns](./use-cases-and-patterns.md) | Practical workflow examples |

## Workflow Patterns

| Document | Description |
|----------|-------------|
| [Conditional Workflows](./conditional-workflows.md) | Runtime decision-making and dynamic steps |
| [Batch Processing](./batch-processing.md) | Parallel processing with cursor-based workers |
| [Handler Resolution](./handler-resolution.md) | How handlers are discovered and dispatched |
| [Retry Semantics](./retry-semantics.md) | Understanding max_attempts and retryable flags |
| [Identity Strategy](./identity-strategy.md) | Task deduplication with STRICT, CALLER_PROVIDED, ALWAYS_UNIQUE |
| [DLQ System](./dlq-system.md) | Dead letter queue investigation and resolution |

## Configuration and Connectivity

| Document | Description |
|----------|-------------|
| [Configuration Management](./configuration-management.md) | Server-side TOML architecture, CLI tools, runtime observability |
| [Client Profiles](./client-profiles.md) | Multi-profile connection management for tasker-ctl and tasker-mcp |
| [Caching](./caching.md) | Distributed caching, backend selection, circuit breaker protection |

## Security

| Document | Description |
|----------|-------------|
| [API Security](./api-security.md) | JWT and API key authentication with permission-based access control |
| [Auth Integration](./auth-integration.md) | External identity provider integration via JWKS endpoints |

## MCP (Model Context Protocol)

| Document | Description |
|----------|-------------|
| [MCP Setup](./mcp/setup.md) | Installing and configuring the tasker-mcp server |
| [MCP Workflow Exercise](./mcp/workflow-exercise.md) | Build a complete workflow using only MCP tools |
| [MCP Local Models](./mcp/local-models.md) | Using tasker-mcp with Ollama and local LLMs |

## When to Read These

- **Getting started**: Begin with Quick Start, then Use Cases and Patterns
- **Building workflows**: Conditional Workflows, Batch Processing, Handler Resolution
- **Handling errors**: Retry Semantics and DLQ System
- **Connecting to servers**: Client Profiles, then MCP Setup
- **Securing endpoints**: API Security, then Auth Integration
- **Deploying**: Configuration Management, Caching

## Related Documentation

- [Architecture](../architecture/) - The "what" - system structure
- [Principles](../principles/) - The "why" - design philosophy
- [Workers](../workers/) - Language-specific handler development
