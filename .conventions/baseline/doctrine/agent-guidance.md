# Agent guidance

How a consuming repository's guidance for AI agents is shaped. Agents read it on every session, so
it is the most-read documentation a repository has — and the one that rots fastest.

**AG-1 — the entry file is a router.** `CLAUDE.md` / `AGENTS.md` is short. It points to one file per
topic (architecture, build and test, conventions, key invariants, code quality, environment) and
holds little content itself. *Why:* a monolithic entry file goes stale in places nobody notices; a
topic file has an owner and a scope.

**AG-2 — a rule states its reason.** Every invariant in the guidance says why it exists, briefly.
*Why:* an agent follows a bare rule to the letter and breaks its intent the first time the letter
does not fit.

**AG-3 — a rule states its enforcement.** Each rule says whether a gate enforces it (and which) or
whether it is convention only. Guidance must not claim enforcement that does not exist, and must not
contradict what CI does. *Why:* tasker-core's guidance said "never use `SQLX_OFFLINE=true`" while its
CI set it — an agent trusting the guidance was misled about what CI checks.

**AG-4 — facts are derived or checked.** Lists that mirror the code — crates, commands, states,
feature flags, generated references — are generated, or checked against the code by a drift gate.
*Why:* tasker-core's guidance listed twelve crates when the workspace had fifteen.

**AG-5 — the shared baseline arrives as a projection.** The parts of this repository's doctrine that
apply in a consumer are projected into that consumer as an agent skill, and a drift check keeps the
projection identical to the pinned baseline. A consumer's own guidance links to it rather than
restating it. (Planned; not yet built.)

**AG-6 — machine-local hooks point at real paths.** A session-start hook or setup script that the
guidance relies on is checked to exist at the path configured. *Why:* tasker-core's SessionStart
hook pointed at `bin/setup-claude-web.sh` while the script lived in `tools/bin/`, so cloud sessions
started without their setup and nothing reported it.
