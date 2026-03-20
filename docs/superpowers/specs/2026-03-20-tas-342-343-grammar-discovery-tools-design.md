# TAS-342/343: Grammar and Capability Discovery Tools

**Date**: 2026-03-20
**Status**: Approved
**Tickets**: TAS-342 (MCP tools), TAS-343 (CLI commands)
**Roadmap Lane**: 3C (Validation Tooling)
**Predecessor**: TAS-323 (core types), TAS-333 (CompositionValidator)
**Deferred**: `composition_explain` deferred to TAS-344

---

## Problem

The grammar system (6 capabilities, vocabulary registry, composition validator) has no
developer-facing discovery or inspection tooling. Template authors — both human
developers using tasker-ctl and LLM agents using tasker-mcp — cannot list available
capabilities, inspect their config schemas, or validate standalone compositions
without reading source code.

## Scope

Add 5 grammar discovery and validation functions to tasker-sdk, expose them as 5
Tier 1 offline MCP tools in tasker-mcp, and as a `grammar` command group in
tasker-ctl. All tools work offline — no running Tasker instance required.

### Deferred

- `composition_explain` (data flow tracing) — deferred to TAS-344

### Out of Scope

- Resource-aware semantic validation (runtime concern)
- Expression variable resolution (diminishing returns, see TAS-339/341 analysis)
- Connected/write tools (all tools are Tier 1 offline)

## Design

### Layer 1: SDK grammar_query module

New module `crates/tasker-sdk/src/grammar_query.rs` providing 5 public functions.
All return SDK-owned structs deriving `Serialize` + `Debug` for consumption by both
MCP (JSON serialization) and CLI (structured formatting).

**Functions:**

| Function | Signature | Source |
|---|---|---|
| `list_grammar_categories` | `() -> Vec<GrammarCategoryInfo>` | Enumerates `GrammarCategoryKind` variants with descriptions |
| `search_capabilities` | `(query: Option<&str>, category: Option<&str>) -> Vec<CapabilitySummary>` | Searches `standard_capability_registry()` by name substring and/or category filter. Both optional — no filters returns all. |
| `inspect_capability` | `(name: &str) -> Option<CapabilityDetail>` | Full detail for one capability: config_schema, mutation_profile, tags, version |
| `document_vocabulary` | `() -> VocabularyDocumentation` | All capabilities organized by category |
| `validate_composition_yaml` | `(yaml_str: &str) -> CompositionValidationReport` | Parse YAML/JSON → `CompositionSpec` → run `validate_composition()` with standard registry |

**Return types:**

```rust
#[derive(Debug, Serialize)]
pub struct GrammarCategoryInfo {
    pub name: String,           // e.g., "transform"
    pub description: String,    // e.g., "Pure data transformation via jaq expressions"
    pub capabilities: Vec<String>,  // capability names in this category
}

#[derive(Debug, Serialize)]
pub struct CapabilitySummary {
    pub name: String,
    pub category: String,
    pub description: String,
    pub is_mutating: bool,
}

#[derive(Debug, Serialize)]
pub struct CapabilityDetail {
    pub name: String,
    pub category: String,
    pub description: String,
    pub config_schema: serde_json::Value,
    pub mutation_profile: String,       // "non_mutating" | "mutating"
    pub supports_idempotency_key: bool,
    pub tags: Vec<String>,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct VocabularyDocumentation {
    pub categories: Vec<GrammarCategoryInfo>,
    pub capabilities: Vec<CapabilityDetail>,
    pub total_capabilities: usize,
}

#[derive(Debug, Serialize)]
pub struct CompositionValidationReport {
    pub valid: bool,
    pub findings: Vec<CompositionFinding>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct CompositionFinding {
    pub severity: String,  // "error" | "warning" | "info"
    pub code: String,
    pub message: String,
    pub invocation_index: Option<usize>,
    pub field_path: Option<String>,
}
```

`validate_composition_yaml` is distinct from the existing `validate_step_composition`
(which validates in template step context). This one takes a standalone composition
spec for offline checking without a template.

### Layer 2: MCP tools (tasker-mcp)

5 new Tier 1 offline tools added to `crates/tasker-mcp/src/tools/developer.rs`.
Each is a pure sync function following existing patterns: params struct → SDK call →
JSON string.

| MCP Tool | Params | SDK Function |
|---|---|---|
| `grammar_list` | *(none)* | `grammar_query::list_grammar_categories()` |
| `capability_search` | `query?: String`, `category?: String` | `grammar_query::search_capabilities()` |
| `capability_inspect` | `name: String` | `grammar_query::inspect_capability()` |
| `vocabulary_document` | *(none)* | `grammar_query::document_vocabulary()` |
| `composition_validate` | `composition_yaml: String` | `grammar_query::validate_composition_yaml()` |

Parameter structs in `params.rs` with `Deserialize + JsonSchema` and `#[schemars(description)]`.
Tool registration in `server.rs` via `#[tool(...)]` macros. No tier constant changes
needed — Tier 1 is the default for functions without client resolution.

### Layer 3: CLI commands (tasker-ctl)

One new top-level `grammar` command group with nested subcommands:

```
tasker-ctl grammar list
tasker-ctl grammar inspect <category>
tasker-ctl grammar capability search [query] [--category <cat>]
tasker-ctl grammar capability inspect <name>
tasker-ctl grammar capability document
tasker-ctl grammar composition validate <path> [--step <name>]
```

**Clap structure:**
```rust
#[derive(Debug, Subcommand)]
pub enum GrammarCommands {
    List,
    Inspect { category: String },
    #[command(subcommand)]
    Capability(CapabilityCommands),
    #[command(subcommand)]
    Composition(CompositionCommands),
}

#[derive(Debug, Subcommand)]
pub enum CapabilityCommands {
    Search { query: Option<String>, #[arg(long)] category: Option<String> },
    Inspect { name: String },
    Document,
}

#[derive(Debug, Subcommand)]
pub enum CompositionCommands {
    Validate { path: PathBuf, #[arg(long)] step: Option<String> },
}
```

Single handler module `commands/grammar.rs`. All commands are offline — no
`ClientConfig` needed. The `--step` flag on `composition validate` is a convenience:
with it, the command parses a full template YAML, finds the named step's composition
field, and validates that. Without it, the file is treated as a standalone
CompositionSpec.

Output respects the existing `--format` global flag (text default, json when piping).

## Files Changed

| File | Change |
|------|--------|
| `crates/tasker-sdk/src/grammar_query.rs` | New module: 5 functions + return types |
| `crates/tasker-sdk/src/lib.rs` | Add `pub mod grammar_query` |
| `crates/tasker-mcp/src/tools/developer.rs` | Add 5 tool functions |
| `crates/tasker-mcp/src/tools/params.rs` | Add param + response structs |
| `crates/tasker-mcp/src/server.rs` | Register 5 new `#[tool(...)]` entries |
| `crates/tasker-ctl/src/main.rs` | Add `Grammar(GrammarCommands)` variant + routing |
| `crates/tasker-ctl/src/commands/grammar.rs` | New handler module |
| `crates/tasker-ctl/src/commands/mod.rs` | Add `grammar` module export |

## Test Plan

**SDK tests** (in `crates/tasker-sdk/src/grammar_query.rs` or sibling test module):
- `list_grammar_categories_returns_all_six` — 6 categories, each with at least one capability
- `search_capabilities_by_name` — substring match returns expected capabilities
- `search_capabilities_by_category` — filter returns only matching category
- `search_capabilities_no_filter` — returns all 6 capabilities
- `inspect_capability_found` — returns full detail for "transform"
- `inspect_capability_not_found` — returns None for unknown name
- `document_vocabulary_complete` — all 6 capabilities present, organized by category
- `validate_composition_yaml_valid` — valid spec produces `valid: true`, empty findings
- `validate_composition_yaml_invalid_yaml` — bad YAML produces parse error finding
- `validate_composition_yaml_invalid_spec` — structurally bad spec produces error findings

**MCP/CLI** — thin wrappers, no dedicated unit tests. Validated by compilation and
manual verification. MCP tools tested indirectly via SDK test coverage.

## Verification

```bash
# SDK tests
cargo test --package tasker-sdk --all-features

# MCP compiles
cargo check --package tasker-mcp --all-features

# CLI compiles
cargo check --package tasker-ctl --all-features

# Full workspace
cargo clippy --all-targets --all-features --workspace
```
