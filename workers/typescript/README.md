# @tasker-systems/tasker

TypeScript worker for the Tasker workflow orchestration system. Supports Bun (native FFI), Node.js (via koffi), and Deno runtimes.

## Status

Production ready. TypeScript worker bindings provide full step handler execution via FFI to the shared Rust `tasker-worker` infrastructure.

## Installation

```bash
# Bun (recommended - native FFI support)
bun add @tasker-systems/tasker

# Node.js (requires koffi for FFI)
npm install @tasker-systems/tasker koffi
```

## Quick Start

```typescript
import { TaskerWorker } from "@tasker-systems/tasker";

const worker = new TaskerWorker({
  workerName: "my-worker",
  namespaces: ["default"],
});

// Register a step handler
worker.registerHandler("process_payment", async (step) => {
  const result = await processPayment(step.context);
  return { status: "complete", data: result };
});

// Start the worker
await worker.start();
```

## Development

### Prerequisites

- Bun 1.0+ (recommended) or Node.js 18+
- Rust 1.70+ (for building the FFI library)

### Setup

```bash
# Install dependencies
bun install

# Build TypeScript
bun run build

# Run tests
bun test

# Type checking
bun run typecheck

# Linting
bun run check
```

### Building the FFI Library

```bash
# Build the Rust FFI shared library
cargo build --release -p tasker-worker-ts

# The library will be at target/release/libtasker_worker_ts.{dylib,so,dll}
```

## Project Structure

```
workers/typescript/
├── src/                  # TypeScript source
│   ├── bootstrap/        # Worker initialization
│   ├── events/           # Event system integration
│   ├── ffi/              # FFI bindings to Rust
│   ├── handler/          # Step handler base classes
│   ├── logging/          # Structured logging (pino)
│   ├── registry/         # Handler registry
│   ├── server/           # HTTP/gRPC server
│   ├── subscriber/       # Queue subscriber
│   ├── types/            # Type definitions
│   └── index.ts          # Package entry point
├── src-rust/             # Rust FFI source
│   └── lib.rs            # Neon/FFI module
├── tests/                # Test suite
├── Cargo.toml            # Rust crate configuration
├── package.json          # npm package configuration
├── tsconfig.json         # TypeScript configuration
└── biome.json            # Linting configuration
```

## Technology Stack

- **FFI Layer**: Bun native FFI / koffi (Node.js)
- **Build Tool**: tsup
- **Runtime**: Bun, Node.js 18+, or Deno
- **Testing**: Bun test runner
- **Linting**: Biome
- **Logging**: pino
- **Events**: eventemitter3

## Runtime Support

| Runtime | FFI Mechanism | Status |
|---------|---------------|--------|
| Bun | Native `bun:ffi` | Recommended |
| Node.js | koffi | Supported |
| Deno | `Deno.dlopen` | Experimental |

## License

MIT
