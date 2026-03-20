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
