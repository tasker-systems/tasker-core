# AGENTS.md - TypeScript Worker

**Status**: TAS-100/TAS-290 | 808+ tests | Bun (napi-rs FFI)

---

## Quick Reference

### Build Commands
```bash
# Full build (Rust FFI + TypeScript)
cargo make build

# Individual targets
cargo make build-ffi        # Rust napi-rs module (debug)
cargo make build-ffi-release # Rust napi-rs module (release)
cargo make build-ts          # TypeScript only

# Testing
cargo make test           # Run all tests
cargo make check          # lint + typecheck + test
cargo make test-ffi       # napi-rs FFI integration tests
bun test                  # Direct test run

# Linting
cargo make lint           # Biome lint
cargo make typecheck      # TypeScript type check
```

### Build Output Location

Build artifacts go to `$CARGO_TARGET_DIR` if set, otherwise `../../target/`:

```bash
# Check your current target directory
echo ${CARGO_TARGET_DIR:-../../target}

# FFI library location (napi-rs produces standard cdylib naming)
ls ${CARGO_TARGET_DIR:-../../target}/debug/libtasker_ts.*
```

**Local Development Note**: If using external cache (see `~/bin/development_cache_init.sh`),
`CARGO_TARGET_DIR` points to `/Volumes/Expansion/Development/Cache/cargo-targets/`.
This keeps large build caches off the main drive.

---

## Architecture

### Directory Structure
```
workers/typescript/
├── Cargo.toml          # Rust cdylib crate definition (napi-rs)
├── build.rs            # napi-build setup
├── Makefile.toml       # cargo-make task definitions
├── package.json        # TypeScript package (Bun/npm)
├── src-rust/           # Rust FFI implementation
│   ├── lib.rs          # #[napi] exports
│   ├── bridge.rs       # Worker lifecycle, global state
│   ├── client_ffi.rs   # Client API FFI functions
│   ├── conversions.rs  # Type conversions
│   ├── error.rs        # Error types
│   └── ffi_logging.rs  # Structured logging FFI
├── src/                # TypeScript source
│   ├── ffi/            # FfiLayer, types
│   ├── events/         # Event emitter, poller
│   ├── client/         # Client API wrapper
│   ├── handler/        # Handler dispatch
│   ├── registry/       # Handler registry
│   ├── server/         # WorkerServer
│   └── index.ts        # Package entry point
├── tests/              # Test suites
│   ├── unit/           # Unit tests (no FFI required)
│   └── integration/    # FFI integration tests
└── dist/               # Built TypeScript output
```

### Runtime Support

napi-rs produces a native Node-API module (`.node` file) that works with any
Node-API compatible runtime:

| Runtime | Loading Method | Status |
|---------|---------------|--------|
| Bun | `require()` via napi-rs | Primary |
| Node.js | `require()` via napi-rs | Supported |

### FFI Module

The Rust napi-rs module (`libtasker_ts.{dylib,so}`) exports functions via `#[napi]`:

- `get_version()` / `get_rust_version()` - Version info
- `health_check()` - Library health
- `bootstrap_worker(config)` - Start worker
- `stop_worker()` / `get_worker_status()` - Lifecycle
- `poll_step_events()` - Pull events from dispatch channel
- `complete_step_event(event_id, result)` - Return results
- `get_ffi_dispatch_metrics()` - Queue metrics
- `log_error/warn/info/debug/trace()` - Structured logging
- `client_create_task()` / `client_get_task()` etc. - Client API

napi-rs auto-converts snake_case to camelCase at the FFI boundary and handles
JSON serialization via serde — no manual C FFI or JSON string passing needed.

---

## Testing Strategy

### Unit Tests
Tests that verify TypeScript logic without FFI:
- Type coherence and JSON serialization
- Event emitter functionality
- Handler registry and dispatch
- WorkerServer lifecycle

```bash
bun test                    # Run all tests
bun test tests/unit/        # Unit tests only
```

### FFI Integration Tests
Tests that load the actual napi-rs Rust module:
- Bootstrap and lifecycle
- Step event polling and completion
- Client API operations

```bash
cargo make test-ffi         # Build FFI + run integration tests
```

---

## CI Integration

The TypeScript worker is part of the CI pipeline:

1. **build-workers.yml**: `cargo make build` compiles napi-rs FFI + TypeScript
2. **test-typescript-framework.yml**: Unit tests, FFI integration tests, client API tests
3. Artifacts uploaded: `dist/`, `libtasker_ts.{so,dylib}`

---

## Development Workflow

### Adding a New FFI Function

1. Add `#[napi]` function in `src-rust/` (bridge.rs or client_ffi.rs)
2. Add TypeScript type in `src/ffi/types.ts`
3. Add to `NapiModule` interface in `src/ffi/ffi-layer.ts`
4. Add test in `tests/unit/` or `tests/integration/ffi/`

### Adding a New Event

1. Add event name constant in `src/events/event-names.ts`
2. Add payload type in `src/events/event-emitter.ts`
3. Add emit helper method to `TaskerEventEmitter`
4. Add test in `tests/unit/events/event-emitter.test.ts`

---

## Troubleshooting

### "napi-rs native module not found" errors
```bash
# Check library exists
ls ${CARGO_TARGET_DIR:-../../target}/debug/libtasker_ts.*

# Rebuild if missing
cargo make build-ffi

# Or set explicit path
export TASKER_FFI_MODULE_PATH=$(pwd)/../../target/debug/libtasker_ts.dylib
```

### "Lockfile had changes" in CI
```bash
# Update lockfile locally
bun install
git add bun.lock
```

### Type errors after FFI changes
```bash
# Regenerate types and rebuild
cargo make clean-ts
cargo make build-ts
```
