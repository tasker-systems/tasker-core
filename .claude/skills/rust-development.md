# Skill: Rust Development

## When to Use

Use this skill when writing, reviewing, or modifying Rust code in the tasker-core workspace. This covers coding standards, lint compliance, error handling patterns, async patterns, and Rust-specific project conventions.

## External Standards Alignment

Tasker Rust code aligns with [Microsoft's Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/) (Universal section) and the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). Key adopted guidelines:

| MS Guideline | Rule | Tasker Enforcement |
|-------------|------|-------------------|
| M-STATIC-VERIFICATION | Use compiler lints, Clippy, rustfmt, cargo-audit | `cargo make check-rust` runs all |
| M-LINT-OVERRIDE-EXPECT | Use `#[expect]` not `#[allow]` with `reason` | TAS-58 lint standards |
| M-PUBLIC-DEBUG | All public types implement `Debug` | Required; sensitive types use custom impl |
| M-SMALLER-CRATES | Split crates when submodules are independently usable | 10-crate workspace |
| M-CONCISE-NAMES | Avoid weasel words (Service, Manager, Factory) | Use Builder for factories |
| M-REGULAR-FN | Prefer free functions over associated functions for non-instance work | Idiomatic Rust style |
| M-PANIC-ON-BUG | Programming bugs are panics, not errors | Contract violations panic |
| M-PANIC-IS-STOP | Panic means program termination | No panic recovery in libraries |
| M-LOG-STRUCTURED | Use structured events with message templates | `tracing` with named fields |
| M-DOCUMENTED-MAGIC | Document non-obvious code | Required for unsafe, complex logic |

## Lint Configuration (TAS-58)

### `#[expect]` Over `#[allow]`

```rust
// BAD: No reason, stale lint risk
#[allow(dead_code)]
fn unused_helper() { ... }

// GOOD: Self-documenting, warns when lint becomes stale
#[expect(dead_code, reason = "used by FFI bindings, not called from Rust")]
fn ffi_helper() { ... }
```

### Required Traits for Public Types

```rust
// All public types MUST implement Debug
#[derive(Debug)]
pub struct TaskConfig { ... }

// Sensitive types: custom Debug that redacts
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey(***)")
    }
}
```

### Build Commands

```bash
# Always use --all-features for consistency
cargo build --all-features
cargo clippy --all-targets --all-features
cargo check --all-features
cargo fmt
```

## Error Handling

### Library Errors: `thiserror`

```rust
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Task not found: {0}")]
    NotFound(Uuid),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: TaskState, to: TaskState },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, TaskError>;
```

### Fail Loudly Tenet

```rust
// BAD: Silent default on missing data
let status = response.status.unwrap_or_default();

// GOOD: Explicit error for missing data
let status = response.status
    .ok_or_else(|| ClientError::invalid_response("missing status field"))?;
```

### No Panics in Library Code

```rust
// BAD: Panic in library
fn get_handler(name: &str) -> &Handler {
    self.handlers.get(name).unwrap()
}

// GOOD: Return Result or Option
fn get_handler(&self, name: &str) -> Option<&Handler> {
    self.handlers.get(name)
}

// ACCEPTABLE: expect() when invariant is guaranteed
fn get_handler(&self, name: &str) -> &Handler {
    self.handlers.get(name)
        .expect("handler was validated at registration time")
}
```

## Async Patterns

### Bounded Channels Only (TAS-51)

```rust
// ALWAYS bounded, ALWAYS from config
let (tx, rx) = tokio::sync::mpsc::channel(config.channel_capacity);

// NEVER unbounded
// let (tx, rx) = tokio::sync::mpsc::unbounded_channel(); // FORBIDDEN
```

### Actor Pattern

```rust
pub struct MyActor {
    receiver: mpsc::Receiver<Message>,
}

impl MyActor {
    pub fn spawn(config: Config) -> ActorHandle {
        let (tx, rx) = mpsc::channel(config.channel_capacity);
        let actor = Self { receiver: rx };
        let handle = tokio::spawn(async move { actor.run().await });
        ActorHandle { sender: tx, handle }
    }

    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            self.handle_message(msg).await;
        }
    }
}
```

## Database Patterns

### SQLx Compile-Time Checking

```rust
let task = sqlx::query_as!(
    Task,
    r#"SELECT id, state as "state: TaskState", created_at FROM tasks WHERE id = $1"#,
    id
)
.fetch_one(&pool)
.await?;
```

### Transactions for Multi-Step Operations

```rust
let mut tx = pool.begin().await?;
sqlx::query!("UPDATE tasks SET state = $1 WHERE id = $2", state, id)
    .execute(&mut *tx).await?;
sqlx::query!("INSERT INTO task_transitions ...")
    .execute(&mut *tx).await?;
tx.commit().await?;
```

### SQLx Cache After Query Changes

```bash
cargo make sqlx-prepare
git add .sqlx/
```

## Naming Conventions

```rust
// Types: PascalCase
struct TaskRequestActor { ... }
enum StepState { ... }

// Functions/methods: snake_case
fn process_step_result() { ... }
async fn handle_message() { ... }

// Constants: SCREAMING_SNAKE_CASE
const DEFAULT_BATCH_SIZE: usize = 100;

// Avoid weasel words (M-CONCISE-NAMES)
// BAD: TaskService, StepManager, HandlerFactory
// GOOD: Tasks, StepCoordinator, HandlerBuilder
```

## Module Organization

```rust
//! Crate-level documentation
// 1. Re-exports
pub use types::{Task, Step};
// 2. Public modules
pub mod actors;
pub mod handlers;
// 3. Internal modules
mod internal;
// 4. Tests
#[cfg(test)]
mod tests;
```

## Testing Patterns

```rust
// Unit tests in same file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transition_from_pending_to_initializing_succeeds() { ... }

    #[tokio::test]
    async fn process_step_with_all_deps_met_executes_handler() { ... }
}
```

**Never remove assertions to fix compilation or test failures** -- fix the underlying issue instead.

## Performance Considerations

```rust
// Accept references when not storing
fn process(name: &str) { ... }      // not String
fn process(name: Cow<'_, str>) { ... } // conditional ownership

// Lazy iteration over collect-then-iterate
for name in items.iter().map(|i| &i.name) { ... }
```

## References

- Best practices: `docs/development/best-practices-rust.md`
- MPSC channels: `docs/development/mpsc-channel-guidelines.md`
- FFI safety: `docs/development/ffi-callback-safety.md`
- Microsoft guidelines: https://microsoft.github.io/rust-guidelines/guidelines/universal/
- Rust API guidelines: https://rust-lang.github.io/api-guidelines/
