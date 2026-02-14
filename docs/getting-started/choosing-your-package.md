# Choosing Your Package

Tasker supports multiple languages through FFI bindings. Each language package provides the same core capabilities with idiomatic APIs.

## Language Guides

| Language | Package | Guide |
|----------|---------|-------|
| **Rust** | `tasker-worker` + `tasker-client` | [Rust Guide](rust.md) |
| **Ruby** | `tasker-rb` | [Ruby Guide](ruby.md) |
| **Python** | `tasker-py` | [Python Guide](python.md) |
| **TypeScript** | `@tasker-systems/tasker` | [TypeScript Guide](typescript.md) |

## How to Choose

### Use Rust

- Need maximum performance with zero-overhead abstractions
- Want compile-time type safety and memory safety guarantees
- Are building native Tasker extensions or the orchestration system itself
- Prefer direct API access without FFI overhead

### Use Ruby

- Have an existing Rails or Ruby application
- Prefer Ruby's expressive DSL capabilities
- Value rapid development with convention-over-configuration
- Want seamless integration with Ruby ecosystem gems

### Use Python

- Are building data pipelines or ML workflows
- Want async/await support with asyncio, aiohttp, etc.
- Need integration with the Python data science ecosystem
- Prefer Python's clean syntax and type hints

### Use TypeScript

- Are building Node.js or Bun applications
- Want strong typing with TypeScript's type system
- Need to integrate with existing JavaScript ecosystems
- Prefer modern async/await patterns

## Package Architecture

All language packages share the same architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    Your Application                          │
├─────────────────────────────────────────────────────────────┤
│  Language Package (tasker-rb, tasker-py, @tasker/tasker)   │
├─────────────────────────────────────────────────────────────┤
│  FFI Layer (Magnus/PyO3/NAPI)                               │
├─────────────────────────────────────────────────────────────┤
│  tasker-worker (Rust core)                                   │
├─────────────────────────────────────────────────────────────┤
│  tasker-client (API client)                                  │
└─────────────────────────────────────────────────────────────┘
```

This means:

- **Same core logic** — All packages use the same Rust implementation
- **Same features** — Handler registration, client SDK, event system
- **Cross-language consistency** — `get_input()`, `get_dependency_result()`, etc. work the same

## Quick Installation

```bash
# Rust
cargo add tasker-worker tasker-client

# Ruby
gem install tasker-rb

# Python
pip install tasker-py

# TypeScript/JavaScript
npm install @tasker-systems/tasker
```

See the individual language guides for detailed setup and examples.
