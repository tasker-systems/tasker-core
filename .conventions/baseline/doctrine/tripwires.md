# Tripwires

A tripwire is a small static check that holds one invariant a compiler or linter cannot: "every
route declares its auth posture", "no credential-bearing type derives `Debug` unreviewed", "every
runtime SQL query is a registered exception". This format comes from temper, which runs about
twenty of them (`.github/scripts/audit-*.sh`).

The baseline owns the **format**. A repo's domain-specific tripwires stay in that repo. The portable
ones — the checks every consumer needs — will be published from this repository.

## The contract

**TW-1 — it says why it exists, and what it does not check.** The header states the invariant, the
incident or risk that motivated it, and a **NOT CHECKED** section naming what a green result does
*not* cover. *Why:* a green tick gets read as covering more than it does unless the gap is written
next to it.

**TW-2 — it carries a reviewed baseline.** The current set of accepted instances lives inside the
script (or a sibling file) as a reviewed list, keyed on stable identity (`file<TAB>item`), not on
line numbers. Adding to the baseline is a reviewed change. A `--list` mode prints what the scan
finds; `UPDATE_BASELINE=1` rewrites the list for review.

**TW-3 — an empty scan fails.** If the scan finds nothing to check — a moved directory, a renamed
pattern — the tripwire fails rather than passing. *Why:* the commonest way a guard dies is by
quietly matching nothing.

**TW-4 — it can be pointed at a fixture.** An override (`SCAN_ROOT`, or similar) lets the script run
against a temporary tree, so it can be tested.

**TW-5 — it has a bite test.** A companion `test-<name>.sh` builds fixture trees and asserts both
directions:
- the violation is caught (non-zero exit, the right message);
- the sanctioned form is not flagged.

*Why:* a passing tripwire is worthless unless something proves it can still fail. This is the
check on the check.

**TW-6 — the set is derived, and CI covers all of it.** One task runs every tripwire by glob
(`audit-*.sh`), not from a hand-kept list. It fails if the glob is empty, or if any tripwire is not
invoked by CI. The bite tests run in their own CI job. *Why:* temper's hand-kept list once fell
from twelve guards to five without anyone noticing.

**TW-7 — it keys on the bug, not on the value's shape.** Detect the dangerous construct (a derive on
a type with a credential field, a call to a runtime-query function), not strings that merely look
suspicious.

## Writing one

1. Write the scan and its `--list` mode. Run it, and review what it finds.
2. Commit the current findings as the baseline, each with its reason ([tenet 4](tenets.md): audit
   before enforce).
3. Write the bite test: a fixture that violates the rule, and one that follows it.
4. Wire it in: the glob picks it up locally. Add its CI invocation, or TW-6 fails.
5. Record the rule in [`rules.md`](rules.md) with the tripwire as its enforcement.

## Portable tripwires (planned)

Generalised from temper. Each will ship from this repository with its bite test:

| Tripwire | Holds | Rule |
|---|---|---|
| credential-debug | credential-bearing types that derive `Debug` are reviewed | SEC-CRED-2 |
| sqlx-exceptions | runtime-checked queries are registered, with reasons | RUST-SQL-1 |
| unsafe-exceptions | crates without `forbid(unsafe_code)` and `expect(unsafe_code)` sites are registered | RUST-UNSAFE-2 |
| ignored-tests | every `#[ignore]` is registered with a reason | TEST-1 |
| action-pins | every external `uses:` is SHA-pinned | SC-GHA-1 |
| workflow-permissions | every workflow declares top-level `permissions:` | SC-GHA-2 |
| migration-order | a new migration sorts after the base branch's newest | — |
| production-features | test-only features are absent from shipped binaries | SEC-TEST-1 |
| channel-literals | no literal channel capacity outside tests | CONC-BOUND-2 |
| merge-group-trigger | every workflow producing a required check triggers on `merge_group` | CI-5 |
