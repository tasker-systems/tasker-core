# Consuming the baseline

How a repository answers to this baseline. Each consumer is pinned to one revision. It carries a
projection of the baseline at that revision, and a checker run inside its required CI check says
whether the repository still matches. Enforcement status per repo is in [`rules.md`](rules.md).
The tool is [`../tools/conventions.py`](../tools/conventions.py).

## What a consumer carries

```text
.conventions/
  consumer.toml     the pin, the adoption map, and where agent guidance lives
  status.toml       this repository's status for each rule, in the register's vocabulary
  exceptions.toml   every local departure from the baseline, each with its reason
  baseline/         the projection: the baseline's files at the pin. Written only by `bump`
```

The projected agent skill is written to the path `consumer.toml` names, for example
`.claude/skills/tasker-systems-conventions/`.

To adopt or move the pin:

```bash
python3 -I path/to/conventions/tools/conventions.py bump --init <name> --revision main  # first time
python3 -I .conventions/baseline/tools/conventions.py bump --revision <sha|main>       # thereafter
```

In CI, inside the required aggregate check:

```bash
python3 -I .conventions/baseline/tools/conventions.py check
python3 -I .conventions/baseline/tools/conventions.py witness
```

`witness` is the checks' bite test inside the consumer ([TW-5](tripwires.md)). It seeds one
violation per case into a copy of the consumer, and requires `check` to refuse each one by name.
The cases are generated from the consumer's own adoption map and statuses, so every consumer runs
the same witness against its own files. It also requires the unseeded copy to pass online with no
credential (CONS-8), and requires every workflow job that runs `check` to reference no secret.

## Rules

**CONS-1 — the pin is a commit, read from the consumer.** `consumer.toml` names one full
40-character commit SHA on the baseline's `main`. *Why:* a branch or tag can move underneath the
consumer ([tenet 10](tenets.md)). And "which baseline does this repository answer to" must be
answerable by reading the repository alone, without asking anyone.

**CONS-2 — the projection is never hand-edited.** `.conventions/baseline/` holds exactly the
baseline's files at the pin, and CI compares it with the pinned commit. Only `bump` writes it. A
failure to fetch the baseline fails the check; it never passes it. *Why:* the projection is what
the checker, the skill and every `extends` read. If it could be edited in place, the baseline could
change a consumer's verdict without a change to the pin, or a consumer could edit its own verdict.

**CONS-3 — adopted config matches the projection, apart from recorded exceptions.** Each adopted
canonical file is compared in one of four modes, over parsed content rather than bytes. Comments
and layout stay the consumer's own.

| Mode | Comparison |
|---|---|
| `exact` | the file equals the canonical one |
| `extends` | the file references the projected file, and only its own overrides of canonical keys count |
| `table` | one TOML table, such as `[workspace.lints]` or a package's `[lints]`, equals the canonical one |
| `superset` | every canonical key holds the canonical value; repo-local content is allowed only under the local slots the baseline declares in [`../tools/slots.toml`](../tools/slots.toml) |

*Why:* "the same in every repository" is checked, not remembered. Slots are declared by the
baseline so that a consumer cannot make its own divergence legal.

**CONS-4 — an exception is local, reasoned and alive.** Every departure from the projection is an
entry in `.conventions/exceptions.toml`, giving the file, the key path and the reason. An entry
without a reason fails the check, and so does one that matches no difference. *Why:* this is
[tenet 6](tenets.md), a suppression as a reviewed exception, applied to the baseline itself. A dead
exception reads as a known divergence that no longer exists, the same failure `RUF100` and
`#[expect]` catch.

**CONS-5 — statuses live in the consumer, and a gate named is a gate that exists.**
`.conventions/status.toml` records this repository's status for each rule it has examined, in the
register's vocabulary. Every status in it must name a rule in the pinned baseline. Every
`gated: <path>[#job]` must name a file, and optionally a workflow job, that exists. When a pin
move changes a rule, the author records the rule's status in the same commit, and sets the file's
`reviewed_at` to the new pin: check F fails while `reviewed_at` is not the pin. `bump` sets it
itself when the move adds, removes or changes no rule. The register's consumer columns are
generated from these files by [`../tools/register.py`](../tools/register.py), and a scheduled job
in the baseline fails, naming each cell, where the register and a consumer disagree; the consumer
wins. *Why:* the status is written beside the gate it describes, where review sees both, and a pin
move is one reviewed act in one repository. The register is derived from what the consumers say,
not kept by hand ([tenet 5](tenets.md)).

*Not checked:* that a named gate actually fails on a violation. That is the bite test the gate
itself owes ([TW-5](tripwires.md)).

**CONS-6 — agents receive the projection, and the entry guidance points at it.** The skill is
rendered from the projection and this consumer's `status.toml`, and CI fails if the committed copy
differs. Every agent entry file named in `consumer.toml` (`CLAUDE.md`, `AGENTS.md`) links the
skill. *Why:* an agent in any consumer is told the same doctrine for the same pin, beside this
repository's real enforcement status. That makes a claim of enforcement that no gate backs visible
as false.

*Judged, not checked:* that the consumer's own guidance does not restate or contradict the
doctrine. A restated rule drifts from its source, so guidance links to the doctrine instead of
paraphrasing it.

**CONS-7 — lag is reported on every run, and never fails.** Each check run states how many commits
and days the pin trails the baseline's `main`. *Why:* being behind is allowed, but being behind
without knowing it is not, and nobody should have to go and look.

**CONS-8 — the checks need nothing an outsider lacks.** They run inside the consumer's required
aggregate check ([CI-2](testing.md)), on every change including fork pull requests and merge-queue
runs. They use no secret: the baseline is public, and the only network read is the comparison in
CONS-2. *Why:* a public repository's CI must not pass or fail depending on whether the person who
triggered it can see something private.

## Not specified

- **How often to move the pin.** CONS-7 makes lag visible but sets no maximum.
- **Versioning the baseline beyond commits.** Tags, if ever added, are labels. The pin is always a
  SHA.
- **Template and example code a consumer ships for users to copy** (tasker-contrib). It is
  user-owned and never in a consumer's adoption map.
