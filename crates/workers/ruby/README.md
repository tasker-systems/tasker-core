# Tasker Core Ruby Bindings

Ruby FFI bindings for the high-performance Tasker Core Rust orchestration engine.

## Status

Production ready. Ruby FFI bindings provide full step handler execution via Magnus.

## Development Commands

```bash
# Install dependencies
bundle install

# Compile the Rust extension (requires Ruby dev environment)
rake compile

# Run tests
rake spec

# Full development setup
rake setup
```

## Architecture

This gem follows the **delegation-based architecture**:

```
Rails Engine ↔ tasker-core-rb (FFI) ↔ tasker-core (Performance Core)
```

- **Rails**: Business logic and step execution
- **Rust**: High-performance orchestration and dependency resolution
- **Ruby Bindings**: Safe FFI bridge between the two

## Performance Targets

- **10-100x faster** dependency resolution vs PostgreSQL functions
- **<1ms FFI overhead** per orchestration call
- **>10k events/sec** cross-language event processing

## Requirements

- **Ruby**: 3.0+ with development headers
- **Rust**: 1.70+ with magnus dependencies
- **PostgreSQL**: 12+ for database operations

## Contributing

This is part of the larger tasker-systems monorepo. See the main project documentation for development guidelines and contribution instructions.

---

🦀 **Built with Rust + Magnus for maximum performance and safety**
