//! Tests for `CompositionValidator`.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::expression::ExpressionEngine;
use crate::types::{
    CapabilityDeclaration, CapabilityInvocation, CompositionSpec, GrammarCategoryKind,
    MutationProfile, OutcomeDeclaration, Severity,
};

use super::validator::{CompositionValidator, ValidationResult};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut registry = HashMap::new();

    registry.insert(
        "transform".to_owned(),
        CapabilityDeclaration {
            name: "transform".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Pure data transformation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["output", "filter"],
                "properties": {
                    "output": { "type": "object" },
                    "filter": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "validate".to_owned(),
        CapabilityDeclaration {
            name: "validate".to_owned(),
            grammar_category: GrammarCategoryKind::Validate,
            description: "JSON Schema validation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["schema"],
                "properties": {
                    "schema": { "type": "object" },
                    "on_failure": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "assert".to_owned(),
        CapabilityDeclaration {
            name: "assert".to_owned(),
            grammar_category: GrammarCategoryKind::Assert,
            description: "Execution gate".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["filter", "error"],
                "properties": {
                    "filter": { "type": "string" },
                    "error": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "acquire".to_owned(),
        CapabilityDeclaration {
            name: "acquire".to_owned(),
            grammar_category: GrammarCategoryKind::Acquire,
            description: "Fetch data".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": { "type": "object" },
                    "params": {},
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" },
                    "constraints": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "persist".to_owned(),
        CapabilityDeclaration {
            name: "persist".to_owned(),
            grammar_category: GrammarCategoryKind::Persist,
            description: "Write state".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": { "type": "object" },
                    "data": {},
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" },
                    "constraints": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "emit".to_owned(),
        CapabilityDeclaration {
            name: "emit".to_owned(),
            grammar_category: GrammarCategoryKind::Emit,
            description: "Fire domain events".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["event_name"],
                "properties": {
                    "event_name": { "type": "string" },
                    "payload": {},
                    "condition": {},
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" },
                    "metadata": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry
}

fn make_engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

fn make_validator<'a>(
    registry: &'a HashMap<String, CapabilityDeclaration>,
    engine: &'a ExpressionEngine,
) -> CompositionValidator<'a> {
    CompositionValidator::new(registry, engine)
}

fn simple_outcome() -> OutcomeDeclaration {
    OutcomeDeclaration {
        description: "Test outcome".to_owned(),
        output_schema: json!({
            "type": "object",
            "required": ["result"],
            "properties": {
                "result": { "type": "string" }
            }
        }),
    }
}

fn has_finding(result: &ValidationResult, code: &str) -> bool {
    result.findings.iter().any(|f| f.code == code)
}

fn has_error(result: &ValidationResult, code: &str) -> bool {
    result
        .findings
        .iter()
        .any(|f| f.code == code && matches!(f.severity, Severity::Error))
}

fn has_warning(result: &ValidationResult, code: &str) -> bool {
    result
        .findings
        .iter()
        .any(|f| f.code == code && matches!(f.severity, Severity::Warning))
}

// ---------------------------------------------------------------------------
// Empty composition
// ---------------------------------------------------------------------------

#[test]
fn empty_composition_is_invalid() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("empty".to_owned()),
        outcome: simple_outcome(),
        invocations: vec![],
    };

    let result = validator.validate(&spec);
    assert!(!result.is_valid());
    assert!(has_error(&result, "EMPTY_COMPOSITION"));
}

// ---------------------------------------------------------------------------
// Capability existence
// ---------------------------------------------------------------------------

#[test]
fn unknown_capability_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: simple_outcome(),
        invocations: vec![CapabilityInvocation {
            capability: "quantum_teleport".to_owned(),
            config: json!({}),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!result.is_valid());
    assert!(has_error(&result, "MISSING_CAPABILITY"));
    assert!(result.findings[0].message.contains("quantum_teleport"));
}

#[test]
fn all_six_capabilities_recognized() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    for name in &[
        "transform",
        "validate",
        "assert",
        "acquire",
        "persist",
        "emit",
    ] {
        let spec = CompositionSpec {
            name: Some("test".to_owned()),
            outcome: OutcomeDeclaration {
                description: "Test".to_owned(),
                output_schema: json!({}),
            },
            invocations: vec![CapabilityInvocation {
                capability: name.to_string(),
                config: make_valid_config(name),
                checkpoint: matches!(*name, "persist" | "emit"),
            }],
        };

        let result = validator.validate(&spec);
        assert!(
            !has_finding(&result, "MISSING_CAPABILITY"),
            "capability '{name}' should be recognized"
        );
    }
}

fn make_valid_config(capability: &str) -> serde_json::Value {
    match capability {
        "transform" => json!({
            "output": { "type": "object", "required": ["x"], "properties": { "x": { "type": "string" } } },
            "filter": ".context"
        }),
        "validate" => json!({
            "schema": { "type": "object" }
        }),
        "assert" => json!({
            "filter": "true",
            "error": "assertion failed"
        }),
        "acquire" => json!({
            "resource": { "type": "api" }
        }),
        "persist" => json!({
            "resource": { "type": "database" },
            "data": ".prev"
        }),
        "emit" => json!({
            "event_name": "order.created",
            "payload": ".prev"
        }),
        _ => json!({}),
    }
}

// ---------------------------------------------------------------------------
// Config schema validation
// ---------------------------------------------------------------------------

#[test]
fn invalid_config_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            // Missing required "output" and "filter" fields
            config: json!({"not_a_valid_field": true}),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "CONFIG_SCHEMA_VIOLATION"));
}

#[test]
fn valid_config_passes() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": { "type": "object", "required": ["x"], "properties": { "x": { "type": "string" } } },
                "filter": ".context"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!has_finding(&result, "CONFIG_SCHEMA_VIOLATION"));
}

// ---------------------------------------------------------------------------
// Output schema presence for transform
// ---------------------------------------------------------------------------

#[test]
fn transform_without_output_schema_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {},
                "filter": ".context"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "MISSING_OUTPUT_SCHEMA"));
}

#[test]
fn transform_with_null_output_schema_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": null,
                "filter": ".context"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "MISSING_OUTPUT_SCHEMA"));
}

// ---------------------------------------------------------------------------
// Expression syntax validation
// ---------------------------------------------------------------------------

#[test]
fn invalid_jaq_expression_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": { "type": "object", "required": ["x"], "properties": { "x": { "type": "string" } } },
                "filter": ".context | invalid_syntax_here!!! {"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
}

#[test]
fn valid_jaq_expression_passes() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": { "type": "object", "required": ["x"], "properties": { "x": { "type": "string" } } },
                "filter": "{x: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!has_finding(&result, "INVALID_EXPRESSION"));
}

#[test]
fn assert_filter_syntax_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "assert".to_owned(),
            config: json!({
                "filter": "this is not valid {{{",
                "error": "check failed"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
}

#[test]
fn persist_data_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": { "expression": "broken expression {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
}

#[test]
fn emit_payload_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
}

#[test]
fn emit_condition_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "condition": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
}

#[test]
fn persist_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": { "expression": ".prev" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.validate_success.expression")
    );
}

#[test]
fn persist_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": { "expression": ".prev" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.result_shape.expression")
    );
}

#[test]
fn acquire_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "acquire".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.validate_success.expression")
    );
}

#[test]
fn acquire_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "acquire".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.result_shape.expression")
    );
}

#[test]
fn emit_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.validate_success.expression")
    );
}

#[test]
fn emit_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "INVALID_EXPRESSION")
        .unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.result_shape.expression")
    );
}

// ---------------------------------------------------------------------------
// Checkpoint coverage
// ---------------------------------------------------------------------------

#[test]
fn mutating_capability_without_checkpoint_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": ".prev"
            }),
            checkpoint: false, // Should be true!
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "CHECKPOINT_REQUIRED"));
}

#[test]
fn mutating_capability_with_checkpoint_passes() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": ".prev"
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!has_finding(&result, "CHECKPOINT_REQUIRED"));
}

#[test]
fn non_mutating_capability_without_checkpoint_is_fine() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": { "type": "object", "required": ["x"], "properties": { "x": { "type": "string" } } },
                "filter": ".context"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!has_finding(&result, "CHECKPOINT_REQUIRED"));
}

#[test]
fn emit_without_checkpoint_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test",
                "payload": ".prev"
            }),
            checkpoint: false, // Should be true!
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "CHECKPOINT_REQUIRED"));
}

// ---------------------------------------------------------------------------
// Contract chaining (output schema compatibility)
// ---------------------------------------------------------------------------

#[test]
fn compatible_contract_chain_passes() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["count"],
                "properties": { "count": { "type": "integer" } }
            }),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {
                        "type": "object",
                        "required": ["records"],
                        "properties": {
                            "records": { "type": "array", "items": { "type": "object" } }
                        }
                    },
                    "filter": "{records: .context.data}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {
                        "type": "object",
                        "required": ["count"],
                        "properties": {
                            "count": { "type": "integer" }
                        }
                    },
                    "filter": "{count: (.prev.records | length)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let result = validator.validate(&spec);
    // No contract chain errors expected (contract chaining currently validates
    // via output schemas, not by inferring consumer expectations from jaq)
    assert!(!has_finding(&result, "MISSING_REQUIRED_FIELD"));
    assert!(!has_finding(&result, "TYPE_MISMATCH"));
}

// ---------------------------------------------------------------------------
// Outcome convergence
// ---------------------------------------------------------------------------

#[test]
fn matching_outcome_passes() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["result"],
                "properties": { "result": { "type": "string" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["result"],
                    "properties": { "result": { "type": "string" } }
                },
                "filter": "{result: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(result.is_valid(), "findings: {:?}", result.findings);
}

#[test]
fn outcome_missing_required_field_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["result", "count"],
                "properties": {
                    "result": { "type": "string" },
                    "count": { "type": "integer" }
                }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["result"],
                    "properties": {
                        "result": { "type": "string" }
                        // Missing "count"
                    }
                },
                "filter": "{result: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!result.is_valid());
    assert!(has_error(&result, "MISSING_REQUIRED_FIELD"));
    let finding = result
        .findings
        .iter()
        .find(|f| f.code == "MISSING_REQUIRED_FIELD")
        .unwrap();
    assert!(finding.message.contains("count"));
}

#[test]
fn outcome_type_mismatch_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["count"],
                "properties": { "count": { "type": "string" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["count"],
                    "properties": { "count": { "type": "integer" } }
                },
                "filter": "{count: 42}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(!result.is_valid());
    assert!(has_error(&result, "TYPE_MISMATCH"));
}

#[test]
fn integer_compatible_with_number_in_outcome() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["count"],
                "properties": { "count": { "type": "number" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["count"],
                    "properties": { "count": { "type": "integer" } }
                },
                "filter": "{count: 42}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(
        result.is_valid(),
        "integer should be compatible with number; findings: {:?}",
        result.findings
    );
}

#[test]
fn extra_producer_fields_are_allowed() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["result"],
                "properties": { "result": { "type": "string" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["result", "extra_data"],
                    "properties": {
                        "result": { "type": "string" },
                        "extra_data": { "type": "object" }
                    }
                },
                "filter": "{result: .context.name, extra_data: .context}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(
        result.is_valid(),
        "extra fields in producer should be fine; findings: {:?}",
        result.findings
    );
}

// ---------------------------------------------------------------------------
// Non-transform final invocation outcome check
// ---------------------------------------------------------------------------

#[test]
fn non_transform_final_invocation_warns_about_unverifiable_outcome() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["result"],
                "properties": { "result": { "type": "string" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": ".prev"
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_warning(&result, "UNVERIFIABLE_OUTCOME"));
}

// ---------------------------------------------------------------------------
// Full realistic composition (fetch → transform → validate → persist → transform)
// ---------------------------------------------------------------------------

#[test]
fn realistic_composition_passes_validation() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("fetch_validate_persist".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Fetch, validate, and persist records".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["persisted_count", "invalid_count"],
                "properties": {
                    "persisted_count": { "type": "integer" },
                    "invalid_count": { "type": "integer" }
                }
            }),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "acquire".to_owned(),
                config: json!({
                    "resource": { "type": "api", "endpoint": "https://api.example.com/records" },
                    "constraints": { "timeout_ms": 10000 }
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {
                        "type": "object",
                        "required": ["records"],
                        "properties": {
                            "records": { "type": "array", "items": { "type": "object" } }
                        }
                    },
                    "filter": "{records: .prev.body.data.records}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "validate".to_owned(),
                config: json!({
                    "schema": { "type": "object" },
                    "on_failure": "partition"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": { "type": "database", "entity": "records_table" },
                    "data": ".prev.valid",
                    "constraints": { "conflict_key": "external_id" }
                }),
                checkpoint: true,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {
                        "type": "object",
                        "required": ["persisted_count", "invalid_count"],
                        "properties": {
                            "persisted_count": { "type": "integer" },
                            "invalid_count": { "type": "integer" }
                        }
                    },
                    "filter": "{persisted_count: .prev.persisted_count, invalid_count: (.prev.invalid // [] | length)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let result = validator.validate(&spec);
    assert!(
        result.is_valid(),
        "realistic composition should pass; findings: {:?}",
        result.findings
    );
    assert_eq!(result.error_count(), 0);
}

// ---------------------------------------------------------------------------
// Multiple errors in single validation
// ---------------------------------------------------------------------------

#[test]
fn multiple_errors_collected() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("bad".to_owned()),
        outcome: simple_outcome(),
        invocations: vec![
            // Unknown capability
            CapabilityInvocation {
                capability: "nonexistent".to_owned(),
                config: json!({}),
                checkpoint: false,
            },
            // Persist without checkpoint
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": { "type": "database" },
                    "data": "broken {{{ syntax"
                }),
                checkpoint: false,
            },
        ],
    };

    let result = validator.validate(&spec);
    assert!(!result.is_valid());
    assert!(result.error_count() >= 2);
    assert!(has_error(&result, "MISSING_CAPABILITY"));
    assert!(has_error(&result, "CHECKPOINT_REQUIRED"));
}

// ---------------------------------------------------------------------------
// ValidationResult API
// ---------------------------------------------------------------------------

#[test]
fn validation_result_api() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    // Valid spec
    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({
                "type": "object",
                "required": ["result"],
                "properties": { "result": { "type": "string" } }
            }),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "required": ["result"],
                    "properties": { "result": { "type": "string" } }
                },
                "filter": "{result: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(result.is_valid());
    assert_eq!(result.error_count(), 0);
    assert!(result.errors().is_empty());
}

// ---------------------------------------------------------------------------
// Schema compatibility edge cases
// ---------------------------------------------------------------------------

#[test]
fn nullable_type_compatibility() {
    use super::schema_compat::check_schema_compatibility;

    // Producer provides ["string", "null"], consumer expects "string"
    let producer = json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": ["string", "null"] }
        }
    });
    let consumer = json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string" }
        }
    });

    let findings = check_schema_compatibility(&producer, &consumer, "test", None);
    // Producer has ["string", "null"] and consumer wants "string" — the "string"
    // type in producer satisfies the consumer's requirement
    assert!(
        findings.is_empty(),
        "nullable producer should be compatible when consumer expects non-null; findings: {findings:?}"
    );
}

#[test]
fn missing_producer_schema_detected() {
    use super::schema_compat::check_schema_compatibility;

    let producer = json!({});
    let consumer = json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string" }
        }
    });

    let findings = check_schema_compatibility(&producer, &consumer, "test", None);
    assert!(
        findings.iter().any(|f| f.code == "MISSING_PRODUCER_SCHEMA"),
        "should detect missing producer schema; findings: {findings:?}"
    );
}

#[test]
fn empty_consumer_schema_always_compatible() {
    use super::schema_compat::check_schema_compatibility;

    let producer = json!({
        "type": "object",
        "required": ["anything"],
        "properties": { "anything": { "type": "string" } }
    });
    let consumer = json!({});

    let findings = check_schema_compatibility(&producer, &consumer, "test", None);
    assert!(
        findings.is_empty(),
        "empty consumer should be compatible with anything"
    );
}

#[test]
fn deeply_nested_schema_produces_depth_warning() {
    use super::schema_compat::check_schema_compatibility;

    // Build a schema nested 40 levels deep (exceeds MAX_SCHEMA_DEPTH of 32)
    fn nest_schema(depth: usize) -> Value {
        if depth == 0 {
            return json!({
                "type": "object",
                "required": ["leaf"],
                "properties": { "leaf": { "type": "string" } }
            });
        }
        let inner = nest_schema(depth - 1);
        json!({
            "type": "object",
            "required": ["nested"],
            "properties": { "nested": inner }
        })
    }

    let schema = nest_schema(40);
    let findings = check_schema_compatibility(&schema, &schema, "test", None);
    assert!(
        findings.iter().any(|f| f.code == "SCHEMA_DEPTH_EXCEEDED"),
        "should produce depth warning for deeply nested schema; findings: {findings:?}"
    );
}

#[test]
fn too_many_invocations_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    // Build a composition with 150 invocations (exceeds MAX_INVOCATION_COUNT of 100)
    let invocations: Vec<CapabilityInvocation> = (0..150)
        .map(|_| CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({"filter": "."}),
            checkpoint: false,
        })
        .collect();

    let spec = CompositionSpec {
        name: Some("oversized".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations,
    };

    let result = validator.validate(&spec);
    assert!(
        !result.errors().is_empty(),
        "should reject oversized composition"
    );
    assert!(
        result
            .errors()
            .iter()
            .any(|f| f.code == "TOO_MANY_INVOCATIONS"),
        "should have TOO_MANY_INVOCATIONS error; findings: {:?}",
        result.errors()
    );
}

#[test]
fn overlong_composition_name_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("x".repeat(300)),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({"filter": "."}),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(
        result
            .errors()
            .iter()
            .any(|f| f.code == "FIELD_TOO_LONG" && f.field_path.as_deref() == Some("name")),
        "should reject overlong name; findings: {:?}",
        result.errors()
    );
}

#[test]
fn overlong_capability_name_produces_error() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "x".repeat(200),
            config: json!({"filter": "."}),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(
        result
            .errors()
            .iter()
            .any(|f| f.code == "FIELD_TOO_LONG" && f.field_path.as_deref() == Some("capability")),
        "should reject overlong capability name; findings: {:?}",
        result.errors()
    );
}
