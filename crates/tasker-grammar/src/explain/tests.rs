use std::collections::HashMap;

use serde_json::json;

use crate::explain::types::SimulationInput;
use crate::explain::ExplainAnalyzer;
use crate::types::{
    CapabilityDeclaration, CapabilityInvocation, CompositionSpec, GrammarCategoryKind,
    MutationProfile, OutcomeDeclaration,
};
use crate::ExpressionEngine;

fn test_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut reg = HashMap::new();
    reg.insert(
        "transform".to_owned(),
        CapabilityDeclaration {
            name: "transform".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Pure data transformation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["output", "filter"],
                "properties": {
                    "output": {"type": "object"},
                    "filter": {"type": "string"}
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );
    reg.insert(
        "persist".to_owned(),
        CapabilityDeclaration {
            name: "persist".to_owned(),
            grammar_category: GrammarCategoryKind::Persist,
            description: "Write data to a resource".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource", "data"],
                "properties": {
                    "resource": {"type": "object"},
                    "data": {"type": "object"}
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );
    reg.insert(
        "assert".to_owned(),
        CapabilityDeclaration {
            name: "assert".to_owned(),
            grammar_category: GrammarCategoryKind::Assert,
            description: "Boolean assertion gate".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["filter"],
                "properties": {
                    "filter": {"type": "string"},
                    "error": {"type": "string"}
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );
    reg
}

fn test_engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

#[test]
fn analyze_single_transform() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test outcome".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {"type": "object", "properties": {"result": {"type": "string"}}},
                "filter": "{result: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let trace = analyzer.analyze(&spec);
    assert!(!trace.simulated);
    assert_eq!(trace.name, Some("test".to_owned()));
    assert_eq!(trace.invocations.len(), 1);

    let inv = &trace.invocations[0];
    assert_eq!(inv.index, 0);
    assert_eq!(inv.capability, "transform");
    assert_eq!(inv.category, GrammarCategoryKind::Transform);
    assert!(!inv.checkpoint);
    assert!(!inv.is_mutating);
    assert!(!inv.envelope_available.has_prev);
    assert!(inv.envelope_available.prev_source.is_none());
    assert!(inv.output_schema.is_some());
    assert!(inv.simulated_output.is_none());

    assert_eq!(inv.expressions.len(), 1);
    assert_eq!(inv.expressions[0].field_path, "config.filter");
    assert!(inv.expressions[0]
        .referenced_paths
        .contains(&".context.name".to_owned()));
}

#[test]
fn analyze_multi_invocation_chain() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("chain".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Chained".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object", "properties": {"x": {"type": "number"}}},
                    "filter": "{x: .context.value}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object", "properties": {"doubled": {"type": "number"}}},
                    "filter": "{doubled: (.prev.x * 2)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let trace = analyzer.analyze(&spec);
    assert_eq!(trace.invocations.len(), 2);

    assert!(!trace.invocations[0].envelope_available.has_prev);

    let inv1 = &trace.invocations[1];
    assert!(inv1.envelope_available.has_prev);
    assert!(inv1
        .envelope_available
        .prev_source
        .as_ref()
        .unwrap()
        .contains("invocation 0"));
    assert!(inv1.envelope_available.prev_schema.is_some());
    assert!(inv1.expressions[0]
        .referenced_paths
        .contains(&".prev.x".to_owned()));
}

#[test]
fn analyze_checkpoint_and_mutating() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Persist test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": {"type": "postgres", "table": "orders"},
                "data": {"expression": ".prev.payload"}
            }),
            checkpoint: true,
        }],
    };

    let trace = analyzer.analyze(&spec);
    let inv = &trace.invocations[0];
    assert!(inv.checkpoint);
    assert!(inv.is_mutating);
    assert_eq!(inv.expressions.len(), 1);
    assert_eq!(inv.expressions[0].field_path, "config.data.expression");
}

#[test]
fn analyze_empty_composition() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Empty".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![],
    };

    let trace = analyzer.analyze(&spec);
    assert!(trace.invocations.is_empty());
    assert!(trace
        .validation
        .iter()
        .any(|f| f.code == "EMPTY_COMPOSITION"));
}

#[test]
fn analyze_missing_capability() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Bad ref".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "nonexistent".to_owned(),
            config: json!({}),
            checkpoint: false,
        }],
    };

    let trace = analyzer.analyze(&spec);
    assert_eq!(trace.invocations.len(), 1);
    assert!(trace
        .validation
        .iter()
        .any(|f| f.code == "MISSING_CAPABILITY"));
}

#[test]
fn analyze_with_simulation_threads_data() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("sim_test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Simulation".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{doubled: (.context.value * 2)}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{result: (.prev.doubled + 1)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let input = SimulationInput {
        context: json!({"value": 21}),
        deps: json!({}),
        step: json!({"name": "test_step"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    assert!(trace.simulated);

    let inv0 = &trace.invocations[0];
    assert_eq!(inv0.simulated_output, Some(json!({"doubled": 42})));
    assert_eq!(
        inv0.expressions[0].simulated_result,
        Some(json!({"doubled": 42}))
    );

    let inv1 = &trace.invocations[1];
    assert_eq!(inv1.simulated_output, Some(json!({"result": 43})));
}

#[test]
fn analyze_with_simulation_mock_outputs() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Mock test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": {"type": "postgres", "table": "orders"},
                    "data": {"expression": ".context.order"}
                }),
                checkpoint: true,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{id: .prev.inserted_id}"
                }),
                checkpoint: false,
            },
        ],
    };

    let mut mock_outputs = HashMap::new();
    mock_outputs.insert(0, json!({"inserted_id": 42}));

    let input = SimulationInput {
        context: json!({"order": {"item": "widget"}}),
        deps: json!({}),
        step: json!({"name": "persist_step"}),
        mock_outputs,
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);

    let inv0 = &trace.invocations[0];
    assert!(inv0.mock_output_used);
    assert_eq!(inv0.simulated_output, Some(json!({"inserted_id": 42})));

    let inv1 = &trace.invocations[1];
    assert!(!inv1.mock_output_used);
    assert_eq!(inv1.simulated_output, Some(json!({"id": 42})));
}

#[test]
fn analyze_with_simulation_missing_mock() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "No mock".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": {"type": "postgres", "table": "orders"},
                "data": {"expression": ".context.order"}
            }),
            checkpoint: true,
        }],
    };

    let input = SimulationInput {
        context: json!({"order": {"item": "widget"}}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    let inv = &trace.invocations[0];
    assert!(!inv.mock_output_used);
    assert!(trace.validation.iter().any(|f| f.message.contains("mock")));
}

#[test]
fn analyze_with_simulation_assert_passthrough() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Assert passthrough".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{x: .context.value}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "assert".to_owned(),
                config: json!({"filter": "(.prev.x > 0)", "error": "must be positive"}),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{result: .prev.x}"
                }),
                checkpoint: false,
            },
        ],
    };

    let input = SimulationInput {
        context: json!({"value": 5}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);

    let assert_inv = &trace.invocations[1];
    assert_eq!(assert_inv.simulated_output, Some(json!({"x": 5})));

    let final_inv = &trace.invocations[2];
    assert_eq!(final_inv.simulated_output, Some(json!({"result": 5})));
}

#[test]
fn analyze_with_simulation_expression_failure() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Eval failure".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {"type": "object"},
                "filter": ".prev.missing_field | .nested"
            }),
            checkpoint: false,
        }],
    };

    let input = SimulationInput {
        context: json!({}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    assert!(trace.simulated);
    // Trace should be produced without panic
    assert_eq!(trace.invocations.len(), 1);
}
