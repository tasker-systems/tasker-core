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

Statuses for **tasker-core**:

| Rule | Status here |
|---|---|
| AG-1 | uncovered: CLAUDE.md is a 455-line monolith (AGENTS.md is a byte copy), not a router |
| AG-2 | uncovered: many CLAUDE.md rules state no reason (e.g. SQLX_OFFLINE, config layout); no check |
| AG-3 | uncovered: CLAUDE.md forbids SQLX_OFFLINE=true, which CI sets in 19 steps; no check |
| AG-4 | uncovered: CLAUDE.md crate list has 12 of 15 crates (no grammar/secure/runtime); no drift gate |
| AG-5 | gated: .github/workflows/ci.yml#conventions |
| AG-6 | practiced |
| API-JSON-1 | uncovered: PascalCase enum values on the wire (23 in orchestration spec, 14 in worker); no check |
| API-JSON-2 | uncovered: spec ApiError is the Rust enum's tagged oneOf, not the wire error shape; no spec-vs-wire test |
| API-JSON-3 | uncovered: 9 of 19 *_at fields in the orchestration spec are typed date-time; no check |
| API-JSON-4 | practiced |
| API-JSON-5 | practiced |
| CI-1 | partial: conventions, ruff and cargo deny run unscoped and first; fmt and audit sit in code-quality behind build-postgres and scope; no machete or tripwires |
| CI-2 | gated: .github/workflows/ci.yml#ci-success |
| CI-3 | partial: lockfiles and missing .sqlx entries fail CI; OpenAPI specs, index.d.ts, docs/generated, generated configs never diffed |
| CI-4 | partial: hook selects on staged .rs and runs cargo fmt only; no other file classes and no secret scanning |
| CI-5 | practiced |
| CI-6 | practiced |
| CONC-BOUND-1 | gated: .github/workflows/code-quality.yml#quality |
| CONC-BOUND-2 | uncovered: mostly config-driven, but a production literal at grpc/services/tasks.rs:361; no channel-literal check |
| CONC-BOUND-3 | convention: the at-bound strategy is a design choice judged in review (mpsc-channel-guidelines.md) |
| CONC-BOUND-4 | practiced |
| CONC-DB-1 | practiced |
| CONC-DB-2 | practiced |
| CONC-DB-3 | convention: transaction scope is judged in review |
| CONC-FAIL-1 | uncovered: catch_unwind containment is defeated by release panic = abort; joining of spawned tasks not checked |
| CONC-FAIL-2 | uncovered: 26 unnamed tokio::spawn sites in crate src alongside 17 spawn_named!; no check |
| CONC-FAIL-3 | uncovered: tasker-example-rs metrics_subscriber.rs recovers poisoned locks via into_inner without a reasoned comment |
| CONS-1 | gated: .github/workflows/ci.yml#conventions |
| CONS-2 | gated: .github/workflows/ci.yml#conventions |
| CONS-3 | gated: .github/workflows/ci.yml#conventions |
| CONS-4 | gated: .github/workflows/ci.yml#conventions |
| CONS-5 | gated: .github/workflows/ci.yml#conventions |
| CONS-6 | gated: .github/workflows/ci.yml#conventions |
| CONS-7 | gated: .github/workflows/ci.yml#conventions |
| CONS-8 | gated: .github/workflows/ci.yml#conventions |
| DOC-1 | uncovered: markdownlint-cli2 runs only as a local cargo-make task; MD025 is off; docs-only PRs skip CI |
| DOC-2 | uncovered: MD040 is on in the local config, but markdownlint is not run in CI |
| DOC-3 | uncovered: MD013 is disabled and markdownlint is not run in CI |
| DOC-4 | uncovered: no offline link check (lychee or equivalent) anywhere |
| PY-DEF-1 | gated: .github/workflows/ci.yml#python-lint |
| PY-ERR-1 | gated: .github/workflows/ci.yml#python-lint |
| PY-ERR-2 | gated: .github/workflows/ci.yml#python-lint |
| PY-GEN-1 | n/a: no generated Python is committed; tasker-ctl codegen output is built and syntax-checked in CI only |
| PY-LINT-1 | gated: .github/workflows/ci.yml#python-lint |
| PY-LINT-2 | partial: the root ruff.toml is pinned to the projection (ci.yml#conventions); an override in crates/tasker-py's [tool.ruff] is caught only in review |
| PY-LINT-3 | partial: a suppression without a code, or an unused one, fails (ci.yml#python-lint); its reason is review |
| PY-LOG-1 | partial: G and LOG refuse f-strings to stdlib loggers (ci.yml#python-lint); tasker_core's log_* helpers take preformatted f-strings unchecked |
| PY-TYPE-1 | uncovered: mypy strict is configured but no workflow runs it |
| PY-TYPE-2 | uncovered: _tasker_core.pyi is never checked against the built module |
| PY-VER-1 | partial: target-version is the projection's (ci.yml#conventions); requires-python, the pyo3 abi3 feature and mypy python_version in tasker-py match it by hand |
| RUST-API-1 | convention: scoping rule for which crates bind the API rules, applied in review |
| RUST-API-2 | uncovered: 0 #[non_exhaustive] in any crate, including publishable ones; no check |
| RUST-API-3 | gated: .github/workflows/code-quality.yml#quality |
| RUST-API-4 | gated: .github/workflows/code-quality.yml#quality |
| RUST-API-5 | partial: wrong_self_convention denied workspace-wide (code-quality.yml#quality); naming beyond it by review |
| RUST-API-6 | practiced |
| RUST-DOC-1 | uncovered: no crate sets missing_docs; rustdoc -D warnings now covers the workspace (code-quality.yml#quality) but only for broken docs |
| RUST-ERR-1 | uncovered: mostly followed, but tasker-shared exposes 13 pub fns returning anyhow::Result; nothing checks |
| RUST-ERR-2 | convention: whether a value is required is a semantic judgement; 129 unwrap_or_default() sites unreviewed |
| RUST-ERR-3 | convention: panic-vs-Result is judged in review; missing_panics_doc not enabled |
| RUST-ERR-4 | partial: unnecessary_unwrap denied workspace-wide (code-quality.yml#quality); unwrap_used not enabled, ~1.4k unwrap() in src files |
| RUST-FMT-1 | gated: .github/workflows/code-quality.yml#quality |
| RUST-INIT-1 | practiced |
| RUST-LINT-1 | partial: the table is the projection's (ci.yml#conventions, check A) and all 16 members inherit it; nothing refuses a new member without [lints] workspace = true |
| RUST-LINT-2 | gated: .github/workflows/code-quality.yml#quality |
| RUST-LINT-3 | gated: .github/workflows/code-quality.yml#quality |
| RUST-LINT-4 | convention: an adoption process judged in review; the table was adopted with every finding fixed or reason-annotated and the dead crate-level #![allow]s removed |
| RUST-LINT-5 | uncovered: style/complexity/perf still run at clippy's defaults (the canonical table does not turn them off); its named single lints are on |
| RUST-MACRO-1 | convention: macro use is judged in review |
| RUST-ORG-1 | uncovered: 72 glob imports beyond use super::*; clippy::wildcard_imports not enabled |
| RUST-ORG-2 | convention: module layout is judged in review; no lint covers it |
| RUST-PANIC-1 | uncovered: 3 production catch_unwind containment sites while [profile.release] sets panic = abort; no check |
| RUST-READ-1 | convention: readability is judged in review |
| RUST-SQL-1 | partial: query! macros compile-checked in CI builds; 81 runtime sqlx::query() calls in src are unregistered, no tripwire |
| RUST-TC-1 | uncovered: toolchain pinned to 1.99.0 in rust-toolchain.toml; no workspace rust-version (only tasker-py, tasker-ts set one) |
| RUST-TC-2 | uncovered: no [workspace.package]; each of 16 manifests sets edition = 2021 on its own |
| RUST-TC-3 | gated: .github/workflows/test-integration.yml#integration-tests |
| RUST-TYPE-1 | gated: .github/workflows/code-quality.yml#quality |
| RUST-TYPE-2 | convention: whether values are confusable is judged in review |
| RUST-UNSAFE-1 | uncovered: no crate root declares #![forbid(unsafe_code)]; unsafe_code is denied by the table instead |
| RUST-UNSAFE-2 | partial: unsafe_code and undocumented_unsafe_blocks denied workspace-wide (code-quality.yml#quality); the one site carries a reasoned expect; no register, no tripwire |
| RUST-UNSAFE-3 | practiced |
| SC-ADV-1 | gated: .github/workflows/code-quality.yml#quality |
| SC-ADV-2 | convention: each ignore is reviewed by hand; its reason, reachability and status are not machine-checked |
| SC-DENY-1 | gated: .github/workflows/ci.yml#supply-chain |
| SC-DENY-2 | gated: .github/workflows/ci.yml#supply-chain |
| SC-DENY-3 | gated: .github/workflows/ci.yml#supply-chain |
| SC-GHA-1 | practiced |
| SC-GHA-2 | practiced |
| SC-GHA-3 | uncovered: release.yml triggers are not pinned by any check |
| SC-IN-1 | uncovered: uv/bun latest, unversioned cargo install of sqlx-cli/cargo-make/cargo-audit, binstall nextest, global typescript |
| SC-IN-2 | uncovered: dependabot misses docker/build/ (12 Dockerfiles); other ecosystems covered; nothing checks coverage |
| SC-IN-3 | partial: Cargo/Gemfile/tasker-py uv locks gated in ci.yml#lockfile-check, bun.lock via --frozen-lockfile; coverage uv.lock unchecked |
| SC-SEC-1 | partial: CodeQL runs per PR from repository settings, outside CI Success; no gitleaks in CI or the pre-commit hook |
| SEC-AUTH-1 | uncovered: disabled auth only logs at debug level; no check stops a production config with auth off |
| SEC-AUTH-2 | practiced |
| SEC-AUTH-3 | convention: existence-hiding is judged per endpoint in review; permissions are route-level |
| SEC-CRED-1 | uncovered: JWT private key and API key held as bare String in tasker-shared config structs |
| SEC-CRED-2 | uncovered: WebAuthConfig derives Debug over jwt_private_key; no tripwire |
| SEC-CRED-3 | practiced |
| SEC-CRED-4 | convention: secret-per-purpose is a design review matter; JWT keys and API key are separate settings |
| SEC-RED-1 | uncovered: redaction helpers duplicated (redact_url in redis.rs and memcached.rs, others in tasker-client); no single module |
| SEC-RED-2 | convention: redaction policy is judged in review; no single redaction module to anchor a check |
| SEC-RPT-1 | uncovered: SECURITY.md names email and response time but no GitHub advisory channel and no out-of-scope list |
| SEC-SQL-1 | practiced |
| SEC-SQL-2 | convention: permissions are route-level; there is no per-caller row scoping to check |
| SEC-TEST-1 | uncovered: test-utils is in the default features of tasker-orchestration and tasker-worker; nothing checks shipped features |
| SH-1 | uncovered: 29 of 104 tracked scripts use #!/bin/bash; no check |
| SH-2 | uncovered: 15 of 104 tracked scripts lack set -euo pipefail; no check |
| SH-3 | uncovered: ShellCheck runs only in the local ci-sanity-check task; no workflow calls it |
| SH-4 | uncovered: no ShellCheck (SC2006) in CI; backticks are unchecked |
| SH-5 | uncovered: eval of constructed command strings (e.g. docker/scripts/dev/validate-optimized-builds.sh:30); no check |
| SH-6 | uncovered: 11 GNU-only constructs (sed -i, readlink -f, date -d, grep -P, mapfile) unchecked; CI runs Linux only |
| SH-7 | convention: where logic belongs is judged in review |
| TEST-1 | uncovered: 3 of 11 #[ignore] carry no reason; no tripwire pins the set |
| TEST-2 | convention: weakening is caught only in review; note the napi FFI test step in TS framework job is continue-on-error |
| TEST-3 | partial: root tests run with test-services; crate tiers never enabled, tests/ of 10 crates and tasker-runtime never run, test-cluster local |
| TEST-4 | practiced |
| TEST-5 | uncovered: coverage-thresholds.json is checked by no CI job; 5 crates declare no floor |
| TS-CFG-1 | partial: tsc gates five of the six flags; noImplicitOverride missing; no shared base to extend |
| TS-CFG-2 | partial: useImportType is an error under the gated biome check; 2 enums in src; erasableSyntaxOnly and noEnum unset |
| TS-DOC-1 | convention: Biome has no doc-comment rule; export doc coverage is judged in review |
| TS-ERR-1 | practiced |
| TS-GEN-1 | uncovered: generated files are excluded from Biome but never regenerated and diffed in CI |
| TS-MOD-1 | practiced |
| TS-NAME-1 | convention: naming is judged in review; useNamingConvention not configured |
| TS-SUP-1 | practiced |
| TS-TOOL-1 | partial: biome check and tsc gated in typescript-framework job (path-scoped); tsconfig excludes tests/ from tsc |
| TS-TOOL-2 | uncovered: local biome.json with three rules at warn; biome check runs without --error-on-warnings |
| TS-TOOL-3 | partial: one Biome/TypeScript via bun.lock + --frozen-lockfile; validate-codegen.yml installs an unpinned global typescript |
| TS-TYPE-1 | practiced |
| TS-TYPE-2 | practiced |
| TW-1 | n/a: tasker-core has no tripwire scripts, so the header format binds nothing |
| TW-2 | n/a: tasker-core has no tripwire scripts, so there is no baseline to keep |
| TW-3 | n/a: tasker-core has no tripwire scripts |
| TW-4 | n/a: tasker-core has no tripwire scripts |
| TW-5 | n/a: tasker-core has no tripwire scripts; test-detect-ci-scope.sh exists but no workflow runs it |
| TW-6 | uncovered: no tripwire set, glob runner or CI job exists, although rules here call for several |
| TW-7 | n/a: tasker-core has no tripwire scripts |

<!-- consumer-status:end -->
