use serde_json::json;
use tasker_grammar::vocabulary::standard_capability_registry;

use super::*;
use crate::template_parser::parse_template_str;
use crate::template_validator;

// ─── Standalone validation tests ────────────────────────────────────────

#[test]
fn validate_composition_valid_spec_returns_empty() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test outcome".to_owned(),
            output_schema: json!({
                "type": "object",
                "properties": { "result": { "type": "string" } },
                "required": ["result"]
            }),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "properties": { "result": { "type": "string" } },
                    "required": ["result"]
                },
                "filter": ".context | {result: .name}"
            }),
            checkpoint: false,
        }],
    };
    let findings = validate_composition(&spec, &registry);
    let errors: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn validate_composition_unknown_capability_returns_error() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: None,
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "nonexistent_cap".to_owned(),
            config: json!({}),
            checkpoint: false,
        }],
    };
    let findings = validate_composition(&spec, &registry);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "COMPOSITION_INVALID" && f.severity == Severity::Error),
        "expected COMPOSITION_INVALID error, got: {findings:?}"
    );
}

#[test]
fn validate_composition_missing_checkpoint_returns_error() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: None,
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "postgres", "target": "orders" },
                "data": ".context"
            }),
            checkpoint: false,
        }],
    };
    let findings = validate_composition(&spec, &registry);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "COMPOSITION_INVALID" && f.severity == Severity::Error),
        "expected checkpoint error, got: {findings:?}"
    );
}

// ─── Step-context validation tests ──────────────────────────────────────

#[test]
fn validate_step_composition_parse_error() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: parse_error_test
namespace_name: test
version: "1.0.0"
steps:
  - name: bad_step
    handler:
      callable: "grammar:bad"
    composition:
      not_a_valid_composition: true
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_PARSE_ERROR"),
        "expected COMPOSITION_PARSE_ERROR, got: {findings:?}"
    );
    assert!(findings
        .iter()
        .all(|f| f.step.as_deref() == Some("bad_step")));
}

#[test]
fn validate_step_composition_result_schema_mismatch() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: schema_mismatch_test
namespace_name: test
version: "1.0.0"
steps:
  - name: mismatched_step
    handler:
      callable: "grammar:test"
    result_schema:
      type: object
      required:
        - field_that_composition_does_not_produce
      properties:
        field_that_composition_does_not_produce:
          type: string
    composition:
      outcome:
        description: "Produces different fields"
        output_schema:
          type: object
          required:
            - actual_field
          properties:
            actual_field:
              type: integer
      invocations:
        - capability: transform
          config:
            output:
              type: object
              required:
                - actual_field
              properties:
                actual_field:
                  type: integer
            filter: ".context | {actual_field: 42}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "COMPOSITION_RESULT_SCHEMA_MISMATCH"),
        "expected COMPOSITION_RESULT_SCHEMA_MISMATCH, got: {findings:?}"
    );
}

#[test]
fn validate_step_composition_callable_convention_warning() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: callable_convention_test
namespace_name: test
version: "1.0.0"
steps:
  - name: wrong_callable
    handler:
      callable: "my_handler"
    composition:
      outcome:
        description: "Test"
        output_schema:
          type: object
          properties:
            result:
              type: string
          required:
            - result
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                result:
                  type: string
              required:
                - result
            filter: ".context | {result: .name}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "COMPOSITION_CALLABLE_CONVENTION"
                && f.severity == Severity::Warning),
        "expected COMPOSITION_CALLABLE_CONVENTION warning, got: {findings:?}"
    );
}

#[test]
fn validate_step_composition_no_composition_returns_empty() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: no_composition_test
namespace_name: test
version: "1.0.0"
steps:
  - name: normal_step
    handler:
      callable: "my_handler"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings.is_empty(),
        "expected no findings for step without composition"
    );
}

// ─── Integration tests: template_validator pipeline ──────────────────────

#[test]
fn template_validate_with_composition_step() {
    let yaml = r#"
name: composition_template
namespace_name: test
version: "1.0.0"
steps:
  - name: validate_order
    handler:
      callable: "grammar:validate_order"
    composition:
      outcome:
        description: "Validate an order"
        output_schema:
          type: object
          properties:
            valid:
              type: boolean
          required:
            - valid
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                valid:
                  type: boolean
              required:
                - valid
            filter: ".context | {valid: true}"
  - name: process_order
    handler:
      callable: "process.order"
    depends_on:
      - validate_order
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(
        composition_errors.is_empty(),
        "unexpected composition errors: {composition_errors:?}"
    );
}

#[test]
fn template_validate_catches_bad_composition() {
    let yaml = r#"
name: bad_composition_template
namespace_name: test
version: "1.0.0"
steps:
  - name: bad_step
    handler:
      callable: "grammar:bad"
    composition:
      outcome:
        description: "Bad"
        output_schema:
          type: object
      invocations:
        - capability: nonexistent_capability
          config: {}
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "COMPOSITION_INVALID"),
        "expected COMPOSITION_INVALID, got: {:?}",
        report.findings
    );
    assert!(!report.valid);
}

#[test]
fn template_validate_mixed_steps_validates_both() {
    let yaml = r#"
name: mixed_template
namespace_name: test
version: "1.0.0"
steps:
  - name: setup
    handler:
      callable: "my.setup_handler"
  - name: grammar_step
    handler:
      callable: "grammar:process"
    depends_on:
      - setup
    composition:
      outcome:
        description: "Process"
        output_schema:
          type: object
          properties:
            done:
              type: boolean
          required:
            - done
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                done:
                  type: boolean
              required:
                - done
            filter: ".context | {done: true}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    assert_eq!(report.step_count, 2);
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(
        composition_errors.is_empty(),
        "unexpected: {composition_errors:?}"
    );
}

#[test]
fn template_validate_backward_compatible_no_composition() {
    let yaml = include_str!("../../../../tests/fixtures/task_templates/codegen_test_template.yaml");
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    assert!(report.valid);
    assert!(!report.has_cycles);
    assert_eq!(report.step_count, 5);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code.starts_with("COMPOSITION_")),
        "should have no composition findings for template without compositions"
    );
}

#[test]
fn template_validate_with_custom_registry() {
    use tasker_grammar::{CapabilityDeclaration, GrammarCategoryKind, MutationProfile};

    let mut registry = standard_capability_registry();
    registry.insert(
        "custom_cap".to_owned(),
        CapabilityDeclaration {
            name: "custom_cap".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Custom capability".to_owned(),
            config_schema: serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    let yaml = r#"
name: custom_registry_test
namespace_name: test
version: "1.0.0"
steps:
  - name: custom_step
    handler:
      callable: "grammar:custom"
    composition:
      outcome:
        description: "Custom"
        output_schema:
          type: object
          properties:
            result:
              type: string
      invocations:
        - capability: custom_cap
          config:
            input: "test"
            output:
              type: object
              properties:
                result:
                  type: string
            filter: ".context | {result: .input}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate_with_registry(&template, &registry);
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == "COMPOSITION_INVALID")
        .collect();
    assert!(
        composition_errors.is_empty(),
        "custom_cap should be valid with custom registry: {composition_errors:?}"
    );
}
