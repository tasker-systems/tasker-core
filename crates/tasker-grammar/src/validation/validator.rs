//! Core `CompositionValidator` implementation.
//!
//! Performs design-time validation of `CompositionSpec` structures:
//! structural correctness and contract compatibility checks that run
//! in tooling and on template load/persist, NOT at runtime.

use std::collections::HashMap;
use std::fmt;

use serde_json::Value;

use crate::expression::ExpressionEngine;
use crate::types::{
    CapabilityDeclaration, CompositionSpec, GrammarCategoryKind, MutationProfile, Severity,
    ValidationFinding,
};

use super::schema_compat::check_schema_compatibility;

/// Maximum number of invocations allowed in a composition.
///
/// This limit prevents resource exhaustion from pathologically large compositions.
/// It aligns with the default [`CompositionExecutorConfig::max_invocation_count`]
/// but is enforced at design-time validation rather than runtime.
const MAX_INVOCATION_COUNT: usize = 100;

/// Field length limits for composition spec strings.
const MAX_NAME_LEN: usize = 256;
const MAX_DESCRIPTION_LEN: usize = 4096;
const MAX_CAPABILITY_NAME_LEN: usize = 128;

/// Registry providing capability declarations for validation.
///
/// The validator looks up capabilities by name to obtain their config schemas,
/// grammar categories, and mutation profiles.
pub trait CapabilityRegistry: Send + Sync {
    /// Look up a capability declaration by name.
    fn get_capability(&self, name: &str) -> Option<&CapabilityDeclaration>;

    /// List all registered capability names.
    fn capability_names(&self) -> Vec<&str>;
}

/// Simple in-memory capability registry for testing and standalone validation.
impl CapabilityRegistry for HashMap<String, CapabilityDeclaration> {
    fn get_capability(&self, name: &str) -> Option<&CapabilityDeclaration> {
        self.get(name)
    }

    fn capability_names(&self) -> Vec<&str> {
        self.keys().map(String::as_str).collect()
    }
}

/// Result of composition validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All validation findings (errors, warnings, info).
    pub findings: Vec<ValidationFinding>,
}

impl ValidationResult {
    /// Whether the composition is valid (no error-level findings).
    pub fn is_valid(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|f| matches!(f.severity, Severity::Error))
    }

    /// Only the error-level findings.
    pub fn errors(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Error))
            .collect()
    }

    /// Only the warning-level findings.
    pub fn warnings(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Warning))
            .collect()
    }

    /// Count of findings by severity.
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Error))
            .count()
    }

    /// Count of warning-level findings.
    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Warning))
            .count()
    }
}

/// Validates `CompositionSpec` structures at design time.
///
/// Checks structural correctness and contract compatibility — this runs in
/// `composition_validate` tooling and on template load/persist, NOT at runtime
/// during execution.
///
/// # Usage
///
/// ```
/// use std::collections::HashMap;
/// use serde_json::json;
/// use tasker_grammar::validation::{CompositionValidator, CapabilityRegistry};
/// use tasker_grammar::types::{
///     CapabilityDeclaration, GrammarCategoryKind, MutationProfile, CompositionSpec,
///     CapabilityInvocation, OutcomeDeclaration,
/// };
/// use tasker_grammar::ExpressionEngine;
///
/// // Set up a capability registry with the built-in vocabulary
/// let mut registry = HashMap::new();
/// registry.insert("transform".to_owned(), CapabilityDeclaration {
///     name: "transform".to_owned(),
///     grammar_category: GrammarCategoryKind::Transform,
///     description: "Pure data transformation".to_owned(),
///     config_schema: json!({"type": "object", "required": ["output", "filter"], "properties": {"output": {"type": "object"}, "filter": {"type": "string"}}}),
///     mutation_profile: MutationProfile::NonMutating,
///     tags: vec![],
///     version: "1.0.0".to_owned(),
/// });
///
/// let engine = ExpressionEngine::with_defaults();
/// let validator = CompositionValidator::new(&registry, &engine);
///
/// let spec = CompositionSpec {
///     name: Some("test".to_owned()),
///     outcome: OutcomeDeclaration {
///         description: "Test".to_owned(),
///         output_schema: json!({"type": "object", "required": ["result"], "properties": {"result": {"type": "string"}}}),
///     },
///     invocations: vec![
///         CapabilityInvocation {
///             capability: "transform".to_owned(),
///             config: json!({"output": {"type": "object", "required": ["result"], "properties": {"result": {"type": "string"}}}, "filter": "{result: .context.name}"}),
///             checkpoint: false,
///         },
///     ],
/// };
///
/// let result = validator.validate(&spec);
/// assert!(result.is_valid());
/// ```
pub struct CompositionValidator<'a> {
    registry: &'a dyn CapabilityRegistry,
    expression_engine: &'a ExpressionEngine,
}

impl fmt::Debug for CompositionValidator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompositionValidator")
            .field("expression_engine", &self.expression_engine)
            .finish_non_exhaustive()
    }
}

/// Extract a jaq expression string from a config field value.
///
/// Handles two shapes:
/// - Flat string: `"filter": ".context.name"` → `Some(".context.name")`
/// - ExpressionField object: `"data": {"expression": ".prev"}` → `Some(".prev")`
/// - Anything else: `None`
fn extract_expression(value: &Value) -> Option<&str> {
    // Flat string (used by transform filter, assert filter)
    if let Some(s) = value.as_str() {
        return Some(s);
    }
    // ExpressionField object (used by persist/acquire/emit fields)
    value
        .as_object()
        .and_then(|obj| obj.get("expression"))
        .and_then(|v| v.as_str())
}

impl<'a> CompositionValidator<'a> {
    /// Create a new validator with the given capability registry and expression engine.
    pub fn new(
        registry: &'a dyn CapabilityRegistry,
        expression_engine: &'a ExpressionEngine,
    ) -> Self {
        Self {
            registry,
            expression_engine,
        }
    }

    /// Validate a composition spec, returning all findings.
    pub fn validate(&self, spec: &CompositionSpec) -> ValidationResult {
        let mut findings = Vec::new();

        // Check 0a: Empty composition
        if spec.invocations.is_empty() {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "EMPTY_COMPOSITION".to_owned(),
                invocation_index: None,
                message: "composition has no invocations".to_owned(),
                field_path: None,
            });
            return ValidationResult { findings };
        }

        // Check 0b: Invocation count limit (prevents resource exhaustion)
        if spec.invocations.len() > MAX_INVOCATION_COUNT {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "TOO_MANY_INVOCATIONS".to_owned(),
                invocation_index: None,
                message: format!(
                    "composition has {} invocations, exceeding maximum of {MAX_INVOCATION_COUNT}",
                    spec.invocations.len()
                ),
                field_path: None,
            });
            return ValidationResult { findings };
        }

        // Check 0c: Field length limits
        if let Some(name) = &spec.name {
            if name.len() > MAX_NAME_LEN {
                findings.push(ValidationFinding {
                    severity: Severity::Error,
                    code: "FIELD_TOO_LONG".to_owned(),
                    invocation_index: None,
                    message: format!(
                        "composition name length {} exceeds maximum of {MAX_NAME_LEN}",
                        name.len()
                    ),
                    field_path: Some("name".to_owned()),
                });
            }
        }
        if spec.outcome.description.len() > MAX_DESCRIPTION_LEN {
            findings.push(ValidationFinding {
                severity: Severity::Error,
                code: "FIELD_TOO_LONG".to_owned(),
                invocation_index: None,
                message: format!(
                    "outcome description length {} exceeds maximum of {MAX_DESCRIPTION_LEN}",
                    spec.outcome.description.len()
                ),
                field_path: Some("outcome.description".to_owned()),
            });
        }
        for (idx, invocation) in spec.invocations.iter().enumerate() {
            if invocation.capability.len() > MAX_CAPABILITY_NAME_LEN {
                findings.push(ValidationFinding {
                    severity: Severity::Error,
                    code: "FIELD_TOO_LONG".to_owned(),
                    invocation_index: Some(idx),
                    message: format!(
                        "capability name length {} exceeds maximum of {MAX_CAPABILITY_NAME_LEN}",
                        invocation.capability.len()
                    ),
                    field_path: Some("capability".to_owned()),
                });
            }
        }

        // Check 1: Structural validity — capability existence
        let resolved = self.check_capability_existence(spec, &mut findings);

        // Check 2: Config schema validation
        self.check_config_schemas(spec, &resolved, &mut findings);

        // Check 3: Output schema presence for transform invocations
        self.check_output_schema_presence(spec, &resolved, &mut findings);

        // Check 4: Expression syntax validation
        self.check_expression_syntax(spec, &resolved, &mut findings);

        // Check 5: Checkpoint coverage
        self.check_checkpoint_coverage(spec, &resolved, &mut findings);

        // Check 6: Contract chaining
        self.check_contract_chaining(spec, &resolved, &mut findings);

        // Check 7: Outcome convergence
        self.check_outcome_convergence(spec, &resolved, &mut findings);

        ValidationResult { findings }
    }

    /// Check that all referenced capabilities exist in the vocabulary.
    /// Returns a vec of Option<&CapabilityDeclaration> parallel to the invocations.
    fn check_capability_existence(
        &self,
        spec: &CompositionSpec,
        findings: &mut Vec<ValidationFinding>,
    ) -> Vec<Option<&'a CapabilityDeclaration>> {
        spec.invocations
            .iter()
            .enumerate()
            .map(
                |(idx, invocation)| match self.registry.get_capability(&invocation.capability) {
                    Some(decl) => Some(decl),
                    None => {
                        findings.push(ValidationFinding {
                            severity: Severity::Error,
                            code: "MISSING_CAPABILITY".to_owned(),
                            invocation_index: Some(idx),
                            message: format!(
                                "capability '{}' not found in vocabulary",
                                invocation.capability
                            ),
                            field_path: Some("capability".to_owned()),
                        });
                        None
                    }
                },
            )
            .collect()
    }

    /// Validate each invocation's config against the capability's config_schema.
    fn check_config_schemas(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        for (idx, (invocation, decl_opt)) in
            spec.invocations.iter().zip(resolved.iter()).enumerate()
        {
            let Some(decl) = decl_opt else {
                continue; // Skip unresolved capabilities
            };

            let config_schema = &decl.config_schema;

            // Only validate if the config_schema is a non-trivial object schema
            if config_schema.is_null() || config_schema.as_object().is_none_or(|o| o.is_empty()) {
                continue;
            }

            match jsonschema::validator_for(config_schema) {
                Ok(validator) => {
                    for error in validator.iter_errors(&invocation.config) {
                        findings.push(ValidationFinding {
                            severity: Severity::Error,
                            code: "CONFIG_SCHEMA_VIOLATION".to_owned(),
                            invocation_index: Some(idx),
                            message: format!(
                                "invocation {} ({}) config validation error: {}",
                                idx, invocation.capability, error
                            ),
                            field_path: Some(error.instance_path.to_string()),
                        });
                    }
                }
                Err(e) => {
                    findings.push(ValidationFinding {
                        severity: Severity::Warning,
                        code: "INVALID_CONFIG_SCHEMA".to_owned(),
                        invocation_index: Some(idx),
                        message: format!(
                            "capability '{}' has an invalid config_schema, cannot validate config: {e}",
                            invocation.capability
                        ),
                        field_path: None,
                    });
                }
            }
        }
    }

    /// Check that every `transform` invocation declares an `output` schema.
    fn check_output_schema_presence(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        for (idx, (invocation, decl_opt)) in
            spec.invocations.iter().zip(resolved.iter()).enumerate()
        {
            let Some(decl) = decl_opt else {
                continue;
            };

            if decl.grammar_category != GrammarCategoryKind::Transform {
                continue;
            }

            let has_output = invocation
                .config
                .get("output")
                .is_some_and(|v| !v.is_null() && v.as_object().is_some_and(|o| !o.is_empty()));

            if !has_output {
                findings.push(ValidationFinding {
                    severity: Severity::Error,
                    code: "MISSING_OUTPUT_SCHEMA".to_owned(),
                    invocation_index: Some(idx),
                    message: format!(
                        "transform invocation {} must declare an 'output' JSON Schema for contract chaining",
                        idx
                    ),
                    field_path: Some("config.output".to_owned()),
                });
            }
        }
    }

    /// Validate jaq expression syntax in configs.
    fn check_expression_syntax(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        for (idx, (invocation, decl_opt)) in
            spec.invocations.iter().zip(resolved.iter()).enumerate()
        {
            let Some(decl) = decl_opt else {
                continue;
            };

            // Determine which config fields contain jaq expressions based on category
            let expression_fields: &[&str] = match decl.grammar_category {
                GrammarCategoryKind::Transform => &["filter"],
                GrammarCategoryKind::Assert => &["filter"],
                GrammarCategoryKind::Persist => &["data", "validate_success", "result_shape"],
                GrammarCategoryKind::Acquire => &["params", "validate_success", "result_shape"],
                GrammarCategoryKind::Emit => {
                    &["payload", "condition", "validate_success", "result_shape"]
                }
                GrammarCategoryKind::Validate => &[],
            };

            for field in expression_fields {
                if let Some(value) = invocation.config.get(*field) {
                    let (expr, field_path) = match extract_expression(value) {
                        Some(e) if value.is_string() => (e, format!("config.{field}")),
                        Some(e) => (e, format!("config.{field}.expression")),
                        None => continue,
                    };
                    if let Err(e) = self.expression_engine.validate_syntax(expr) {
                        findings.push(ValidationFinding {
                            severity: Severity::Error,
                            code: "INVALID_EXPRESSION".to_owned(),
                            invocation_index: Some(idx),
                            message: format!(
                                "invocation {} ({}) has invalid jaq expression in '{}': {e}",
                                idx, invocation.capability, field_path
                            ),
                            field_path: Some(field_path),
                        });
                    }
                }
            }

            // Emit metadata expressions (nested one level deeper).
            // Metadata fields are always ExpressionField objects in practice.
            if matches!(decl.grammar_category, GrammarCategoryKind::Emit) {
                if let Some(metadata) = invocation.config.get("metadata").and_then(Value::as_object)
                {
                    for meta_field in &["correlation_id", "idempotency_key"] {
                        if let Some(value) = metadata.get(*meta_field) {
                            if let Some(expr) = extract_expression(value) {
                                let field_path = format!("config.metadata.{meta_field}.expression");
                                if let Err(e) = self.expression_engine.validate_syntax(expr) {
                                    findings.push(ValidationFinding {
                                        severity: Severity::Error,
                                        code: "INVALID_EXPRESSION".to_owned(),
                                        invocation_index: Some(idx),
                                        message: format!(
                                            "invocation {} ({}) has invalid jaq expression in '{}': {e}",
                                            idx, invocation.capability, field_path
                                        ),
                                        field_path: Some(field_path),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check that all mutating capabilities have checkpoint markers.
    fn check_checkpoint_coverage(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        for (idx, (invocation, decl_opt)) in
            spec.invocations.iter().zip(resolved.iter()).enumerate()
        {
            let Some(decl) = decl_opt else {
                continue;
            };

            let is_mutating = matches!(decl.mutation_profile, MutationProfile::Mutating { .. });

            if is_mutating && !invocation.checkpoint {
                findings.push(ValidationFinding {
                    severity: Severity::Error,
                    code: "CHECKPOINT_REQUIRED".to_owned(),
                    invocation_index: Some(idx),
                    message: format!(
                        "mutating capability '{}' at invocation {} must be a checkpoint boundary",
                        invocation.capability, idx
                    ),
                    field_path: Some("checkpoint".to_owned()),
                });
            }
        }
    }

    /// Check contract chaining between sequential invocations.
    ///
    /// For `transform` invocations, the declared `output` schema of invocation N
    /// becomes available as `.prev` for invocation N+1. The validator checks that
    /// the output schema is compatible with what the next invocation expects.
    fn check_contract_chaining(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        if spec.invocations.len() < 2 {
            return;
        }

        // Build a chain of output schemas: each invocation's output becomes
        // the next invocation's `.prev`
        let mut prev_schema: Option<&Value> = None;

        for (idx, (invocation, decl_opt)) in
            spec.invocations.iter().zip(resolved.iter()).enumerate()
        {
            let Some(decl) = decl_opt else {
                // Can't validate chain if capability is unknown; reset prev
                prev_schema = None;
                continue;
            };

            // For non-first invocations, check that the previous output is
            // compatible with what this invocation might expect
            if idx > 0 {
                if let Some(producer) = prev_schema {
                    // The consumer expectation depends on the category
                    if let Some(consumer) = self.infer_input_expectation(invocation, decl) {
                        let label = format!("contract chain {}→{}", idx - 1, idx);
                        let chain_findings =
                            check_schema_compatibility(producer, &consumer, &label, Some(idx));
                        findings.extend(chain_findings);
                    }
                }
            }

            // Update prev_schema based on this invocation's output
            prev_schema = self.extract_output_schema(invocation, decl);
        }
    }

    /// Check that the final invocation's output is compatible with the declared outcome.
    fn check_outcome_convergence(
        &self,
        spec: &CompositionSpec,
        resolved: &[Option<&CapabilityDeclaration>],
        findings: &mut Vec<ValidationFinding>,
    ) {
        let outcome_schema = &spec.outcome.output_schema;
        if outcome_schema.is_null() || outcome_schema.as_object().is_none_or(|o| o.is_empty()) {
            return;
        }

        // Find the last invocation with a resolvable output schema
        let last_idx = spec.invocations.len() - 1;
        if let Some(decl) = resolved[last_idx] {
            if let Some(final_output) =
                self.extract_output_schema(&spec.invocations[last_idx], decl)
            {
                let chain_findings = check_schema_compatibility(
                    final_output,
                    outcome_schema,
                    "outcome convergence",
                    Some(last_idx),
                );
                findings.extend(chain_findings);
            } else {
                findings.push(ValidationFinding {
                    severity: Severity::Warning,
                    code: "UNVERIFIABLE_OUTCOME".to_owned(),
                    invocation_index: Some(last_idx),
                    message: format!(
                        "cannot verify outcome convergence: final invocation {} ({}) has no declared output schema",
                        last_idx, spec.invocations[last_idx].capability
                    ),
                    field_path: None,
                });
            }
        }
    }

    /// Extract the output schema declared by an invocation, if any.
    ///
    /// - `transform`: uses `config.output`
    /// - Action capabilities with `result_shape`: future extension
    /// - Others: None (output schema not statically declared)
    fn extract_output_schema<'b>(
        &self,
        invocation: &'b crate::types::CapabilityInvocation,
        decl: &CapabilityDeclaration,
    ) -> Option<&'b Value> {
        match decl.grammar_category {
            GrammarCategoryKind::Transform => invocation
                .config
                .get("output")
                .filter(|v| !v.is_null() && v.as_object().is_some_and(|o| !o.is_empty())),
            // Assert passes through .prev unchanged — no new output schema
            GrammarCategoryKind::Assert => None,
            // Validate, acquire, persist, emit: future extension for result_shape
            _ => None,
        }
    }

    /// Infer what input schema an invocation expects from `.prev`.
    ///
    /// This is category-specific:
    /// - `transform`: reads `.prev` via jaq filter — we can't statically infer
    ///   exactly which fields are needed without parsing jaq, so we skip this
    ///   (the contract chain is validated via output schemas only)
    /// - `validate`: expects the data to validate — generally compatible with any input
    /// - `assert`: evaluates a boolean on `.prev` — same as transform
    /// - `persist`: `data` field is a jaq expression reading from `.prev`
    /// - `acquire`: `params` is a jaq expression
    /// - `emit`: `payload` is a jaq expression
    ///
    /// For now, we return None for most categories since jaq expressions handle
    /// field selection dynamically. The contract chain validation relies on
    /// output schema declarations (the producer side) rather than trying to
    /// infer consumer expectations from jaq expressions.
    fn infer_input_expectation(
        &self,
        _invocation: &crate::types::CapabilityInvocation,
        _decl: &CapabilityDeclaration,
    ) -> Option<Value> {
        // Contract chaining is primarily validated via output schema declarations.
        // The jaq expressions in each invocation's config access `.prev` fields
        // dynamically — static inference of required fields from jaq expressions
        // is a Phase 2 concern (TAS-341: expression variable resolution checker).
        None
    }
}
