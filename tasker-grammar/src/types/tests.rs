use serde_json::json;

use super::*;

// ---------------------------------------------------------------------------
// MutationProfile & IdempotencyProfile serialization
// ---------------------------------------------------------------------------

#[test]
fn mutation_profile_non_mutating_roundtrip() {
    let profile = MutationProfile::NonMutating;
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(json, json!({"type": "NonMutating"}));
    let back: MutationProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
}

#[test]
fn mutation_profile_mutating_roundtrip() {
    let profile = MutationProfile::Mutating {
        supports_idempotency_key: true,
    };
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(
        json,
        json!({"type": "Mutating", "supports_idempotency_key": true})
    );
    let back: MutationProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
}

#[test]
fn idempotency_profile_roundtrip() {
    for (profile, expected) in [
        (IdempotencyProfile::Inherent, "Inherent"),
        (IdempotencyProfile::WithKey, "WithKey"),
        (IdempotencyProfile::CapabilityDefined, "CapabilityDefined"),
    ] {
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json, json!(expected));
        let back: IdempotencyProfile = serde_json::from_value(json).unwrap();
        assert_eq!(back, profile);
    }
}

// ---------------------------------------------------------------------------
// GrammarCategoryKind enum — 6 variants, one per capability
// ---------------------------------------------------------------------------

#[test]
fn grammar_category_kind_exhaustive_match() {
    let kinds = [
        GrammarCategoryKind::Transform,
        GrammarCategoryKind::Validate,
        GrammarCategoryKind::Assert,
        GrammarCategoryKind::Acquire,
        GrammarCategoryKind::Persist,
        GrammarCategoryKind::Emit,
    ];

    for kind in kinds {
        match kind {
            GrammarCategoryKind::Transform => assert_eq!(kind.to_string(), "Transform"),
            GrammarCategoryKind::Validate => assert_eq!(kind.to_string(), "Validate"),
            GrammarCategoryKind::Assert => assert_eq!(kind.to_string(), "Assert"),
            GrammarCategoryKind::Acquire => assert_eq!(kind.to_string(), "Acquire"),
            GrammarCategoryKind::Persist => assert_eq!(kind.to_string(), "Persist"),
            GrammarCategoryKind::Emit => assert_eq!(kind.to_string(), "Emit"),
        }
    }
}

#[test]
fn grammar_category_kind_roundtrip() {
    for kind in [
        GrammarCategoryKind::Transform,
        GrammarCategoryKind::Validate,
        GrammarCategoryKind::Assert,
        GrammarCategoryKind::Acquire,
        GrammarCategoryKind::Persist,
        GrammarCategoryKind::Emit,
    ] {
        let json = serde_json::to_value(kind).unwrap();
        let back: GrammarCategoryKind = serde_json::from_value(json).unwrap();
        assert_eq!(back, kind);
    }
}

// ---------------------------------------------------------------------------
// Built-in GrammarCategory implementations — all 6
// ---------------------------------------------------------------------------

#[test]
fn transform_category_properties() {
    let cat = TransformCategory;
    assert_eq!(cat.name(), "Transform");
    assert_eq!(cat.kind(), GrammarCategoryKind::Transform);
    assert_eq!(cat.mutation_profile(), MutationProfile::NonMutating);
    assert_eq!(cat.idempotency(), IdempotencyProfile::Inherent);
    assert!(!cat.requires_checkpointing());
    assert!(cat.composition_constraints().is_empty());
}

#[test]
fn validate_category_properties() {
    let cat = ValidateCategory;
    assert_eq!(cat.name(), "Validate");
    assert_eq!(cat.kind(), GrammarCategoryKind::Validate);
    assert_eq!(cat.mutation_profile(), MutationProfile::NonMutating);
    assert_eq!(cat.idempotency(), IdempotencyProfile::Inherent);
    assert!(!cat.requires_checkpointing());

    // Validate config schema requires a JSON Schema
    let schema = cat.config_schema();
    let props = schema.get("properties").unwrap();
    assert!(
        props.get("schema").is_some(),
        "validate needs a 'schema' property"
    );
    assert!(
        props.get("coercion").is_some(),
        "validate needs a 'coercion' property"
    );
    assert!(
        props.get("on_failure").is_some(),
        "validate needs an 'on_failure' property"
    );
}

#[test]
fn assert_category_properties() {
    let cat = AssertCategory;
    assert_eq!(cat.name(), "Assert");
    assert_eq!(cat.kind(), GrammarCategoryKind::Assert);
    assert_eq!(cat.mutation_profile(), MutationProfile::NonMutating);
    assert_eq!(cat.idempotency(), IdempotencyProfile::Inherent);
    assert!(!cat.requires_checkpointing());

    // Assert config schema requires filter + error
    let schema = cat.config_schema();
    let props = schema.get("properties").unwrap();
    assert!(
        props.get("filter").is_some(),
        "assert needs a 'filter' property"
    );
    assert!(
        props.get("error").is_some(),
        "assert needs an 'error' property"
    );
}

#[test]
fn acquire_category_properties() {
    let cat = AcquireCategory;
    assert_eq!(cat.name(), "Acquire");
    assert_eq!(cat.kind(), GrammarCategoryKind::Acquire);
    assert_eq!(cat.mutation_profile(), MutationProfile::NonMutating);
    assert_eq!(cat.idempotency(), IdempotencyProfile::Inherent);
    assert!(!cat.requires_checkpointing());
}

#[test]
fn persist_category_properties() {
    let cat = PersistCategory;
    assert_eq!(cat.name(), "Persist");
    assert_eq!(cat.kind(), GrammarCategoryKind::Persist);
    assert_eq!(
        cat.mutation_profile(),
        MutationProfile::Mutating {
            supports_idempotency_key: true
        }
    );
    assert_eq!(cat.idempotency(), IdempotencyProfile::WithKey);
    assert!(cat.requires_checkpointing());
}

#[test]
fn emit_category_properties() {
    let cat = EmitCategory;
    assert_eq!(cat.name(), "Emit");
    assert_eq!(cat.kind(), GrammarCategoryKind::Emit);
    assert_eq!(
        cat.mutation_profile(),
        MutationProfile::Mutating {
            supports_idempotency_key: true
        }
    );
    assert_eq!(cat.idempotency(), IdempotencyProfile::WithKey);
    assert!(cat.requires_checkpointing());
}

#[test]
fn grammar_category_config_schemas_are_valid_json() {
    let categories: Vec<Box<dyn GrammarCategory>> = vec![
        Box::new(TransformCategory),
        Box::new(ValidateCategory),
        Box::new(AssertCategory),
        Box::new(AcquireCategory),
        Box::new(PersistCategory),
        Box::new(EmitCategory),
    ];

    for cat in &categories {
        let schema = cat.config_schema();
        assert!(
            schema.is_object(),
            "{} config schema should be an object",
            cat.name()
        );
    }
}

#[test]
fn category_kind_matches_trait_impl() {
    let cases: Vec<(Box<dyn GrammarCategory>, GrammarCategoryKind)> = vec![
        (Box::new(TransformCategory), GrammarCategoryKind::Transform),
        (Box::new(ValidateCategory), GrammarCategoryKind::Validate),
        (Box::new(AssertCategory), GrammarCategoryKind::Assert),
        (Box::new(AcquireCategory), GrammarCategoryKind::Acquire),
        (Box::new(PersistCategory), GrammarCategoryKind::Persist),
        (Box::new(EmitCategory), GrammarCategoryKind::Emit),
    ];

    for (cat, expected_kind) in cases {
        assert_eq!(
            cat.kind(),
            expected_kind,
            "kind() mismatch for {}",
            cat.name()
        );
    }
}

// ---------------------------------------------------------------------------
// CapabilityDeclaration serialization
// ---------------------------------------------------------------------------

#[test]
fn capability_declaration_roundtrip() {
    let decl = CapabilityDeclaration {
        name: "json_transform".into(),
        action: "transform".into(),
        grammar_category: GrammarCategoryKind::Transform,
        description: "Transform JSON data using jaq filters".into(),
        config_schema: json!({
            "type": "object",
            "properties": {
                "filter": {"type": "string"},
                "output": {"type": "object"}
            }
        }),
        mutation_profile: MutationProfile::NonMutating,
        tags: vec!["json".into(), "transform".into()],
        version: "1.0.0".into(),
    };

    let json = serde_json::to_value(&decl).unwrap();
    let back: CapabilityDeclaration = serde_json::from_value(json).unwrap();
    assert_eq!(back.name, "json_transform");
    assert_eq!(back.action, "transform");
    assert_eq!(back.grammar_category, GrammarCategoryKind::Transform);
    assert_eq!(back.mutation_profile, MutationProfile::NonMutating);
    assert_eq!(back.tags, vec!["json", "transform"]);
}

// ---------------------------------------------------------------------------
// CompositionSpec serialization — mirrors spec Shape 1:
// Validate → Transform → Assert → Persist → Emit
// ---------------------------------------------------------------------------

#[test]
fn composition_spec_roundtrip() {
    let spec = CompositionSpec {
        name: Some("order_processing".into()),
        outcome: OutcomeDeclaration {
            description: "Process and confirm an order".into(),
            output_schema: json!({"type": "object", "properties": {"order_id": {"type": "string"}}}),
        },
        steps: vec![
            CompositionStep {
                capability: "validate".into(),
                config: json!({
                    "schema": {"type": "object", "required": ["email", "items"]},
                    "coercion": "permissive",
                    "on_failure": "fail"
                }),
                checkpoint: false,
            },
            CompositionStep {
                capability: "transform".into(),
                config: json!({
                    "filter": ".context | {order_id: .id, total: .items | map(.price * .qty) | add}",
                    "output": {"type": "object", "properties": {"order_id": {"type": "string"}, "total": {"type": "number"}}}
                }),
                checkpoint: false,
            },
            CompositionStep {
                capability: "assert".into(),
                config: json!({"filter": ".prev.total > 0", "error": "Order total must be positive"}),
                checkpoint: false,
            },
            CompositionStep {
                capability: "persist".into(),
                config: json!({"resource": {"type": "database", "entity": "orders"}, "data": ".prev"}),
                checkpoint: true,
            },
            CompositionStep {
                capability: "emit".into(),
                config: json!({
                    "event_name": "order.confirmed",
                    "payload": "{order_id: .prev.order_id, total: .prev.total}"
                }),
                checkpoint: true,
            },
        ],
    };

    let json = serde_json::to_value(&spec).unwrap();
    let back: CompositionSpec = serde_json::from_value(json).unwrap();
    assert_eq!(back.name, Some("order_processing".into()));
    assert_eq!(back.steps.len(), 5);
    assert_eq!(back.steps[0].capability, "validate");
    assert_eq!(back.steps[1].capability, "transform");
    assert_eq!(back.steps[2].capability, "assert");
    assert_eq!(back.steps[3].capability, "persist");
    assert_eq!(back.steps[4].capability, "emit");
    assert!(back.steps[3].checkpoint);
    assert!(back.steps[4].checkpoint);
    assert!(!back.steps[0].checkpoint);
}

#[test]
fn composition_step_defaults() {
    let step: CompositionStep = serde_json::from_value(json!({
        "capability": "transform",
        "config": {"filter": "."}
    }))
    .unwrap();
    assert!(!step.checkpoint);
}

// ---------------------------------------------------------------------------
// CompositionCheckpoint serialization
// ---------------------------------------------------------------------------

#[test]
fn composition_checkpoint_roundtrip() {
    let checkpoint = CompositionCheckpoint {
        completed_step_index: 2,
        completed_capability: "persist".into(),
        step_output: json!({"order_id": "ord_123"}),
        all_step_outputs: [
            (0, json!("valid")),
            (1, json!({"total": 99.99})),
            (2, json!({"order_id": "ord_123"})),
        ]
        .into_iter()
        .collect(),
        was_mutation: true,
    };

    let json = serde_json::to_value(&checkpoint).unwrap();
    let back: CompositionCheckpoint = serde_json::from_value(json).unwrap();
    assert_eq!(back.completed_step_index, 2);
    assert_eq!(back.completed_capability, "persist");
    assert!(back.was_mutation);
    assert_eq!(back.all_step_outputs.len(), 3);
}

// ---------------------------------------------------------------------------
// Validation findings
// ---------------------------------------------------------------------------

#[test]
fn validation_finding_display() {
    let finding = ValidationFinding {
        severity: Severity::Error,
        code: "MISSING_CAPABILITY".into(),
        step_index: Some(3),
        message: "capability 'postgres_upsert' not found".into(),
        field_path: Some("steps[3].capability".into()),
    };

    let display = format!("{finding}");
    assert!(display.contains("MISSING_CAPABILITY"));
    assert!(display.contains("not found"));
}

#[test]
fn severity_roundtrip() {
    for severity in [Severity::Error, Severity::Warning, Severity::Info] {
        let json = serde_json::to_value(&severity).unwrap();
        let back: Severity = serde_json::from_value(json).unwrap();
        assert_eq!(back, severity);
    }
}

// ---------------------------------------------------------------------------
// ExecutionContext
// ---------------------------------------------------------------------------

#[test]
fn execution_context_construction() {
    let ctx = ExecutionContext {
        step_name: "validate_cart".into(),
        attempt: 1,
        checkpoint_state: None,
    };
    assert_eq!(ctx.step_name, "validate_cart");
    assert_eq!(ctx.attempt, 1);
    assert!(ctx.checkpoint_state.is_none());
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[test]
fn capability_error_display() {
    let err = CapabilityError::ExpressionEvaluation("null iterator".into());
    assert_eq!(
        err.to_string(),
        "expression evaluation error: null iterator"
    );
}

#[test]
fn composition_error_display() {
    let err = CompositionError::StepExecution {
        step_index: 2,
        capability: "persist".into(),
        cause: CapabilityError::Timeout,
    };
    let display = err.to_string();
    assert!(display.contains("step 2"));
    assert!(display.contains("persist"));
}

#[test]
fn registration_error_display() {
    let err = RegistrationError::NameConflict("http_get".into());
    assert!(err.to_string().contains("http_get"));
}

// ---------------------------------------------------------------------------
// GrammarCategory trait is object-safe — all 6 categories
// ---------------------------------------------------------------------------

#[test]
fn grammar_category_is_object_safe() {
    let categories: Vec<Box<dyn GrammarCategory>> = vec![
        Box::new(TransformCategory),
        Box::new(ValidateCategory),
        Box::new(AssertCategory),
        Box::new(AcquireCategory),
        Box::new(PersistCategory),
        Box::new(EmitCategory),
    ];

    let names: Vec<&str> = categories.iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        [
            "Transform",
            "Validate",
            "Assert",
            "Acquire",
            "Persist",
            "Emit"
        ]
    );
}

// ---------------------------------------------------------------------------
// CapabilityExecutor trait is object-safe
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct MockExecutor;

impl CapabilityExecutor for MockExecutor {
    fn execute(
        &self,
        _input: &serde_json::Value,
        _config: &serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({"mock": true}))
    }

    fn capability_name(&self) -> &str {
        "mock"
    }
}

#[test]
fn capability_executor_is_object_safe() {
    let executor: Box<dyn CapabilityExecutor> = Box::new(MockExecutor);
    assert_eq!(executor.capability_name(), "mock");
    let result = executor
        .execute(
            &json!({}),
            &json!({}),
            &ExecutionContext {
                step_name: "test".into(),
                attempt: 1,
                checkpoint_state: None,
            },
        )
        .unwrap();
    assert_eq!(result, json!({"mock": true}));
}
