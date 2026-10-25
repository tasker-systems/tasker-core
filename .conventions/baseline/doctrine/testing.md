# Testing and CI

**TEST-1 — an ignored test is a registered exception.** Every `#[ignore]` carries a reason
(`#[ignore = "..."]`), and a tripwire pins the set. *Why:* an ignored test is a deleted test that
still looks present.

**TEST-2 — never weaken a test to get green.** Do not remove an assertion, loosen a check, or add an
ignore or skip to make a failure go away ([tenet 12](tenets.md)). If an assertion needs a dependency,
add the dependency. If it fails, fix the cause.

**TEST-3 — test tiers are features, and CI runs every tier.** Tests that need infrastructure
(database, messaging, running services, embeddings) are gated by Cargo features. CI enables every
tier somewhere; a tier that silently compiles out in one job is covered in another. *Why:* a feature
gate that CI never turns on hides its tests without anyone noticing.

**TEST-4 — retries are reported, not hidden.** The nextest CI profile may retry, but every test that
passed only on retry is surfaced as a CI annotation. A flaky test is a finding, not a shrug.

**TEST-5 — coverage floors run in CI.** Per-crate line-coverage floors (`coverage-thresholds.json`)
are checked by a CI job, at least on a nightly schedule. A new crate must declare its floor or the
check fails.

**CI-1 — a cheap gate first.** A stage-1 job with no path conditions and no compilation runs
formatting, tripwires, `cargo audit`, `cargo deny` and `cargo machete` on every change. The expensive
jobs need it to pass.

**CI-2 — one required check.** Change-scope detection decides which jobs run, and a single aggregate
check (`CI Success`) is the only required status. It validates every in-scope job, so a skipped job
counts as passing only when it was out of scope. Scope detection fails closed: a change to the
detector itself runs everything.

**CI-3 — generated artifacts are drift-checked.** Anything committed that is generated from
something else (OpenAPI specs, SDKs, type bindings, reference docs, query caches) is regenerated in
CI and compared to what is committed.

**CI-4 — the local hook runs what the change can affect.** The pre-commit hook classifies what is
staged and runs only the checks those files can break, plus secret scanning on everything. *Why:* a
hook that always runs everything trains `--no-verify`, and a bypassed hook protects nothing.

**CI-5 — required checks run in the merge queue.** If a repository uses a merge queue, every
workflow that produces a required check triggers on `merge_group`. Scope detection there diffs
against `merge_group.base_sha`. *Why:* the queue tests the exact commit that will land. A required
check that never runs on `gh-readonly-queue/*` never reports, so every queued PR waits out the
queue's timeout and is ejected. tasker-core's queue could not merge anything until its `ci.yml`
gained the trigger.

**CI-6 — job timeouts budget for cold caches.** A job's `timeout-minutes` allows for the slowest
legitimate run, not the typical one. Merge-queue runs read only the base branch's caches, so they
build colder than PR runs. *Why:* a timeout that cancels a job after every test has passed reports a
failure that is not one. tasker-core's integration job passed all 474 tests in the queue and was
cancelled at 25 minutes; it now has 40.

