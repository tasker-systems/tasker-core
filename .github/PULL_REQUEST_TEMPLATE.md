## Summary

Brief description of what this PR does and why.

## Changes

-

## Test Plan

- [ ] `cargo make check` passes
- [ ] `cargo make test` passes (or relevant subset)
- [ ] New tests added for new functionality
- [ ] SQLx query cache updated (`cargo sqlx prepare`) if SQL changed

## Checklist

- [ ] Code follows existing patterns in the codebase
- [ ] Public API changes have updated rustdoc comments
- [ ] No security vulnerabilities introduced
- [ ] MPSC channels are bounded and configured via TOML (if applicable)
- [ ] Configuration follows role-based TOML structure (if applicable)
