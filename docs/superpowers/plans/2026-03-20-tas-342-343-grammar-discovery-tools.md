# TAS-342/343: Grammar and Capability Discovery Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add grammar discovery, capability inspection, and standalone composition validation to tasker-sdk, tasker-mcp, and tasker-ctl.

**Architecture:** SDK `grammar_query` module provides 5 functions with serializable return types. MCP adds 5 Tier 1 offline tools as thin wrappers. CLI adds a `grammar` command group with nested `capability` and `composition` subcommands. Both consumers delegate to SDK — no grammar logic in presentation layers.

**Tech Stack:** Rust, serde/serde_json/serde_yaml, tasker-grammar (via tasker-sdk), rmcp (MCP macros), clap (CLI)

**Spec:** `docs/superpowers/specs/2026-03-20-tas-342-343-grammar-discovery-tools-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/tasker-sdk/src/grammar_query.rs` | New: 5 public functions + return type structs |
| `crates/tasker-sdk/src/lib.rs` | Modify: add `pub mod grammar_query` |
| `crates/tasker-mcp/src/tools/developer.rs` | Modify: add 5 tool functions |
| `crates/tasker-mcp/src/tools/params.rs` | Modify: add param structs |
| `crates/tasker-mcp/src/server.rs` | Modify: register 5 tools, update test assertions |
| `crates/tasker-ctl/src/main.rs` | Modify: add Grammar command enum + routing |
| `crates/tasker-ctl/src/commands/grammar.rs` | New: handler module |
| `crates/tasker-ctl/src/commands/mod.rs` | Modify: add grammar export |

---

## Task 1: SDK grammar_query module — return types and list/search/inspect functions

**Files:**
- Create: `crates/tasker-sdk/src/grammar_query.rs`
- Modify: `crates/tasker-sdk/src/lib.rs`

- [ ] **Step 1: Create grammar_query.rs with return type structs**

Create `crates/tasker-sdk/src/grammar_query.rs`:

```rust
//! Grammar discovery and capability inspection queries.
//!
//! Provides offline introspection of the grammar vocabulary for use by
//! tasker-mcp (MCP tools) and tasker-ctl (CLI commands).
//!
//! **Tickets**: TAS-342, TAS-343

use serde::Serialize;
use tasker_grammar::{
    standard_capability_registry, GrammarCategoryKind, MutationProfile,
};

// ---------------------------------------------------------------------------
// Return types
// ---------------------------------------------------------------------------

/// Summary of a grammar category with its associated capabilities.
#[derive(Debug, Serialize)]
pub struct GrammarCategoryInfo {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
}

/// Brief summary of a capability for search results.
#[derive(Debug, Serialize)]
pub struct CapabilitySummary {
    pub name: String,
    pub category: String,
    pub description: String,
    pub is_mutating: bool,
}

/// Full detail of a single capability.
#[derive(Debug, Serialize)]
pub struct CapabilityDetail {
    pub name: String,
    pub category: String,
    pub description: String,
    pub config_schema: serde_json::Value,
    pub mutation_profile: String,
    pub supports_idempotency_key: Option<bool>,
    pub tags: Vec<String>,
    pub version: String,
}

/// Complete vocabulary documentation.
#[derive(Debug, Serialize)]
pub struct VocabularyDocumentation {
    pub categories: Vec<GrammarCategoryInfo>,
    pub capabilities: Vec<CapabilityDetail>,
    pub total_capabilities: usize,
}

/// Result of validating a standalone composition spec.
#[derive(Debug, Serialize)]
pub struct CompositionValidationReport {
    pub valid: bool,
    pub findings: Vec<CompositionFinding>,
    pub summary: String,
}

/// A single finding from composition validation.
#[derive(Debug, Serialize)]
pub struct CompositionFinding {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub invocation_index: Option<usize>,
    pub field_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// All grammar category kinds in canonical order.
const ALL_CATEGORIES: &[GrammarCategoryKind] = &[
    GrammarCategoryKind::Transform,
    GrammarCategoryKind::Validate,
    GrammarCategoryKind::Assert,
    GrammarCategoryKind::Acquire,
    GrammarCategoryKind::Persist,
    GrammarCategoryKind::Emit,
];

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn mutation_profile_str(profile: &MutationProfile) -> String {
    match profile {
        MutationProfile::NonMutating => "non_mutating".to_owned(),
        MutationProfile::Mutating { .. } => "mutating".to_owned(),
        MutationProfile::ConfigDependent => "config_dependent".to_owned(),
    }
}

fn supports_idempotency(profile: &MutationProfile) -> Option<bool> {
    match profile {
        MutationProfile::Mutating { supports_idempotency_key } => Some(*supports_idempotency_key),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// List all grammar categories with their descriptions and associated capabilities.
pub fn list_grammar_categories() -> Vec<GrammarCategoryInfo> {
    let registry = standard_capability_registry();

    ALL_CATEGORIES
        .iter()
        .map(|kind| {
            let category = kind.into_category();
            let capabilities: Vec<String> = registry
                .values()
                .filter(|cap| cap.grammar_category == *kind)
                .map(|cap| cap.name.clone())
                .collect();
            GrammarCategoryInfo {
                name: kind.to_string().to_lowercase(),
                description: category.description().to_owned(),
                capabilities,
            }
        })
        .collect()
}

/// Search capabilities by name substring and/or category filter.
///
/// Both parameters are optional — no filters returns all capabilities.
pub fn search_capabilities(
    query: Option<&str>,
    category: Option<&str>,
) -> Vec<CapabilitySummary> {
    let registry = standard_capability_registry();
    let query_lower = query.map(|q| q.to_ascii_lowercase());
    let category_lower = category.map(|c| c.to_ascii_lowercase());

    let mut results: Vec<CapabilitySummary> = registry
        .values()
        .filter(|cap| {
            if let Some(ref q) = query_lower {
                if !cap.name.to_ascii_lowercase().contains(q) {
                    return false;
                }
            }
            if let Some(ref c) = category_lower {
                if cap.grammar_category.to_string().to_ascii_lowercase() != *c {
                    return false;
                }
            }
            true
        })
        .map(|cap| CapabilitySummary {
            name: cap.name.clone(),
            category: cap.grammar_category.to_string().to_lowercase(),
            description: cap.description.clone(),
            is_mutating: matches!(cap.mutation_profile, MutationProfile::Mutating { .. }),
        })
        .collect();

    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

/// Get full detail for a single capability by name.
pub fn inspect_capability(name: &str) -> Option<CapabilityDetail> {
    let registry = standard_capability_registry();
    registry.get(name).map(|cap| CapabilityDetail {
        name: cap.name.clone(),
        category: cap.grammar_category.to_string().to_lowercase(),
        description: cap.description.clone(),
        config_schema: cap.config_schema.clone(),
        mutation_profile: mutation_profile_str(&cap.mutation_profile),
        supports_idempotency_key: supports_idempotency(&cap.mutation_profile),
        tags: cap.tags.clone(),
        version: cap.version.clone(),
    })
}

/// Generate complete vocabulary documentation.
pub fn document_vocabulary() -> VocabularyDocumentation {
    let categories = list_grammar_categories();
    let capabilities: Vec<CapabilityDetail> = {
        let registry = standard_capability_registry();
        let mut caps: Vec<CapabilityDetail> = registry
            .values()
            .map(|cap| CapabilityDetail {
                name: cap.name.clone(),
                category: cap.grammar_category.to_string().to_lowercase(),
                description: cap.description.clone(),
                config_schema: cap.config_schema.clone(),
                mutation_profile: mutation_profile_str(&cap.mutation_profile),
                supports_idempotency_key: supports_idempotency(&cap.mutation_profile),
                tags: cap.tags.clone(),
                version: cap.version.clone(),
            })
            .collect();
        caps.sort_by(|a, b| a.name.cmp(&b.name));
        caps
    };
    let total_capabilities = capabilities.len();
    VocabularyDocumentation {
        categories,
        capabilities,
        total_capabilities,
    }
}
```

- [ ] **Step 2: Add module export to lib.rs**

In `crates/tasker-sdk/src/lib.rs`, add after the `pub mod composition_validator;` line:

```rust
pub mod grammar_query;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --package tasker-sdk --all-features`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-sdk/src/grammar_query.rs crates/tasker-sdk/src/lib.rs
git commit -m "feat(TAS-342): add grammar_query module with category/capability discovery

Provides list_grammar_categories, search_capabilities, inspect_capability,
and document_vocabulary functions in tasker-sdk for consumption by
tasker-mcp and tasker-ctl."
```

---

## Task 2: SDK grammar_query — validate_composition_yaml + tests

**Files:**
- Modify: `crates/tasker-sdk/src/grammar_query.rs`

- [ ] **Step 1: Add validate_composition_yaml function**

Append to `grammar_query.rs` before the closing of the file:

```rust
/// Validate a standalone composition spec from YAML or JSON string.
///
/// Parses the input, runs `CompositionValidator` with the standard capability
/// registry, and returns a structured report.
pub fn validate_composition_yaml(yaml_str: &str) -> CompositionValidationReport {
    use tasker_grammar::{CompositionSpec, ExpressionEngine, Severity};
    use tasker_grammar::validation::CompositionValidator;

    // Try YAML first, then JSON
    let spec: CompositionSpec = match serde_yaml::from_str(yaml_str) {
        Ok(s) => s,
        Err(yaml_err) => match serde_json::from_str(yaml_str) {
            Ok(s) => s,
            Err(_) => {
                return CompositionValidationReport {
                    valid: false,
                    findings: vec![CompositionFinding {
                        severity: "error".to_owned(),
                        code: "PARSE_ERROR".to_owned(),
                        message: format!("Failed to parse composition: {yaml_err}"),
                        invocation_index: None,
                        field_path: None,
                    }],
                    summary: "Composition could not be parsed".to_owned(),
                };
            }
        },
    };

    let registry = standard_capability_registry();
    let engine = ExpressionEngine::with_defaults();
    let validator = CompositionValidator::new(&registry, &engine);
    let result = validator.validate(&spec);

    let findings: Vec<CompositionFinding> = result
        .findings
        .iter()
        .map(|f| CompositionFinding {
            severity: match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            }
            .to_owned(),
            code: f.code.clone(),
            message: f.message.clone(),
            invocation_index: f.invocation_index,
            field_path: f.field_path.clone(),
        })
        .collect();

    let error_count = findings.iter().filter(|f| f.severity == "error").count();
    let warning_count = findings.iter().filter(|f| f.severity == "warning").count();
    let valid = error_count == 0;

    let summary = if valid && warning_count == 0 {
        "Composition is valid".to_owned()
    } else if valid {
        format!("Composition is valid with {warning_count} warning(s)")
    } else {
        format!("Composition has {error_count} error(s) and {warning_count} warning(s)")
    };

    CompositionValidationReport {
        valid,
        findings,
        summary,
    }
}
```

- [ ] **Step 2: Add `serde_yaml` dependency to tasker-sdk if not already present**

Check `crates/tasker-sdk/Cargo.toml` for `serde_yaml`. If missing, add it under `[dependencies]`:
```toml
serde_yaml = { workspace = true }
```

- [ ] **Step 3: Add tests module**

Append to `grammar_query.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_grammar_categories_returns_all_six() {
        let categories = list_grammar_categories();
        assert_eq!(categories.len(), 6);
        let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"transform"));
        assert!(names.contains(&"validate"));
        assert!(names.contains(&"assert"));
        assert!(names.contains(&"acquire"));
        assert!(names.contains(&"persist"));
        assert!(names.contains(&"emit"));
        // Each category has at least one capability
        for cat in &categories {
            assert!(!cat.capabilities.is_empty(), "{} has no capabilities", cat.name);
        }
    }

    #[test]
    fn search_capabilities_by_name() {
        let results = search_capabilities(Some("trans"), None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "transform");
    }

    #[test]
    fn search_capabilities_by_category() {
        let results = search_capabilities(None, Some("persist"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "persist");
        assert!(results[0].is_mutating);
    }

    #[test]
    fn search_capabilities_no_filter() {
        let results = search_capabilities(None, None);
        assert_eq!(results.len(), 6);
    }

    #[test]
    fn inspect_capability_found() {
        let detail = inspect_capability("transform").unwrap();
        assert_eq!(detail.name, "transform");
        assert_eq!(detail.category, "transform");
        assert_eq!(detail.mutation_profile, "non_mutating");
        assert!(detail.supports_idempotency_key.is_none());
        assert!(detail.config_schema.is_object());
    }

    #[test]
    fn inspect_capability_not_found() {
        assert!(inspect_capability("nonexistent").is_none());
    }

    #[test]
    fn inspect_capability_mutating_has_idempotency() {
        let detail = inspect_capability("persist").unwrap();
        assert_eq!(detail.mutation_profile, "mutating");
        assert_eq!(detail.supports_idempotency_key, Some(true));
    }

    #[test]
    fn document_vocabulary_complete() {
        let doc = document_vocabulary();
        assert_eq!(doc.total_capabilities, 6);
        assert_eq!(doc.categories.len(), 6);
        assert_eq!(doc.capabilities.len(), 6);
    }

    #[test]
    fn validate_composition_yaml_valid() {
        let yaml = r#"
name: test
outcome:
  description: Test outcome
  output_schema: {}
invocations:
  - capability: transform
    config:
      output:
        type: object
        properties:
          x:
            type: string
        required: [x]
      filter: "{x: .context.name}"
    checkpoint: false
"#;
        let report = validate_composition_yaml(yaml);
        assert!(report.valid, "Expected valid but got: {}", report.summary);
    }

    #[test]
    fn validate_composition_yaml_invalid_yaml() {
        let report = validate_composition_yaml("not: valid: yaml: [[[");
        assert!(!report.valid);
        assert_eq!(report.findings[0].code, "PARSE_ERROR");
    }

    #[test]
    fn validate_composition_yaml_invalid_spec() {
        let yaml = r#"
name: test
outcome:
  description: Test
  output_schema: {}
invocations:
  - capability: nonexistent_capability
    config: {}
    checkpoint: false
"#;
        let report = validate_composition_yaml(yaml);
        assert!(!report.valid);
        assert!(report.findings.iter().any(|f| f.code == "MISSING_CAPABILITY"));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package tasker-sdk --all-features -- grammar_query`
Expected: All 11 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-sdk/src/grammar_query.rs crates/tasker-sdk/Cargo.toml
git commit -m "feat(TAS-342): add validate_composition_yaml and tests

Complete the grammar_query module with standalone composition validation
and 11 unit tests covering all 5 public functions."
```

---

## Task 3: MCP tools — parameter structs and tool functions

**Files:**
- Modify: `crates/tasker-mcp/src/tools/params.rs`
- Modify: `crates/tasker-mcp/src/tools/developer.rs`

- [ ] **Step 1: Add parameter structs to params.rs**

Append to `crates/tasker-mcp/src/tools/params.rs`:

```rust
// ── grammar_list ──
// No params needed (uses unit input)

// ── capability_search ──

/// Parameters for the `capability_search` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapabilitySearchParams {
    /// Optional name substring to search for (case-insensitive).
    #[schemars(description = "Optional capability name substring to search for (case-insensitive)")]
    #[serde(default)]
    pub query: Option<String>,
    /// Optional grammar category filter (e.g., 'transform', 'persist').
    #[schemars(description = "Optional grammar category filter (e.g., 'transform', 'persist', 'emit')")]
    #[serde(default)]
    pub category: Option<String>,
}

// ── capability_inspect ──

/// Parameters for the `capability_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapabilityInspectParams {
    /// Capability name to inspect (e.g., 'transform', 'persist').
    #[schemars(description = "Capability name to inspect (e.g., 'transform', 'persist', 'acquire')")]
    pub name: String,
}

// ── vocabulary_document ──
// No params needed

// ── composition_validate ──

/// Parameters for the `composition_validate` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompositionValidateParams {
    /// Composition spec as YAML or JSON string.
    #[schemars(
        description = "Composition spec as YAML or JSON string. Must include name, outcome (with output_schema), and invocations array."
    )]
    pub composition_yaml: String,
}
```

- [ ] **Step 2: Add tool functions to developer.rs**

Add these imports at the top of `developer.rs`:

```rust
use tasker_sdk::grammar_query;
```

Add the param imports to the existing `use super::params::{...}` block:

```rust
CapabilitySearchParams, CapabilityInspectParams, CompositionValidateParams,
```

Then append 5 tool functions:

```rust
pub fn grammar_list() -> String {
    let categories = grammar_query::list_grammar_categories();
    serde_json::to_string_pretty(&categories)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}

pub fn capability_search(params: CapabilitySearchParams) -> String {
    let results = grammar_query::search_capabilities(
        params.query.as_deref(),
        params.category.as_deref(),
    );
    serde_json::to_string_pretty(&results)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}

pub fn capability_inspect(params: CapabilityInspectParams) -> String {
    match grammar_query::inspect_capability(&params.name) {
        Some(detail) => serde_json::to_string_pretty(&detail)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
        None => error_json(
            "capability_not_found",
            &format!("No capability named '{}'. Use grammar_list to see available capabilities.", params.name),
        ),
    }
}

pub fn vocabulary_document() -> String {
    let doc = grammar_query::document_vocabulary();
    serde_json::to_string_pretty(&doc)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}

pub fn composition_validate(params: CompositionValidateParams) -> String {
    let report = grammar_query::validate_composition_yaml(&params.composition_yaml);
    serde_json::to_string_pretty(&report)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --package tasker-mcp --all-features`
Expected: Compiles (tools not yet registered in server.rs but functions exist)

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-mcp/src/tools/params.rs crates/tasker-mcp/src/tools/developer.rs
git commit -m "feat(TAS-342): add MCP grammar tool functions and parameter structs

5 new Tier 1 offline tool functions: grammar_list, capability_search,
capability_inspect, vocabulary_document, composition_validate."
```

---

## Task 4: MCP tools — server registration and test assertion updates

**Files:**
- Modify: `crates/tasker-mcp/src/server.rs`

- [ ] **Step 1: Register 5 new tools in server.rs**

In the `#[tool_router(router = tool_router)] impl TaskerMcpServer` block, after the existing Tier 1 tools (after the `schema_diff` tool registration), add:

```rust
    /// List all grammar categories with descriptions and associated capabilities.
    #[tool(
        name = "grammar_list",
        description = "List all grammar categories (transform, validate, assert, acquire, persist, emit) with descriptions and associated capabilities. Works offline."
    )]
    pub async fn grammar_list(&self) -> String {
        developer::grammar_list()
    }

    /// Search capabilities by name or category.
    #[tool(
        name = "capability_search",
        description = "Search grammar capabilities by name substring and/or category filter. Both parameters are optional — omit both to list all capabilities. Works offline."
    )]
    pub async fn capability_search(
        &self,
        Parameters(params): Parameters<CapabilitySearchParams>,
    ) -> String {
        developer::capability_search(params)
    }

    /// Inspect a capability's full configuration schema and metadata.
    #[tool(
        name = "capability_inspect",
        description = "Show full details for a capability: config_schema, mutation_profile, tags, and version. Use grammar_list first to see available capability names. Works offline."
    )]
    pub async fn capability_inspect(
        &self,
        Parameters(params): Parameters<CapabilityInspectParams>,
    ) -> String {
        developer::capability_inspect(params)
    }

    /// Generate complete vocabulary documentation.
    #[tool(
        name = "vocabulary_document",
        description = "Generate complete documentation for all registered grammar capabilities, organized by category with full config schemas. Works offline."
    )]
    pub async fn vocabulary_document(&self) -> String {
        developer::vocabulary_document()
    }

    /// Validate a standalone composition spec.
    #[tool(
        name = "composition_validate",
        description = "Validate a standalone CompositionSpec (YAML or JSON) for structural correctness: capability existence, config schemas, expression syntax, contract chaining, and checkpoint coverage. Works offline."
    )]
    pub async fn composition_validate(
        &self,
        Parameters(params): Parameters<CompositionValidateParams>,
    ) -> String {
        developer::composition_validate(params)
    }
```

Also add the new param types to the imports at the top of server.rs where other param types are imported.

- [ ] **Step 2: Update test assertions**

In server.rs tests, update the tool count assertions:
- Line ~977: `assert_eq!(names.len(), 8, ...)` → `assert_eq!(names.len(), 13, "Expected 13 Tier 1 tools, got: {:?}", names);`
- Line ~1002: T1+T2 test assertion `assert_eq!(names.len(), 25, ...)` → `assert_eq!(names.len(), 30, ...)` (13 T1 + 1 connection_status + 16 T2 = 30)
- Line ~1024: `assert_eq!(tools.len(), 31, ...)` → `assert_eq!(tools.len(), 36, "Expected all 36 tools");`
- Any other tool count assertions that reference the old counts

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo check --package tasker-mcp --all-features`
Run: `cargo test --package tasker-mcp --all-features`
Expected: Compiles and all MCP tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-mcp/src/server.rs
git commit -m "feat(TAS-342): register grammar MCP tools and update test assertions

Register grammar_list, capability_search, capability_inspect,
vocabulary_document, composition_validate as Tier 1 offline tools.
Update tool count assertions (8→13 T1, 31→36 total)."
```

---

## Task 5: CLI commands — grammar command group in tasker-ctl

**Files:**
- Create: `crates/tasker-ctl/src/commands/grammar.rs`
- Modify: `crates/tasker-ctl/src/commands/mod.rs`
- Modify: `crates/tasker-ctl/src/main.rs`

- [ ] **Step 1: Create grammar.rs handler module**

Create `crates/tasker-ctl/src/commands/grammar.rs`:

```rust
//! Grammar discovery and composition validation commands.
//!
//! All commands are offline — no running Tasker instance required.
//!
//! **Tickets**: TAS-342, TAS-343

use std::path::PathBuf;

use clap::Subcommand;
use tasker_sdk::grammar_query;

use crate::output;

#[derive(Debug, Subcommand)]
pub(crate) enum GrammarCommands {
    /// List all grammar categories with descriptions
    List,

    /// Inspect a specific grammar category and its capabilities
    Inspect {
        /// Category name (e.g., transform, persist, emit)
        category: String,
    },

    /// Capability discovery and inspection
    #[command(subcommand)]
    Capability(CapabilityCommands),

    /// Composition validation
    #[command(subcommand)]
    Composition(CompositionCommands),
}

#[derive(Debug, Subcommand)]
pub(crate) enum CapabilityCommands {
    /// Search capabilities by name or category
    Search {
        /// Optional name substring to search for
        query: Option<String>,
        /// Filter by grammar category
        #[arg(long)]
        category: Option<String>,
    },

    /// Inspect a capability's full configuration schema and metadata
    Inspect {
        /// Capability name (e.g., transform, persist, acquire)
        name: String,
    },

    /// Generate complete vocabulary documentation
    Document,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompositionCommands {
    /// Validate a composition spec for correctness
    Validate {
        /// Path to composition YAML/JSON file
        path: PathBuf,
        /// Optional step name — if provided, extracts composition from a full template
        #[arg(long)]
        step: Option<String>,
    },
}

pub(crate) async fn handle_grammar_command(
    cmd: GrammarCommands,
    format: &str,
) -> tasker_client::ClientResult<()> {
    match cmd {
        GrammarCommands::List => grammar_list(format),
        GrammarCommands::Inspect { category } => grammar_inspect(&category, format),
        GrammarCommands::Capability(cap_cmd) => match cap_cmd {
            CapabilityCommands::Search { query, category } => {
                capability_search(query.as_deref(), category.as_deref(), format)
            }
            CapabilityCommands::Inspect { name } => capability_inspect(&name, format),
            CapabilityCommands::Document => vocabulary_document(format),
        },
        GrammarCommands::Composition(comp_cmd) => match comp_cmd {
            CompositionCommands::Validate { path, step } => {
                composition_validate(&path, step.as_deref(), format)
            }
        },
    }
}

fn grammar_list(format: &str) -> tasker_client::ClientResult<()> {
    let categories = grammar_query::list_grammar_categories();
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&categories)?);
    } else {
        for cat in &categories {
            println!("{}  {}", cat.name, cat.description);
            for cap in &cat.capabilities {
                println!("  - {cap}");
            }
        }
    }
    Ok(())
}

fn grammar_inspect(category: &str, format: &str) -> tasker_client::ClientResult<()> {
    let categories = grammar_query::list_grammar_categories();
    let cat = categories
        .iter()
        .find(|c| c.name == category.to_ascii_lowercase());

    match cat {
        Some(info) => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(info)?);
            } else {
                println!("Category: {}", info.name);
                println!("Description: {}", info.description);
                println!("Capabilities:");
                for cap_name in &info.capabilities {
                    if let Some(detail) = grammar_query::inspect_capability(cap_name) {
                        println!("  {} — {}", detail.name, detail.description);
                        println!("    mutation: {}", detail.mutation_profile);
                        println!("    version: {}", detail.version);
                    }
                }
            }
            Ok(())
        }
        None => {
            let valid: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
            output::error(format!(
                "Unknown category '{}'. Valid categories: {}",
                category,
                valid.join(", ")
            ));
            Ok(())
        }
    }
}

fn capability_search(
    query: Option<&str>,
    category: Option<&str>,
    format: &str,
) -> tasker_client::ClientResult<()> {
    let results = grammar_query::search_capabilities(query, category);
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() {
        println!("No capabilities found matching the given criteria.");
    } else {
        for cap in &results {
            let mutating = if cap.is_mutating { " [mutating]" } else { "" };
            println!("{}  ({}){}", cap.name, cap.category, mutating);
            println!("  {}", cap.description);
        }
    }
    Ok(())
}

fn capability_inspect(name: &str, format: &str) -> tasker_client::ClientResult<()> {
    match grammar_query::inspect_capability(name) {
        Some(detail) => {
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&detail)?);
            } else {
                println!("Name: {}", detail.name);
                println!("Category: {}", detail.category);
                println!("Description: {}", detail.description);
                println!("Mutation: {}", detail.mutation_profile);
                if let Some(idempotent) = detail.supports_idempotency_key {
                    println!("Idempotency key: {idempotent}");
                }
                println!("Version: {}", detail.version);
                if !detail.tags.is_empty() {
                    println!("Tags: {}", detail.tags.join(", "));
                }
                println!("Config schema:");
                println!("{}", serde_json::to_string_pretty(&detail.config_schema)?);
            }
            Ok(())
        }
        None => {
            output::error(format!(
                "No capability named '{}'. Use 'tasker-ctl grammar list' to see available capabilities.",
                name
            ));
            Ok(())
        }
    }
}

fn vocabulary_document(format: &str) -> tasker_client::ClientResult<()> {
    let doc = grammar_query::document_vocabulary();
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        println!("# Tasker Grammar Vocabulary ({} capabilities)\n", doc.total_capabilities);
        for cat in &doc.categories {
            println!("## {}\n", cat.name);
            println!("{}\n", cat.description);
        }
        println!("## Capabilities\n");
        for cap in &doc.capabilities {
            println!("### {}\n", cap.name);
            println!("- Category: {}", cap.category);
            println!("- Description: {}", cap.description);
            println!("- Mutation: {}", cap.mutation_profile);
            println!("- Version: {}", cap.version);
            println!("- Config schema:\n```json\n{}\n```\n",
                serde_json::to_string_pretty(&cap.config_schema)?);
        }
    }
    Ok(())
}

fn composition_validate(
    path: &std::path::Path,
    step: Option<&str>,
    format: &str,
) -> tasker_client::ClientResult<()> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        tasker_client::ClientError::Internal(format!("Failed to read {}: {e}", path.display()))
    })?;

    let yaml_to_validate = if let Some(step_name) = step {
        // Extract composition from a full template's named step
        let template = tasker_sdk::template_parser::parse_template_str(&content)
            .map_err(|e| tasker_client::ClientError::Internal(format!("Template parse error: {e}")))?;
        let step_def = template
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or_else(|| {
                tasker_client::ClientError::Internal(format!(
                    "Step '{}' not found in template. Available: {}",
                    step_name,
                    template.steps.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
                ))
            })?;
        let composition = step_def.composition.as_ref().ok_or_else(|| {
            tasker_client::ClientError::Internal(format!(
                "Step '{}' has no composition field",
                step_name
            ))
        })?;
        serde_json::to_string(composition)?
    } else {
        content
    };

    let report = grammar_query::validate_composition_yaml(&yaml_to_validate);

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.summary);
        for finding in &report.findings {
            let prefix = match finding.severity.as_str() {
                "error" => "ERROR",
                "warning" => "WARN",
                _ => "INFO",
            };
            print!("  [{prefix}] {}: {}", finding.code, finding.message);
            if let Some(ref fp) = finding.field_path {
                print!(" (at {fp})");
            }
            println!();
        }
    }

    if report.valid {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
```

- [ ] **Step 2: Add grammar module to commands/mod.rs**

In `crates/tasker-ctl/src/commands/mod.rs`, add:

```rust
pub(crate) mod grammar;
pub(crate) use grammar::handle_grammar_command;
```

- [ ] **Step 3: Add Grammar command to main.rs**

In the `Commands` enum (around line 60), add after the last variant (before the closing `}`):

```rust
    /// Grammar discovery and composition validation (TAS-342/343)
    #[command(subcommand)]
    Grammar(commands::grammar::GrammarCommands),
```

In the command match block (around line 947), add:

```rust
        Commands::Grammar(grammar_cmd) => handle_grammar_command(grammar_cmd, &cli.format).await,
```

And add `handle_grammar_command` to the imports from commands at the top of main.rs.

- [ ] **Step 4: Verify compilation**

Run: `cargo check --package tasker-ctl --all-features`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-ctl/src/commands/grammar.rs crates/tasker-ctl/src/commands/mod.rs crates/tasker-ctl/src/main.rs
git commit -m "feat(TAS-343): add grammar command group to tasker-ctl

Grammar discovery, capability inspection, and composition validation
via tasker-ctl grammar {list,inspect,capability,composition} commands.
All commands work offline."
```

---

## Task 6: Final verification and TAS-344 Linear update

- [ ] **Step 1: Run full SDK test suite**

Run: `cargo test --package tasker-sdk --all-features`
Expected: All tests pass

- [ ] **Step 2: Run workspace clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: Zero warnings

- [ ] **Step 3: Run workspace check**

Run: `cargo check --all-features --workspace`
Expected: Clean compilation

- [ ] **Step 4: Run MCP tests**

Run: `cargo test --package tasker-mcp --all-features`
Expected: All tests pass (including updated tool count assertions)

- [ ] **Step 5: Update TAS-344 in Linear**

Update TAS-344's description to include that `composition_explain` MCP tool and CLI command are deferred to that ticket from TAS-342/343. The tool should:
- Accept a CompositionSpec YAML/JSON
- Produce a data flow trace showing how data threads through the composition envelope (`.context`, `.deps`, `.prev`) at each invocation
- Be exposed as both an MCP tool (`composition_explain`) and CLI command (`tasker-ctl grammar composition explain`)

- [ ] **Step 6: Final commit if any cleanup needed**

If clippy or formatting requires changes:
```bash
cargo fmt --all
git add -A
git commit -m "chore(TAS-342/343): clippy and formatting fixes"
```
