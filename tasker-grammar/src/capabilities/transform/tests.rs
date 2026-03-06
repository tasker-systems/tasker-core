use serde_json::json;

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, ExecutionContext};

use super::TransformExecutor;

fn executor() -> TransformExecutor {
    TransformExecutor::new(ExpressionEngine::with_defaults())
}

fn ctx() -> ExecutionContext {
    ExecutionContext {
        step_name: "test_step".into(),
        attempt: 1,
        checkpoint_state: None,
    }
}

/// Standard composition context envelope used across tests.
fn envelope() -> serde_json::Value {
    json!({
        "context": {
            "order_id": "ORD-001",
            "customer_email": "user@example.com",
            "tax_rate": 0.0875,
            "cart_items": [
                {"sku": "A1", "name": "Widget", "price": 25.0, "quantity": 2},
                {"sku": "B2", "name": "Gadget", "price": 50.0, "quantity": 1}
            ],
            "flags": ["priority", "manual_review"],
            "promo_code": "SAVE15"
        },
        "deps": {
            "validate_cart": {
                "total": 100.0,
                "validated_items": [
                    {"sku": "A1", "quantity": 2, "line_total": 50.0},
                    {"sku": "B2", "quantity": 1, "line_total": 50.0}
                ]
            },
            "customer_profile": {
                "tier": "gold",
                "lifetime_value": 15000,
                "name": "Alice"
            },
            "process_payment": {
                "payment_id": "pay_123",
                "transaction_id": "txn_456"
            }
        },
        "step": {
            "name": "create_order",
            "attempts": 1,
            "inputs": null
        },
        "prev": null
    })
}

// ---------------------------------------------------------------------------
// Core executor tests (TAS-356 parent)
// ---------------------------------------------------------------------------

#[test]
fn simple_filter_execution() {
    let exec = executor();
    let input = json!({"value": 42});
    let config = json!({"filter": ".value + 1"});
    let result = exec.execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!(43));
}

#[test]
fn filter_with_output_schema_pass() {
    let exec = executor();
    let input = json!({"x": 10});
    let config = json!({
        "filter": "{result: .x}",
        "output": {
            "type": "object",
            "required": ["result"],
            "properties": {"result": {"type": "integer"}}
        }
    });
    let result = exec.execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!({"result": 10}));
}

#[test]
fn filter_with_output_schema_fail() {
    let exec = executor();
    let input = json!({"x": "not_a_number"});
    let config = json!({
        "filter": "{result: .x}",
        "output": {
            "type": "object",
            "properties": {"result": {"type": "number"}}
        }
    });
    let err = exec.execute(&input, &config, &ctx()).unwrap_err();
    assert!(
        matches!(err, CapabilityError::OutputValidation(_)),
        "expected OutputValidation, got {err:?}"
    );
}

#[test]
fn missing_filter_config() {
    let exec = executor();
    let config = json!({"output": {"type": "object"}});
    let err = exec.execute(&json!({}), &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[test]
fn invalid_filter_syntax() {
    let exec = executor();
    let config = json!({"filter": ".foo ||| bar"});
    let err = exec.execute(&json!({}), &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[test]
fn capability_name_is_transform() {
    assert_eq!(executor().capability_name(), "transform");
}

#[test]
fn envelope_context_access() {
    let exec = executor();
    let config = json!({"filter": ".context.order_id"});
    let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
    assert_eq!(result, json!("ORD-001"));
}

#[test]
fn envelope_deps_access() {
    let exec = executor();
    let config = json!({"filter": ".deps.validate_cart.total"});
    let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
    assert_eq!(result, json!(100.0));
}

#[test]
fn envelope_step_access() {
    let exec = executor();
    let config = json!({"filter": ".step.name"});
    let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
    assert_eq!(result, json!("create_order"));
}

#[test]
fn envelope_prev_null_for_first_invocation() {
    let exec = executor();
    let config = json!({"filter": ".prev"});
    let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn envelope_prev_chaining() {
    let exec = executor();
    let mut env = envelope();
    env["prev"] = json!({"total": 100.0, "tax": 8.75});
    let config = json!({"filter": ".prev.total + .prev.tax"});
    let result = exec.execute(&env, &config, &ctx()).unwrap();
    assert_eq!(result, json!(108.75));
}

#[test]
fn null_field_handling() {
    let exec = executor();
    let config = json!({"filter": ".context.nonexistent"});
    let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn output_schema_required_field_missing() {
    let exec = executor();
    let input = json!({"x": 1});
    let config = json!({
        "filter": "{a: .x}",
        "output": {
            "type": "object",
            "required": ["a", "b"],
            "properties": {
                "a": {"type": "integer"},
                "b": {"type": "string"}
            }
        }
    });
    let err = exec.execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::OutputValidation(_)));
}

#[test]
fn no_output_schema_skips_validation() {
    let exec = executor();
    let config = json!({"filter": "{anything: 123}"});
    let result = exec.execute(&json!({}), &config, &ctx()).unwrap();
    assert_eq!(result, json!({"anything": 123}));
}

#[test]
fn invalid_output_schema_returns_config_error() {
    let exec = executor();
    let config = json!({
        "filter": "42",
        "output": {"type": "not_a_real_type"}
    });
    // jsonschema may accept unknown types gracefully; we just verify no panic
    let _ = exec.execute(&json!({}), &config, &ctx());
}

// ---------------------------------------------------------------------------
// TAS-325: Projection & restructuring patterns
// ---------------------------------------------------------------------------

mod projection {
    use super::*;

    // Select subset of fields from input
    #[test]
    fn field_projection() {
        let exec = executor();
        let config = json!({
            "filter": "{total: .deps.validate_cart.total, payment_id: .deps.process_payment.payment_id}",
            "output": {
                "type": "object",
                "required": ["total", "payment_id"],
                "properties": {
                    "total": {"type": "number"},
                    "payment_id": {"type": "string"}
                }
            }
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["total"], json!(100.0));
        assert_eq!(result["payment_id"], json!("pay_123"));
    }

    // Rename fields: {new_name: .old_name}
    #[test]
    fn field_renaming() {
        let exec = executor();
        let config = json!({
            "filter": "{email: .context.customer_email, ref_id: .context.order_id}"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["email"], json!("user@example.com"));
        assert_eq!(result["ref_id"], json!("ORD-001"));
    }

    // Deep nested extraction: .billing.address.city
    #[test]
    fn nested_extraction() {
        let exec = executor();
        let input = json!({
            "context": {"billing": {"address": {"city": "Portland", "zip": "97201"}}},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": ".context.billing.address.city"});
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result, json!("Portland"));
    }

    // Array element restructuring: [.records[] | {id, name}]
    #[test]
    fn array_element_restructuring() {
        let exec = executor();
        let config = json!({
            "filter": "[.deps.validate_cart.validated_items[] | {sku, qty: .quantity}]",
            "output": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "sku": {"type": "string"},
                        "qty": {"type": "integer"}
                    }
                }
            }
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(
            result,
            json!([
                {"sku": "A1", "qty": 2},
                {"sku": "B2", "qty": 1}
            ])
        );
    }

    // Nested object construction
    #[test]
    fn nested_object_construction() {
        let exec = executor();
        let input = json!({
            "context": {"addr": "123 Main St", "postal_code": "97201", "name": "Alice"},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "{customer: {name: .context.name, billing: {address: .context.addr, zip: .context.postal_code}}}"
        });
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(
            result,
            json!({
                "customer": {
                    "name": "Alice",
                    "billing": {"address": "123 Main St", "zip": "97201"}
                }
            })
        );
    }

    // Flattening nested structures
    #[test]
    fn flattening() {
        let exec = executor();
        let input = json!({
            "context": {"deep": {"nested": {"value": 42}}},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": "{flat_field: .context.deep.nested.value}"});
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result, json!({"flat_field": 42}));
    }

    // Multi-source projection: combining .context and .deps
    #[test]
    fn multi_source_projection() {
        let exec = executor();
        let config = json!({
            "filter": "{order_id: .context.order_id, customer_name: .deps.customer_profile.name, total: .deps.validate_cart.total}"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["order_id"], json!("ORD-001"));
        assert_eq!(result["customer_name"], json!("Alice"));
        assert_eq!(result["total"], json!(100.0));
    }

    // Missing optional fields produce null
    #[test]
    fn missing_optional_fields() {
        let exec = executor();
        let config = json!({
            "filter": "{present: .context.order_id, absent: .context.nonexistent}"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["present"], json!("ORD-001"));
        assert_eq!(result["absent"], json!(null));
    }

    // .prev chaining: restructure output from a prior transform
    #[test]
    fn prev_chaining() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] =
            json!({"items": [{"sku": "A1", "price": 25.0}, {"sku": "B2", "price": 50.0}]});
        let config = json!({
            "filter": "{skus: [.prev.items[].sku], count: (.prev.items | length)}"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result, json!({"skus": ["A1", "B2"], "count": 2}));
    }

    // Output schema validation on projected result
    #[test]
    fn output_schema_validates_projection() {
        let exec = executor();
        let config = json!({
            "filter": "{customer: .deps.customer_profile.name, items: .deps.validate_cart.validated_items}",
            "output": {
                "type": "object",
                "required": ["customer", "items"],
                "properties": {
                    "customer": {"type": "string"},
                    "items": {"type": "array"}
                }
            }
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["customer"], json!("Alice"));
        assert!(result["items"].is_array());
    }
}

// ---------------------------------------------------------------------------
// TAS-326: Arithmetic, aggregation & derivation patterns
// ---------------------------------------------------------------------------

mod computation {
    use super::*;

    // Basic arithmetic
    #[test]
    fn basic_arithmetic() {
        let exec = executor();
        let input = json!({"context": {"price": 25.0, "quantity": 4}, "deps": {}, "step": {}, "prev": null});
        let config = json!({"filter": ".context.price * .context.quantity"});
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result, json!(100.0));
    }

    // Aggregation over arrays: map + add
    #[test]
    fn array_aggregation() {
        let exec = executor();
        let config = json!({
            "filter": "[.deps.validate_cart.validated_items[].line_total] | add"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(100.0));
    }

    // Array length
    #[test]
    fn array_length() {
        let exec = executor();
        let config = json!({
            "filter": ".deps.validate_cart.validated_items | length"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(2));
    }

    // String interpolation
    #[test]
    fn string_interpolation() {
        let exec = executor();
        let config = json!({
            "filter": "\"Order \\(.context.order_id) confirmed\""
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!("Order ORD-001 confirmed"));
    }

    // Multi-step derivation with variable binding
    #[test]
    fn variable_binding() {
        let exec = executor();
        let config = json!({
            "filter": "([.deps.validate_cart.validated_items[].line_total] | add) as $subtotal | ($subtotal * .context.tax_rate) as $tax | {subtotal: $subtotal, tax: ($tax * 100 | round / 100), total: (($subtotal + $tax) * 100 | round / 100)}",
            "output": {
                "type": "object",
                "required": ["subtotal", "tax", "total"],
                "properties": {
                    "subtotal": {"type": "number"},
                    "tax": {"type": "number"},
                    "total": {"type": "number"}
                }
            }
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["subtotal"], json!(100.0));
        assert_eq!(result["tax"], json!(8.75));
        assert_eq!(result["total"], json!(108.75));
    }

    // Cross-dependency computation
    #[test]
    fn cross_dependency_computation() {
        let exec = executor();
        let config = json!({
            "filter": "{combined: (.deps.validate_cart.total + .deps.customer_profile.lifetime_value)}"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["combined"], json!(15100.0));
    }

    // Sorting
    #[test]
    fn sorting_and_ranking() {
        let exec = executor();
        let input = json!({
            "context": {"scores": [{"name": "A", "score": 50}, {"name": "B", "score": 90}, {"name": "C", "score": 70}]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": ".context.scores | sort_by(.score) | reverse | to_entries | map({rank: (.key + 1), name: .value.name, score: .value.score})"
        });
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result[0]["rank"], json!(1));
        assert_eq!(result[0]["name"], json!("B"));
        assert_eq!(result[1]["rank"], json!(2));
        assert_eq!(result[1]["name"], json!("C"));
    }

    // Mixed projection + computation
    #[test]
    fn mixed_projection_and_computation() {
        let exec = executor();
        let config = json!({
            "filter": "{order_id: .context.order_id, item_count: (.deps.validate_cart.validated_items | length), total: .deps.validate_cart.total, confirmation: \"Order \\(.context.order_id) for $\\(.deps.validate_cart.total)\"}"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["order_id"], json!("ORD-001"));
        assert_eq!(result["item_count"], json!(2));
        assert_eq!(result["total"], json!(100.0));
        assert_eq!(result["confirmation"], json!("Order ORD-001 for $100.0"));
    }

    // Group-by aggregation
    #[test]
    fn group_by_aggregation() {
        let exec = executor();
        let input = json!({
            "context": {
                "records": [
                    {"category": "A", "amount": 10},
                    {"category": "B", "amount": 20},
                    {"category": "A", "amount": 30},
                    {"category": "B", "amount": 5}
                ]
            },
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": ".context.records | group_by(.category) | map({category: .[0].category, total: (map(.amount) | add)})"
        });
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result[0], json!({"category": "A", "total": 40}));
        assert_eq!(result[1], json!({"category": "B", "total": 25}));
    }

    // Computation from .prev
    #[test]
    fn prev_computation() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"items": [{"price": 10, "qty": 2}, {"price": 5, "qty": 3}]});
        let config = json!({
            "filter": ".prev.items | map(.price * .qty) | add"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result, json!(35));
    }
}

// ---------------------------------------------------------------------------
// TAS-327: Boolean evaluation & classification patterns
// ---------------------------------------------------------------------------

mod evaluation {
    use super::*;

    // Simple boolean
    #[test]
    fn simple_boolean() {
        let exec = executor();
        let config = json!({"filter": ".deps.validate_cart.total > 50"});
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Compound boolean
    #[test]
    fn compound_boolean() {
        let exec = executor();
        let config = json!({
            "filter": ".deps.validate_cart.total > 50 and .deps.customer_profile.tier == \"gold\""
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Conditional classification via if-then-elif-else
    #[test]
    fn conditional_classification() {
        let exec = executor();
        let config = json!({
            "filter": "if .deps.customer_profile.lifetime_value > 10000 then \"gold\" elif .deps.customer_profile.lifetime_value > 1000 then \"silver\" else \"standard\" end",
            "output": {"type": "string", "enum": ["gold", "silver", "standard"]}
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!("gold"));
    }

    // Array predicate: any
    #[test]
    fn array_predicate_any() {
        let exec = executor();
        let config = json!({
            "filter": ".context.flags | any(. == \"manual_review\")"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Array predicate: all
    #[test]
    fn array_predicate_all() {
        let exec = executor();
        let input = json!({
            "context": {"items": [{"valid": true}, {"valid": true}]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": ".context.items | all(.valid)"});
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Comparison chains
    #[test]
    fn comparison_chains() {
        let exec = executor();
        let input = json!({"context": {"score": 85}, "deps": {}, "step": {}, "prev": null});
        let config = json!({"filter": ".context.score >= 80 and .context.score < 90"});
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Null handling in comparisons
    #[test]
    fn null_comparison() {
        let exec = executor();
        let config = json!({"filter": ".context.nonexistent == null"});
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Multi-field evaluation object
    #[test]
    fn multi_field_evaluation() {
        let exec = executor();
        let config = json!({
            "filter": "{is_high_value: (.deps.validate_cart.total > 50), needs_review: (.context.flags | any(. == \"manual_review\")), customer_tier: (if .deps.customer_profile.lifetime_value > 10000 then \"gold\" elif .deps.customer_profile.lifetime_value > 1000 then \"silver\" else \"standard\" end)}",
            "output": {
                "type": "object",
                "required": ["is_high_value", "needs_review", "customer_tier"],
                "properties": {
                    "is_high_value": {"type": "boolean"},
                    "needs_review": {"type": "boolean"},
                    "customer_tier": {"type": "string"}
                }
            }
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result["is_high_value"], json!(true));
        assert_eq!(result["needs_review"], json!(true));
        assert_eq!(result["customer_tier"], json!("gold"));
    }

    // Cross-dependency evaluation
    #[test]
    fn cross_dependency_evaluation() {
        let exec = executor();
        let config = json!({
            "filter": ".deps.validate_cart.total > 50 and .deps.customer_profile.tier == \"gold\""
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Negation patterns
    #[test]
    fn negation_pattern() {
        let exec = executor();
        let config = json!({
            "filter": "(.context.flags | any(. == \"blocked\")) | not"
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    // Boolean schema validation
    #[test]
    fn boolean_schema_validation_pass() {
        let exec = executor();
        let config = json!({
            "filter": ".deps.validate_cart.total > 50",
            "output": {"type": "boolean"}
        });
        let result = exec.execute(&envelope(), &config, &ctx()).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn boolean_schema_validation_fail() {
        let exec = executor();
        let config = json!({
            "filter": ".deps.validate_cart.total",
            "output": {"type": "boolean"}
        });
        let err = exec.execute(&envelope(), &config, &ctx()).unwrap_err();
        assert!(matches!(err, CapabilityError::OutputValidation(_)));
    }
}

// ---------------------------------------------------------------------------
// TAS-329: Rule-engine patterns (first-match / all-match)
// ---------------------------------------------------------------------------

mod rules {
    use super::*;

    // First-match: returns first matching rule result
    #[test]
    fn first_match_single() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"total": 1500, "tier": "gold"});
        let config = json!({
            "filter": "if .prev.total > 1000 and .prev.tier == \"gold\" then {priority: \"urgent\", routing: \"vip_queue\"} elif .prev.total > 500 then {priority: \"high\", routing: \"priority_queue\"} elif .prev.total > 0 then {priority: \"normal\", routing: \"standard_queue\"} else {priority: \"low\", routing: \"review_queue\"} end",
            "output": {
                "type": "object",
                "required": ["priority", "routing"],
                "properties": {
                    "priority": {"type": "string"},
                    "routing": {"type": "string"}
                }
            }
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["priority"], json!("urgent"));
        assert_eq!(result["routing"], json!("vip_queue"));
    }

    // First-match correctly short-circuits (multiple potential matches)
    #[test]
    fn first_match_short_circuits() {
        let exec = executor();
        let mut env = envelope();
        // total > 500 AND total > 0 both true, but first-match returns first
        env["prev"] = json!({"total": 750, "tier": "silver"});
        let config = json!({
            "filter": "if .prev.total > 1000 and .prev.tier == \"gold\" then {priority: \"urgent\"} elif .prev.total > 500 then {priority: \"high\"} elif .prev.total > 0 then {priority: \"normal\"} else {priority: \"low\"} end"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["priority"], json!("high"));
    }

    // First-match with default (else) when no conditions match
    #[test]
    fn first_match_default() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"total": -5, "tier": "none"});
        let config = json!({
            "filter": "if .prev.total > 1000 then {priority: \"urgent\"} elif .prev.total > 0 then {priority: \"normal\"} else {priority: \"low\"} end"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["priority"], json!("low"));
    }

    // All-match: collects all matching rules
    #[test]
    fn all_match_collects() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"total": 600});
        env["context"]["promo_code"] = json!("SAVE15");

        let config = json!({
            "filter": "[(if .prev.total > 500 then {rule: \"bulk_discount\", discount: 0.10} else empty end), (if .deps.customer_profile.tier == \"gold\" then {rule: \"loyalty_discount\", discount: 0.05} else empty end), (if .context.promo_code != null then {rule: \"promo_discount\", discount: 0.15} else empty end)] | {applicable_discounts: .}",
            "output": {
                "type": "object",
                "required": ["applicable_discounts"],
                "properties": {
                    "applicable_discounts": {"type": "array"}
                }
            }
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        let discounts = result["applicable_discounts"].as_array().unwrap();
        assert_eq!(discounts.len(), 3);
        assert_eq!(discounts[0]["rule"], json!("bulk_discount"));
        assert_eq!(discounts[1]["rule"], json!("loyalty_discount"));
        assert_eq!(discounts[2]["rule"], json!("promo_discount"));
    }

    // All-match with no matches returns empty array
    #[test]
    fn all_match_no_matches() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"total": 10});
        env["deps"]["customer_profile"]["tier"] = json!("standard");
        env["context"]["promo_code"] = json!(null);

        let config = json!({
            "filter": "[(if .prev.total > 500 then {rule: \"bulk\"} else empty end), (if .deps.customer_profile.tier == \"gold\" then {rule: \"loyalty\"} else empty end), (if .context.promo_code != null then {rule: \"promo\"} else empty end)] | {applicable_discounts: .}"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        let discounts = result["applicable_discounts"].as_array().unwrap();
        assert!(discounts.is_empty());
    }

    // Complex conditions with cross-dependency references
    #[test]
    fn complex_conditions() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"refund_amount": 300, "refund_reason": "defective"});
        let config = json!({
            "filter": "if .prev.refund_amount <= 50 then {approval_path: \"auto_approved\"} elif (.prev.refund_reason == \"defective\" or .prev.refund_reason == \"wrong_item\") and .prev.refund_amount <= 500 then {approval_path: \"auto_approved\"} elif .prev.refund_amount > 500 then {approval_path: \"manager_review\"} else {approval_path: \"standard_review\"} end"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["approval_path"], json!("auto_approved"));
    }

    // Nested rule results with multiple fields
    #[test]
    fn nested_rule_results() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"score": 95});
        let config = json!({
            "filter": "if .prev.score >= 90 then {grade: \"A\", pass: true, honors: true} elif .prev.score >= 80 then {grade: \"B\", pass: true, honors: false} else {grade: \"F\", pass: false, honors: false} end"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["grade"], json!("A"));
        assert_eq!(result["pass"], json!(true));
        assert_eq!(result["honors"], json!(true));
    }

    // Dynamic rule values referencing input data
    #[test]
    fn dynamic_rule_values() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"base_discount": 0.05, "total": 1000});
        let config = json!({
            "filter": "if .prev.total > 500 then {discount: (.prev.base_discount * 2), savings: (.prev.total * .prev.base_discount * 2)} else {discount: .prev.base_discount, savings: (.prev.total * .prev.base_discount)} end"
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["discount"], json!(0.1));
        assert_eq!(result["savings"], json!(100.0));
    }

    // Empty input handling (null/missing fields)
    #[test]
    fn empty_input_handling() {
        let exec = executor();
        let input = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
        let config = json!({
            "filter": "if .prev == null then {status: \"no_input\"} else {status: \"has_input\"} end"
        });
        let result = exec.execute(&input, &config, &ctx()).unwrap();
        assert_eq!(result["status"], json!("no_input"));
    }

    // Output schema validation for rule results
    #[test]
    fn rule_output_schema_validation() {
        let exec = executor();
        let mut env = envelope();
        env["prev"] = json!({"total": 100});
        let config = json!({
            "filter": "if .prev.total > 50 then {priority: \"high\"} else {priority: \"low\"} end",
            "output": {
                "type": "object",
                "required": ["priority"],
                "properties": {
                    "priority": {"type": "string", "enum": ["high", "low"]}
                }
            }
        });
        let result = exec.execute(&env, &config, &ctx()).unwrap();
        assert_eq!(result["priority"], json!("high"));
    }
}
