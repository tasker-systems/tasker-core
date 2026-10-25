# Supply chain

Everything a build pulls in from outside the repository. Canonical config:
[`rust/deny.toml`](../rust/deny.toml), [`rust/audit.toml`](../rust/audit.toml).

## Advisories

**SC-ADV-1 — `cargo audit` gates.** It runs on every pull request and fails the build on any
advisory not in the reviewed ignore list. No `continue-on-error`, no `|| true`. *Why:* an advisory
gate that cannot fail reports nothing; a change that pulls in a vulnerable crate passes unseen.

**SC-ADV-2 — every ignore is reviewed and reasoned.** `.cargo/audit.toml` is the only escape hatch.
Each id carries a reason, its reachability (what `cargo tree -i <crate>` shows), and a status:
`UNFIXABLE`, or `FIX-PENDING` (a fix exists, adopting it is its own change). When a `FIX-PENDING`
dependency is bumped, its entry is deleted in the same change. Warnings (unmaintained, yanked,
unsound) are not ignored there; they stay visible without gating.

## Sources, licenses and bans

**SC-DENY-1 — only crates.io.** `cargo deny check sources` rejects any dependency from another
registry or a git repository until it is allowed by name. *Why:* an unreviewed source is an
unreviewed supplier.

**SC-DENY-2 — licenses from an allowlist.** `cargo deny check licenses` requires every crate's
license expression to be satisfiable from a shared, permissive allowlist. A new license, above all
a copyleft one, is a decision before it is added.

**SC-DENY-3 — bans.** `cargo deny check bans` runs and gates on:
- wildcard (`*`) version requirements;
- an explicit list of crates no consumer should ever depend on.

Duplicate versions (`multiple-versions`) are reported as warnings only. *Why:* duplicates are
pervasive in temper's and tasker-core's trees today, and gating on them would block every dependency
bump. A warning keeps them visible; promoting `multiple-versions` to `deny`, with a reviewed `skip`
list as the ratchet, is a later ruling.

## GitHub Actions

**SC-GHA-1 — every Action is pinned to a commit.** Every external `uses:` reference is a full
40-character commit SHA with the tag it corresponds to as a trailing comment:
`uses: actions/checkout@<sha> # v6`. Local `./` actions are exempt. Branch and tag references
(`@main`, `@v6`, `@release/v1`, `@stable`) are refused. *Why:* a tag or branch can be moved to new
code by whoever controls the upstream repository, and a workflow runs that code with the repo's
token. Dependabot keeps the pins current.

**SC-GHA-2 — workflows declare least privilege.** Every workflow sets a top-level `permissions:`,
by default `contents: read`. A job that needs more declares it on the job, with a comment saying
why. *Why:* the default token is broad; an Action compromised through any of the above inherits
whatever the job holds.

**SC-GHA-3 — the release chain's triggers are pinned.** The workflows that sign or publish run only
on their declared triggers, and adding a trigger to them fails a check. *Why:* a new trigger is a
new way to reach the signing identity.

## Other inputs

**SC-IN-1 — tool installs are pinned or cooled.** Tools installed in CI or by a version manager are
pinned to a version, and new releases are held behind a minimum age (temper's `mise` sets
`minimum_release_age = "7d"`). *Why:* a freshly published release is the window a hijacked package
uses.

*Security releases are the exception to the cooldown.* When a security release lands inside the
release-age window, a fuzzy pin still resolves to the vulnerable version. In that case:
- pin the fixed version **exactly**, since exact pins bypass the window;
- write a comment beside the pin saying when to return it to the fuzzy pin;
- pin the same exact version in every CI setup step (`setup-node`, `setup-python`, toolchain
  actions). A fuzzy CI version such as `node-version: "24"` can be satisfied by an older release
  already in the runner's tool cache.

(temper, Node 24.22.1, 2026-10-22.)

**SC-IN-2 — every ecosystem present has dependency updates.** Dependabot covers every lockfile in
the repo — cargo, Actions, and each language and container ecosystem — grouped weekly. *Why:*
pinning without updates trades a supply-chain risk for an unpatched one.

**SC-IN-3 — lockfiles match manifests.** CI checks that each committed lockfile is the one its
manifests resolve to (`cargo metadata --locked` and the equivalents per ecosystem).

## Secrets in the tree

**SC-SEC-1 — secret scanning on every change.** gitleaks runs in CI on every pull request
(unconditionally, including docs-only changes) and in the pre-commit hook, with a reviewed allowlist.
Static analysis (CodeQL) runs per pull request and on a weekly schedule.
