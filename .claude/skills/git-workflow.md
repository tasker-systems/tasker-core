# Git Workflow

Conventions for branches, commits, hooks, and pull requests.

## Branch Naming

Format: `username/ticket-id-short-description`

```
jcoletaylor/tas-190-add-version-fields
petetaylor/tas-221-config-cleanup
```

Always branch from `main`.

## Commit Messages

Format: `type(scope): description`

| Type | Use |
|------|-----|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Restructure without behavior change |
| `chore` | Build, CI, dependencies, tooling |
| `docs` | Documentation only |
| `test` | Test additions or corrections |
| `perf` | Performance improvement |

**Scope** is the crate or area affected: `orchestration`, `worker`, `shared`, `cli`, `ci`, etc.

Examples:
```
feat(worker): add Python FFI dispatch channel
fix(orchestration): handle timeout in step enqueuer
chore(ci): update nextest profile for JUnit output
docs(shared): add module-level rustdoc for types
```

## Git Hooks

A pre-commit hook auto-formats staged Rust files via `cargo fmt --all`.

**Installation** (automatic via `bin/setup-dev.sh`, or manual):
```bash
git config core.hooksPath .githooks
```

**What it does:**
1. Checks if any `.rs` files are staged — exits early if none
2. Runs `cargo fmt --all`
3. Re-stages only files that were already staged

**Skip once:** `git commit --no-verify`

The hook exits gracefully if `cargo` is not available. Clippy and tests are left to CI.

## Pull Request Process

1. Create a branch from `main` using the naming convention above
2. Make focused changes — one logical change per PR
3. Ensure `cargo make check` and `cargo make test` pass
4. Open a PR against `main` — fill in the template:
   - Link the Linear ticket (`Resolves TAS-XXX`)
   - Check the type of change
   - Describe breaking changes (or write "None")
   - Complete the test plan and checklist

### What reviewers look for

- Correctness and test coverage
- No security vulnerabilities (OWASP top 10)
- Follows existing codebase patterns
- No unnecessary complexity or over-engineering
- Configuration follows role-based TOML structure
- MPSC channels are bounded and configured via TOML

## Quick Reference

| Task | Command |
|------|---------|
| Install hooks | `git config core.hooksPath .githooks` |
| Skip hook once | `git commit --no-verify` |
| Format all Rust | `cargo fmt --all` |
| Full quality check | `cargo make check` |
| Run tests | `cargo make test` |

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for the full contributor guide.
