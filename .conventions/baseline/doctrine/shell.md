# Shell

Shell scripts are load-bearing here: most [tripwires](tripwires.md), drift checks and CI gates are
bash. A gate script that mis-handles a failure is a gate that passes when it should not, so these
rules are mostly about failing correctly. Enforcement status per repo is in [`rules.md`](rules.md);
the Google Shell style guide ledger is [`../sources/google/shell.md`](../sources/google/shell.md).

**SH-1 — executables are bash, started with `#!/usr/bin/env bash`.** `sh` only where a platform
forces it (Vercel build hooks), with a comment saying so; no zsh. A sourced library is named `.sh`,
is not executable, and has no shebang requirement. *Why:* one dialect that ShellCheck can parse.
`env` finds a modern bash; `/bin/bash` on macOS is 3.2. tasker-core has 29 `#!/bin/bash` scripts,
temper four zsh scripts ShellCheck cannot read.

**SH-2 — strict mode before any work.** `set -euo pipefail` is the first statement after the header.
A script that deliberately drops `-e` — a bite test that collects failures and reports them together
— says so in a comment on the `set` line. Sourced libraries are exempt (they inherit the caller's
mode). *Why:* without `-e` and `pipefail`, a failed step in a gate is skipped and the gate reports
success ([tenets 2 and 3](tenets.md)).

**SH-3 — ShellCheck gates in CI.** Every tracked shell script — a derived set, failing if empty
([TW-3, TW-6](tripwires.md)) — passes `shellcheck -S warning`, then ratchets to `-S info` from a
reviewed baseline ([tenet 4](tenets.md)). An inline `# shellcheck disable=SCxxxx` carries its reason
on the same line. *Why:* ShellCheck is the only static analysis bash has, and a pre-commit hook can
be skipped. temper runs it only in its pre-commit hook at `-S error`; tasker-core has a
`-S warning` pass that no workflow calls.

**SH-4 — `$(…)`, never backticks.** ShellCheck's SC2006 is style severity, so it is enabled
explicitly (`--include=SC2006`) on top of the `-S warning` pass. *Why:* backticks nest badly and
execute inside double quotes. temper's `check-cli-reference-drift.sh` writes `` `--help` `` inside a
double-quoted error message, so its own failure path runs `--help` as a command and prints "command
not found" in place of the explanation.

**SH-5 — commands are arrays, not strings.** Build an argument list as an array and run it as
`"${cmd[@]}"`. No `eval` of a constructed string. The one allowed form is
`eval "$(trusted-tool --print-env)"` — a tool's environment exports — with a comment. *Why:* a
command held in a string splits on spaces and globs, which is how both the bugs and the suppressions
start: temper's drift gates keep commands in strings and run `eval $BUILD_CMD`, which is behind most
of their 18 `SC2086` suppressions.

**SH-6 — portable between GNU and BSD userlands.** No unguarded `sed -i`, `readlink -f`, `date -d`,
`grep -P` or `mapfile`, and no bash 4 features, unless the script checks for them. *Why:* gates run
on developers' Macs as well as Linux CI, and a gate that breaks on a Mac is a gate people learn to
skip. In temper, `sed -i` failed five probes of a bite test on macOS and aborted another under
`set -e`, unnoticed because CI runs only on Linux; the pre-commit hook now runs staged harnesses
locally to catch it. Google's guide is silent here.

**SH-7 — shell orchestrates; logic lives elsewhere.** When a script needs data structures, parsing,
or an embedded interpreter in a heredoc, that part moves to a real program — a `python3 -I`
script, a `jq` filter, or a Rust tool — which shell then calls. *Why:* Google caps shell at about
100 lines; we keep the intent, not the number (a [TW-1](tripwires.md) header alone can run to 40
lines). The real risk is untested logic hidden in a heredoc. Judged in review.

## Not specified

- `[[ ]]` vs `[ ]`, `local`, `readonly`, a `main` function, function header comments, naming: Google
  states them; they prevent little that quoting rules and ShellCheck do not already catch.
- Formatting (`shfmt`, 2-space indent, 80 columns): would reformat every script without fixing a
  bug class. Deferred.
