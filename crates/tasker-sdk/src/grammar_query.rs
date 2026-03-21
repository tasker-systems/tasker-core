//! Grammar vocabulary query functions for capability discovery.
//!
//! This module provides read-only introspection into the Tasker grammar system,
//! enabling `tasker-ctl` and `tasker-mcp` to list categories, search capabilities,
//! inspect individual capability details, and generate full vocabulary documentation.

use serde::Serialize;
use serde_json::Value;
use tasker_grammar::{standard_capability_registry, GrammarCategoryKind, MutationProfile};

// ---------------------------------------------------------------------------
// Return type structs
// ---------------------------------------------------------------------------

/// Summary information about a grammar category.
#[derive(Debug, Serialize)]
pub struct GrammarCategoryInfo {
    /// Category name (e.g., "Transform", "Persist").
    pub name: String,
    /// Human-readable description of the category.
    pub description: String,
    /// Names of capabilities belonging to this category.
    pub capabilities: Vec<String>,
}

/// Lightweight summary of a capability (used in search results).
#[derive(Debug, Serialize)]
pub struct CapabilitySummary {
    /// Capability name (e.g., "transform", "persist").
    pub name: String,
    /// Category this capability belongs to.
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this capability mutates external state.
    pub is_mutating: bool,
}

/// Full detail for a single capability.
#[derive(Debug, Serialize)]
pub struct CapabilityDetail {
    /// Capability name.
    pub name: String,
    /// Category this capability belongs to.
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the capability's configuration.
    pub config_schema: Value,
    /// Mutation profile string: "non_mutating", "mutating", or "config_dependent".
    pub mutation_profile: String,
    /// Whether this capability supports idempotency keys (Some for Mutating, None otherwise).
    pub supports_idempotency_key: Option<bool>,
    /// Discovery tags.
    pub tags: Vec<String>,
    /// Semantic version of the capability declaration.
    pub version: String,
}

/// Complete vocabulary documentation combining categories and capability details.
#[derive(Debug, Serialize)]
pub struct VocabularyDocumentation {
    /// All grammar categories with their descriptions and capability lists.
    pub categories: Vec<GrammarCategoryInfo>,
    /// Full detail for every registered capability.
    pub capabilities: Vec<CapabilityDetail>,
    /// Total number of capabilities in the vocabulary.
    pub total_capabilities: usize,
}

/// A single finding from composition validation.
#[derive(Debug, Serialize)]
pub struct CompositionFinding {
    /// Severity level (e.g., "error", "warning", "info").
    pub severity: String,
    /// Machine-readable finding code.
    pub code: String,
    /// Human-readable message describing the finding.
    pub message: String,
    /// Index of the capability invocation that triggered the finding, if applicable.
    pub invocation_index: Option<usize>,
    /// Dot-separated field path within the invocation, if applicable.
    pub field_path: Option<String>,
}

/// Result of validating a composition YAML/JSON document.
#[derive(Debug, Serialize)]
pub struct CompositionValidationReport {
    /// Whether the composition passed all checks.
    pub valid: bool,
    /// Individual findings (errors, warnings, info).
    pub findings: Vec<CompositionFinding>,
    /// Human-readable summary of validation results.
    pub summary: String,
}

/// Summary of the declared outcome for a composition explanation.
#[derive(Debug, Serialize)]
pub struct OutcomeInfo {
    /// Human-readable description of what the composition achieves.
    pub description: String,
    /// JSON Schema for the composition's output.
    pub output_schema: Value,
}

/// Snapshot of what envelope fields are available at an invocation point.
#[derive(Debug, Serialize)]
pub struct EnvelopeSnapshotInfo {
    /// Always true — task-level input.
    pub context: bool,
    /// Always true — dependency step results.
    pub deps: bool,
    /// Always true — step metadata.
    pub step: bool,
    /// Whether .prev is non-null at this point.
    pub has_prev: bool,
    /// Description of what .prev contains.
    pub prev_source: Option<String>,
    /// Schema of .prev if known (from prior invocation's output schema).
    pub prev_schema: Option<Value>,
}

/// A jaq expression found in an invocation's config.
#[derive(Debug, Serialize)]
pub struct ExpressionReferenceInfo {
    /// Config field path (e.g., "filter", "data.expression").
    pub field_path: String,
    /// The raw expression string.
    pub expression: String,
    /// Envelope paths referenced (e.g., [".context.order_id", ".prev.total"]).
    pub referenced_paths: Vec<String>,
    /// Simulated result value (when sample data provided).
    pub simulated_result: Option<Value>,
}

/// Trace for a single capability invocation within a composition explanation.
#[derive(Debug, Serialize)]
pub struct InvocationExplanation {
    /// Position in the invocation chain (0-based).
    pub index: usize,
    /// Capability name.
    pub capability: String,
    /// Grammar category (as string, e.g., "Transform", "Persist").
    pub category: String,
    /// Whether this is a checkpoint boundary.
    pub checkpoint: bool,
    /// Whether this capability mutates external state.
    pub is_mutating: bool,
    /// Envelope fields available at this invocation.
    pub envelope_available: EnvelopeSnapshotInfo,
    /// Jaq expressions found in config and which envelope paths they reference.
    pub expressions: Vec<ExpressionReferenceInfo>,
    /// Declared output schema (if any — transforms declare this).
    pub output_schema: Option<Value>,
    /// Simulated output value (when sample data provided).
    pub simulated_output: Option<Value>,
    /// For side-effecting capabilities: whether a mock output was provided.
    pub mock_output_used: bool,
}

/// Result of explaining data flow through a composition.
#[derive(Debug, Serialize)]
pub struct CompositionExplanation {
    /// Composition name (if declared).
    pub name: Option<String>,
    /// Declared outcome description and output schema.
    pub outcome: OutcomeInfo,
    /// Per-invocation trace entries, in execution order.
    pub invocations: Vec<InvocationExplanation>,
    /// Validation findings (errors/warnings) from the underlying validator.
    pub findings: Vec<CompositionFinding>,
    /// Whether simulation was performed (sample data provided).
    pub simulated: bool,
    /// Human-readable summary of the composition.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Constants and helpers
// ---------------------------------------------------------------------------

/// All grammar category kinds in canonical order.
pub const ALL_CATEGORIES: &[GrammarCategoryKind] = &[
    GrammarCategoryKind::Transform,
    GrammarCategoryKind::Validate,
    GrammarCategoryKind::Assert,
    GrammarCategoryKind::Acquire,
    GrammarCategoryKind::Persist,
    GrammarCategoryKind::Emit,
];

/// Convert a [`MutationProfile`] to its canonical string representation.
pub fn mutation_profile_str(profile: &MutationProfile) -> String {
    match profile {
        MutationProfile::NonMutating => "non_mutating".to_owned(),
        MutationProfile::Mutating { .. } => "mutating".to_owned(),
        MutationProfile::ConfigDependent => "config_dependent".to_owned(),
    }
}

/// Extract the idempotency key support flag from a [`MutationProfile`].
///
/// Returns `Some(bool)` for `Mutating` profiles and `None` otherwise.
pub fn supports_idempotency(profile: &MutationProfile) -> Option<bool> {
    match profile {
        MutationProfile::Mutating {
            supports_idempotency_key,
        } => Some(*supports_idempotency_key),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Public query functions
// ---------------------------------------------------------------------------

/// List all grammar categories with their descriptions and associated capabilities.
///
/// Categories are returned in canonical order (Transform, Validate, Assert,
/// Acquire, Persist, Emit). Each category includes the names of capabilities
/// from the standard registry that belong to it.
pub fn list_grammar_categories() -> Vec<GrammarCategoryInfo> {
    let registry = standard_capability_registry();

    ALL_CATEGORIES
        .iter()
        .map(|kind| {
            let category = kind.into_category();
            let mut capabilities: Vec<String> = registry
                .values()
                .filter(|decl| decl.grammar_category == *kind)
                .map(|decl| decl.name.clone())
                .collect();
            capabilities.sort();

            GrammarCategoryInfo {
                name: kind.to_string(),
                description: category.description().to_owned(),
                capabilities,
            }
        })
        .collect()
}

/// Search capabilities by name substring and/or category filter.
///
/// Both filters are case-insensitive. When both are provided, results must
/// match both criteria. Results are sorted alphabetically by name.
pub fn search_capabilities(query: Option<&str>, category: Option<&str>) -> Vec<CapabilitySummary> {
    let registry = standard_capability_registry();
    let query_lower = query.map(|q| q.to_ascii_lowercase());
    let category_lower = category.map(|c| c.to_ascii_lowercase());

    let mut results: Vec<CapabilitySummary> = registry
        .values()
        .filter(|decl| {
            if let Some(ref q) = query_lower {
                if !decl.name.to_ascii_lowercase().contains(q.as_str()) {
                    return false;
                }
            }
            if let Some(ref c) = category_lower {
                if decl.grammar_category.to_string().to_ascii_lowercase() != *c {
                    return false;
                }
            }
            true
        })
        .map(|decl| CapabilitySummary {
            name: decl.name.clone(),
            category: decl.grammar_category.to_string(),
            description: decl.description.clone(),
            is_mutating: matches!(decl.mutation_profile, MutationProfile::Mutating { .. }),
        })
        .collect();

    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

/// Inspect a single capability by exact name, returning full detail.
///
/// Returns `None` if no capability with the given name exists in the standard registry.
pub fn inspect_capability(name: &str) -> Option<CapabilityDetail> {
    let registry = standard_capability_registry();
    registry.get(name).map(|decl| CapabilityDetail {
        name: decl.name.clone(),
        category: decl.grammar_category.to_string(),
        description: decl.description.clone(),
        config_schema: decl.config_schema.clone(),
        mutation_profile: mutation_profile_str(&decl.mutation_profile),
        supports_idempotency_key: supports_idempotency(&decl.mutation_profile),
        tags: decl.tags.clone(),
        version: decl.version.clone(),
    })
}

/// Validate a standalone composition spec from YAML or JSON string.
///
/// Parses the input, runs `CompositionValidator` with the standard capability
/// registry, and returns a structured report.
pub fn validate_composition_yaml(yaml_str: &str) -> CompositionValidationReport {
    use tasker_grammar::validation::CompositionValidator;
    use tasker_grammar::{CompositionSpec, ExpressionEngine, Severity};

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

/// Explain data flow through a composition, optionally with simulation.
///
/// Parses the input (YAML first, then JSON), constructs an `ExplainAnalyzer`
/// with the standard capability registry, and returns a structured trace.
/// If simulation input is provided, expressions are evaluated against sample data.
pub fn explain_composition(
    yaml_str: &str,
    simulation: Option<tasker_grammar::SimulationInput>,
) -> CompositionExplanation {
    use tasker_grammar::{CompositionSpec, ExplainAnalyzer, ExpressionEngine, Severity};

    // Try YAML first, then JSON
    let spec: CompositionSpec = match serde_yaml::from_str(yaml_str) {
        Ok(s) => s,
        Err(yaml_err) => match serde_json::from_str(yaml_str) {
            Ok(s) => s,
            Err(_) => {
                return CompositionExplanation {
                    name: None,
                    outcome: OutcomeInfo {
                        description: String::new(),
                        output_schema: Value::Object(Default::default()),
                    },
                    invocations: vec![],
                    findings: vec![CompositionFinding {
                        severity: "error".to_owned(),
                        code: "PARSE_ERROR".to_owned(),
                        message: format!("Failed to parse composition: {yaml_err}"),
                        invocation_index: None,
                        field_path: None,
                    }],
                    simulated: false,
                    summary: "Composition could not be parsed".to_owned(),
                };
            }
        },
    };

    let registry = standard_capability_registry();
    let engine = ExpressionEngine::with_defaults();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let trace = if let Some(sim) = simulation {
        analyzer.analyze_with_simulation(&spec, &sim)
    } else {
        analyzer.analyze(&spec)
    };

    // Map validation findings
    let findings: Vec<CompositionFinding> = trace
        .validation
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

    // Map invocations
    let invocations: Vec<InvocationExplanation> = trace
        .invocations
        .into_iter()
        .map(|inv| InvocationExplanation {
            index: inv.index,
            capability: inv.capability,
            category: inv.category.to_string(),
            checkpoint: inv.checkpoint,
            is_mutating: inv.is_mutating,
            envelope_available: EnvelopeSnapshotInfo {
                context: inv.envelope_available.context,
                deps: inv.envelope_available.deps,
                step: inv.envelope_available.step,
                has_prev: inv.envelope_available.has_prev,
                prev_source: inv.envelope_available.prev_source,
                prev_schema: inv.envelope_available.prev_schema,
            },
            expressions: inv
                .expressions
                .into_iter()
                .map(|e| ExpressionReferenceInfo {
                    field_path: e.field_path,
                    expression: e.expression,
                    referenced_paths: e.referenced_paths,
                    simulated_result: e.simulated_result,
                })
                .collect(),
            output_schema: inv.output_schema,
            simulated_output: inv.simulated_output,
            mock_output_used: inv.mock_output_used,
        })
        .collect();

    // Build summary
    let checkpoint_count = invocations.iter().filter(|i| i.checkpoint).count();
    let summary = if checkpoint_count > 0 {
        format!(
            "Composition has {} invocation(s), {} checkpoint(s)",
            invocations.len(),
            checkpoint_count
        )
    } else {
        format!("Composition has {} invocation(s)", invocations.len())
    };

    CompositionExplanation {
        name: trace.name,
        outcome: OutcomeInfo {
            description: trace.outcome.description,
            output_schema: trace.outcome.output_schema,
        },
        invocations,
        findings,
        simulated: trace.simulated,
        summary,
    }
}

/// Generate complete vocabulary documentation covering all categories and capabilities.
pub fn document_vocabulary() -> VocabularyDocumentation {
    let categories = list_grammar_categories();
    let registry = standard_capability_registry();

    let mut capabilities: Vec<CapabilityDetail> = registry
        .values()
        .map(|decl| CapabilityDetail {
            name: decl.name.clone(),
            category: decl.grammar_category.to_string(),
            description: decl.description.clone(),
            config_schema: decl.config_schema.clone(),
            mutation_profile: mutation_profile_str(&decl.mutation_profile),
            supports_idempotency_key: supports_idempotency(&decl.mutation_profile),
            tags: decl.tags.clone(),
            version: decl.version.clone(),
        })
        .collect();
    capabilities.sort_by(|a, b| a.name.cmp(&b.name));

    let total_capabilities = capabilities.len();

    VocabularyDocumentation {
        categories,
        capabilities,
        total_capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_grammar_categories_returns_all_six() {
        let categories = list_grammar_categories();
        assert_eq!(categories.len(), 6);
        let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Transform"));
        assert!(names.contains(&"Validate"));
        assert!(names.contains(&"Assert"));
        assert!(names.contains(&"Acquire"));
        assert!(names.contains(&"Persist"));
        assert!(names.contains(&"Emit"));
        for cat in &categories {
            assert!(
                !cat.capabilities.is_empty(),
                "{} has no capabilities",
                cat.name
            );
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
        assert_eq!(detail.category, "Transform");
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
    fn explain_composition_static() {
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
        let explanation = explain_composition(yaml, None);
        assert!(!explanation.simulated);
        assert_eq!(explanation.invocations.len(), 1);
        assert_eq!(explanation.invocations[0].capability, "transform");
        assert_eq!(explanation.invocations[0].category, "Transform");
        assert!(!explanation.invocations[0].expressions.is_empty());
    }

    #[test]
    fn explain_composition_with_simulation() {
        let yaml = r#"
name: sim
outcome:
  description: Simulation
  output_schema: {}
invocations:
  - capability: transform
    config:
      output: {type: object}
      filter: "{doubled: (.context.value * 2)}"
    checkpoint: false
"#;
        let sim = tasker_grammar::SimulationInput {
            context: serde_json::json!({"value": 21}),
            deps: serde_json::json!({}),
            step: serde_json::json!({"name": "test"}),
            mock_outputs: std::collections::HashMap::new(),
        };
        let explanation = explain_composition(yaml, Some(sim));
        assert!(explanation.simulated);
        assert_eq!(
            explanation.invocations[0].simulated_output,
            Some(serde_json::json!({"doubled": 42}))
        );
    }

    #[test]
    fn explain_composition_invalid_yaml() {
        let explanation = explain_composition("not: valid: yaml: [[[", None);
        assert!(explanation.findings.iter().any(|f| f.code == "PARSE_ERROR"));
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
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == "MISSING_CAPABILITY"));
    }
}
