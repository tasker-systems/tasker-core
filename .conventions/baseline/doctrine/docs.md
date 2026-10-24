# Markdown documentation

How Markdown is written in every tasker-systems repository: READMEs, `docs/`, agent guidance, and
this repository. Agent guidance has its own rules for *what* it says
([`agent-guidance.md`](agent-guidance.md)); these are about the shape every Markdown file shares.
Enforcement status per repo is in [`rules.md`](rules.md); the Google Markdown guide ledger is
[`../sources/google/docs-and-json.md`](../sources/google/docs-and-json.md).

**DOC-1 — one Markdown shape, linted.** ATX headings (`#`), one H1 per file, fenced code blocks with
backticks, `-` for unordered lists, no trailing whitespace, no two sibling headings with the same
text. markdownlint (`markdownlint-cli2`) runs in CI with MD003, MD004, MD009, MD024
(`siblings_only`), MD025, MD046 and MD048. *Why:* agents and renderers parse these files; a
consistent shape is what makes a heading a reliable anchor and a fence a reliable boundary.

**DOC-2 — a code block declares its language.** `rust`, `bash`, `toml`, `json` — and `text` for a
diagram or plain output. MD040 enforces it. *Why:* renderers highlight by it, and an agent copying
a block uses the tag to tell a command from its output.

**DOC-3 — prose wraps at 100 columns.** MD013 at 100, with tables, code blocks, headings and URLs
exempt. *Why:* the same width rustfmt and ruff use, so a diff of prose reads like a diff of code.
Google's 80 is rejected; about 95% of lines in temper and this repository already fit 100.

**DOC-4 — links inside a repository resolve.** A relative link to a file or heading in the same
repository is checked offline in CI (`lychee --offline`, or an equivalent). *Why:* a dead link in
agent guidance fails silently: the agent does not find the file and carries on without it, which is
the same failure [AG-6](agent-guidance.md) names for hooks.

## Not specified

- **Lists versus tables** — Google prefers lists; the rule register is tables, and agents read
  tables well. Either.
- **Google's page template, `[TOC]`, absolute link paths** — Gitiles-specific. Relative links are
  what work on GitHub.
- **Inline HTML** — allowed where Markdown cannot express it (a `<details>` block); not ruled.
