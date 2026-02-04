<!-- Replace TAS-XXX with the Linear ticket ID, or remove if no ticket -->
Resolves TAS-

## Summary

<!-- 1-3 sentences: what changed and why -->

## Type of Change

- [ ] `feat` — New feature
- [ ] `fix` — Bug fix
- [ ] `refactor` — Code restructuring (no behavior change)
- [ ] `chore` — Build, CI, dependencies, tooling
- [ ] `docs` — Documentation only
- [ ] `test` — Test additions or corrections
- [ ] `perf` — Performance improvement

## Breaking Changes

<!-- If none, write "None" and remove the list -->

-

## Test Plan

- [ ] `cargo make check` passes
- [ ] `cargo make test` passes (or relevant subset)
- [ ] New tests added for new functionality
- [ ] SQLx query cache updated (`cargo sqlx prepare`) if SQL changed

## Checklist

- [ ] Code follows existing patterns in the codebase
- [ ] Commit messages use `type(scope): description` format
- [ ] Public API changes have updated rustdoc comments
- [ ] No security vulnerabilities introduced
- [ ] MPSC channels are bounded and configured via TOML (if applicable)
- [ ] Configuration follows role-based TOML structure (if applicable)
