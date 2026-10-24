---
name: tasker-systems-conventions
description: The tasker-systems engineering baseline as this repository is pinned to it — the rules,
  why each exists, and whether a gate enforces it here. Use before adding a lint suppression, an
  unsafe block, a dependency, a CI workflow, a test exception, or agent guidance; and whenever you
  are about to claim a rule is enforced.
---

# tasker-systems conventions

This skill is **projected** into this repository from the `tasker-systems/conventions` baseline at
the revision pinned in `.conventions/consumer.toml`. Do not edit it here: `conventions.py bump`
rewrites it, and CI fails if it differs from the projection (CONS-6).

The doctrine itself is in `.conventions/baseline/doctrine/`. Read the file for the area you are
touching; each rule states its *why* beside it, and its id is what you cite in a PR or an
`#[expect(reason)]`.

| Touching | Read |
|---|---|
| The rule register: every rule, and its status in each consumer | `.conventions/baseline/doctrine/rules.md` |
| The short list everything follows from | `.conventions/baseline/doctrine/tenets.md` |
| Rust: lints, formatting, API shape, unsafe, errors, panics, toolchain | `.conventions/baseline/doctrine/rust.md` |
| Python | `.conventions/baseline/doctrine/python.md` |
| TypeScript | `.conventions/baseline/doctrine/typescript.md` |
| Shell scripts and CI gate scripts | `.conventions/baseline/doctrine/shell.md` |
| Markdown | `.conventions/baseline/doctrine/docs.md` |
| HTTP API JSON and OpenAPI | `.conventions/baseline/doctrine/api.md` |
| Dependencies, advisories, licenses, Actions, tool pins | `.conventions/baseline/doctrine/supply-chain.md` |
| Credentials, redaction, SQL, auth | `.conventions/baseline/doctrine/security.md` |
| Channels, spawned work, locks, database coordination | `.conventions/baseline/doctrine/concurrency.md` |
| Tests and CI shape | `.conventions/baseline/doctrine/testing.md` |
| Writing a tripwire | `.conventions/baseline/doctrine/tripwires.md` |
| Agent guidance | `.conventions/baseline/doctrine/agent-guidance.md` |
| This repository's pin, exceptions and statuses | `.conventions/baseline/doctrine/consuming.md` |

## Before you claim a rule is enforced

Check the status below. `gated: <gate>` means a named check fails on a violation **here**. Anything
else — `partial`, `practiced`, `convention`, `uncovered`, `unexamined` — means nothing here would
catch a violation, and saying otherwise is the defect tenet 1 names. An unlisted rule is
unexamined in this repository, not covered.

Local departures from the baseline, each with its reason, are in `.conventions/exceptions.toml`.

<!-- consumer-status:begin -->
<!-- consumer-status:end -->
