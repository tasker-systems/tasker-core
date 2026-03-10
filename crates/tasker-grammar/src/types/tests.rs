use serde_json::json;
use validator::Validate;

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
// GrammarCategoryKind — FromStr (case-insensitive parsing)
// ---------------------------------------------------------------------------

#[test]
fn grammar_category_kind_from_str_lowercase() {
    for (input, expected) in [
        ("transform", GrammarCategoryKind::Transform),
        ("validate", GrammarCategoryKind::Validate),
        ("assert", GrammarCategoryKind::Assert),
        ("acquire", GrammarCategoryKind::Acquire),
        ("persist", GrammarCategoryKind::Persist),
        ("emit", GrammarCategoryKind::Emit),
    ] {
        let parsed: GrammarCategoryKind = input.parse().unwrap();
        assert_eq!(parsed, expected);
    }
}

#[test]
fn grammar_category_kind_from_str_case_insensitive() {
    assert_eq!(
        "Transform".parse::<GrammarCategoryKind>().unwrap(),
        GrammarCategoryKind::Transform
    );
    assert_eq!(
        "PERSIST".parse::<GrammarCategoryKind>().unwrap(),
        GrammarCategoryKind::Persist
    );
}

#[test]
fn grammar_category_kind_from_str_unknown() {
    let err = "compute".parse::<GrammarCategoryKind>().unwrap_err();
    assert!(err.to_string().contains("compute"));
    assert!(err.to_string().contains("unknown grammar category"));
}

// ---------------------------------------------------------------------------
// GrammarCategoryKind — into_category factory
// ---------------------------------------------------------------------------

#[test]
fn grammar_category_kind_into_category_roundtrip() {
    for kind in [
        GrammarCategoryKind::Transform,
        GrammarCategoryKind::Validate,
        GrammarCategoryKind::Assert,
        GrammarCategoryKind::Acquire,
        GrammarCategoryKind::Persist,
        GrammarCategoryKind::Emit,
    ] {
        let category = kind.into_category();
        assert_eq!(
            category.kind(),
            kind,
            "into_category().kind() should round-trip"
        );
    }
}

#[test]
fn grammar_category_kind_from_str_to_category() {
    let category = "persist"
        .parse::<GrammarCategoryKind>()
        .unwrap()
        .into_category();

    assert_eq!(category.name(), "Persist");
    assert!(category.requires_checkpointing());
    assert_eq!(
        category.mutation_profile(),
        MutationProfile::Mutating {
            supports_idempotency_key: true,
        }
    );
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
        props.get("coerce").is_some(),
        "validate needs a 'coerce' property"
    );
    assert!(
        props.get("filter_extra").is_some(),
        "validate needs a 'filter_extra' property"
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

    // Acquire config schema has success_criteria and result_shape
    let schema = cat.config_schema();
    let props = schema.get("properties").unwrap();
    assert!(
        props.get("success_criteria").is_some(),
        "acquire needs 'success_criteria'"
    );
    assert!(
        props.get("result_shape").is_some(),
        "acquire needs 'result_shape'"
    );
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

    // Persist config schema has success_criteria and result_shape
    let schema = cat.config_schema();
    let props = schema.get("properties").unwrap();
    assert!(
        props.get("success_criteria").is_some(),
        "persist needs 'success_criteria'"
    );
    assert!(
        props.get("result_shape").is_some(),
        "persist needs 'result_shape'"
    );
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
// CapabilityDeclaration serialization + validation
// ---------------------------------------------------------------------------

fn make_test_declaration() -> CapabilityDeclaration {
    CapabilityDeclaration {
        name: "json_transform".into(),
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
    }
}

#[test]
fn capability_declaration_roundtrip() {
    let decl = make_test_declaration();

    let json = serde_json::to_value(&decl).unwrap();
    let back: CapabilityDeclaration = serde_json::from_value(json).unwrap();
    assert_eq!(back.name, "json_transform");
    assert_eq!(back.grammar_category, GrammarCategoryKind::Transform);
    assert_eq!(back.mutation_profile, MutationProfile::NonMutating);
    assert_eq!(back.tags, vec!["json", "transform"]);
}

#[test]
fn capability_declaration_no_action_field() {
    let decl = make_test_declaration();
    let json = serde_json::to_value(&decl).unwrap();
    assert!(
        json.get("action").is_none(),
        "action field should not exist — grammar_category is 1:1 with capability"
    );
}

#[test]
fn capability_declaration_validates_name_not_empty() {
    let mut decl = make_test_declaration();
    decl.name = String::new();
    assert!(decl.validate().is_err());
}

#[test]
fn capability_declaration_validates_version_not_empty() {
    let mut decl = make_test_declaration();
    decl.version = String::new();
    assert!(decl.validate().is_err());
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
        invocations: vec![
            CapabilityInvocation {
                capability: "validate".into(),
                config: json!({
                    "schema": {"type": "object", "required": ["email", "items"]},
                    "coerce": true,
                    "on_failure": "error"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".into(),
                config: json!({
                    "filter": ".context | {order_id: .id, total: .items | map(.price * .qty) | add}",
                    "output": {"type": "object", "properties": {"order_id": {"type": "string"}, "total": {"type": "number"}}}
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "assert".into(),
                config: json!({"filter": ".prev.total > 0", "error": "Order total must be positive"}),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "persist".into(),
                config: json!({"resource": {"type": "database", "entity": "orders"}, "data": ".prev"}),
                checkpoint: true,
            },
            CapabilityInvocation {
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
    assert_eq!(back.invocations.len(), 5);
    assert_eq!(back.invocations[0].capability, "validate");
    assert_eq!(back.invocations[1].capability, "transform");
    assert_eq!(back.invocations[2].capability, "assert");
    assert_eq!(back.invocations[3].capability, "persist");
    assert_eq!(back.invocations[4].capability, "emit");
    assert!(back.invocations[3].checkpoint);
    assert!(back.invocations[4].checkpoint);
    assert!(!back.invocations[0].checkpoint);
}

#[test]
fn capability_invocation_defaults() {
    let invocation: CapabilityInvocation = serde_json::from_value(json!({
        "capability": "transform",
        "config": {"filter": "."}
    }))
    .unwrap();
    assert!(!invocation.checkpoint);
}

// ---------------------------------------------------------------------------
// CompositionCheckpoint serialization
// ---------------------------------------------------------------------------

#[test]
fn composition_checkpoint_roundtrip() {
    let checkpoint = CompositionCheckpoint {
        completed_invocation_index: 2,
        completed_capability: "persist".into(),
        invocation_output: json!({"order_id": "ord_123"}),
        all_invocation_outputs: [
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
    assert_eq!(back.completed_invocation_index, 2);
    assert_eq!(back.completed_capability, "persist");
    assert!(back.was_mutation);
    assert_eq!(back.all_invocation_outputs.len(), 3);
}

// ---------------------------------------------------------------------------
// Validation findings
// ---------------------------------------------------------------------------

#[test]
fn validation_finding_display() {
    let finding = ValidationFinding {
        severity: Severity::Error,
        code: "MISSING_CAPABILITY".into(),
        invocation_index: Some(3),
        message: "capability 'postgres_upsert' not found".into(),
        field_path: Some("invocations[3].capability".into()),
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
    let err = CompositionError::InvocationFailure {
        invocation_index: 2,
        capability: "persist".into(),
        cause: CapabilityError::Timeout,
    };
    let display = err.to_string();
    assert!(display.contains("invocation 2"));
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

/// Direct impl of CapabilityExecutor (for test mocks / truly dynamic config).
#[derive(Debug)]
struct DirectMockExecutor;

impl CapabilityExecutor for DirectMockExecutor {
    fn execute(
        &self,
        _envelope: &CompositionEnvelope<'_>,
        _config: &serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({"mock": true}))
    }

    fn capability_name(&self) -> &str {
        "direct_mock"
    }
}

#[test]
fn direct_executor_is_object_safe() {
    let executor: Box<dyn CapabilityExecutor> = Box::new(DirectMockExecutor);
    assert_eq!(executor.capability_name(), "direct_mock");
    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
    let envelope = CompositionEnvelope::new(&raw);
    let result = executor
        .execute(
            &envelope,
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

// ---------------------------------------------------------------------------
// TypedCapabilityExecutor — blanket impl provides CapabilityExecutor
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct TypedMockConfig {
    greeting: String,
}

#[derive(Debug)]
struct TypedMockExecutor;

impl TypedCapabilityExecutor for TypedMockExecutor {
    type Config = TypedMockConfig;

    fn execute_typed(
        &self,
        _envelope: &CompositionEnvelope<'_>,
        config: &TypedMockConfig,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, CapabilityError> {
        Ok(json!({ "message": config.greeting }))
    }

    fn capability_name(&self) -> &str {
        "typed_mock"
    }
}

#[test]
fn typed_executor_is_object_safe_via_blanket() {
    // TypedCapabilityExecutor → CapabilityExecutor via blanket impl
    let executor: Box<dyn CapabilityExecutor> = Box::new(TypedMockExecutor);
    assert_eq!(executor.capability_name(), "typed_mock");

    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
    let envelope = CompositionEnvelope::new(&raw);
    let config = json!({"greeting": "hello"});

    let result = executor
        .execute(
            &envelope,
            &config,
            &ExecutionContext {
                step_name: "test".into(),
                attempt: 1,
                checkpoint_state: None,
            },
        )
        .unwrap();
    assert_eq!(result, json!({"message": "hello"}));
}

#[test]
fn typed_executor_validate_config_accepts_valid() {
    let executor: Box<dyn CapabilityExecutor> = Box::new(TypedMockExecutor);
    assert!(executor.validate_config(&json!({"greeting": "hi"})).is_ok());
}

#[test]
fn typed_executor_validate_config_rejects_invalid() {
    let executor: Box<dyn CapabilityExecutor> = Box::new(TypedMockExecutor);
    let err = executor
        .validate_config(&json!({"wrong_field": 42}))
        .unwrap_err();
    assert!(
        matches!(&err, CapabilityError::ConfigValidation(msg) if msg.contains("data type mismatch")),
        "expected ConfigValidation with data type mismatch, got: {err:?}"
    );
}

#[test]
fn typed_executor_execute_rejects_bad_config() {
    let executor: Box<dyn CapabilityExecutor> = Box::new(TypedMockExecutor);
    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
    let envelope = CompositionEnvelope::new(&raw);
    let bad_config = json!({"wrong": true});

    let err = executor
        .execute(
            &envelope,
            &bad_config,
            &ExecutionContext {
                step_name: "test".into(),
                attempt: 1,
                checkpoint_state: None,
            },
        )
        .unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[test]
fn direct_executor_validate_config_defaults_to_ok() {
    // Direct impl has no typed config — default validate_config returns Ok
    let executor: Box<dyn CapabilityExecutor> = Box::new(DirectMockExecutor);
    assert!(executor
        .validate_config(&json!({"anything": "works"}))
        .is_ok());
}

// ---------------------------------------------------------------------------
// CompositionEnvelope
// ---------------------------------------------------------------------------

#[test]
fn envelope_context_accessor() {
    let raw = json!({"context": {"id": 1}, "deps": {}, "step": {}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.context()["id"], json!(1));
}

#[test]
fn envelope_deps_accessor() {
    let raw = json!({"context": {}, "deps": {"step_a": {"total": 42}}, "step": {}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.deps()["step_a"]["total"], json!(42));
    assert_eq!(env.dep("step_a")["total"], json!(42));
    assert_eq!(env.dep("missing"), &json!(null));
}

#[test]
fn envelope_step_accessor() {
    let raw =
        json!({"context": {}, "deps": {}, "step": {"name": "s1", "attempts": 1}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.step()["name"], json!("s1"));
}

#[test]
fn envelope_prev_null_means_no_prev() {
    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert!(!env.has_prev());
    assert_eq!(env.prev(), &json!(null));
}

#[test]
fn envelope_prev_present() {
    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": {"result": true}});
    let env = CompositionEnvelope::new(&raw);
    assert!(env.has_prev());
    assert_eq!(env.prev()["result"], json!(true));
}

#[test]
fn envelope_resolve_target_uses_prev_when_present() {
    let raw = json!({"context": {"orig": true}, "deps": {}, "step": {}, "prev": {"data": 1}});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.resolve_target()["data"], json!(1));
}

#[test]
fn envelope_resolve_target_falls_back_to_context() {
    let raw = json!({"context": {"orig": true}, "deps": {}, "step": {}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.resolve_target()["orig"], json!(true));
}

#[test]
fn envelope_raw_returns_original() {
    let raw = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.raw(), &raw);
}

#[test]
fn envelope_missing_fields_return_null() {
    let raw = json!({});
    let env = CompositionEnvelope::new(&raw);
    assert_eq!(env.context(), &json!(null));
    assert_eq!(env.deps(), &json!(null));
    assert_eq!(env.step(), &json!(null));
    assert_eq!(env.prev(), &json!(null));
    assert!(!env.has_prev());
}

// ---------------------------------------------------------------------------
// OnFailure enum
// ---------------------------------------------------------------------------

#[test]
fn on_failure_default_is_error() {
    assert_eq!(OnFailure::default(), OnFailure::Error);
}

#[test]
fn on_failure_from_str() {
    assert_eq!("error".parse::<OnFailure>().unwrap(), OnFailure::Error);
    assert_eq!("warn".parse::<OnFailure>().unwrap(), OnFailure::Warn);
    assert_eq!("skip".parse::<OnFailure>().unwrap(), OnFailure::Skip);
}

#[test]
fn on_failure_from_str_case_insensitive() {
    assert_eq!("ERROR".parse::<OnFailure>().unwrap(), OnFailure::Error);
    assert_eq!("Warn".parse::<OnFailure>().unwrap(), OnFailure::Warn);
    assert_eq!("SKIP".parse::<OnFailure>().unwrap(), OnFailure::Skip);
}

#[test]
fn on_failure_from_str_unknown() {
    let err = "fail".parse::<OnFailure>().unwrap_err();
    assert!(err.to_string().contains("fail"));
    assert!(err.to_string().contains("error, warn, skip"));
}

#[test]
fn on_failure_display() {
    assert_eq!(OnFailure::Error.to_string(), "error");
    assert_eq!(OnFailure::Warn.to_string(), "warn");
    assert_eq!(OnFailure::Skip.to_string(), "skip");
}

#[test]
fn on_failure_serde_roundtrip() {
    for variant in [OnFailure::Error, OnFailure::Warn, OnFailure::Skip] {
        let json = serde_json::to_value(variant).unwrap();
        let back: OnFailure = serde_json::from_value(json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn on_failure_serde_lowercase() {
    assert_eq!(
        serde_json::to_value(OnFailure::Error).unwrap(),
        json!("error")
    );
    assert_eq!(
        serde_json::to_value(OnFailure::Warn).unwrap(),
        json!("warn")
    );
    assert_eq!(
        serde_json::to_value(OnFailure::Skip).unwrap(),
        json!("skip")
    );
}
