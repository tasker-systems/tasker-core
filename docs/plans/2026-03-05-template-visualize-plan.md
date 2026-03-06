# TAS-316: Template Visualize Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Offline Mermaid diagram + detail table generation from TaskTemplate YAML, exposed via tasker-sdk, tasker-ctl CLI, and tasker-mcp.

**Architecture:** Core visualization logic in `tasker-sdk/src/visualization/` as pure functions. tasker-ctl and tasker-mcp are thin wrappers. No new crate dependencies — just string generation from parsed `TaskTemplate`.

**Tech Stack:** Rust, serde_yaml (already in deps), tasker-shared types (`TaskTemplate`, `StepDefinition`, `StepType`, `RetryConfiguration`)

**Design Doc:** `docs/plans/2026-03-05-template-visualize-design.md`

---

### Task 1: SDK visualization module — types and public API

**Files:**
- Create: `tasker-sdk/src/visualization/mod.rs`
- Modify: `tasker-sdk/src/lib.rs:21-29`

**Step 1: Create the module with types and public API stub**

Create `tasker-sdk/src/visualization/mod.rs`:

```rust
//! Template visualization: Mermaid diagram and detail table generation.

mod detail_table;
mod mermaid;

use std::collections::HashMap;

use serde::Serialize;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Options controlling visualization output.
#[derive(Debug, Default)]
pub struct VisualizeOptions {
    /// When true, only the Mermaid graph is included (no detail table).
    pub graph_only: bool,
}

/// Output from template visualization.
#[derive(Debug, Serialize)]
pub struct VisualizationOutput {
    /// Raw Mermaid graph syntax (no fenced code block markers).
    pub mermaid: String,
    /// Markdown detail table (None when graph_only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_table: Option<String>,
    /// Complete markdown document (fenced Mermaid block + detail table).
    pub markdown: String,
    /// Warnings (e.g., annotation references unknown steps).
    pub warnings: Vec<String>,
}

/// Generate a Mermaid visualization of a task template.
///
/// `annotations` maps step names to developer notes rendered as callouts.
pub fn visualize_template(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> VisualizationOutput {
    let mut warnings = Vec::new();

    // Warn about annotations referencing unknown steps
    let step_names: std::collections::HashSet<&str> =
        template.steps.iter().map(|s| s.name.as_str()).collect();
    for key in annotations.keys() {
        if !step_names.contains(key.as_str()) {
            warnings.push(format!("Annotation references unknown step: '{key}'"));
        }
    }

    let mermaid = mermaid::generate_mermaid(template, annotations);
    let detail_table = if options.graph_only {
        None
    } else {
        Some(detail_table::generate_detail_table(template))
    };

    let markdown = build_markdown(&template.name, &mermaid, detail_table.as_deref());

    VisualizationOutput {
        mermaid,
        detail_table,
        markdown,
        warnings,
    }
}

fn build_markdown(name: &str, mermaid: &str, detail_table: Option<&str>) -> String {
    let mut doc = format!("# {name}\n\n```mermaid\n{mermaid}```\n");
    if let Some(table) = detail_table {
        doc.push_str("\n## Step Details\n\n");
        doc.push_str(table);
    }
    doc
}
```

**Step 2: Register the module in lib.rs**

Modify `tasker-sdk/src/lib.rs`. Add after line 29 (`pub mod template_validator;`):

```rust
pub mod visualization;
```

Also add to the doc comment (after the `template_validator` line):

```rust
//! - [`visualization`] — Mermaid diagram and detail table generation from task templates
```

**Step 3: Create stub files for submodules**

Create `tasker-sdk/src/visualization/mermaid.rs`:

```rust
//! Mermaid flowchart generation from TaskTemplate.

use std::collections::HashMap;
use tasker_shared::models::core::task_template::TaskTemplate;

pub(super) fn generate_mermaid(
    _template: &TaskTemplate,
    _annotations: &HashMap<String, String>,
) -> String {
    todo!()
}
```

Create `tasker-sdk/src/visualization/detail_table.rs`:

```rust
//! Markdown detail table generation from TaskTemplate.

use tasker_shared::models::core::task_template::TaskTemplate;

pub(super) fn generate_detail_table(_template: &TaskTemplate) -> String {
    todo!()
}
```

**Step 4: Verify it compiles**

Run: `cargo check --all-features -p tasker-sdk`
Expected: compiles (stubs are unused at this point)

**Step 5: Commit**

```bash
git add tasker-sdk/src/visualization/ tasker-sdk/src/lib.rs
git commit -m "feat(TAS-316): add visualization module structure with types and public API"
```

---

### Task 2: Mermaid graph generation

**Files:**
- Modify: `tasker-sdk/src/visualization/mermaid.rs`

**Step 1: Write tests for Mermaid generation**

Add to the bottom of `tasker-sdk/src/visualization/mermaid.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tasker_sdk::template_parser::parse_template_str;

    #[test]
    fn test_diamond_dag_generates_valid_mermaid() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        assert!(result.starts_with("graph TD\n"));
        // Should have 4 nodes
        assert!(result.contains("diamond_start_py[diamond_start_py]"));
        assert!(result.contains("diamond_branch_b_py[diamond_branch_b_py]"));
        assert!(result.contains("diamond_branch_c_py[diamond_branch_c_py]"));
        assert!(result.contains("diamond_end_py[diamond_end_py]"));
        // Should have 4 edges
        assert!(result.contains("diamond_start_py --> diamond_branch_b_py"));
        assert!(result.contains("diamond_start_py --> diamond_branch_c_py"));
        assert!(result.contains("diamond_branch_b_py --> diamond_end_py"));
        assert!(result.contains("diamond_branch_c_py --> diamond_end_py"));
    }

    #[test]
    fn test_linear_chain_generates_sequential_edges() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        assert!(result.contains("linear_step_1_py --> linear_step_2_py"));
        assert!(result.contains("linear_step_2_py --> linear_step_3_py"));
        assert!(result.contains("linear_step_3_py --> linear_step_4_py"));
    }

    #[test]
    fn test_convergent_dag_with_schema() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        // Convergence: both enrich_order and process_payment feed into generate_report
        assert!(result.contains("enrich_order --> generate_report"));
        assert!(result.contains("process_payment --> generate_report"));
    }

    #[test]
    fn test_annotations_render_in_node_label() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("process_payment".to_string(), "Not retry-safe".to_string());

        let result = generate_mermaid(&template, &annotations);

        // Annotated node should have the annotation text and the annotated class
        assert!(result.contains("classDef annotated fill:#fff3cd,stroke:#ffc107"));
        assert!(result.contains("Not retry-safe"));
        assert!(result.contains(":::annotated"));
    }

    #[test]
    fn test_decision_step_uses_diamond_shape() {
        let yaml = r#"
name: decision_test
namespace_name: test
version: "1.0.0"
steps:
  - name: check_input
    handler:
      callable: test.check
  - name: decide
    handler:
      callable: test.decide
    type: decision
    depends_on: [check_input]
"#;
        let template = parse_template_str(yaml).unwrap();
        let result = generate_mermaid(&template, &HashMap::new());

        // Decision step should use diamond shape
        assert!(result.contains("decide{decide}"));
        // Standard step should use rectangle
        assert!(result.contains("check_input[check_input]"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --features test-messaging -p tasker-sdk visualization::mermaid::tests -- --nocapture`
Expected: FAIL (todo! panics)

**Step 3: Implement generate_mermaid**

Replace the stub in `tasker-sdk/src/visualization/mermaid.rs`:

```rust
//! Mermaid flowchart generation from TaskTemplate.

use std::collections::HashMap;
use std::fmt::Write;

use tasker_shared::models::core::task_template::{StepType, TaskTemplate};

pub(super) fn generate_mermaid(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
) -> String {
    let mut out = String::new();
    writeln!(out, "graph TD").unwrap();

    // Class definitions for annotated nodes
    if !annotations.is_empty() {
        writeln!(out, "    classDef annotated fill:#fff3cd,stroke:#ffc107").unwrap();
    }

    // Node definitions
    for step in &template.steps {
        let annotation = annotations.get(&step.name);
        let node = format_node(&step.name, step.step_type, annotation);
        writeln!(out, "    {node}").unwrap();
    }

    writeln!(out).unwrap();

    // Edge definitions (sorted for deterministic output)
    let mut edges: Vec<String> = Vec::new();
    for step in &template.steps {
        let mut deps: Vec<&str> = step.dependencies.iter().map(|s| s.as_str()).collect();
        deps.sort();
        for dep in deps {
            edges.push(format!("    {dep} --> {}", step.name));
        }
    }
    for edge in &edges {
        writeln!(out, "{edge}").unwrap();
    }

    out
}

fn format_node(name: &str, step_type: StepType, annotation: Option<&String>) -> String {
    match (step_type, annotation) {
        (StepType::Decision, Some(text)) => {
            format!("{name}{{\"{}\\n{} {text}\"}}", name, "⚠")
        }
        (StepType::Decision, None) => {
            format!("{name}{{{name}}}")
        }
        (_, Some(text)) => {
            format!("{name}[\"{name}\\n{} {text}\"]:::annotated", "⚠")
        }
        (_, None) => {
            format!("{name}[{name}]")
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --features test-messaging -p tasker-sdk visualization::mermaid::tests -- --nocapture`
Expected: all 5 tests PASS

**Step 5: Commit**

```bash
git add tasker-sdk/src/visualization/mermaid.rs
git commit -m "feat(TAS-316): implement Mermaid graph generation with node styling and annotations"
```

---

### Task 3: Detail table generation

**Files:**
- Modify: `tasker-sdk/src/visualization/detail_table.rs`

**Step 1: Write tests for detail table**

Add to `tasker-sdk/src/visualization/detail_table.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tasker_sdk::template_parser::parse_template_str;

    #[test]
    fn test_detail_table_has_header_row() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        assert!(result.contains("| Step | Type | Handler | Dependencies | Schema Fields | Retry |"));
        assert!(result.contains("|------|------|---------|"));
    }

    #[test]
    fn test_detail_table_rows_in_topological_order() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        // validate_order should appear before enrich_order (it's a dependency)
        let validate_pos = result.find("validate_order").unwrap();
        let enrich_pos = result.find("enrich_order").unwrap();
        assert!(validate_pos < enrich_pos);
    }

    #[test]
    fn test_detail_table_shows_schema_fields() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        // validate_order has result_schema with fields
        let lines: Vec<&str> = result.lines().collect();
        let validate_line = lines.iter().find(|l| l.contains("| validate_order")).unwrap();
        // Should NOT show "—" for schema fields (it has a schema)
        assert!(!validate_line.ends_with("— |"));
    }

    #[test]
    fn test_detail_table_root_step_shows_dash_for_deps() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        let lines: Vec<&str> = result.lines().collect();
        let validate_line = lines.iter().find(|l| l.contains("| validate_order")).unwrap();
        // Root step has no dependencies
        assert!(validate_line.contains("| \u{2014} |") || validate_line.contains("| — |"));
    }

    #[test]
    fn test_detail_table_shows_retry_config() {
        let yaml = r#"
name: retry_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_with_retry
    handler:
      callable: test.retry_step
    retry:
      retryable: true
      max_attempts: 5
      backoff: exponential
  - name: step_no_retry
    handler:
      callable: test.no_retry
    retry:
      retryable: false
    depends_on: [step_with_retry]
"#;
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        assert!(result.contains("5x exponential"));
        assert!(result.contains("\u{2014}") || result.contains("—")); // no-retry step
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --features test-messaging -p tasker-sdk visualization::detail_table::tests -- --nocapture`
Expected: FAIL (todo! panics)

**Step 3: Implement generate_detail_table**

Replace the stub in `tasker-sdk/src/visualization/detail_table.rs`:

```rust
//! Markdown detail table generation from TaskTemplate.

use std::collections::HashMap;
use std::fmt::Write;

use tasker_shared::models::core::task_template::TaskTemplate;

pub(super) fn generate_detail_table(template: &TaskTemplate) -> String {
    let mut out = String::new();

    // Header
    writeln!(out, "| Step | Type | Handler | Dependencies | Schema Fields | Retry |").unwrap();
    writeln!(out, "|------|------|---------|--------------|---------------|-------|").unwrap();

    // Build step lookup for topological ordering
    let order = topological_order(template);

    for name in &order {
        let Some(step) = template.steps.iter().find(|s| &s.name == name) else {
            continue;
        };

        let step_type = format!("{:?}", step.step_type);
        let handler = &step.handler.callable;
        let deps = if step.dependencies.is_empty() {
            "\u{2014}".to_string()
        } else {
            step.dependencies.join(", ")
        };
        let schema_fields = extract_schema_field_names(&step.result_schema);
        let retry = format_retry(&step.retry);

        writeln!(
            out,
            "| {name} | {step_type} | {handler} | {deps} | {schema_fields} | {retry} |"
        )
        .unwrap();
    }

    out
}

fn topological_order(template: &TaskTemplate) -> Vec<String> {
    use std::collections::VecDeque;

    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &template.steps {
        in_degree.entry(&step.name).or_insert(0);
        for dep in &step.dependencies {
            adj.entry(dep.as_str()).or_default().push(&step.name);
            *in_degree.entry(&step.name).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = {
        let mut roots: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name)
            .collect();
        roots.sort();
        roots.into_iter().collect()
    };

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next: Vec<&str> = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(neighbor);
                    }
                }
            }
            next.sort();
            queue.extend(next);
        }
    }

    result
}

fn extract_schema_field_names(schema: &Option<serde_json::Value>) -> String {
    let Some(schema) = schema else {
        return "\u{2014}".to_string();
    };
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return "\u{2014}".to_string();
    };
    let mut names: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
    names.sort();
    if names.is_empty() {
        "\u{2014}".to_string()
    } else {
        names.join(", ")
    }
}

fn format_retry(retry: &tasker_shared::models::core::task_template::RetryConfiguration) -> String {
    if !retry.retryable {
        return "\u{2014}".to_string();
    }
    // Default retry config (retryable=true, 3 attempts, exponential) is the norm —
    // only show non-default configs
    if retry.max_attempts == 3
        && retry.backoff == tasker_shared::models::core::task_template::BackoffStrategy::Exponential
    {
        return "\u{2014}".to_string();
    }
    format!("{}x {:?}", retry.max_attempts, retry.backoff).to_lowercase()
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --features test-messaging -p tasker-sdk visualization::detail_table::tests -- --nocapture`
Expected: all 5 tests PASS

**Step 5: Commit**

```bash
git add tasker-sdk/src/visualization/detail_table.rs
git commit -m "feat(TAS-316): implement detail table generation with topological ordering"
```

---

### Task 4: Integration tests for the public API (visualize_template)

**Files:**
- Modify: `tasker-sdk/src/visualization/mod.rs` (add tests)

**Step 1: Write integration tests for visualize_template**

Add to the bottom of `tasker-sdk/src/visualization/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tasker_sdk::template_parser::parse_template_str;

    #[test]
    fn test_full_markdown_output() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        // Markdown contains fenced mermaid block
        assert!(output.markdown.contains("```mermaid"));
        assert!(output.markdown.contains("graph TD"));
        // Markdown contains detail table
        assert!(output.markdown.contains("## Step Details"));
        assert!(output.markdown.contains("| Step |"));
        // detail_table is present
        assert!(output.detail_table.is_some());
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_graph_only_mode() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let options = VisualizeOptions { graph_only: true };
        let output = visualize_template(&template, &HashMap::new(), &options);

        assert!(output.detail_table.is_none());
        assert!(!output.markdown.contains("## Step Details"));
        // But mermaid should still be present
        assert!(output.markdown.contains("```mermaid"));
    }

    #[test]
    fn test_annotation_warning_for_unknown_step() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let mut annotations = HashMap::new();
        annotations.insert("nonexistent_step".to_string(), "note".to_string());

        let output = visualize_template(&template, &annotations, &VisualizeOptions::default());

        assert_eq!(output.warnings.len(), 1);
        assert!(output.warnings[0].contains("nonexistent_step"));
    }

    #[test]
    fn test_diamond_dag_full_output() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/python/diamond_workflow_handler_py.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output.mermaid.contains("diamond_start_py --> diamond_branch_b_py"));
        assert!(output.detail_table.is_some());
    }

    #[test]
    fn test_linear_chain_full_output() {
        let yaml = include_str!("../../../tests/fixtures/task_templates/python/linear_workflow_handler_py.yaml");
        let template = parse_template_str(yaml).unwrap();
        let output = visualize_template(&template, &HashMap::new(), &VisualizeOptions::default());

        assert!(output.mermaid.contains("linear_step_1_py --> linear_step_2_py"));
        assert!(output.detail_table.is_some());
    }
}
```

**Step 2: Run all visualization tests**

Run: `cargo test --features test-messaging -p tasker-sdk visualization -- --nocapture`
Expected: all tests PASS (15 total across mermaid, detail_table, and mod)

**Step 3: Commit**

```bash
git add tasker-sdk/src/visualization/mod.rs
git commit -m "test(TAS-316): add integration tests for visualize_template public API"
```

---

### Task 5: tasker-ctl template visualize command

**Files:**
- Modify: `tasker-ctl/src/main.rs:634-704` (add Visualize variant)
- Modify: `tasker-ctl/src/commands/template.rs` (add handler)

**Step 1: Add Visualize variant to TemplateCommands**

In `tasker-ctl/src/main.rs`, add after the `Generate` variant (after line 703, before the closing `}`):

```rust
    /// Generate a Mermaid diagram visualization of a task template's DAG structure
    Visualize {
        /// Path to template YAML file, or "-" for stdin
        #[arg(value_name = "TEMPLATE")]
        template: String,

        /// Path to annotations YAML file (step_name: "note" pairs)
        #[arg(short, long)]
        annotations: Option<String>,

        /// Write output to file instead of stdout
        #[arg(short, long)]
        output: Option<String>,

        /// Emit only the raw Mermaid graph (no markdown fences or detail table)
        #[arg(long)]
        graph_only: bool,
    },
```

**Step 2: Add handler in template.rs**

In `tasker-ctl/src/commands/template.rs`, add the match arm in `handle_template_command` (after the `Generate` arm, before the closing `}`):

```rust
        TemplateCommands::Visualize {
            template,
            annotations,
            output: output_path,
            graph_only,
        } => visualize_template_command(&template, annotations.as_deref(), output_path.as_deref(), graph_only),
```

Add the necessary imports at the top of `template.rs`:

```rust
use std::io::Read;
use tasker_sdk::template_parser::parse_template_str;
use tasker_sdk::visualization::{self, VisualizeOptions};
```

Add the handler function at the bottom of `template.rs`:

```rust
fn visualize_template_command(
    template_path: &str,
    annotations_path: Option<&str>,
    output_path: Option<&str>,
    graph_only: bool,
) -> tasker_client::ClientResult<()> {
    // Read template YAML from file or stdin
    let yaml = if template_path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!("failed to read stdin: {e}"))
        })?;
        buf
    } else {
        std::fs::read_to_string(template_path).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!(
                "failed to read template '{}': {e}",
                template_path
            ))
        })?
    };

    // Parse template
    let template = parse_template_str(&yaml).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("invalid template YAML: {e}"))
    })?;

    // Load annotations if provided
    let annotations = if let Some(path) = annotations_path {
        let content = std::fs::read_to_string(path).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!(
                "failed to read annotations '{}': {e}",
                path
            ))
        })?;
        serde_yaml::from_str(&content).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!("invalid annotations YAML: {e}"))
        })?
    } else {
        std::collections::HashMap::new()
    };

    let options = VisualizeOptions { graph_only };
    let result = visualization::visualize_template(&template, &annotations, &options);

    // Print warnings
    for warning in &result.warnings {
        output::warning(warning);
    }

    // Determine output content
    let content = if graph_only {
        &result.mermaid
    } else {
        &result.markdown
    };

    // Write to file or stdout
    if let Some(path) = output_path {
        std::fs::write(path, content).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!(
                "failed to write output '{}': {e}",
                path
            ))
        })?;
        output::success(format!("Wrote visualization to {path}"));
    } else {
        print!("{content}");
    }

    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cargo check --all-features -p tasker-ctl`
Expected: compiles

**Step 4: Test the CLI command manually**

Run: `cargo run --all-features -p tasker-ctl -- template visualize tests/fixtures/task_templates/codegen_test_template.yaml`
Expected: Markdown output with Mermaid diagram and detail table printed to stdout

Run: `cargo run --all-features -p tasker-ctl -- template visualize tests/fixtures/task_templates/codegen_test_template.yaml --graph-only`
Expected: Only the raw Mermaid graph printed

Run: `cat tests/fixtures/task_templates/codegen_test_template.yaml | cargo run --all-features -p tasker-ctl -- template visualize -`
Expected: Same output as file input

**Step 5: Commit**

```bash
git add tasker-ctl/src/main.rs tasker-ctl/src/commands/template.rs
git commit -m "feat(TAS-316): add 'template visualize' CLI command with stdin, annotations, and graph-only support"
```

---

### Task 6: MCP template_visualize tool

**Files:**
- Modify: `tasker-mcp/src/tools/params.rs` (add param/response types)
- Modify: `tasker-mcp/src/tools/developer.rs` (add tool function)
- Modify: `tasker-mcp/src/server.rs` (register tool)

**Step 1: Add param and response types to params.rs**

Add after the `TemplateValidateParams` section (after line 15) in `tasker-mcp/src/tools/params.rs`:

```rust
// ── template_visualize ──

/// Parameters for the `template_visualize` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateVisualizeParams {
    /// Task template YAML content to visualize.
    #[schemars(description = "Task template YAML content to visualize as a Mermaid flowchart")]
    pub template_yaml: String,
    /// Optional annotations mapping step names to developer notes.
    #[schemars(description = "Optional annotations: step_name → note text (rendered as callouts on diagram nodes)")]
    #[serde(default)]
    pub annotations: Option<HashMap<String, String>>,
    /// When true, returns only the Mermaid graph without detail table.
    #[schemars(description = "When true, returns only the Mermaid graph (no detail table)")]
    #[serde(default)]
    pub graph_only: Option<bool>,
}
```

Add `HashMap` import at the top of params.rs:

```rust
use std::collections::HashMap;
```

**Step 2: Add tool function to developer.rs**

Add after the `template_validate` function (after line 31) in `tasker-mcp/src/tools/developer.rs`:

```rust
pub fn template_visualize(params: TemplateVisualizeParams) -> String {
    match parse_template_str(&params.template_yaml) {
        Ok(template) => {
            let annotations = params.annotations.unwrap_or_default();
            let options = tasker_sdk::visualization::VisualizeOptions {
                graph_only: params.graph_only.unwrap_or(false),
            };
            let output = tasker_sdk::visualization::visualize_template(&template, &annotations, &options);
            serde_json::to_string_pretty(&output)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("yaml_parse_error", &e.to_string()),
    }
}
```

Add to the import block at the top of developer.rs (in the `use super::params::` block):

```rust
TemplateVisualizeParams,
```

**Step 3: Register tool in server.rs**

Add after the `template_validate` tool registration (after line 309) in `tasker-mcp/src/server.rs`:

```rust
    /// Generate a Mermaid flowchart diagram from a task template, showing the step
    /// DAG with dependency edges and per-step details.
    #[tool(
        name = "template_visualize",
        description = "Generate a Mermaid flowchart diagram from a task template. Shows step DAG with dependency edges, node styling for step types (standard/decision), optional developer annotations, and a detail table with handler, schema, and retry information. Works entirely offline."
    )]
    pub async fn template_visualize(
        &self,
        Parameters(params): Parameters<TemplateVisualizeParams>,
    ) -> String {
        developer::template_visualize(params)
    }
```

**Step 4: Add test in developer.rs**

Add to the `#[cfg(test)] mod tests` section in `developer.rs`:

```rust
    #[test]
    fn test_template_visualize() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = template_visualize(TemplateVisualizeParams {
            template_yaml: yaml.to_string(),
            annotations: None,
            graph_only: None,
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["mermaid"].as_str().unwrap().contains("graph TD"));
        assert!(parsed["detail_table"].is_string());
        assert!(parsed["markdown"].as_str().unwrap().contains("```mermaid"));
        assert!(parsed["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_template_visualize_with_annotations() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let mut annotations = std::collections::HashMap::new();
        annotations.insert("validate_order".to_string(), "Needs schema".to_string());
        annotations.insert("ghost_step".to_string(), "Does not exist".to_string());

        let result = template_visualize(TemplateVisualizeParams {
            template_yaml: yaml.to_string(),
            annotations: Some(annotations),
            graph_only: None,
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["mermaid"].as_str().unwrap().contains("Needs schema"));
        // Should warn about unknown step
        let warnings = parsed["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].as_str().unwrap().contains("ghost_step"));
    }

    #[test]
    fn test_template_visualize_graph_only() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = template_visualize(TemplateVisualizeParams {
            template_yaml: yaml.to_string(),
            annotations: None,
            graph_only: Some(true),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["detail_table"].is_null());
    }
```

**Step 5: Run MCP tests**

Run: `cargo test --features test-messaging -p tasker-mcp template_visualize -- --nocapture`
Expected: all 3 tests PASS

**Step 6: Run full quality checks**

Run: `cargo check --all-features && cargo clippy --all-targets --all-features --workspace`
Expected: no errors, no warnings

**Step 7: Commit**

```bash
git add tasker-mcp/src/tools/params.rs tasker-mcp/src/tools/developer.rs tasker-mcp/src/server.rs
git commit -m "feat(TAS-316): add template_visualize MCP Tier 1 tool"
```

---

### Task 7: Quality checks and final verification

**Files:**
- No new files

**Step 1: Run the full test suite for affected crates**

Run: `cargo test --features test-messaging -p tasker-sdk -p tasker-mcp -p tasker-ctl -- --nocapture`
Expected: all tests PASS

**Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: zero warnings

**Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues

**Step 4: Run the full check suite**

Run: `cargo make check`
Expected: all checks pass

---

### Task 8: Generate visualizations for contrib examples (validation + deliverable)

**Note:** This task requires access to `tasker-contrib/examples/` templates. If not available locally, skip this task and note it for follow-up.

**Step 1: Find available example templates**

Run: `find ../tasker-contrib/examples -name "*.yaml" -o -name "*.yml" | head -20`
Expected: list of example template YAML files

**Step 2: Generate Mermaid markdown for each example**

For each template found, run:

```bash
cargo run --all-features -p tasker-ctl -- template visualize <path-to-template.yaml> --output <path-to-template-viz.md>
```

**Step 3: Review generated output**

Manually inspect 2-3 generated files to verify:
- Mermaid graph renders correctly (paste into a Mermaid preview)
- Detail table columns are populated correctly
- No warnings in output

**Step 4: Commit generated visualizations**

```bash
# In tasker-contrib repo
git add examples/
git commit -m "docs(TAS-316): add generated Mermaid visualizations for example templates"
```

---

## Summary

| Task | Component | Deliverable |
|------|-----------|-------------|
| 1 | tasker-sdk | Module structure, types, public API |
| 2 | tasker-sdk | Mermaid graph generation |
| 3 | tasker-sdk | Detail table generation |
| 4 | tasker-sdk | Integration tests for public API |
| 5 | tasker-ctl | `template visualize` CLI command |
| 6 | tasker-mcp | `template_visualize` Tier 1 MCP tool |
| 7 | All | Quality checks and full verification |
| 8 | tasker-contrib | Generated visualization files for examples |
