# conventions

The shared engineering baseline for tasker-systems repositories. It is public, and five
repositories consume it:

| Consumer | Visibility |
|---|---|
| [`temper`](https://github.com/tasker-systems/temper) | public |
| [`tasker-core`](https://github.com/tasker-systems/tasker-core) | public |
| [`temper-contrib`](https://github.com/tasker-systems/temper-contrib) | public |
| [`tasker-contrib`](https://github.com/tasker-systems/tasker-contrib) | public (its own surfaces; templates and example apps are user-owned and out of scope) |
| `storyteller` | private |

The baseline began from the first two. They grew up separately and are each strong where the other
is thin. tasker-core carries the code-level doctrine (written tenets, a phased lint table, bounded
concurrency, the distributed-systems primitives); temper carries the enforcement machinery
(bite-tested tripwires, blocking supply-chain gates, drift checks on everything generated). This
repository is where the two halves meet, so that a rule written once is enforced everywhere it
claims to be.

## The one idea

**A rule that is written down is either enforced by a named gate, or says why it is not.** Every
rule here has an id, and [`doctrine/rules.md`](doctrine/rules.md) records, per consuming repo,
which gate enforces it — or that it is convention-only and why. A rule whose status is unknown is a
hole, not a default.

## Layout

```
doctrine/        the stance, in prose — why each rule exists
  tenets.md        the short list everything else follows from
  rules.md         the rule register: id, statement, enforcement per repo
  rust.md          lints, formatting, API shape, unsafe, errors, panics, toolchain
  python.md        ruff, mypy, errors, logging, generated code
  typescript.md    Biome, tsconfig, types, suppressions, generated code
  shell.md         bash, strict mode, ShellCheck, portability
  docs.md          Markdown shape and links
  api.md           JSON on the wire and in the OpenAPI spec
  supply-chain.md  dependencies, advisories, licenses, Actions, toolchain inputs
  security.md      credentials, redaction, SQL, auth posture
  concurrency.md   bounded resources, contained failure, coordination in the database
  testing.md       test tiers, ignored tests, CI shape
  tripwires.md     how a repo-specific guard is written, tested and wired
  agent-guidance.md how a consuming repo's agent guidance is shaped
  consuming.md     how a repo pins, projects and checks the baseline (CONS-*)
rust/            canonical config a consuming repo adopts
  workspace-lints.toml   the [workspace.lints] table
  clippy.toml            disallowed methods and thresholds
  deny.toml              cargo-deny: sources, licenses, bans
  audit.toml             the shape of a reviewed advisory-ignore list
python/
  ruff.toml              the canonical ruff selection
typescript/
  biome.json             the shared Biome rule set
  tsconfig.base.json     the strict compiler base
skill/           the agent skill projected into each consumer, with that consumer's statuses
tools/
  conventions.py         `check` (A–F) and `bump`, run by consumers from their projection
  register.py            generates rules.md's consumer columns from consumers' status.toml
  consumers.toml         the registered consumers that register.py reads
  slots.toml             where a consumer may hold repo-local content in a `superset` file
  check-config-valid.py  every canonical config parses and still bites
  check-register.py      every doctrine rule has exactly one register row
  check-links.py         every relative link resolves
  tests/                 bite tests for all of the above
sources/         ledgers of external guides read against the baseline — what was adopted,
                 adapted or rejected, and why (Google's style guides, 2026-10-23)
```

Still to come, each its own change: the tripwire harness and the portable tripwires, and reusable
GitHub workflows for the stage-1 gate.

## Adopting

A consumer pins one revision of this repository and carries a projection of it under
`.conventions/`. A checker, run from that projection inside the consumer's required CI check, fails
when the consumer's adopted config, projection or skill departs from the pin without a recorded
exception, and reports how far the pin lags `main`. The contract, the four comparison modes and the
commands are in [`doctrine/consuming.md`](doctrine/consuming.md). "The same in every repo" is then a
checked invariant, not a habit.

## Scope

Rust was v0, and the Rust baseline is still the most complete. Python, TypeScript, shell, Markdown
and HTTP API conventions were added on 2026-10-23, from a reading of Google's style guides against
what temper and tasker-core actually do ([`sources/`](sources/)). Each of those rules carries a
status like any other; many are `uncovered` or `partial` today, which is the point of writing them
down. Ruby has no external guide behind it yet and no doctrine file. Domain-specific guards
(temper's authorization sinks, tasker-core's state-machine rules) stay in their own repositories;
this repository owns the *shape* of a guard, not every guard.

The register's statuses were taken against temper and tasker-core. temper-contrib, tasker-contrib
and storyteller are registered consumers whose statuses are unexamined until each adopts the
baseline (see [`doctrine/rules.md`](doctrine/rules.md)).

The goal this repository serves, its clauses and their coverage, live in the `+tasker-systems/conventions`
context in Temper.
