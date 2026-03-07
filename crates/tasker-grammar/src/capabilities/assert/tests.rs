use serde_json::{json, Value};

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

use super::AssertExecutor;

fn executor() -> AssertExecutor {
    AssertExecutor::new(ExpressionEngine::with_defaults())
}

fn ctx() -> ExecutionContext {
    ExecutionContext {
        step_name: "test_step".into(),
        attempt: 1,
        checkpoint_state: None,
    }
}

/// Execute assert against a raw envelope value, wrapping in CompositionEnvelope.
fn exec(input: &Value, config: &Value) -> Result<Value, CapabilityError> {
    let envelope = CompositionEnvelope::new(input);
    executor().execute(&envelope, config, &ctx())
}

/// Standard composition context envelope used across tests.
fn raw_envelope() -> Value {
    json!({
        "context": {
            "order_id": "ORD-001",
            "customer_email": "user@example.com",
        },
        "deps": {
            "validate_cart": {"total": 99.99, "validated": true},
            "process_payment": {"payment_id": "pay_123"}
        },
        "step": {
            "name": "assert_order",
            "attempt": 1
        },
        "prev": null
    })
}

fn envelope_with_prev(prev: Value) -> Value {
    let mut env = raw_envelope();
    env["prev"] = prev;
    env
}

// ===========================================================================
// Simple form — filter + error
// ===========================================================================

#[test]
fn simple_assertion_passes() {
    let input = envelope_with_prev(json!({
        "total": 100, "subtotal": 90, "tax": 10
    }));

    let config = json!({
        "filter": ".prev.total == (.prev.subtotal + .prev.tax)",
        "error": "Totals do not balance"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["total"], json!(100));
    assert_eq!(result["subtotal"], json!(90));
}

#[test]
fn simple_assertion_fails_with_error_message() {
    let input = envelope_with_prev(json!({
        "total": 100, "subtotal": 80, "tax": 10
    }));

    let config = json!({
        "filter": ".prev.total == (.prev.subtotal + .prev.tax)",
        "error": "Totals do not balance"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("Totals do not balance"),
                "should contain error message: {msg}"
            );
            assert!(
                msg.contains("filter:"),
                "should contain filter reference: {msg}"
            );
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn simple_assertion_default_error_message() {
    let input = envelope_with_prev(json!({"value": false}));

    let config = json!({
        "filter": ".prev.value"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("assertion failed"),
                "should contain default message: {msg}"
            );
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn simple_assertion_with_compound_boolean() {
    let input = envelope_with_prev(json!({
        "payment_validated": true,
        "fraud_passed": true,
        "auto_approved": true
    }));

    let config = json!({
        "filter": "(.prev.payment_validated and .prev.fraud_passed) and .prev.auto_approved",
        "error": "Prerequisites not met"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["payment_validated"], json!(true));
}

#[test]
fn simple_assertion_returns_prev_on_success() {
    let prev_data = json!({"key": "value", "count": 42});
    let input = envelope_with_prev(prev_data.clone());

    let config = json!({
        "filter": ".prev.count > 0",
        "error": "Count must be positive"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result, prev_data);
}

#[test]
fn simple_assertion_falls_back_to_context_when_prev_null() {
    let input = raw_envelope(); // prev is null

    let config = json!({
        "filter": ".context.order_id == \"ORD-001\"",
        "error": "Order ID mismatch"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["order_id"], json!("ORD-001"));
}

// ===========================================================================
// Named conditions form — conditions + quantifier
// ===========================================================================

#[test]
fn all_conditions_pass_with_quantifier_all() {
    let input = envelope_with_prev(json!({
        "total": 100,
        "items": [1, 2, 3],
        "currency": "USD"
    }));

    let config = json!({
        "conditions": [
            {"name": "positive_total", "expression": ".prev.total > 0"},
            {"name": "has_items", "expression": ".prev.items | length > 0"},
            {"name": "valid_currency", "expression": ".prev.currency == \"USD\" or .prev.currency == \"EUR\""}
        ],
        "quantifier": "all"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["total"], json!(100));
}

#[test]
fn one_condition_fails_with_quantifier_all() {
    let input = envelope_with_prev(json!({
        "total": -5,
        "items": [1, 2, 3]
    }));

    let config = json!({
        "conditions": [
            {"name": "positive_total", "expression": ".prev.total > 0"},
            {"name": "has_items", "expression": ".prev.items | length > 0"}
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("positive_total"),
                "should name the failing condition: {msg}"
            );
            assert!(
                !msg.contains("has_items"),
                "should not name passing conditions: {msg}"
            );
            assert!(msg.contains("all"), "should mention quantifier: {msg}");
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn quantifier_any_with_at_least_one_passing() {
    let input = envelope_with_prev(json!({
        "total": -5,
        "items": [1, 2, 3]
    }));

    let config = json!({
        "conditions": [
            {"name": "positive_total", "expression": ".prev.total > 0"},
            {"name": "has_items", "expression": ".prev.items | length > 0"}
        ],
        "quantifier": "any"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["items"], json!([1, 2, 3]));
}

#[test]
fn quantifier_any_with_none_passing() {
    let input = envelope_with_prev(json!({
        "total": -5,
        "items": []
    }));

    let config = json!({
        "conditions": [
            {"name": "positive_total", "expression": ".prev.total > 0"},
            {"name": "has_items", "expression": ".prev.items | length > 0"}
        ],
        "quantifier": "any"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("positive_total"),
                "should list failing conditions: {msg}"
            );
            assert!(
                msg.contains("has_items"),
                "should list all failing conditions: {msg}"
            );
            assert!(msg.contains("any"), "should mention quantifier: {msg}");
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn quantifier_none_with_all_failing() {
    let input = envelope_with_prev(json!({
        "blacklisted": false,
        "sanctioned": false
    }));

    let config = json!({
        "conditions": [
            {"name": "not_blacklisted", "expression": ".prev.blacklisted"},
            {"name": "not_sanctioned", "expression": ".prev.sanctioned"}
        ],
        "quantifier": "none"
    });

    // Both conditions evaluate to false, and quantifier "none" requires all to fail
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["blacklisted"], json!(false));
}

#[test]
fn quantifier_none_fails_when_one_passes() {
    let input = envelope_with_prev(json!({
        "blacklisted": true,
        "sanctioned": false
    }));

    let config = json!({
        "conditions": [
            {"name": "not_blacklisted", "expression": ".prev.blacklisted"},
            {"name": "not_sanctioned", "expression": ".prev.sanctioned"}
        ],
        "quantifier": "none"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            // For "none", the "failed" conditions are the ones that passed (truthy)
            assert!(
                msg.contains("not_blacklisted"),
                "should name the condition that was truthy: {msg}"
            );
            assert!(
                !msg.contains("not_sanctioned"),
                "should not name the condition that was falsy: {msg}"
            );
            assert!(msg.contains("none"), "should mention quantifier: {msg}");
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn quantifier_defaults_to_all() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "conditions": [
            {"name": "check_x", "expression": ".prev.x == 1"}
        ]
        // quantifier not specified — defaults to "all"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["x"], json!(1));
}

// ===========================================================================
// Dependency precedent — conditional skip
// ===========================================================================

#[test]
fn dependency_precedent_skips_when_dep_missing() {
    let input = envelope_with_prev(json!({"value": 42}));

    let config = json!({
        "filter": ".prev.value > 100",
        "error": "Value too low",
        "dependency_precedent": "nonexistent_step"
    });

    // The dep doesn't exist, so assertion is skipped entirely
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["value"], json!(42));
}

#[test]
fn dependency_precedent_runs_when_dep_present() {
    let input = envelope_with_prev(json!({"value": 42}));
    // "validate_cart" exists in the envelope deps

    let config = json!({
        "filter": ".prev.value > 100",
        "error": "Value too low",
        "dependency_precedent": "validate_cart"
    });

    // The dep exists, so the assertion runs — and fails
    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
}

#[test]
fn dependency_precedent_with_conditions_form() {
    let input = envelope_with_prev(json!({"amount": 5}));

    let config = json!({
        "conditions": [
            {"name": "high_enough", "expression": ".prev.amount > 100"}
        ],
        "quantifier": "all",
        "dependency_precedent": "nonexistent_step"
    });

    // Skipped because dep doesn't exist
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["amount"], json!(5));
}

// ===========================================================================
// Error messages include condition names and expressions
// ===========================================================================

#[test]
fn error_includes_condition_name_and_expression() {
    let input = envelope_with_prev(json!({"total": 0}));

    let config = json!({
        "conditions": [
            {"name": "totals_balance", "expression": ".prev.total > 0"}
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("totals_balance"),
                "should include condition name: {msg}"
            );
            assert!(
                msg.contains(".prev.total > 0"),
                "should include expression: {msg}"
            );
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn multiple_failures_listed() {
    let input = envelope_with_prev(json!({
        "total": 0,
        "items": [],
        "valid": false
    }));

    let config = json!({
        "conditions": [
            {"name": "positive_total", "expression": ".prev.total > 0"},
            {"name": "has_items", "expression": ".prev.items | length > 0"},
            {"name": "is_valid", "expression": ".prev.valid"}
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("positive_total"),
                "should list first failure: {msg}"
            );
            assert!(
                msg.contains("has_items"),
                "should list second failure: {msg}"
            );
            assert!(msg.contains("is_valid"), "should list third failure: {msg}");
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

// ===========================================================================
// Config validation errors
// ===========================================================================

#[test]
fn missing_filter_and_conditions_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "error": "orphan error message"
    });

    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[test]
fn empty_conditions_array_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "conditions": [],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::ConfigValidation(msg) => {
            assert!(
                msg.contains("empty"),
                "should mention empty conditions: {msg}"
            );
        }
        other => panic!("expected ConfigValidation, got: {other:?}"),
    }
}

#[test]
fn condition_missing_expression_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "conditions": [
            {"name": "check_x"}
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::ConfigValidation(msg) => {
            assert!(
                msg.contains("expression"),
                "should mention missing expression: {msg}"
            );
        }
        other => panic!("expected ConfigValidation, got: {other:?}"),
    }
}

#[test]
fn unknown_quantifier_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "conditions": [
            {"name": "c", "expression": "true"}
        ],
        "quantifier": "exactly_two"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::ConfigValidation(msg) => {
            assert!(
                msg.contains("exactly_two"),
                "should mention invalid quantifier: {msg}"
            );
        }
        other => panic!("expected ConfigValidation, got: {other:?}"),
    }
}

#[test]
fn invalid_filter_expression_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "filter": "this is not valid jq @@@@"
    });

    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[test]
fn invalid_condition_expression_errors() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "conditions": [
            {"name": "bad_expr", "expression": "@@@ invalid jq"}
        ]
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::ExpressionEvaluation(msg) => {
            assert!(msg.contains("bad_expr"), "should identify condition: {msg}");
        }
        other => panic!("expected ExpressionEvaluation, got: {other:?}"),
    }
}

// ===========================================================================
// jq truthiness semantics
// ===========================================================================

#[test]
fn null_is_falsy() {
    let input = envelope_with_prev(json!({"x": null}));

    let config = json!({
        "filter": ".prev.x",
        "error": "x is null"
    });

    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
}

#[test]
fn false_is_falsy() {
    let input = envelope_with_prev(json!({"x": false}));

    let config = json!({
        "filter": ".prev.x",
        "error": "x is false"
    });

    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
}

#[test]
fn zero_is_truthy() {
    let input = envelope_with_prev(json!({"x": 0}));

    let config = json!({
        "filter": ".prev.x",
        "error": "x is zero"
    });

    // In jq, 0 is truthy (only null and false are falsy)
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["x"], json!(0));
}

#[test]
fn empty_string_is_truthy() {
    let input = envelope_with_prev(json!({"x": ""}));

    let config = json!({
        "filter": ".prev.x",
        "error": "x is empty string"
    });

    // In jq, "" is truthy
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["x"], json!(""));
}

#[test]
fn empty_array_is_truthy() {
    let input = envelope_with_prev(json!({"x": []}));

    let config = json!({
        "filter": ".prev.x",
        "error": "x is empty array"
    });

    // In jq, [] is truthy
    let result = exec(&input, &config).unwrap();
    assert_eq!(result["x"], json!([]));
}

// ===========================================================================
// Accessing different envelope fields
// ===========================================================================

#[test]
fn assert_on_deps() {
    let input = envelope_with_prev(json!({"passthrough": true}));

    let config = json!({
        "filter": ".deps.validate_cart.validated == true",
        "error": "Cart not validated"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["passthrough"], json!(true));
}

#[test]
fn assert_on_context() {
    let input = envelope_with_prev(json!({"passthrough": true}));

    let config = json!({
        "filter": ".context.order_id == \"ORD-001\"",
        "error": "Wrong order ID"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["passthrough"], json!(true));
}

#[test]
fn assert_on_step_metadata() {
    let input = envelope_with_prev(json!({"passthrough": true}));

    let config = json!({
        "filter": ".step.attempt == 1",
        "error": "Not first attempt"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["passthrough"], json!(true));
}

// ===========================================================================
// Capability name
// ===========================================================================

#[test]
fn capability_name_is_assert() {
    assert_eq!(executor().capability_name(), "assert");
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn condition_with_unnamed_condition_uses_default() {
    let input = envelope_with_prev(json!({"x": 0}));

    let config = json!({
        "conditions": [
            {"expression": ".prev.x > 0"}
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    match &err {
        CapabilityError::Execution(msg) => {
            assert!(
                msg.contains("unnamed"),
                "unnamed conditions get default label: {msg}"
            );
        }
        other => panic!("expected Execution error, got: {other:?}"),
    }
}

#[test]
fn single_condition_any_passes_when_true() {
    let input = envelope_with_prev(json!({"ok": true}));

    let config = json!({
        "conditions": [
            {"name": "is_ok", "expression": ".prev.ok"}
        ],
        "quantifier": "any"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["ok"], json!(true));
}

#[test]
fn complex_jq_expression_in_condition() {
    let input = envelope_with_prev(json!({
        "items": [
            {"price": 10, "qty": 2},
            {"price": 25, "qty": 1}
        ],
        "total": 45
    }));

    let config = json!({
        "conditions": [
            {
                "name": "total_matches_items",
                "expression": ".prev.total == ([.prev.items[] | .price * .qty] | add)"
            }
        ],
        "quantifier": "all"
    });

    let result = exec(&input, &config).unwrap();
    assert_eq!(result["total"], json!(45));
}

#[test]
fn complex_jq_expression_in_condition_fails() {
    let input = envelope_with_prev(json!({
        "items": [
            {"price": 10, "qty": 2},
            {"price": 25, "qty": 1}
        ],
        "total": 999
    }));

    let config = json!({
        "conditions": [
            {
                "name": "total_matches_items",
                "expression": ".prev.total == ([.prev.items[] | .price * .qty] | add)"
            }
        ],
        "quantifier": "all"
    });

    let err = exec(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
}
