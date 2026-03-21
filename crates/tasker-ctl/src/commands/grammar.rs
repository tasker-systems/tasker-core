//! Grammar discovery and composition validation command handlers (TAS-342/343).
//!
//! Provides CLI commands for listing grammar categories, searching/inspecting
//! capabilities, generating vocabulary documentation, and validating composition
//! specs. All commands work offline — no server connection required.

use std::path::PathBuf;

use clap::Subcommand;
use tasker_client::ClientResult;

use crate::output;

/// Top-level grammar subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum GrammarCommands {
    /// List all grammar categories with their capabilities
    List,

    /// Inspect a specific grammar category
    Inspect {
        /// Category name (e.g., Transform, Validate, Assert, Acquire, Persist, Emit)
        category: String,
    },

    /// Capability discovery and inspection
    #[command(subcommand)]
    Capability(CapabilityCommands),

    /// Composition validation
    #[command(subcommand)]
    Composition(CompositionCommands),
}

/// Capability subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum CapabilityCommands {
    /// Search capabilities by name or category
    Search {
        /// Search query (substring match on capability name)
        query: Option<String>,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },

    /// Inspect a single capability in detail
    Inspect {
        /// Capability name (e.g., transform, persist, emit)
        name: String,
    },

    /// Generate full vocabulary documentation
    Document,
}

/// Composition subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum CompositionCommands {
    /// Validate a composition spec file (YAML or JSON)
    Validate {
        /// Path to the composition file
        path: PathBuf,

        /// Validate only the named step's composition within a full task template
        #[arg(long)]
        step: Option<String>,
    },

    /// Explain data flow through a composition spec file
    Explain {
        /// Path to the composition file
        path: PathBuf,

        /// Explain only the named step's composition within a full task template
        #[arg(long)]
        step: Option<String>,

        /// Sample context data (inline JSON or @path/to/file.json)
        #[arg(long)]
        sample_context: Option<String>,

        /// Sample dependency results (inline JSON or @path/to/file.json)
        #[arg(long)]
        sample_deps: Option<String>,

        /// Sample step metadata (inline JSON or @path/to/file.json)
        #[arg(long)]
        sample_step: Option<String>,

        /// Mock outputs for side-effecting invocations (inline JSON or @path/to/file.json)
        #[arg(long)]
        mock_outputs: Option<String>,
    },
}

/// Dispatch a grammar command to the appropriate handler.
pub(crate) async fn handle_grammar_command(cmd: GrammarCommands, format: &str) -> ClientResult<()> {
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
            CompositionCommands::Explain {
                path,
                step,
                sample_context,
                sample_deps,
                sample_step,
                mock_outputs,
            } => composition_explain(
                &path,
                step.as_deref(),
                sample_context.as_deref(),
                sample_deps.as_deref(),
                sample_step.as_deref(),
                mock_outputs.as_deref(),
                format,
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Sub-handlers
// ---------------------------------------------------------------------------

fn grammar_list(format: &str) -> ClientResult<()> {
    let categories = tasker_sdk::grammar_query::list_grammar_categories();

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&categories)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
        return Ok(());
    }

    output::header("Grammar Categories");
    println!();
    for cat in &categories {
        output::label(&cat.name, &cat.description);
        output::dim(format!("  Capabilities: {}", cat.capabilities.join(", ")));
        println!();
    }
    Ok(())
}

fn grammar_inspect(category: &str, format: &str) -> ClientResult<()> {
    let categories = tasker_sdk::grammar_query::list_grammar_categories();

    let cat = categories
        .into_iter()
        .find(|c| c.name.eq_ignore_ascii_case(category));

    let Some(cat) = cat else {
        output::error(format!("Unknown grammar category: {category}"));
        return Err(tasker_client::ClientError::Internal(format!(
            "Unknown grammar category: {category}"
        )));
    };

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&cat)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
        return Ok(());
    }

    output::header(format!("Category: {}", cat.name));
    println!();
    output::label("Description", &cat.description);
    output::label(
        "Capabilities",
        format!("{} registered", cat.capabilities.len()),
    );
    println!();
    for cap_name in &cat.capabilities {
        output::dim(format!("  - {cap_name}"));
    }
    Ok(())
}

fn capability_search(
    query: Option<&str>,
    category: Option<&str>,
    format: &str,
) -> ClientResult<()> {
    let results = tasker_sdk::grammar_query::search_capabilities(query, category);

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&results)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
        return Ok(());
    }

    if results.is_empty() {
        output::warning("No capabilities found matching the given criteria");
        return Ok(());
    }

    output::header(format!("Capabilities ({} found)", results.len()));
    println!();
    println!(
        "  {:<16} {:<12} {:<10} DESCRIPTION",
        "NAME", "CATEGORY", "MUTATING"
    );
    println!("  {}", "-".repeat(72));
    for cap in &results {
        let mutating = if cap.is_mutating { "yes" } else { "no" };
        println!(
            "  {:<16} {:<12} {:<10} {}",
            cap.name, cap.category, mutating, cap.description
        );
    }
    Ok(())
}

fn capability_inspect(name: &str, format: &str) -> ClientResult<()> {
    let Some(detail) = tasker_sdk::grammar_query::inspect_capability(name) else {
        output::error(format!("Unknown capability: {name}"));
        return Err(tasker_client::ClientError::Internal(format!(
            "Unknown capability: {name}"
        )));
    };

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&detail)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
        return Ok(());
    }

    output::header(format!("Capability: {}", detail.name));
    println!();
    output::label("Category", &detail.category);
    output::label("Description", &detail.description);
    output::label("Mutation Profile", &detail.mutation_profile);
    if let Some(idempotency) = detail.supports_idempotency_key {
        output::label("Supports Idempotency Key", idempotency.to_string());
    }
    output::label("Version", &detail.version);
    if !detail.tags.is_empty() {
        output::label("Tags", detail.tags.join(", "));
    }
    println!();
    output::label("Config Schema", "");
    println!(
        "{}",
        serde_json::to_string_pretty(&detail.config_schema).unwrap_or_else(|_| "{}".to_owned())
    );
    Ok(())
}

fn vocabulary_document(format: &str) -> ClientResult<()> {
    let doc = tasker_sdk::grammar_query::document_vocabulary();

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&doc)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
        return Ok(());
    }

    // Markdown-formatted output
    println!("# Tasker Grammar Vocabulary");
    println!();
    println!("Total capabilities: **{}**", doc.total_capabilities);
    println!();

    println!("## Categories");
    println!();
    for cat in &doc.categories {
        println!("### {}", cat.name);
        println!();
        println!("{}", cat.description);
        println!();
        for cap_name in &cat.capabilities {
            println!("- `{cap_name}`");
        }
        println!();
    }

    println!("## Capability Details");
    println!();
    for cap in &doc.capabilities {
        println!("### `{}`", cap.name);
        println!();
        println!("- **Category**: {}", cap.category);
        println!("- **Description**: {}", cap.description);
        println!("- **Mutation Profile**: {}", cap.mutation_profile);
        if let Some(idempotency) = cap.supports_idempotency_key {
            println!("- **Supports Idempotency Key**: {idempotency}");
        }
        println!("- **Version**: {}", cap.version);
        if !cap.tags.is_empty() {
            println!("- **Tags**: {}", cap.tags.join(", "));
        }
        println!();
    }

    Ok(())
}

fn composition_validate(path: &PathBuf, step: Option<&str>, format: &str) -> ClientResult<()> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        tasker_client::ClientError::Internal(format!(
            "Failed to read file '{}': {e}",
            path.display()
        ))
    })?;

    let yaml_to_validate = if let Some(step_name) = step {
        // Parse as full task template and extract the named step's composition
        let template = tasker_sdk::template_parser::parse_template_str(&content).map_err(|e| {
            tasker_client::ClientError::Internal(format!(
                "Failed to parse template '{}': {e}",
                path.display()
            ))
        })?;

        let step_def = template
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or_else(|| {
                tasker_client::ClientError::Internal(format!(
                    "Step '{step_name}' not found in template"
                ))
            })?;

        let composition_value = step_def.composition.as_ref().ok_or_else(|| {
            tasker_client::ClientError::Internal(format!(
                "Step '{step_name}' has no composition field"
            ))
        })?;

        // Convert the composition Value back to YAML for validation
        serde_yaml::to_string(composition_value).map_err(|e| {
            tasker_client::ClientError::Internal(format!(
                "Failed to serialize composition for step '{step_name}': {e}"
            ))
        })?
    } else {
        content
    };

    let report = tasker_sdk::grammar_query::validate_composition_yaml(&yaml_to_validate);

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
    } else {
        if report.valid {
            output::success(&report.summary);
        } else {
            output::error(&report.summary);
        }

        for finding in &report.findings {
            let prefix = match finding.severity.as_str() {
                "error" => "  ERROR",
                "warning" => "  WARN ",
                _ => "  INFO ",
            };
            let location = match (&finding.invocation_index, &finding.field_path) {
                (Some(idx), Some(field)) => format!(" [invocation {idx}, {field}]"),
                (Some(idx), None) => format!(" [invocation {idx}]"),
                _ => String::new(),
            };
            println!(
                "{prefix}{location}: [{code}] {msg}",
                code = finding.code,
                msg = finding.message
            );
        }
    }

    if !report.valid {
        std::process::exit(1);
    }

    Ok(())
}

/// Load a JSON argument: if it starts with `@`, read from file; otherwise parse as inline JSON.
fn load_json_arg(value: &str) -> ClientResult<serde_json::Value> {
    let content = if let Some(path) = value.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|e| {
            tasker_client::ClientError::Internal(format!("Failed to read {path}: {e}"))
        })?
    } else {
        value.to_owned()
    };
    serde_json::from_str(&content)
        .map_err(|e| tasker_client::ClientError::Internal(format!("Invalid JSON: {e}")))
}

fn composition_explain(
    path: &PathBuf,
    step: Option<&str>,
    sample_context: Option<&str>,
    sample_deps: Option<&str>,
    sample_step: Option<&str>,
    mock_outputs: Option<&str>,
    format: &str,
) -> ClientResult<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| tasker_client::ClientError::Internal(format!("Failed to read file: {e}")))?;

    // If --step provided, extract the step's composition from a full template
    let yaml_to_explain = if let Some(step_name) = step {
        let template = tasker_sdk::template_parser::parse_template_str(&content).map_err(|e| {
            tasker_client::ClientError::Internal(format!(
                "Failed to parse template '{}': {e}",
                path.display()
            ))
        })?;

        let step_def = template
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or_else(|| {
                tasker_client::ClientError::Internal(format!(
                    "Step '{step_name}' not found in template"
                ))
            })?;

        let composition_value = step_def.composition.as_ref().ok_or_else(|| {
            tasker_client::ClientError::Internal(format!(
                "Step '{step_name}' has no composition field"
            ))
        })?;

        serde_yaml::to_string(composition_value).map_err(|e| {
            tasker_client::ClientError::Internal(format!(
                "Failed to serialize composition for step '{step_name}': {e}"
            ))
        })?
    } else {
        content
    };

    // Build SimulationInput if any sample data provided
    let simulation = if sample_context.is_some()
        || sample_deps.is_some()
        || sample_step.is_some()
        || mock_outputs.is_some()
    {
        let ctx = sample_context
            .map(load_json_arg)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        let deps = sample_deps
            .map(load_json_arg)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        let step_meta = sample_step
            .map(load_json_arg)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        let mocks: std::collections::HashMap<usize, serde_json::Value> = mock_outputs
            .map(load_json_arg)
            .transpose()?
            .and_then(|v| {
                v.as_object().map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v.clone())))
                        .collect()
                })
            })
            .unwrap_or_default();

        Some(tasker_grammar::SimulationInput {
            context: ctx,
            deps,
            step: step_meta,
            mock_outputs: mocks,
        })
    } else {
        None
    };

    let explanation = tasker_sdk::grammar_query::explain_composition(&yaml_to_explain, simulation);

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&explanation)
                .map_err(|e| tasker_client::ClientError::Internal(e.to_string()))?
        );
    } else {
        // Table output: show invocation chain summary
        if let Some(ref name) = explanation.name {
            output::header(format!("Composition: {name}"));
        } else {
            output::header("Composition Explain");
        }

        println!("Outcome: {}", explanation.outcome.description);
        if explanation.simulated {
            output::success("Simulation mode (sample data provided)");
        }
        println!();

        // Show invocation chain
        if !explanation.invocations.is_empty() {
            println!(
                "  {:<5} {:<12} {:<12} {:<10} {:<30} {:<5}",
                "Index", "Capability", "Category", "Checkpoint", ".prev source", "Exprs"
            );
            println!("  {}", "-".repeat(75));

            for inv in &explanation.invocations {
                let prev_src = inv
                    .envelope_available
                    .prev_source
                    .as_deref()
                    .unwrap_or("(none)");
                let checkpoint = if inv.checkpoint { "yes" } else { "no" };
                println!(
                    "  {:<5} {:<12} {:<12} {:<10} {:<30} {:<5}",
                    inv.index,
                    inv.capability,
                    inv.category,
                    checkpoint,
                    prev_src,
                    inv.expressions.len()
                );

                // Show simulated values if present
                if let Some(ref output) = inv.simulated_output {
                    println!(
                        "         → simulated output: {}",
                        serde_json::to_string(output).unwrap_or_else(|_| "?".to_owned())
                    );
                }
            }
        }

        // Show findings if any
        if !explanation.findings.is_empty() {
            println!();
            for finding in &explanation.findings {
                let prefix = match finding.severity.as_str() {
                    "error" => "  ERROR",
                    "warning" => "  WARN ",
                    _ => "  INFO ",
                };
                let location = match (&finding.invocation_index, &finding.field_path) {
                    (Some(idx), Some(field)) => format!(" [invocation {idx}, {field}]"),
                    (Some(idx), None) => format!(" [invocation {idx}]"),
                    _ => String::new(),
                };
                println!(
                    "{prefix}{location}: [{code}] {msg}",
                    code = finding.code,
                    msg = finding.message
                );
            }
        }

        println!();
        output::dim(&explanation.summary);
    }

    Ok(())
}
