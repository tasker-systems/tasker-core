//! Composition validation for task templates.
//!
//! Bridges `tasker-grammar`'s [`CompositionValidator`] into the SDK's template
//! validation pipeline. Provides both standalone composition validation and
//! step-context-aware validation that integrates with `template_validator`.
//!
//! **Ticket**: TAS-337

use tasker_grammar::validation::{CapabilityRegistry, CompositionValidator};
use tasker_grammar::{CompositionSpec, ExpressionEngine, Severity};
use tasker_shared::models::core::task_template::StepDefinition;

use crate::template_validator::ValidationFinding;

/// Translate a grammar-level `ValidationFinding` into an SDK `ValidationFinding`.
///
/// Grammar findings include `invocation_index` and `field_path` which are
/// encoded into the message for human readability. The `step` field is set
/// by the caller when validating in step context.
fn translate_finding(
    finding: &tasker_grammar::ValidationFinding,
    step_name: Option<&str>,
) -> ValidationFinding {
    let prefix = match finding.invocation_index {
        Some(idx) => format!("invocation[{idx}]: "),
        None => String::new(),
    };
    let suffix = match &finding.field_path {
        Some(path) => format!(" (at {path})"),
        None => String::new(),
    };
    let code = match finding.severity {
        Severity::Error => "COMPOSITION_INVALID",
        Severity::Warning | Severity::Info => "COMPOSITION_WARNING",
    };
    ValidationFinding {
        code: code.to_owned(),
        severity: finding.severity.clone(),
        message: format!("{prefix}{}{suffix}", finding.message),
        step: step_name.map(str::to_owned),
    }
}

/// Validate a standalone `CompositionSpec` against a capability registry.
///
/// Constructs an `ExpressionEngine` with default config internally.
/// Returns SDK-level `ValidationFinding`s with no step context.
pub fn validate_composition(
    spec: &CompositionSpec,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding> {
    let engine = ExpressionEngine::with_defaults();
    let validator = CompositionValidator::new(registry, &engine);
    let result = validator.validate(spec);

    result
        .findings
        .iter()
        .map(|f| translate_finding(f, None))
        .collect()
}

/// Validate a composition in the context of a template step.
///
/// If the step has no `composition` field, returns empty.
/// Otherwise: deserializes to `CompositionSpec`, runs `CompositionValidator`,
/// checks result_schema compatibility, and checks callable convention.
/// All findings are tagged with the step name.
pub fn validate_step_composition(
    step: &StepDefinition,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding> {
    let composition_value = match &step.composition {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut findings = Vec::new();

    // Deserialize to CompositionSpec
    let spec: CompositionSpec = match serde_json::from_value(composition_value.clone()) {
        Ok(s) => s,
        Err(e) => {
            findings.push(ValidationFinding {
                code: "COMPOSITION_PARSE_ERROR".to_owned(),
                severity: Severity::Error,
                message: format!("failed to parse composition: {e}"),
                step: Some(step.name.clone()),
            });
            return findings;
        }
    };

    // Run grammar-level validation
    let grammar_findings = validate_composition(&spec, registry);
    findings.extend(grammar_findings.into_iter().map(|mut f| {
        f.step = Some(step.name.clone());
        f
    }));

    // Check result_schema compatibility
    if let Some(result_schema) = &step.result_schema {
        let outcome_schema = &spec.outcome.output_schema;
        let compat_findings = tasker_grammar::check_schema_compatibility(
            outcome_schema, // producer: what the composition produces
            result_schema,  // consumer: what the step declares
            "step result_schema vs composition outcome",
            None,
        );
        for cf in &compat_findings {
            if matches!(cf.severity, Severity::Error | Severity::Warning) {
                findings.push(ValidationFinding {
                    code: "COMPOSITION_RESULT_SCHEMA_MISMATCH".to_owned(),
                    severity: cf.severity.clone(),
                    message: cf.message.clone(),
                    step: Some(step.name.clone()),
                });
            }
        }
    }

    // Check callable convention
    if !step.handler.callable.starts_with("grammar:") {
        findings.push(ValidationFinding {
            code: "COMPOSITION_CALLABLE_CONVENTION".to_owned(),
            severity: Severity::Warning,
            message: format!(
                "step has a composition but callable '{}' does not use the 'grammar:' prefix",
                step.handler.callable
            ),
            step: Some(step.name.clone()),
        });
    }

    findings
}

#[cfg(test)]
mod tests;
