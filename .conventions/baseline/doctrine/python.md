# Python

Applies to every hand-written Python file in a tasker-systems repository: published packages
(tasker-core's `tasker-py` wheel, temper's `temper-py` SDK), their tests, and repository scripts
(CI gate scripts, tooling). Generated Python follows [PY-GEN-1](#generated-code). Canonical config
is [`../python/ruff.toml`](../python/ruff.toml); enforcement status per repo is in
[`rules.md`](rules.md). What was taken from, and rejected from, Google's Python style guide is in
[`../sources/google/python.md`](../sources/google/python.md).

Most of Google's guide is formatting and naming that `ruff format` and ruff's rule set settle
mechanically. The rules here are the ones that need saying: what is gated, what the gate covers,
and the few defects a formatter does not see.

## Tooling

**PY-LINT-1 — every Python file is under `ruff check` and `ruff format --check` in CI.** The config
sits at the repository root, so a script outside any package is still covered. *Why:* a config that
only one package's directory sees leaves the rest unlinted. temper's CI gate scripts and
tasker-core's `tools/` are outside every ruff config today, and tasker-core's CI does not run ruff
at all — its `cargo make check-python` passes but only runs on a developer's machine.

**PY-LINT-2 — one canonical rule selection.** Consumers adopt
[`python/ruff.toml`](../python/ruff.toml) and may add rules, not remove them. Line length is 100
and belongs to the formatter. *Why:* three repositories had picked three overlapping selections;
one table is one stance, as [RUST-LINT-1](rust.md#lints) is for clippy.

**PY-LINT-3 — a suppression names its code and its reason, and an unused one fails.** `# noqa:
BLE001 — boundary converts to a step result`, never a bare `# noqa`; `# type: ignore[attr-defined]`
with a reason, never a bare `# type: ignore`. `PGH003`, `PGH004` and `RUF100` enforce the shape;
mypy's `warn_unused_ignores` and `enable_error_code = ["ignore-without-code"]` enforce it for type
ignores. *Why:* [tenet 6](tenets.md) — a suppression is a reviewed exception. An unused one is
worse than none: it reads as a known problem that no longer exists.

## Types

**PY-TYPE-1 — published packages pass `mypy --strict` in CI.** Hand-written code in a package
published to PyPI is checked with `strict = true`. Tests may relax only the untyped-definition
checks. *Why:* type hints are a promise to callers, and a checker that does not run in CI promises
nothing.

**PY-TYPE-2 — a native module's stub is checked against the module.** The `.pyi` for a compiled
extension (tasker-py's `_tasker_core.pyi`) is generated from the Rust bindings or verified against
the built module (`python -m mypy.stubtest`) in CI. *Why:* [tenet 5](tenets.md). `mypy --strict`
checks every caller against the stub; a stub that drifts from the `#[pyfunction]`s makes the type
check confidently wrong.

## Errors and defaults

**PY-ERR-1 — no bare `except`; a broad `except` only at a declared boundary.** `except Exception`
appears only where the code converts any failure into a result or a logged, re-raised error — a
handler-dispatch loop, an FFI entry point — and carries `# noqa: BLE001` with that reason. Inside an
`except` block, a new exception is raised `from` the original. `E722`, `BLE001` and `B904` enforce
it. *Why:* [tenets 3 and 8](tenets.md). A broad catch elsewhere swallows the bug it was never meant
to see, and an unchained raise throws away the cause.

**PY-ERR-2 — `assert` is not validation.** Library code checks inputs with an explicit `raise`;
`assert` is for tests and for internal invariants that may vanish. `S101` enforces it outside tests.
*Why:* `python -O` strips asserts, so a check written as one is not a check in an optimised build.

**PY-DEF-1 — no shared mutable defaults.** No mutable default argument (`B006`, `B008`), no mutable
class-level default (`RUF012`), no closure capturing a loop variable (`B023`), and a `global`
statement only with a reason (`PLW0603`). *Why:* each one is state shared between calls that looks
private to each call — the bug appears on the second call, far from its cause.

## Logging

**PY-LOG-1 — a log message is a constant; data goes in fields.** `logger.info("step done",
extra={...})` or tasker-py's `log_info("step done", {"step_uuid": ...})`, never an f-string message.
Ruff's `G004` and `LOG` rules enforce it for the standard library; tasker-py's FFI helpers need a
tripwire, because ruff cannot see a custom logging function. *Why:* a collector groups by the
message template, and redaction works per field ([SEC-RED-2](security.md#redaction)). An
interpolated message is a new template every time, and whatever was interpolated into it is beyond
the reach of redaction.

## Generated code

**PY-GEN-1 — generated Python is drift-gated, not linted.** A generated tree (temper-py's
`temper/generated/`, tasker-ctl's codegen output) is excluded from ruff and from mypy's strict
checks. It is regenerated in CI and compared ([CI-3](testing.md)), and never edited by hand. mypy
follows it with `follow_imports = "silent"`, never `"skip"`. *Why:* linting generated code invites a
hand edit the next generation erases. `skip` turns every generated type into `Any`, so the
hand-written layer on top is no longer type-checked against it.

## Versions

**PY-VER-1 — one Python floor, stated once.** `requires-python` names a supported CPython, and
ruff's `target-version` and mypy's `python_version` equal that floor. *Why:* three settings that
must agree, and drift between them means the linter rewrites code into syntax the floor cannot run
(or holds back syntax it can).

The floor is **3.11**, the value of `target-version` in [`../python/ruff.toml`](../python/ruff.toml).
*Why 3.11:* 3.10 reaches end of life in October 2026, so a 3.10 floor promises support CPython no
longer gives. And 3.11 is the first release with `tomllib`. The baseline's own tools read TOML,
must stay standard-library only, and run in every consumer's CI, so they share the floor rather
than carrying an exception to it.

## Not specified

- **Layout** — indentation, wrapping, quotes, blank lines, trailing commas: `ruff format` decides.
  Google's 80 columns is rejected; all three repositories use 100.
- **Import style beyond sorting** — Google's "import modules, not names" is a monorepo-refactoring
  convenience; importing names is idiomatic for typed and pydantic code.
- **Docstrings.** Google-convention docstrings (`Args:` / `Returns:` / `Raises:`) are the house
  style where docstrings are written, and tasker-py follows it; whether to gate the public API of
  published packages with ruff's `D` rules (`convention = "google"`) is open. temper-py would have
  27 findings, tasker-py about 33.
- **Exception naming.** `N818` (exception names end in `Error`) is off: the SDKs keep class names
  identical across languages (temper-py's `NotFound` is temper-rb's `NotFound`).
- **Review judgment**, not rules: properties vs attributes, `staticmethod`, nested functions,
  decorators, function length, comprehension complexity, `from __future__ import annotations`, TODO
  format, pytest style.
- **Bounded queues in Python.** [CONC-BOUND-1](concurrency.md) is written for Rust channels. Whether
  `queue.Queue()` and `asyncio.Queue()` without `maxsize` fall under it has not been inventoried.
