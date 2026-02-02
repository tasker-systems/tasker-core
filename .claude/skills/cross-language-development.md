# Skill: Cross-Language Development

## When to Use

Use this skill when working with polyglot workers (Ruby, Python, TypeScript), FFI bindings, handler implementation in non-Rust languages, or ensuring cross-language API consistency.

## Language Workers

Tasker supports step handler execution in multiple languages through FFI (Foreign Function Interface) bindings built on the Rust `tasker-worker` cdylib:

| Language | Crate | Build Tool | FFI Framework | Package Manager |
|----------|-------|------------|---------------|-----------------|
| **Ruby** | `workers/ruby/ext/tasker_core` | `rake compile` (rb_sys) | magnus | bundle |
| **Python** | `workers/python` | maturin | pyo3 | uv |
| **TypeScript** | `workers/typescript` | cargo build + tsup | C ABI (napi-like) | bun |
| **Rust** | `workers/rust` | cargo build | native | cargo |

## Cross-Language Consistency Tenet

All language workers must expose the same developer-facing API patterns:

### Handler Pattern (All Languages)

```
class/struct Handler {
  call(context: StepContext) -> StepResult
}
```

### Result Factories (All Languages)

```
success(result_data, metadata?) -> StepResult
failure(message, error_type, error_code?, retryable?, metadata?) -> StepResult
```

### Mixin Capabilities (All Languages)

| Mixin | Ruby | Python | TypeScript |
|-------|------|--------|------------|
| API (HTTP) | `include API` | `@api_capable` | `implements APICapable` |
| Decision | `include Decision` | `@decision_capable` | `implements DecisionCapable` |
| Batchable | `include Batchable` | `@batch_capable` | `implements BatchCapable` |

### StepContext (All Languages)

| Field | Type | Available In |
|-------|------|-------------|
| `task_uuid` | String | All |
| `step_uuid` | String | All |
| `input_data` | Dict/Hash/Object | All |
| `step_config` | Dict/Hash/Object | All |
| `dependency_results` | Wrapper | All |
| `retry_count` | Integer | All |
| `max_retries` | Integer | All |

## Worker Setup

### Ruby Worker

```bash
cd workers/ruby
bundle install
bundle exec rake compile        # Build native extension
bundle exec rake spec           # Run tests

# Integration tests
DATABASE_URL=postgresql://tasker:tasker@localhost/tasker_rust_test \
TASKER_ENV=test bundle exec rspec spec/integration/ --format documentation

# Clean rebuild
cd workers/ruby && rake clean && rake compile
```

### Python Worker

```bash
cd workers/python
uv sync                         # Install dependencies
uv run maturin develop          # Build in dev mode
uv run pytest                   # Run tests
```

### TypeScript Worker

```bash
cd workers/typescript
bun install                     # Install dependencies
cargo build -p tasker-worker-ts --release  # Build Rust cdylib
bun run build                   # Build TypeScript
bun test                        # Run tests
```

## Using cargo-make for Workers

```bash
# Setup all workers
cargo make setup-workers

# Clean all worker artifacts
cargo make clean-workers

# Check individual languages
cargo make check-ruby
cargo make check-python
cargo make check-typescript

# FFI integration tests
cargo make test-ruby-ffi
cargo make test-python-ffi
cargo make test-typescript-ffi
cargo make test-ffi-all
```

## Worker Architecture

### Dispatch Flow

```
Queue Message -> Worker (Rust core)
  -> Handler Resolution (resolver chain)
  -> FFI Dispatch (for non-Rust handlers)
  -> Handler Execution (in target language)
  -> Result Collection (back to Rust)
  -> Queue Completion Message
```

### Handler Resolution Chain

1. **ExplicitMapping**: Direct handler -> class mapping in template
2. **Custom Resolver**: User-provided resolution logic
3. **ClassLookup**: Convention-based class name resolution

### FFI Dispatch Channel

- Pull-based polling model for Ruby/Python (GIL constraints)
- Semaphore-bounded concurrent execution
- Configurable via TOML (worker settings)

## FFI Callback Safety

- Ruby and Python have GIL (Global Interpreter Lock) -- cannot execute handlers concurrently within a single interpreter
- `FfiDispatchChannel` uses pull-based model: language runtime pulls work when ready
- TypeScript (Bun) can handle concurrent callbacks
- See: `docs/development/ffi-callback-safety.md`

## Adding a New Language Worker

1. Create Rust cdylib crate in `workers/<language>/`
2. Implement FFI bindings exposing handler dispatch, context, and result types
3. Create language-native wrapper providing idiomatic API
4. Ensure `call(context) -> result` pattern matches other languages
5. Include `success()`/`failure()` result factories
6. Add to `Makefile.toml` with check/test/coverage tasks
7. Add to version update scripts in `scripts/release/`

## Port Allocation

| Service | REST Port | gRPC Port |
|---------|-----------|-----------|
| Orchestration | 8080 | 9190 |
| Rust Worker | 8081 | 9191 |
| Ruby Worker | 8082 | 9200 |
| Python Worker | 8083 | 9300 |
| TypeScript Worker | 8085 | 9400 |

## References

- Cross-language consistency: `docs/principles/cross-language-consistency.md`
- Composition over inheritance: `docs/principles/composition-over-inheritance.md`
- FFI callback safety: `docs/development/ffi-callback-safety.md`
- Worker architecture: `tasker-worker/AGENTS.md`
- Worker event systems: `docs/architecture/worker-event-systems.md`
