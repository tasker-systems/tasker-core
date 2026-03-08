# @tasker-systems/tasker

TypeScript worker for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Uses napi-rs native addons for high-performance FFI to the shared Rust worker infrastructure. Supports Bun (primary) and Node.js runtimes.

## Installation

```bash
# Bun (recommended)
bun add @tasker-systems/tasker

# Node.js
npm install @tasker-systems/tasker
```

## Quick Start

```typescript
import { WorkerServer, StepHandler, type StepContext, type StepHandlerResult } from "@tasker-systems/tasker";

// Define a step handler
class ProcessPaymentHandler extends StepHandler {
  static handlerName = "process_payment";
  static handlerVersion = "1.0.0";

  async call(context: StepContext): Promise<StepHandlerResult> {
    const amount = context.getInput<number>("amount");
    // ... business logic ...
    return this.success({ amount, status: "processed" });
  }
}

// Create and start the worker server
const server = new WorkerServer();
await server.start({ namespace: "default" });

// Register handlers
const handlerSystem = server.getHandlerSystem();
handlerSystem.register(ProcessPaymentHandler.handlerName, ProcessPaymentHandler);

// Server is now processing tasks — shut down gracefully on exit
process.on("SIGINT", () => server.shutdown());
```

## Handler Types

| Type | Use Case |
|------|----------|
| `StepHandler` | General-purpose step execution |
| `ApiHandler` | HTTP API integration with automatic error classification |
| `DecisionHandler` | Dynamic workflow routing |
| `BatchableStepHandler` | Large dataset processing in chunks |

## Development

### Prerequisites

- Bun 1.0+ (recommended) or Node.js 18+
- Rust 1.70+ (for building the napi-rs native addon)

### Build

```bash
bun install              # Install dependencies
bun run build:napi       # Build napi-rs native addon (debug)
bun run build            # Build TypeScript

bun test                 # Run tests
bun run typecheck        # Type checking
bun run check            # Lint (Biome)
```

## Documentation

- [TypeScript Worker Guide](../../docs/workers/typescript.md) — full API reference, handler patterns, event system, configuration
- [Example App (Bun + Hono)](https://github.com/tasker-systems/tasker-contrib/tree/main/examples/bun-app) — production-style example with multiple handler types

## License

MIT
