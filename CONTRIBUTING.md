# Contributing to Tasker Core

Thank you for your interest in contributing to Tasker Core. This guide covers the development workflow, testing expectations, and PR process.

## Getting Started

### Prerequisites

- **Rust** 1.75+ (stable toolchain)
- **PostgreSQL** 17+ (with PGMQ extension)
- **Docker** (for services)
- **cargo-make** (`cargo install cargo-make`)
- Optional: Ruby 3.4+, Python 3.12+, Bun 1.x (for FFI worker development)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/tasker-systems/tasker-core.git
cd tasker-core

# Start PostgreSQL (includes PGMQ extension)
docker-compose up -d postgres

# Set up the database
export DATABASE_URL="postgresql://tasker:tasker@localhost/tasker_rust_test"
cargo sqlx migrate run

# Build and test
cargo make check    # Lint + format + build
cargo make test     # Run tests (requires services)
```

For a full automated setup (Homebrew, Rust, cargo tools, git hooks, worker dependencies), run `bin/setup-dev.sh`.

See [CLAUDE.md](CLAUDE.md) for full development context including all commands and troubleshooting.

## Development Workflow

### Branch Naming

Use the format: `username/ticket-id-short-description`

Example: `jcoletaylor/tas-190-add-version-fields`

### Build Commands

```bash
cargo make check       # c  - All quality checks
cargo make test        # t  - All tests
cargo make fix         # f  - Auto-fix issues
cargo make build       # b  - Build everything
```

Always use `--all-features` when running cargo commands directly.

### Testing

Tests are organized into infrastructure levels via feature flags:

| Level | Flag | Requires |
|-------|------|----------|
| Unit | `test-messaging` | PostgreSQL + messaging |
| E2E | `test-services` | + running services |
| Cluster | `test-cluster` | + multi-instance cluster |

```bash
cargo make test-rust-unit     # tu - Unit tests
cargo make test-rust-e2e      # te - E2E tests
cargo make test-rust-all      # All test levels
```

**Testing rules:**

- Never use `SQLX_OFFLINE=true` — always export `DATABASE_URL`
- Always use `--all-features` for consistency
- Never remove assertions to fix compilation or test failures

### SQLx Query Cache

After modifying `sqlx::query!` macros or SQL schema:

```bash
DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test \
cargo sqlx prepare --workspace -- --all-targets --all-features

git add .sqlx/
```

### Linting Standards

- Microsoft Universal Guidelines + Rust API Guidelines enforced
- Use `#[expect(lint_name, reason = "...")]` instead of `#[allow]`
- All public types must implement `Debug`
- Run `cargo fmt` and `cargo clippy --all-targets --all-features` before committing

### Git Hooks

The project includes a pre-commit hook that automatically runs `cargo fmt --all` on staged Rust files. It only touches files you've already staged, so unstaged work-in-progress is unaffected.

**Install hooks** (automatic with `bin/setup-dev.sh`, or manual):

```bash
git config core.hooksPath .githooks
```

To skip the hook for a single commit: `git commit --no-verify`

## Submitting Changes

### Pull Request Process

1. Create a branch from `main`
2. Make focused changes — one logical change per PR
3. Ensure all tests pass: `cargo make check && cargo make test`
4. Update documentation if your change affects public APIs or behavior
5. Open a PR against `main`

### PR Expectations

- **Tests**: New functionality should include tests. Bug fixes should include a regression test.
- **Documentation**: Public API changes need updated rustdoc comments. Significant behavior changes should update relevant docs in `docs/`.
- **Commit messages**: Use the format `type(scope): description` — e.g., `fix(orchestration): handle timeout in step enqueuer`
- **Scope**: Keep PRs focused. Split large changes into reviewable chunks.

### What We Look For in Review

- Correctness and test coverage
- No security vulnerabilities (OWASP top 10)
- Follows existing patterns in the codebase
- No unnecessary complexity or over-engineering
- Configuration follows the role-based TOML structure (never create separate component files)
- MPSC channels are bounded and configured via TOML

## Design Influences

The project's systems design is informed by the [Twelve-Factor App](https://12factor.net/) methodology. Key principles for contributors:

- **Config in the environment**: Use environment variables, not hard-coded values. TOML files reference `${ENV_VAR:-default}`.
- **Backing services as attached resources**: Database, message queue, and cache are swappable via configuration.
- **Stateless processes**: All workflow state lives in PostgreSQL, not in memory.
- **Logs as event streams**: Use the `tracing` crate, write to stdout, include structured fields.

See `docs/principles/twelve-factor-alignment.md` for the full mapping with codebase examples and honest gap assessment.

## Architecture Overview

Understanding the architecture helps with contribution quality:

- **Actor pattern**: Four actors handle orchestration (see `docs/architecture/actors.md`)
- **Dual state machines**: 12 task states, 8 step states (see `docs/architecture/states-and-lifecycles.md`)
- **Event-driven workers**: Push notifications with polling fallback (see `docs/architecture/worker-event-systems.md`)
- **Configuration**: Role-based TOML with base/environment layering (see `docs/guides/configuration-management.md`)

## Code of Conduct

This project follows the [Contributor Covenant 3.0](CODE_OF_CONDUCT.md). Please read it before participating.

## Questions?

- Open a [discussion](https://github.com/tasker-systems/tasker-core/discussions) for questions
- File an [issue](https://github.com/tasker-systems/tasker-core/issues) for bugs or feature requests
- See [CLAUDE.md](CLAUDE.md) for detailed development reference
