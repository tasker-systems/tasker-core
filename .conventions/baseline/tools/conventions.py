#!/usr/bin/env python3
"""Check and move a consuming repository's pin on the tasker-systems conventions baseline.

Doctrine: doctrine/consuming.md (CONS-*). Run from the consumer's *projected* copy, never from the
baseline's main, so that the verdict a consumer gets is the one its pin chose:

    python3 -I .conventions/baseline/tools/conventions.py check [--offline]
    python3 -I .conventions/baseline/tools/conventions.py bump --revision <sha|main> [--init --name N]
    python3 -I .conventions/baseline/tools/conventions.py witness

`check` runs six checks and exits non-zero if any fails:

  A  adopted config matches the projection, apart from recorded exceptions   (CONS-3, CONS-4)
  B  the projection matches the pinned commit, byte for byte                 (CONS-1, CONS-2)
  C  lag behind the baseline's main is reported; it never fails              (CONS-7)
  D  every `gated:` status names a gate that exists                          (CONS-5)
  E  the agent entry files link the projected skill, and the skill is current (CONS-6)
  F  the statuses were reviewed against the pinned revision                   (CONS-5)

B and C fetch the baseline (it is public; no credential is used or needed). A fetch failure fails
B: the projection being hand-edited is exactly what B guards, so it never passes unchecked.
`--offline` runs A, D, E and F only, and says so.

`witness` is the checks' bite test in a consumer (TW-5). It seeds one violation per case into a copy
of the consumer and requires `check` to refuse it, naming what was seeded. The cases are built from
the consumer's own `consumer.toml` and `status.toml`: an override and an addition (or a dropped
reference) for each adopted file, the exception rules, the pin, the skill and each entry file, the
statuses, `reviewed_at`, and lag. The unseeded copy runs first, online, in an environment with no
token, credential helper or git configuration: that is the control, and the witness that the check
needs nothing an outsider lacks. Every workflow job that runs `check` must reference no secret. A
seed that finds nothing to edit fails rather than passing vacuously.

NOT CHECKED, by design:
  - that a `gated:` gate actually fails on a violation (that is each consumer's bite test, TW-5);
  - that a consumer's own guidance does not restate or contradict the doctrine (judged in review);
  - formatting of adopted files: comparison is over parsed TOML/JSON, so comments and layout are
    the consumer's own.

Standard library only; Python 3.11+ (tomllib).
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

DEFAULT_BASELINE = "https://github.com/tasker-systems/conventions"
CONVENTIONS_DIR = ".conventions"
BASELINE_DIR = f"{CONVENTIONS_DIR}/baseline"

# What a consumer's projection holds. Kept here, in the projected tool, so that check B and the
# projection it checks always agree on the set: both come from the same pinned revision.
PROJECTED = (
    "README.md",
    "LICENSE",
    "doctrine",
    "rust",
    "python",
    "typescript",
    "skill",
    "tools/conventions.py",
    "tools/slots.toml",
)

SHA_RE = re.compile(r"^[0-9a-f]{40}$")
RULE_ROW_RE = re.compile(r"^\|\s*([A-Z]+(?:-[A-Z]+)*-[0-9]+)\s*\|(.*)$")
STATUS_WORDS = ("gated", "partial", "practiced", "convention", "uncovered", "unexamined", "n/a")
MODES = ("exact", "extends", "table", "superset")
SKILL_BEGIN = "<!-- consumer-status:begin -->"
SKILL_END = "<!-- consumer-status:end -->"


# --------------------------------------------------------------------------------------------
# Reporting
# --------------------------------------------------------------------------------------------


@dataclass
class Report:
    errors: list[str] = field(default_factory=list)
    notices: list[str] = field(default_factory=list)

    def fail(self, check: str, message: str) -> None:
        self.errors.append(f"{check}: {message}")

    def note(self, check: str, message: str) -> None:
        self.notices.append(f"{check}: {message}")

    def emit(self) -> int:
        in_actions = os.environ.get("GITHUB_ACTIONS") == "true"
        for line in self.notices:
            print(f"::notice::{line}" if in_actions else f"note  {line}")
        for line in self.errors:
            print(f"::error::{line}" if in_actions else f"FAIL  {line}", file=sys.stderr)
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a", encoding="utf-8") as handle:
                handle.write("## Conventions baseline\n\n")
                for line in self.notices:
                    handle.write(f"- {line}\n")
                for line in self.errors:
                    handle.write(f"- **FAIL** {line}\n")
                if not self.errors:
                    handle.write("- all checks passed\n")
        if not self.errors:
            print("conventions: all checks passed")
        return 1 if self.errors else 0


class UsageError(Exception):
    """A malformed consumer: the checks cannot run, so this is a failure, not a skip."""


# --------------------------------------------------------------------------------------------
# Parsing
# --------------------------------------------------------------------------------------------


def strip_jsonc(text: str) -> str:
    """Remove // and /* */ comments and trailing commas outside strings (tsconfig, biome)."""
    out: list[str] = []
    i, n = 0, len(text)
    in_string = False
    while i < n:
        c = text[i]
        if in_string:
            out.append(c)
            if c == "\\" and i + 1 < n:
                out.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                in_string = False
            i += 1
        elif c == '"':
            in_string = True
            out.append(c)
            i += 1
        elif text.startswith("//", i):
            while i < n and text[i] != "\n":
                i += 1
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = n if end == -1 else end + 2
        else:
            out.append(c)
            i += 1
    return re.sub(r",(\s*[}\]])", r"\1", "".join(out))


def load_structured(path: Path) -> dict[str, Any] | None:
    """Parse a TOML or JSON(C) file; None for any other format (compared as bytes)."""
    if path.suffix == ".toml":
        with path.open("rb") as handle:
            return tomllib.load(handle)
    if path.suffix == ".json":
        loaded = json.loads(strip_jsonc(path.read_text(encoding="utf-8")))
        if not isinstance(loaded, dict):
            raise UsageError(f"{path}: top level is not an object")
        return loaded
    return None


def flatten(value: Any, prefix: str = "") -> dict[str, Any]:
    """Tables become dotted key paths; arrays and scalars are leaves."""
    if isinstance(value, dict) and value:
        flat: dict[str, Any] = {}
        for key, child in value.items():
            flat.update(flatten(child, f"{prefix}.{key}" if prefix else str(key)))
        return flat
    return {prefix: value}


def subtable(data: dict[str, Any], dotted: str) -> Any:
    node: Any = data
    for part in dotted.split("."):
        if not isinstance(node, dict) or part not in node:
            return None
        node = node[part]
    return node


def rule_rows(rules_md: Path) -> dict[str, str]:
    """Each rule's id and statement: the register row's first two cells. The per-consumer
    columns after them are generated and move on their own, so they are not part of the rule."""
    rows: dict[str, str] = {}
    for line in rules_md.read_text(encoding="utf-8").splitlines():
        match = RULE_ROW_RE.match(line)
        if match:
            rows[match.group(1)] = re.split(r"(?<!\\)\|", match.group(2), maxsplit=1)[0].strip()
    return rows


# --------------------------------------------------------------------------------------------
# The consumer
# --------------------------------------------------------------------------------------------


@dataclass
class Adopted:
    canonical: str
    path: str
    mode: str
    table: str
    canonical_table: str
    key: str


@dataclass
class Consumer:
    root: Path
    name: str
    revision: str
    baseline: str
    adopted: list[Adopted]
    entries: list[str]
    skill: str

    @property
    def baseline_dir(self) -> Path:
        return self.root / BASELINE_DIR


def load_consumer(root: Path) -> Consumer:
    config_path = root / CONVENTIONS_DIR / "consumer.toml"
    if not config_path.is_file():
        raise UsageError(f"{config_path} not found; this repository has not adopted the baseline")
    with config_path.open("rb") as handle:
        config = tomllib.load(handle)

    name = config.get("name")
    revision = config.get("revision")
    if not isinstance(name, str) or not name:
        raise UsageError("consumer.toml: `name` is required")
    if not isinstance(revision, str) or not SHA_RE.match(revision):
        raise UsageError(
            "consumer.toml: `revision` must be a full 40-character commit SHA; a branch or tag "
            f"can move underneath the consumer (got {revision!r})"
        )

    adopted: list[Adopted] = []
    for index, entry in enumerate(config.get("adopted", [])):
        where = f"consumer.toml adopted[{index}]"
        mode = entry.get("mode")
        if mode not in MODES:
            raise UsageError(f"{where}: `mode` must be one of {', '.join(MODES)}")
        for required in ("canonical", "path"):
            if not isinstance(entry.get(required), str):
                raise UsageError(f"{where}: `{required}` is required")
        if mode == "table" and not isinstance(entry.get("table"), str):
            raise UsageError(f"{where}: `table` mode needs `table` (the consumer's table path)")
        adopted.append(
            Adopted(
                canonical=entry["canonical"],
                path=entry["path"],
                mode=mode,
                table=entry.get("table", ""),
                canonical_table=entry.get("canonical_table", ""),
                key=entry.get("key", ""),
            )
        )

    guidance = config.get("guidance", {})
    entries = guidance.get("entry", [])
    skill = guidance.get("skill", "")
    if not entries or not isinstance(skill, str) or not skill:
        raise UsageError("consumer.toml: [guidance] needs `entry` (files) and `skill` (a path)")

    return Consumer(
        root=root,
        name=name,
        revision=revision,
        baseline=config.get("baseline", DEFAULT_BASELINE),
        adopted=adopted,
        entries=list(entries),
        skill=skill,
    )


@dataclass
class LocalException:
    path: str
    key: str
    reason: str
    used: bool = False

    def covers(self, path: str, key: str) -> bool:
        return self.path == path and (key == self.key or key.startswith(self.key + "."))


def load_exceptions(consumer: Consumer, report: Report) -> list[LocalException]:
    path = consumer.root / CONVENTIONS_DIR / "exceptions.toml"
    if not path.is_file():
        return []
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    out: list[LocalException] = []
    for index, entry in enumerate(data.get("exception", [])):
        reason = entry.get("reason", "")
        if not isinstance(reason, str) or not reason.strip():
            report.fail("A", f"exceptions.toml exception[{index}] has no reason")
        if not isinstance(entry.get("path"), str) or not isinstance(entry.get("key"), str):
            report.fail("A", f"exceptions.toml exception[{index}] needs `path` and `key`")
            continue
        out.append(LocalException(entry["path"], entry["key"], str(reason)))
    return out


def load_statuses(consumer: Consumer) -> dict[str, str]:
    path = consumer.root / CONVENTIONS_DIR / "status.toml"
    if not path.is_file():
        return {}
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    rules = data.get("rules", {})
    return {str(k): str(v) for k, v in rules.items()}


def load_reviewed_at(root: Path) -> str | None:
    path = root / CONVENTIONS_DIR / "status.toml"
    if not path.is_file():
        return None
    with path.open("rb") as handle:
        value = tomllib.load(handle).get("reviewed_at")
    return value if isinstance(value, str) else None


def set_reviewed_at(root: Path, sha: str) -> None:
    """Write `reviewed_at` above the first table of status.toml, replacing any earlier value."""
    path = root / CONVENTIONS_DIR / "status.toml"
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    line = f'reviewed_at = "{sha}"\n'
    for index, existing in enumerate(lines):
        if re.match(r"^reviewed_at\s*=", existing):
            lines[index] = line
            break
    else:
        first_table = next((i for i, x in enumerate(lines) if x.startswith("[")), len(lines))
        lines.insert(first_table, line)
    path.write_text("".join(lines), encoding="utf-8")


# --------------------------------------------------------------------------------------------
# Check A — adopted config matches the projection, modulo exceptions
# --------------------------------------------------------------------------------------------


def local_slots(consumer: Consumer, canonical: str) -> list[str]:
    path = consumer.baseline_dir / "tools" / "slots.toml"
    if not path.is_file():
        return []
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    return list(data.get(canonical, {}).get("slots", []))


def differences(adopted: Adopted, consumer: Consumer) -> list[str]:
    canonical_path = consumer.baseline_dir / adopted.canonical
    local_path = consumer.root / adopted.path
    if not canonical_path.is_file():
        return [f"(canonical file {adopted.canonical} is not in the projection)"]
    if not local_path.is_file():
        return ["(file missing)"]

    canon = load_structured(canonical_path)
    local = load_structured(local_path)
    if canon is None or local is None:
        if adopted.mode != "exact":
            raise UsageError(f"{adopted.path}: `{adopted.mode}` mode needs a TOML or JSON file")
        same = canonical_path.read_bytes() == local_path.read_bytes()
        return [] if same else ["(content differs)"]

    if adopted.mode == "table":
        canon = subtable(canon, adopted.canonical_table) if adopted.canonical_table else canon
        local = subtable(local, adopted.table)
        if local is None:
            return [f"{adopted.table} (table missing)"]
        prefix = adopted.table
    else:
        prefix = ""

    if adopted.mode == "extends":
        key = adopted.key or ("extend" if adopted.path.endswith(".toml") else "extends")
        refs = local.get(key)
        refs = [refs] if isinstance(refs, str) else refs or []
        target = canonical_path.resolve()
        base = local_path.parent
        if not any((base / ref).resolve() == target for ref in refs if isinstance(ref, str)):
            return [f"{key} (does not reference {BASELINE_DIR}/{adopted.canonical})"]
        local = {k: v for k, v in local.items() if k != key}

    flat_canon = flatten(canon)
    flat_local = flatten(local)
    slots = local_slots(consumer, adopted.canonical) if adopted.mode == "superset" else []
    found: list[str] = []
    for key, value in flat_canon.items():
        if key not in flat_local:
            if adopted.mode != "extends":
                found.append(_join(prefix, key) + " (missing)")
        elif not _same(flat_local[key], value) and not _in_slot(key, slots):
            found.append(_join(prefix, key) + f" (canonical {value!r}, here {flat_local[key]!r})")
    if adopted.mode in ("exact", "table"):
        for key in flat_local.keys() - flat_canon.keys():
            found.append(_join(prefix, key) + " (not in canonical)")
    if adopted.mode == "superset":
        for key in flat_local.keys() - flat_canon.keys():
            if not _in_slot(key, slots):
                found.append(key + " (not in canonical, and not a declared local slot)")
    return sorted(found)


def _same(left: Any, right: Any) -> bool:
    """Equal values; arrays of scalars compare as sets of items, since neither a lint selection
    nor a license allow-list means anything by its order."""
    scalar = (str, int, float, bool)
    if (
        isinstance(left, list)
        and isinstance(right, list)
        and all(isinstance(item, scalar) for item in [*left, *right])
    ):
        return sorted(map(repr, left)) == sorted(map(repr, right))
    return left == right


def _join(prefix: str, key: str) -> str:
    return f"{prefix}.{key}" if prefix and key else prefix or key


def _in_slot(key: str, slots: list[str]) -> bool:
    return any(key == slot or key.startswith(slot + ".") for slot in slots)


def check_adopted(consumer: Consumer, report: Report) -> None:
    exceptions = load_exceptions(consumer, report)
    for adopted in consumer.adopted:
        for diff in differences(adopted, consumer):
            key = diff.split(" (", 1)[0]
            match = next((e for e in exceptions if e.covers(adopted.path, key)), None)
            if match:
                match.used = True
            else:
                report.fail("A", f"{adopted.path}: {diff} — not a recorded exception")
    for exception in exceptions:
        if not exception.used:
            report.fail(
                "A",
                f"exception {exception.path}:{exception.key} matches no difference; remove it "
                "(a dead exception reads as a known divergence that no longer exists)",
            )


# --------------------------------------------------------------------------------------------
# Checks B and C — the projection against the pinned commit; lag behind main
# --------------------------------------------------------------------------------------------


def git(args: list[str], cwd: Path) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, check=False)


class Fetched:
    """A temporary clone of the baseline's main. Only main is fetched, so a revision that resolves
    here is, by construction, a commit on main: a pin to a side branch cannot be found at all."""

    def __init__(self, url: str) -> None:
        self._dir = tempfile.TemporaryDirectory(prefix="conventions-")
        self.path = Path(self._dir.name)
        steps = (
            ["init", "-q"],
            ["fetch", "-q", "--no-tags", url, "+refs/heads/main:refs/remotes/baseline/main"],
        )
        for step in steps:
            result = git(step, self.path)
            if result.returncode != 0:
                self.close()
                message = result.stderr.decode(errors="replace").strip()
                raise OSError(f"git {step[0]} {url} failed: {message}")

    def resolve(self, revision: str) -> str | None:
        ref = "refs/remotes/baseline/main" if revision == "main" else f"{revision}^{{commit}}"
        result = git(["rev-parse", "--verify", "-q", ref], self.path)
        return result.stdout.decode().strip() or None if result.returncode == 0 else None

    def files(self, revision: str) -> dict[str, bytes]:
        listed = git(["ls-tree", "-r", "-z", "--name-only", revision, "--", *PROJECTED], self.path)
        out: dict[str, bytes] = {}
        for name in filter(None, listed.stdout.decode().split("\0")):
            out[name] = git(["show", f"{revision}:{name}"], self.path).stdout
        return out

    def lag(self, revision: str) -> tuple[int, int]:
        main = "refs/remotes/baseline/main"
        count = git(["rev-list", "--count", f"{revision}..{main}"], self.path).stdout.decode()
        stamps = git(["log", "-1", "--format=%ct", revision], self.path).stdout.decode()
        main_stamp = git(["log", "-1", "--format=%ct", main], self.path).stdout.decode()
        days = (int(main_stamp.strip()) - int(stamps.strip())) // 86400
        return int(count.strip() or 0), max(days, 0)

    def close(self) -> None:
        self._dir.cleanup()


def projected_on_disk(consumer: Consumer) -> dict[str, bytes]:
    base = consumer.baseline_dir
    out: dict[str, bytes] = {}
    if base.is_dir():
        for path in sorted(base.rglob("*")):
            if path.is_file():
                out[path.relative_to(base).as_posix()] = path.read_bytes()
    return out


def check_pin(consumer: Consumer, report: Report, url: str) -> None:
    try:
        fetched = Fetched(url)
    except OSError as error:
        report.fail("B", f"could not fetch the baseline to compare the projection: {error}")
        report.note("C", "lag unknown: the baseline could not be fetched")
        return
    try:
        if not fetched.resolve(consumer.revision):
            report.fail("B", f"revision {consumer.revision} is not a commit on the baseline's main")
            return
        expected = fetched.files(consumer.revision)
        actual = projected_on_disk(consumer)
        for name in sorted(expected.keys() - actual.keys()):
            report.fail("B", f"{BASELINE_DIR}/{name} is missing from the projection")
        for name in sorted(actual.keys() - expected.keys()):
            report.fail("B", f"{BASELINE_DIR}/{name} is not in the baseline at the pin")
        for name in sorted(expected.keys() & actual.keys()):
            if expected[name] != actual[name]:
                report.fail("B", f"{BASELINE_DIR}/{name} differs from the pinned commit")
        behind, days = fetched.lag(consumer.revision)
        if behind:
            report.note(
                "C", f"{consumer.name} is {behind} commit(s), {days} day(s) behind the baseline"
            )
        else:
            report.note("C", f"{consumer.name} is on the baseline's current main")
    finally:
        fetched.close()


# --------------------------------------------------------------------------------------------
# Check D — statuses name rules that exist, and `gated:` names a gate that exists
# --------------------------------------------------------------------------------------------


def gate_exists(root: Path, ref: str) -> bool:
    path_part, _, job = ref.partition("#")
    path = root / path_part.strip()
    if not path_part.strip() or not path.is_file():
        return False
    if not job:
        return True
    text = path.read_text(encoding="utf-8")
    jobs = re.search(r"^jobs:\s*$(.*)", text, re.MULTILINE | re.DOTALL)
    return bool(jobs and re.search(rf"^  {re.escape(job.strip())}:\s*$", jobs.group(1), re.M))


def check_statuses(consumer: Consumer, report: Report) -> None:
    rules_md = consumer.baseline_dir / "doctrine" / "rules.md"
    known = rule_rows(rules_md) if rules_md.is_file() else {}
    for rule_id, status in load_statuses(consumer).items():
        if known and rule_id not in known:
            report.fail("D", f"status.toml: {rule_id} is not a rule in the pinned baseline")
            continue
        word, _, rest = status.partition(":")
        word = word.strip()
        if word not in STATUS_WORDS:
            report.fail("D", f"status.toml: {rule_id} = {status!r} uses an unknown status word")
        elif word == "gated":
            refs = [r.strip() for r in rest.split(",") if r.strip()]
            if not refs:
                report.fail("D", f"status.toml: {rule_id} is `gated` but names no gate")
            for ref in refs:
                if not gate_exists(consumer.root, ref):
                    report.fail(
                        "D", f"status.toml: {rule_id} names gate {ref!r}, which does not exist"
                    )


# --------------------------------------------------------------------------------------------
# Check E — the projected skill is current and the entry guidance links it
# --------------------------------------------------------------------------------------------


def render_skill(
    baseline_dir: Path, consumer_name: str, statuses: dict[str, str]
) -> dict[str, bytes]:
    source = baseline_dir / "skill"
    out: dict[str, bytes] = {}
    if not source.is_dir():
        return out
    rows = ["| Rule | Status here |", "|---|---|"]
    rows += [f"| {rule} | {status} |" for rule, status in sorted(statuses.items())]
    table = "\n".join(rows) if statuses else "_No statuses recorded yet: every rule is unexamined._"
    block = f"{SKILL_BEGIN}\n\nStatuses for **{consumer_name}**:\n\n{table}\n\n{SKILL_END}"
    for path in sorted(source.rglob("*")):
        if not path.is_file():
            continue
        data = path.read_bytes()
        if path.name == "SKILL.md":
            text = data.decode("utf-8")
            pattern = re.compile(re.escape(SKILL_BEGIN) + ".*?" + re.escape(SKILL_END), re.DOTALL)
            text = pattern.sub(lambda _: block, text)
            data = text.encode("utf-8")
        out[path.relative_to(source).as_posix()] = data
    return out


def check_guidance(consumer: Consumer, report: Report) -> None:
    expected = render_skill(consumer.baseline_dir, consumer.name, load_statuses(consumer))
    skill_dir = consumer.root / consumer.skill
    actual: dict[str, bytes] = {}
    if skill_dir.is_dir():
        for path in sorted(skill_dir.rglob("*")):
            if path.is_file():
                actual[path.relative_to(skill_dir).as_posix()] = path.read_bytes()
    if not expected:
        report.fail("E", "the projection has no skill/ to project")
    elif actual != expected:
        report.fail(
            "E",
            f"{consumer.skill} is not the skill rendered from the projection and status.toml; "
            "re-run `conventions.py bump` (statuses changed, or the skill was hand-edited)",
        )
    for entry in consumer.entries:
        path = consumer.root / entry
        if not path.is_file():
            report.fail("E", f"guidance entry {entry} does not exist")
        elif consumer.skill.rstrip("/") not in path.read_text(encoding="utf-8"):
            report.fail("E", f"{entry} does not link the projected skill ({consumer.skill})")


# --------------------------------------------------------------------------------------------
# Check F — the statuses were reviewed against the pinned revision
# --------------------------------------------------------------------------------------------


def check_reviewed(consumer: Consumer, report: Report) -> None:
    reviewed = load_reviewed_at(consumer.root)
    if reviewed is None:
        report.fail(
            "F",
            "status.toml has no `reviewed_at`: name the baseline revision these statuses were "
            "reviewed against (`bump` writes it when the pin moves without changing a rule)",
        )
    elif reviewed != consumer.revision:
        report.fail(
            "F",
            f"status.toml was reviewed at {reviewed}, but the pin is {consumer.revision}: record "
            "the status of each rule `bump` reported added or changed, then set `reviewed_at` to "
            "the pin, in the same commit as the pin move",
        )


# --------------------------------------------------------------------------------------------
# bump
# --------------------------------------------------------------------------------------------


def write_tree(target: Path, files: dict[str, bytes]) -> None:
    if target.exists():
        shutil.rmtree(target)
    for name, data in files.items():
        path = target / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def bump(root: Path, revision: str, url: str | None, init_name: str | None) -> int:
    config_path = root / CONVENTIONS_DIR / "consumer.toml"
    if init_name:
        if config_path.exists():
            print(f"{config_path} already exists; drop --init", file=sys.stderr)
            return 2
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_text(INIT_CONSUMER.format(name=init_name, sha="0" * 40), encoding="utf-8")
        (root / CONVENTIONS_DIR / "status.toml").write_text(INIT_STATUS, encoding="utf-8")
        (root / CONVENTIONS_DIR / "exceptions.toml").write_text(INIT_EXCEPTIONS, encoding="utf-8")
    try:
        consumer = load_consumer(root)
    except UsageError as error:
        print(f"conventions: {error}", file=sys.stderr)
        return 2
    url = url or consumer.baseline
    old_rules_md = consumer.baseline_dir / "doctrine" / "rules.md"
    old_rules = rule_rows(old_rules_md) if old_rules_md.is_file() else {}

    fetched = Fetched(url)
    try:
        sha = fetched.resolve(revision)
        if not sha:
            print(
                f"conventions: {revision} is not a commit on the baseline's main", file=sys.stderr
            )
            return 2
        write_tree(consumer.baseline_dir, fetched.files(sha))
    finally:
        fetched.close()

    text = config_path.read_text(encoding="utf-8")
    text = re.sub(r"^revision\s*=.*$", f'revision = "{sha}"', text, count=1, flags=re.MULTILINE)
    config_path.write_text(text, encoding="utf-8")
    consumer = load_consumer(root)
    write_tree(
        root / consumer.skill,
        render_skill(consumer.baseline_dir, consumer.name, load_statuses(consumer)),
    )

    new_rules = rule_rows(consumer.baseline_dir / "doctrine" / "rules.md")
    print(f"pinned {consumer.name} to {sha}")
    if not old_rules:
        set_reviewed_at(root, sha)
        print(
            f"  first projection: {len(new_rules)} rules. Each is unexamined here until you record"
            " its status in .conventions/status.toml"
        )
        return 0
    moved = False
    for label, ids in (
        ("added", sorted(new_rules.keys() - old_rules.keys())),
        ("removed", sorted(old_rules.keys() - new_rules.keys())),
        (
            "changed",
            sorted(k for k in new_rules.keys() & old_rules.keys() if new_rules[k] != old_rules[k]),
        ),
    ):
        for rule in ids:
            moved = True
            print(f"  rule {label}: {rule}")
    if not moved:
        set_reviewed_at(root, sha)
        print("  no rule was added, removed or changed; status.toml is reviewed at the new pin")
        return 0
    print(
        "record the status of each rule above in .conventions/status.toml, then set its "
        f'`reviewed_at = "{sha}"`, in this commit (check F fails until you do)'
    )
    return 0


INIT_CONSUMER = """\
# This repository's pin on the tasker-systems conventions baseline. Doctrine: CONS-* in
# .conventions/baseline/doctrine/consuming.md. `revision` is written by `conventions.py bump`.
name = "{name}"
revision = "{sha}"

# Each adopted canonical file: where it lives here, and how it is compared (exact, extends,
# table, superset). Example:
# [[adopted]]
# canonical = "python/ruff.toml"
# path = "ruff.toml"
# mode = "extends"

[guidance]
# Agent entry files that must link the projected skill, and where the skill is projected.
entry = ["AGENTS.md"]
skill = ".claude/skills/tasker-systems-conventions"
"""

INIT_STATUS = """\
# This repository's status for each baseline rule, in the register's vocabulary:
# gated: <path>[#job], partial: …, practiced, convention: …, uncovered, unexamined, n/a.
# `reviewed_at` is the baseline revision these statuses were reviewed against (check F).
[rules]
"""

INIT_EXCEPTIONS = """\
# Every local departure from the pinned baseline, one entry each, with its reason.
# [[exception]]
# path = "ruff.toml"
# key = "target-version"
# reason = "the plugin's declared floor is py39"
"""


# --------------------------------------------------------------------------------------------
# witness — each check shown red on a seeded violation in a copy of this consumer
# --------------------------------------------------------------------------------------------

WITNESS_VALUE = "conventions-witness"
ZERO_SHA = "0" * 40

# What survives into the emptied environment the checks run in: where programs are, and (when a
# network proxy is in use) how to reach the network. No token, no credential, no git configuration.
PASSED_THROUGH = ("PATH", "HTTPS_PROXY", "https_proxy", "SSL_CERT_FILE", "GIT_SSL_CAINFO")


class SeedMissed(Exception):
    """A seed did not find what it edits: the case would pass without testing anything."""


def toml_dumps(data: dict[str, Any]) -> str:
    """Every leaf as a quoted dotted key at the root: valid TOML for anything tomllib returns, and
    enough for a copy whose content, not its layout, is what the checks read."""

    def key(parts: list[str]) -> str:
        return ".".join(json.dumps(part, ensure_ascii=False) for part in parts)

    def value(item: Any) -> str:
        if isinstance(item, bool):
            return "true" if item else "false"
        if isinstance(item, float):
            if math.isnan(item):
                return "nan"
            if item in (float("inf"), float("-inf")):
                return "inf" if item > 0 else "-inf"
            return repr(item)
        if isinstance(item, int):
            return str(item)
        if isinstance(item, str):
            return json.dumps(item, ensure_ascii=False).replace("\x7f", "\\u007f")
        if isinstance(item, list):
            return "[" + ", ".join(value(x) for x in item) + "]"
        if isinstance(item, dict):
            return "{" + ", ".join(f"{key([k])} = {value(v)}" for k, v in item.items()) + "}"
        return item.isoformat()  # datetime, date, time

    lines: list[str] = []

    def walk(node: dict[str, Any], parts: list[str]) -> None:
        for name, child in node.items():
            if isinstance(child, dict) and child:
                walk(child, [*parts, name])
            else:
                lines.append(f"{key([*parts, name])} = {value(child)}")

    walk(data, [])
    return "\n".join(lines) + "\n"


def rewrite(path: Path, change: Any) -> None:
    """Parse a TOML or JSON(C) file, let `change` edit the data, and write it back."""
    data = load_structured(path)
    if data is None:
        raise SeedMissed(f"{path.name} is not TOML or JSON")
    change(data)
    if path.suffix == ".toml":
        path.write_text(toml_dumps(data), encoding="utf-8")
    else:
        path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def set_path(data: dict[str, Any], dotted: str, item: Any) -> None:
    *parents, last = dotted.split(".")
    node = data
    for part in parents:
        child = node.get(part)
        if not isinstance(child, dict):
            child = node[part] = {}
        node = child
    node[last] = item


def edit_text(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SeedMissed(f"{path.name} does not contain {old!r}")
    path.write_text(text.replace(old, new), encoding="utf-8")


def append_text(path: Path, text: str) -> None:
    if not path.is_file():
        raise SeedMissed(f"{path} does not exist")
    with path.open("a", encoding="utf-8") as handle:
        handle.write(text)


def conventions_jobs(root: Path) -> list[tuple[str, str, list[str]]]:
    """(workflow, job, lines) for each workflow job that runs `conventions.py check`."""
    found: list[tuple[str, str, list[str]]] = []
    workflows = root / ".github" / "workflows"
    for path in sorted([*workflows.glob("*.yml"), *workflows.glob("*.yaml")]):
        lines = path.read_text(encoding="utf-8").splitlines()
        job, body = "", []
        for line in [*lines, "end-of-file:"]:
            match = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
            if match or (line and not line.startswith(" ")):
                if job and any("conventions.py check" in x for x in body):
                    found.append((path.relative_to(root).as_posix(), job, body))
                job, body = (match.group(1) if match else ""), []
            elif job:
                body.append(line)
    return found


@dataclass
class Case:
    name: str
    seed: Any
    expected: str | None  # the text the red verdict must contain; None for a case that stays green
    offline: bool = True


def first_leaf(adopted: Adopted, consumer: Consumer) -> str | None:
    """The first canonical key a seed can override: a leaf outside any declared local slot. None
    when every canonical key is a slot (audit.toml's ignore list), where only an addition bites."""
    canon = load_structured(consumer.baseline_dir / adopted.canonical) or {}
    if adopted.mode == "table" and adopted.canonical_table:
        canon = subtable(canon, adopted.canonical_table) or {}
    slots = local_slots(consumer, adopted.canonical) if adopted.mode == "superset" else []
    skip = {adopted.key or "extend", "extends", "$schema", "$comment"}
    for leaf in flatten(canon):
        if leaf and leaf not in skip and not _in_slot(leaf, slots):
            return leaf
    return None


def adopted_cases(consumer: Consumer) -> list[Case]:
    cases: list[Case] = []
    for adopted in consumer.adopted:
        rel = adopted.path
        if load_structured(consumer.baseline_dir / adopted.canonical) is None:
            cases.append(
                Case(
                    f"{rel} differs from its canonical bytes",
                    lambda c, rel=rel: append_text(c / rel, "\n"),
                    f"A: {rel}: (content differs)",
                )
            )
            continue
        leaf = first_leaf(adopted, consumer)
        if leaf is None and adopted.mode != "superset":
            raise SeedMissed(f"{adopted.canonical} has no key a seed can override")
        if leaf is not None:
            local_key = _join(adopted.table, leaf) if adopted.mode == "table" else leaf
            cases.append(
                Case(
                    f"{rel} overrides canonical {local_key}",
                    lambda c, rel=rel, k=local_key: rewrite(
                        c / rel, lambda d: set_path(d, k, WITNESS_VALUE)
                    ),
                    f"A: {rel}: {local_key} (canonical",
                )
            )
        if adopted.mode == "extends":
            ref_key = adopted.key or ("extend" if rel.endswith(".toml") else "extends")
            cases.append(
                Case(
                    f"{rel} stops referencing the projection",
                    lambda c, rel=rel, k=ref_key: rewrite(c / rel, lambda d: d.pop(k, None)),
                    f"A: {rel}: {ref_key} (does not reference",
                )
            )
        else:
            extra = _join(adopted.table if adopted.mode == "table" else "", WITNESS_VALUE)
            cases.append(
                Case(
                    f"{rel} adds a key the baseline does not hold",
                    lambda c, rel=rel, k=extra: rewrite(c / rel, lambda d: set_path(d, k, "x")),
                    f"A: {rel}: {extra} (not in canonical",
                )
            )
    return cases


def exception_cases(consumer: Consumer) -> list[Case]:
    if not consumer.adopted:
        raise SeedMissed("nothing is adopted, so no exception can be seeded")
    seedable = [
        (a, leaf)
        for a in consumer.adopted
        if load_structured(consumer.baseline_dir / a.canonical)
        and (leaf := first_leaf(a, consumer)) is not None
    ]
    if not seedable:
        raise SeedMissed("no adopted file has a canonical key to diverge from")
    adopted, leaf = seedable[0]
    rel = adopted.path
    key = _join(adopted.table, leaf) if adopted.mode == "table" else leaf

    def diverge(copy: Path) -> None:
        rewrite(copy / rel, lambda d: set_path(d, key, WITNESS_VALUE))

    def record(copy: Path, reason: str | None) -> None:
        entry = f"\n[[exception]]\npath = {json.dumps(rel)}\nkey = {json.dumps(key)}\n"
        if reason is not None:
            entry += f"reason = {json.dumps(reason)}\n"
        path = copy / CONVENTIONS_DIR / "exceptions.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(entry)

    return [
        Case(
            "a reasoned exception covers the divergence it records",
            lambda c: (diverge(c), record(c, "a seeded, reasoned divergence")),
            None,
        ),
        Case(
            "an exception carries no reason",
            lambda c: (diverge(c), record(c, None)),
            "has no reason",
        ),
        Case(
            "an exception matches no difference",
            lambda c: record(c, "a divergence that does not exist"),
            f"exception {rel}:{key} matches no difference",
        ),
    ]


def status_cases(consumer: Consumer, workflow: str) -> list[Case]:
    statuses = load_statuses(consumer)
    rules = rule_rows(consumer.baseline_dir / "doctrine" / "rules.md")
    rule = min(statuses) if statuses else min(rules, default="")
    if not rule:
        raise SeedMissed("the projection names no rule to seed a status for")
    current = statuses.get(rule, "unexamined").partition(":")[0].strip()
    other = "uncovered" if current == "practiced" else "practiced"
    status_path = f"{CONVENTIONS_DIR}/status.toml"

    def set_status(copy: Path, rule_id: str, status: str) -> None:
        rewrite(copy / status_path, lambda d: set_path(d, f"rules.{rule_id}", status))

    def drop_reviewed(copy: Path) -> None:
        rewrite(copy / status_path, lambda d: d.pop("reviewed_at"))

    missing_file = ".github/workflows/conventions-witness-absent.yml"
    cases = [
        Case(
            "a status changes without the skill being re-rendered",
            lambda c: set_status(c, rule, other),
            f"E: {consumer.skill} is not the skill rendered from the projection and status.toml",
        ),
        Case(
            "a status names a rule the pinned baseline does not have",
            lambda c: set_status(c, "WITNESS-0", "practiced"),
            "D: status.toml: WITNESS-0 is not a rule in the pinned baseline",
        ),
        Case(
            "a gated status names a file that does not exist",
            lambda c: set_status(c, rule, f"gated: {missing_file}"),
            f"D: status.toml: {rule} names gate '{missing_file}', which does not exist",
        ),
        Case(
            "a gated status names a job that does not exist",
            lambda c: set_status(c, rule, f"gated: {workflow}#{WITNESS_VALUE}"),
            f"D: status.toml: {rule} names gate '{workflow}#{WITNESS_VALUE}', which does not exist",
        ),
        Case(
            "the statuses were reviewed against another revision than the pin",
            lambda c: rewrite(c / status_path, lambda d: d.update(reviewed_at=ZERO_SHA)),
            f"F: status.toml was reviewed at {ZERO_SHA}",
        ),
        Case(
            "the statuses name no revision they were reviewed against",
            drop_reviewed,
            "F: status.toml has no `reviewed_at`",
        ),
    ]
    return cases


def witness_cases(consumer: Consumer, workflow: str) -> list[Case]:
    base = BASELINE_DIR
    skill = consumer.skill.rstrip("/")
    cases = [
        Case(
            "the pin names a branch, not a commit",
            lambda c: edit_text(
                c / CONVENTIONS_DIR / "consumer.toml",
                f'"{consumer.revision}"',
                '"main"',
            ),
            "`revision` must be a full 40-character commit SHA",
        ),
        Case(
            "a projected doctrine file is edited",
            lambda c: append_text(c / base / "doctrine/tenets.md", "\nA local amendment.\n"),
            f"B: {base}/doctrine/tenets.md differs from the pinned commit",
            offline=False,
        ),
        Case(
            "a file is added to the projection",
            lambda c: (c / base / "doctrine/local.md").write_text("# Local\n"),
            f"B: {base}/doctrine/local.md is not in the baseline at the pin",
            offline=False,
        ),
        Case(
            "a file is removed from the projection",
            lambda c: (c / base / "LICENSE").unlink(),
            f"B: {base}/LICENSE is missing from the projection",
            offline=False,
        ),
        *adopted_cases(consumer),
        *exception_cases(consumer),
        Case(
            "the projected skill is hand-edited",
            lambda c: append_text(c / skill / "SKILL.md", "\nA local rule.\n"),
            f"E: {consumer.skill} is not the skill rendered from the projection and status.toml",
        ),
    ]
    for entry in consumer.entries:
        cases.append(
            Case(
                f"{entry} stops linking the skill",
                lambda c, entry=entry: edit_text(c / entry, skill, "the conventions skill"),
                f"E: {entry} does not link the projected skill",
            )
        )
    cases += status_cases(consumer, workflow)
    return cases


def witness_copy(consumer: Consumer, dest: Path) -> None:
    """Copy what the checks read, and nothing else: `.conventions/`, the adopted files, the agent
    entry files and skill, the workflows, and every file a `gated:` status names. The verdict on
    this copy is the verdict on the tree, and a large repository is not copied once per case."""
    names = {CONVENTIONS_DIR, consumer.skill.rstrip("/"), ".github/workflows"}
    names |= {a.path for a in consumer.adopted} | set(consumer.entries)
    for status in load_statuses(consumer).values():
        word, _, rest = status.partition(":")
        if word.strip() == "gated":
            names |= {r.strip().partition("#")[0] for r in rest.split(",") if r.strip()}
    for name in sorted(names):
        source = consumer.root / name
        target = dest / name
        if source.is_dir():
            shutil.copytree(source, target, dirs_exist_ok=True)
        elif source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)


def witness(root: Path, url: str | None) -> int:
    """Seed one violation per case into a copy of this consumer and require the checks to refuse
    it, naming what was seeded. The unseeded copy runs first, online, in an emptied environment."""
    try:
        consumer = load_consumer(root)
    except UsageError as error:
        print(f"conventions: {error}", file=sys.stderr)
        return 2
    url = url or consumer.baseline
    tool = Path(__file__).resolve()
    failures: list[str] = []
    passed = 0

    def report(ok: bool, line: str, out: str = "") -> None:
        nonlocal passed
        print(f"  {'PASS' if ok else 'FAIL'}: {line}")
        if ok:
            passed += 1
        else:
            failures.append(line)
            print("\n".join("      " + x for x in out.splitlines()))

    jobs = conventions_jobs(root)
    if not jobs:
        failures.append("no workflow job runs `conventions.py check`")
        workflow = ".github/workflows/ci.yml"
    else:
        workflow = jobs[0][0]
        for path, job, body in jobs:
            privileged = [x for x in body if re.search(r"secrets\.|TOKEN", x)]
            report(not privileged, f"{path}#{job} runs the check and references no secret")

    try:
        cases = witness_cases(consumer, workflow)
    except SeedMissed as error:
        print(f"FAIL  the cases could not be built: {error}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="conventions-witness-") as tmp:
        scratch = Path(tmp)
        home = scratch / "home"
        home.mkdir()
        env = {k: os.environ[k] for k in PASSED_THROUGH if k in os.environ}
        env.update(
            HOME=str(home), GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
            GIT_TERMINAL_PROMPT="0",
        )  # fmt: skip
        pristine = scratch / "pristine"
        witness_copy(consumer, pristine)

        def fresh(name: str) -> Path:
            copy = scratch / name
            shutil.copytree(pristine, copy)
            return copy

        def run(copy: Path, *args: str) -> tuple[int, str]:
            command = [sys.executable, "-I", str(tool), *args, "--root", str(copy)]
            command += ["--baseline-repo", url]
            result = subprocess.run(command, capture_output=True, text=True, env=env, check=False)
            return result.returncode, result.stdout + result.stderr

        code, out = run(fresh("control"), "check")
        lag_line = f"C: {consumer.name} is"
        report(
            code == 0 and lag_line in out,
            "the unseeded copy is green, online, with no credential, and names its lag",
            out,
        )

        for index, case in enumerate(cases):
            copy = fresh(f"case-{index}")
            try:
                case.seed(copy)
            except (SeedMissed, OSError, KeyError) as error:
                report(False, f"{case.name}: the seed did not apply ({error})")
                continue
            code, out = run(copy, "check", *(["--offline"] if case.offline else []))
            if case.expected is None:
                ok, verdict = code == 0, "stays green" if code == 0 else "was refused"
            elif code != 0 and case.expected in out:
                ok, verdict = True, "is refused"
            else:
                ok = False
                verdict = "was accepted" if code == 0 else f"was refused without {case.expected!r}"
            report(ok, f"{case.name} {verdict}", out)

        copy = fresh("lag")
        fetched = Fetched(url)
        try:
            parent = fetched.resolve(f"{consumer.revision}^1")
        finally:
            fetched.close()
        if not parent:
            report(False, "the pin has no parent on the baseline's main to seed lag with")
        else:
            run(copy, "bump", "--revision", parent)
            _, out = run(copy, "check")
            behind = re.search(rf"C: {re.escape(consumer.name)} is [1-9][0-9]* commit\(s\)", out)
            report(bool(behind), "a pin behind the baseline's main is named as lag", out)

    for failure in failures:
        print(f"FAIL  {failure}", file=sys.stderr)
    if failures:
        return 1
    print(f"conventions witness: {passed} cases held")
    return 0


# --------------------------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="conventions.py", description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    check = sub.add_parser("check", help="run checks A-F against the consumer at --root")
    check.add_argument("--root", type=Path, default=Path.cwd())
    check.add_argument("--offline", action="store_true", help="run A, D, E and F only, and say so")
    check.add_argument("--baseline-repo", help="override the baseline URL (tests)")
    move = sub.add_parser("bump", help="move the pin and rewrite the projection and skill")
    move.add_argument("--root", type=Path, default=Path.cwd())
    move.add_argument(
        "--revision", required=True, help="a commit SHA on the baseline's main, or `main`"
    )
    move.add_argument("--baseline-repo", help="override the baseline URL (tests)")
    move.add_argument("--init", metavar="NAME", help="create .conventions/ for consumer NAME first")
    seeded = sub.add_parser("witness", help="show each check red on a seeded violation (TW-5)")
    seeded.add_argument("--root", type=Path, default=Path.cwd())
    seeded.add_argument("--baseline-repo", help="override the baseline URL (tests)")
    args = parser.parse_args(argv)

    root = args.root.resolve()
    if args.command == "bump":
        return bump(root, args.revision, args.baseline_repo, args.init)
    if args.command == "witness":
        return witness(root, args.baseline_repo)

    report = Report()
    try:
        consumer = load_consumer(root)
        check_adopted(consumer, report)
        if args.offline:
            report.note("B", "not run (--offline): the projection was not compared with the pin")
            report.note("C", "not run (--offline): lag is unknown")
        else:
            check_pin(consumer, report, args.baseline_repo or consumer.baseline)
        check_statuses(consumer, report)
        check_guidance(consumer, report)
        check_reviewed(consumer, report)
    except UsageError as error:
        report.fail("config", str(error))
    return report.emit()


if __name__ == "__main__":
    sys.exit(main())
