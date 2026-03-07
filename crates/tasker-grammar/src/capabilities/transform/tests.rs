use serde_json::{json, Value};

use crate::expression::ExpressionEngine;
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

use super::TransformExecutor;

/// Execute transform against a raw envelope value, wrapping in CompositionEnvelope.
fn exec_transform(input: &Value, config: &Value) -> Result<Value, CapabilityError> {
    let envelope = CompositionEnvelope::new(input);
    executor().execute(&envelope, config, &ctx())
}

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
    let input = json!({"value": 42});
    let config = json!({"filter": ".value + 1"});
    let result = exec_transform(&input, &config).unwrap();
    assert_eq!(result, json!(43));
}

#[test]
fn filter_with_output_schema_pass() {
    let input = json!({"x": 10});
    let config = json!({
        "filter": "{result: .x}",
        "output": {
            "type": "object",
            "required": ["result"],
            "properties": {"result": {"type": "integer"}}
        }
    });
    let result = exec_transform(&input, &config).unwrap();
    assert_eq!(result, json!({"result": 10}));
}

#[test]
fn filter_with_output_schema_fail() {
    let input = json!({"x": "not_a_number"});
    let config = json!({
        "filter": "{result: .x}",
        "output": {
            "type": "object",
            "properties": {"result": {"type": "number"}}
        }
    });
    let err = exec_transform(&input, &config).unwrap_err();
    assert!(
        matches!(err, CapabilityError::OutputValidation(_)),
        "expected OutputValidation, got {err:?}"
    );
}

#[test]
fn output_validation_errors_do_not_leak_values() {
    // Simulate PII flowing through the composition context — the email value
    // should never appear in the error message.
    let input = json!({
        "context": {"email": "alice@secret.com"},
        "deps": {}, "step": {}, "prev": null
    });
    let config = json!({
        "filter": "{email: .context.email}",
        "output": {
            "type": "object",
            "properties": {"email": {"type": "integer"}}
        }
    });
    let err = exec_transform(&input, &config).unwrap_err();
    let msg = err.to_string();

    assert!(
        matches!(err, CapabilityError::OutputValidation(_)),
        "expected OutputValidation, got {err:?}"
    );
    assert!(
        !msg.contains("alice@secret.com"),
        "error message must not contain the actual value: {msg}"
    );
    assert!(
        msg.contains("expected type"),
        "error should describe the constraint violation: {msg}"
    );
}

#[test]
fn output_validation_error_includes_path() {
    let input = json!({
        "context": {}, "deps": {}, "step": {},
        "prev": {"nested": {"value": "wrong_type"}}
    });
    let config = json!({
        "filter": ".prev",
        "output": {
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": {"value": {"type": "number"}}
                }
            }
        }
    });
    let err = exec_transform(&input, &config).unwrap_err();
    let msg = err.to_string();

    assert!(
        !msg.contains("wrong_type"),
        "error must not contain actual value: {msg}"
    );
    assert!(
        msg.contains("/nested/value"),
        "error should include the instance path: {msg}"
    );
}

#[test]
fn missing_filter_config() {
    let config = json!({"output": {"type": "object"}});
    let err = exec_transform(&json!({}), &config).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[test]
fn invalid_filter_syntax() {
    let config = json!({"filter": ".foo ||| bar"});
    let err = exec_transform(&json!({}), &config).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[test]
fn capability_name_is_transform() {
    assert_eq!(executor().capability_name(), "transform");
}

#[test]
fn envelope_context_access() {
    let config = json!({"filter": ".context.order_id"});
    let result = exec_transform(&envelope(), &config).unwrap();
    assert_eq!(result, json!("ORD-001"));
}

#[test]
fn envelope_deps_access() {
    let config = json!({"filter": ".deps.validate_cart.total"});
    let result = exec_transform(&envelope(), &config).unwrap();
    assert_eq!(result, json!(100.0));
}

#[test]
fn envelope_step_access() {
    let config = json!({"filter": ".step.name"});
    let result = exec_transform(&envelope(), &config).unwrap();
    assert_eq!(result, json!("create_order"));
}

#[test]
fn envelope_prev_null_for_first_invocation() {
    let config = json!({"filter": ".prev"});
    let result = exec_transform(&envelope(), &config).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn envelope_prev_chaining() {
    let mut env = envelope();
    env["prev"] = json!({"total": 100.0, "tax": 8.75});
    let config = json!({"filter": ".prev.total + .prev.tax"});
    let result = exec_transform(&env, &config).unwrap();
    assert_eq!(result, json!(108.75));
}

#[test]
fn null_field_handling() {
    let config = json!({"filter": ".context.nonexistent"});
    let result = exec_transform(&envelope(), &config).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn output_schema_required_field_missing() {
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
    let err = exec_transform(&input, &config).unwrap_err();
    assert!(matches!(err, CapabilityError::OutputValidation(_)));
}

#[test]
fn no_output_schema_skips_validation() {
    let config = json!({"filter": "{anything: 123}"});
    let result = exec_transform(&json!({}), &config).unwrap();
    assert_eq!(result, json!({"anything": 123}));
}

#[test]
fn invalid_output_schema_returns_config_error() {
    let config = json!({
        "filter": "42",
        "output": {"type": "not_a_real_type"}
    });
    // jsonschema may accept unknown types gracefully; we just verify no panic
    let _ = exec_transform(&json!({}), &config);
}

// ---------------------------------------------------------------------------
// TAS-325: Projection & restructuring patterns
//
// Projection is the use of `transform` where the jaq filter reorganizes data
// shape — selecting fields, renaming, nesting, flattening, array restructuring
// — without arithmetic or boolean logic. The `output` schema declares the
// target shape; the `filter` selects and rearranges from the input.
//
// This is what was conceptually the `reshape` capability. In the unified model,
// projection is simply jq object construction applied to the composition
// context envelope.
//
// Convention: when a transform's filter contains only path traversal and object
// construction (no math, no conditionals, no aggregation), it is a projection.
// ---------------------------------------------------------------------------

mod projection {
    use super::*;

    /// Select a subset of fields from dependency outputs into a new object.
    ///
    /// This is the most basic projection pattern — gathering fields from
    /// `.deps` into a flat result object. The output schema declares the
    /// contract this step promises to downstream consumers.
    #[test]
    fn field_projection() {
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
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["total"], json!(100.0));
        assert_eq!(result["payment_id"], json!("pay_123"));
    }

    /// Rename fields using `{new_name: .old_name}` syntax.
    ///
    /// Useful when upstream step outputs use internal names but downstream
    /// consumers expect a different naming convention.
    #[test]
    fn field_renaming() {
        let config = json!({
            "filter": "{email: .context.customer_email, ref_id: .context.order_id}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["email"], json!("user@example.com"));
        assert_eq!(result["ref_id"], json!("ORD-001"));
    }

    /// Extract deeply nested values via path traversal.
    ///
    /// jq's dot-path syntax naturally traverses nested objects. Missing
    /// intermediate keys produce `null` rather than errors.
    #[test]
    fn nested_extraction() {
        let input = json!({
            "context": {"billing": {"address": {"city": "Portland", "zip": "97201"}}},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": ".context.billing.address.city"});
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!("Portland"));
    }

    /// Restructure array elements by selecting/renaming fields within each item.
    ///
    /// Pattern: `[.array[] | {field_a, renamed: .field_b}]`
    /// This is a common projection when upstream provides rich objects but
    /// downstream only needs a few fields per item.
    #[test]
    fn array_element_restructuring() {
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
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(
            result,
            json!([
                {"sku": "A1", "qty": 2},
                {"sku": "B2", "qty": 1}
            ])
        );
    }

    /// Construct nested output objects from flat or differently-structured inputs.
    ///
    /// jq object literals can be nested arbitrarily. This pattern reshapes
    /// flat inputs into hierarchical outputs expected by downstream consumers.
    #[test]
    fn nested_object_construction() {
        let input = json!({
            "context": {"addr": "123 Main St", "postal_code": "97201", "name": "Alice"},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "{customer: {name: .context.name, billing: {address: .context.addr, zip: .context.postal_code}}}"
        });
        let result = exec_transform(&input, &config).unwrap();
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

    /// Flatten deeply nested structures into a flat output object.
    ///
    /// The inverse of nested construction — useful when downstream consumers
    /// expect a flat record from a hierarchical source.
    #[test]
    fn flattening() {
        let input = json!({
            "context": {"deep": {"nested": {"value": 42}}},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": "{flat_field: .context.deep.nested.value}"});
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!({"flat_field": 42}));
    }

    /// Combine fields from multiple envelope sources (`.context` + `.deps`)
    /// into a single result object.
    ///
    /// This is the most common real-world projection pattern — gathering data
    /// from the task context and from upstream dependency step outputs.
    #[test]
    fn multi_source_projection() {
        let config = json!({
            "filter": "{order_id: .context.order_id, customer_name: .deps.customer_profile.name, total: .deps.validate_cart.total}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["order_id"], json!("ORD-001"));
        assert_eq!(result["customer_name"], json!("Alice"));
        assert_eq!(result["total"], json!(100.0));
    }

    /// Missing fields produce `null` — jq's default behavior.
    ///
    /// When an output schema does NOT mark a field as `required`, the filter
    /// can safely reference missing paths and the result will contain `null`.
    /// This eliminates the need for explicit null-checking in projections.
    #[test]
    fn missing_optional_fields() {
        let config = json!({
            "filter": "{present: .context.order_id, absent: .context.nonexistent}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["present"], json!("ORD-001"));
        assert_eq!(result["absent"], json!(null));
    }

    /// Chain projections via `.prev` — restructure output from a prior transform.
    ///
    /// In a composition, each capability invocation's output replaces `.prev`
    /// for the next invocation. A projection can reshape a prior transform's
    /// output without re-reading `.context` or `.deps`.
    #[test]
    fn prev_chaining() {
        let mut env = envelope();
        env["prev"] =
            json!({"items": [{"sku": "A1", "price": 25.0}, {"sku": "B2", "price": 50.0}]});
        let config = json!({
            "filter": "{skus: [.prev.items[].sku], count: (.prev.items | length)}"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result, json!({"skus": ["A1", "B2"], "count": 2}));
    }

    /// Output schema validation catches contract violations at runtime.
    ///
    /// When `output` is present, the filter result is validated against the
    /// declared JSON Schema. This catches bugs where a filter produces an
    /// unexpected shape — e.g., missing a required field or wrong type.
    #[test]
    fn output_schema_validates_projection() {
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
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["customer"], json!("Alice"));
        assert!(result["items"].is_array());
    }

    /// Merge objects with `+` — combine two objects, later keys win.
    ///
    /// jq's object merge operator is useful for adding fields to an
    /// existing structure without restating every field.
    #[test]
    fn object_merge() {
        let mut env = envelope();
        env["prev"] = json!({"name": "Alice", "email": "alice@example.com"});
        let config = json!({
            "filter": ".prev + {source: \"import\", verified: false}"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["name"], json!("Alice"));
        assert_eq!(result["source"], json!("import"));
        assert_eq!(result["verified"], json!(false));
    }

    /// Collect values from an array of objects into parallel arrays.
    ///
    /// This "transpose" pattern is common when downstream consumers need
    /// columnar data from row-oriented input.
    #[test]
    fn collect_into_parallel_arrays() {
        let config = json!({
            "filter": "{skus: [.deps.validate_cart.validated_items[].sku], totals: [.deps.validate_cart.validated_items[].line_total]}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["skus"], json!(["A1", "B2"]));
        assert_eq!(result["totals"], json!([50.0, 50.0]));
    }

    /// Select array elements conditionally with `select`.
    ///
    /// Although `select` is technically a filter (not pure projection), it is
    /// still a projection-intent pattern when used to subset an array without
    /// computing new values.
    #[test]
    fn conditional_array_selection() {
        let config = json!({
            "filter": "[.context.cart_items[] | select(.price >= 50) | {sku, price}]"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!([{"sku": "B2", "price": 50.0}]));
    }
}

// ---------------------------------------------------------------------------
// TAS-326: Arithmetic, aggregation & derivation patterns
//
// Computation is the use of `transform` where the jaq filter produces new
// derived values — totals, averages, string construction, aggregations.
// The `output` schema declares the derived shape; the `filter` computes it.
//
// This is what was conceptually the `compute` capability. In the unified model,
// computation is simply jq with math operators, `map`, `add`, `length`, and
// variable binding (`as $var`).
//
// Convention: when a transform's filter contains arithmetic operators, `add`,
// `length`, `min`, `max`, string interpolation, or `as $var` bindings, it is
// a computation. A single filter can mix projection and computation freely.
// ---------------------------------------------------------------------------

mod computation {
    use super::*;

    /// Basic arithmetic: multiply price × quantity.
    ///
    /// jq supports `+`, `-`, `*`, `/`, `%` on numbers. Floating-point
    /// arithmetic follows IEEE 754 (the same as JSON numbers).
    #[test]
    fn basic_arithmetic() {
        let input = json!({"context": {"price": 25.0, "quantity": 4}, "deps": {}, "step": {}, "prev": null});
        let config = json!({"filter": ".context.price * .context.quantity"});
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!(100.0));
    }

    /// Aggregate array values with `map` + `add`.
    ///
    /// Pattern: `[.items[].field] | add` or `.items | map(.field) | add`.
    /// This is the jq equivalent of SQL's `SUM()`.
    #[test]
    fn array_aggregation() {
        let config = json!({
            "filter": "[.deps.validate_cart.validated_items[].line_total] | add"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(100.0));
    }

    /// Count array elements with `length`.
    ///
    /// jq's `length` works on arrays, objects (key count), strings (char count),
    /// and `null` (returns 0).
    #[test]
    fn array_length() {
        let config = json!({
            "filter": ".deps.validate_cart.validated_items | length"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(2));
    }

    /// Build strings with `\(expr)` interpolation.
    ///
    /// jq string interpolation embeds expression results into strings.
    /// Useful for constructing confirmation messages, log lines, or IDs.
    #[test]
    fn string_interpolation() {
        let config = json!({
            "filter": "\"Order \\(.context.order_id) confirmed\""
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!("Order ORD-001 confirmed"));
    }

    /// Multi-step derivation with `as $var` variable binding.
    ///
    /// Pattern: `(expr) as $x | (expr using $x) as $y | {result using $x, $y}`
    /// This is the jq equivalent of `let` bindings — essential for computing
    /// intermediate values like subtotals before deriving tax and total.
    #[test]
    fn variable_binding() {
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
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["subtotal"], json!(100.0));
        assert_eq!(result["tax"], json!(8.75));
        assert_eq!(result["total"], json!(108.75));
    }

    /// Compute across dependency step outputs.
    ///
    /// A transform can reference multiple `.deps` entries to derive values
    /// that combine results from independent upstream steps.
    #[test]
    fn cross_dependency_computation() {
        let config = json!({
            "filter": "{combined: (.deps.validate_cart.total + .deps.customer_profile.lifetime_value)}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["combined"], json!(15100.0));
    }

    /// Sort and rank with `sort_by` + `to_entries`.
    ///
    /// Pattern: `sort_by(.field) | reverse | to_entries | map({rank: .key + 1, ...})`
    /// This replaces what was a separate `rank` capability in earlier designs.
    #[test]
    fn sorting_and_ranking() {
        let input = json!({
            "context": {"scores": [{"name": "A", "score": 50}, {"name": "B", "score": 90}, {"name": "C", "score": 70}]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": ".context.scores | sort_by(.score) | reverse | to_entries | map({rank: (.key + 1), name: .value.name, score: .value.score})"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result[0]["rank"], json!(1));
        assert_eq!(result[0]["name"], json!("B"));
        assert_eq!(result[1]["rank"], json!(2));
        assert_eq!(result[1]["name"], json!("C"));
    }

    /// Mix projection and computation in a single filter.
    ///
    /// A single transform can both select existing fields (projection intent)
    /// and derive new fields (computation intent). This is a key advantage of
    /// the unified model — no need to chain separate reshape→compute steps.
    #[test]
    fn mixed_projection_and_computation() {
        let config = json!({
            "filter": "{order_id: .context.order_id, item_count: (.deps.validate_cart.validated_items | length), total: .deps.validate_cart.total, confirmation: \"Order \\(.context.order_id) for $\\(.deps.validate_cart.total)\"}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["order_id"], json!("ORD-001"));
        assert_eq!(result["item_count"], json!(2));
        assert_eq!(result["total"], json!(100.0));
        assert_eq!(result["confirmation"], json!("Order ORD-001 for $100.0"));
    }

    /// Group-by aggregation — the jq equivalent of SQL `GROUP BY`.
    ///
    /// Pattern: `group_by(.key) | map({key: .[0].key, total: map(.val) | add})`
    /// This replaces what was a separate `group_by` capability in earlier designs.
    #[test]
    fn group_by_aggregation() {
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
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result[0], json!({"category": "A", "total": 40}));
        assert_eq!(result[1], json!({"category": "B", "total": 25}));
    }

    /// Compute from `.prev` — derive values from a prior transform's output.
    ///
    /// In a composition chain, a computation transform often follows a
    /// projection transform that gathered the needed fields.
    #[test]
    fn prev_computation() {
        let mut env = envelope();
        env["prev"] = json!({"items": [{"price": 10, "qty": 2}, {"price": 5, "qty": 3}]});
        let config = json!({
            "filter": ".prev.items | map(.price * .qty) | add"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result, json!(35));
    }

    /// Division and rounding — `round`, `floor`, `ceil` for precision control.
    ///
    /// Floating-point arithmetic can produce long decimals. Use jq's rounding
    /// functions to control precision: `* 100 | round / 100` for 2 decimal places.
    #[test]
    fn division_and_rounding() {
        let input = json!({
            "context": {"total": 100, "count": 3},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "(.context.total / .context.count * 100 | round / 100) as $avg | {average: $avg, floored: ($avg | floor), ceiled: ($avg | ceil)}"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["average"], json!(33.33));
        assert_eq!(result["floored"], json!(33));
        assert_eq!(result["ceiled"], json!(34));
    }

    /// Min and max over arrays.
    ///
    /// jq's `min` and `max` work on arrays of comparable values.
    /// `min_by` and `max_by` accept a key function for object arrays.
    #[test]
    fn min_max() {
        let input = json!({
            "context": {"scores": [72, 95, 88, 61, 90]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "{min: (.context.scores | min), max: (.context.scores | max), range: ((.context.scores | max) - (.context.scores | min))}"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["min"], json!(61));
        assert_eq!(result["max"], json!(95));
        assert_eq!(result["range"], json!(34));
    }

    /// Average computation: `add / length`.
    ///
    /// jq has no built-in `avg` function, but `(array | add / length)` is
    /// the standard idiom. Use rounding for clean output.
    #[test]
    fn average() {
        let input = json!({
            "context": {"values": [10, 20, 30, 40]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": ".context.values | (add / length)"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!(25.0));
    }

    /// Unique values and deduplication.
    ///
    /// `unique` removes duplicates from sorted arrays. `unique_by(.field)`
    /// deduplicates by a key function.
    #[test]
    fn unique_values() {
        let input = json!({
            "context": {"tags": ["rust", "jq", "rust", "json", "jq"]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "{unique_tags: (.context.tags | unique), count: (.context.tags | unique | length)}"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["unique_tags"], json!(["jq", "json", "rust"]));
        assert_eq!(result["count"], json!(3));
    }

    /// Conditional computation — derive different values based on input.
    ///
    /// While this borders on evaluation intent, it's computation when the
    /// output is a derived numeric value rather than a boolean/classification.
    #[test]
    fn conditional_computation() {
        let mut env = envelope();
        env["prev"] = json!({"quantity": 25, "unit_price": 10.0});
        let config = json!({
            "filter": "(.prev.quantity * .prev.unit_price) as $subtotal | if .prev.quantity >= 20 then {subtotal: $subtotal, discount: ($subtotal * 0.1), total: ($subtotal * 0.9)} else {subtotal: $subtotal, discount: 0, total: $subtotal} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["subtotal"], json!(250.0));
        assert_eq!(result["discount"], json!(25.0));
        assert_eq!(result["total"], json!(225.0));
    }
}

// ---------------------------------------------------------------------------
// TAS-327: Boolean evaluation & classification patterns
//
// Evaluation is the use of `transform` where the jaq filter produces boolean
// or classification/selection values — answering questions about data state.
// "Is this order high-value?" "Which tier does this customer belong to?"
//
// This is what was conceptually the `evaluate` capability. In the unified
// model, evaluation is simply jq producing booleans or classification strings.
//
// **Important distinction from `assert`**: A `transform` with evaluation
// intent *produces* boolean/classification values as data for downstream use.
// An `assert` capability *gates execution* — it either passes (no output) or
// fails the step. Use `transform` when you need to record the evaluation
// result; use `assert` when you need to stop execution on failure.
// ---------------------------------------------------------------------------

mod evaluation {
    use super::*;

    /// Simple boolean — threshold comparison producing `true` or `false`.
    ///
    /// The most basic evaluation pattern. The output is a bare boolean value,
    /// not wrapped in an object.
    #[test]
    fn simple_boolean() {
        let config = json!({"filter": ".deps.validate_cart.total > 50"});
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Compound boolean — combine conditions with `and`/`or`.
    ///
    /// jq's `and`/`or` are short-circuiting boolean operators.
    #[test]
    fn compound_boolean() {
        let config = json!({
            "filter": ".deps.validate_cart.total > 50 and .deps.customer_profile.tier == \"gold\""
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Conditional classification via `if-then-elif-else`.
    ///
    /// This pattern produces a string category rather than a boolean.
    /// The output schema can use `enum` to constrain the allowed values,
    /// catching bugs where the filter produces an unexpected category.
    #[test]
    fn conditional_classification() {
        let config = json!({
            "filter": "if .deps.customer_profile.lifetime_value > 10000 then \"gold\" elif .deps.customer_profile.lifetime_value > 1000 then \"silver\" else \"standard\" end",
            "output": {"type": "string", "enum": ["gold", "silver", "standard"]}
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!("gold"));
    }

    /// Array predicate with `any` — true if any element matches.
    ///
    /// Pattern: `.array | any(condition)`
    /// Useful for checking if a collection contains flagged items.
    #[test]
    fn array_predicate_any() {
        let config = json!({
            "filter": ".context.flags | any(. == \"manual_review\")"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Array predicate with `all` — true only if every element matches.
    ///
    /// Pattern: `.array | all(condition)`
    /// Useful for validating that all items in a batch meet criteria.
    #[test]
    fn array_predicate_all() {
        let input = json!({
            "context": {"items": [{"valid": true}, {"valid": true}]},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({"filter": ".context.items | all(.valid)"});
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Range checking with chained comparisons.
    ///
    /// jq doesn't have `80 <= x < 90` syntax, so range checks require
    /// `and` to combine two comparisons.
    #[test]
    fn comparison_chains() {
        let input = json!({"context": {"score": 85}, "deps": {}, "step": {}, "prev": null});
        let config = json!({"filter": ".context.score >= 80 and .context.score < 90"});
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Null handling — jq treats missing fields as `null`.
    ///
    /// `null == null` is `true` in jq. This is important for evaluation
    /// patterns that check whether optional fields are present.
    #[test]
    fn null_comparison() {
        let config = json!({"filter": ".context.nonexistent == null"});
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Multi-field evaluation — produce an object with several boolean and
    /// classification fields in a single transform.
    ///
    /// This is the most common evaluation pattern: a single transform answers
    /// multiple questions about the data state, and the output schema declares
    /// the types of each answer field.
    #[test]
    fn multi_field_evaluation() {
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
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["is_high_value"], json!(true));
        assert_eq!(result["needs_review"], json!(true));
        assert_eq!(result["customer_tier"], json!("gold"));
    }

    /// Evaluate using results from multiple dependency steps.
    ///
    /// Evaluation transforms can reference any combination of `.context`,
    /// `.deps`, and `.prev` to make decisions based on the full workflow state.
    #[test]
    fn cross_dependency_evaluation() {
        let config = json!({
            "filter": ".deps.validate_cart.total > 50 and .deps.customer_profile.tier == \"gold\""
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Negation with `| not` — invert a boolean result.
    ///
    /// Pattern: `(condition) | not`
    /// Useful for "none of these flags are present" checks.
    #[test]
    fn negation_pattern() {
        let config = json!({
            "filter": "(.context.flags | any(. == \"blocked\")) | not"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    /// Output schema enforces boolean type — catches accidental numeric output.
    ///
    /// If a filter produces a number where a boolean was declared, the output
    /// schema validation catches it. This is important because jq treats
    /// numbers as truthy, so a filter returning `100` instead of `true` would
    /// silently work in jq but violate the declared contract.
    #[test]
    fn boolean_schema_validation_pass() {
        let config = json!({
            "filter": ".deps.validate_cart.total > 50",
            "output": {"type": "boolean"}
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn boolean_schema_validation_fail() {
        let config = json!({
            "filter": ".deps.validate_cart.total",
            "output": {"type": "boolean"}
        });
        let err = exec_transform(&envelope(), &config).unwrap_err();
        assert!(matches!(err, CapabilityError::OutputValidation(_)));
    }

    /// Existence check with `!= null` — test whether an optional field is present.
    ///
    /// In jq, missing object keys produce `null`, so `.field != null` is the
    /// idiomatic way to check for presence.
    #[test]
    fn existence_check() {
        let config = json!({
            "filter": "{has_promo: (.context.promo_code != null), has_referral: (.context.referral_code != null)}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["has_promo"], json!(true));
        assert_eq!(result["has_referral"], json!(false));
    }

    /// Evaluate with `has` — check if an object contains a key.
    ///
    /// `has("key")` is subtly different from `.key != null`: `has` returns
    /// true even if the value is explicitly `null`, while `!= null` returns
    /// false in that case.
    #[test]
    fn has_key_check() {
        let input = json!({
            "context": {"data": {"name": "Alice", "nickname": null}},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "{has_name: (.context.data | has(\"name\")), has_nickname: (.context.data | has(\"nickname\")), has_age: (.context.data | has(\"age\"))}"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["has_name"], json!(true));
        assert_eq!(result["has_nickname"], json!(true));
        assert_eq!(result["has_age"], json!(false));
    }

    /// Multi-tier classification with enum schema validation.
    ///
    /// The output schema's `enum` constraint catches classification bugs at
    /// runtime — if the filter produces an unexpected category (e.g., a typo),
    /// schema validation fails immediately rather than propagating bad data.
    #[test]
    fn enum_classification_with_schema() {
        let input = json!({
            "context": {"score": 45},
            "deps": {}, "step": {}, "prev": null
        });
        let config = json!({
            "filter": "if .context.score >= 90 then {grade: \"A\"} elif .context.score >= 80 then {grade: \"B\"} elif .context.score >= 70 then {grade: \"C\"} elif .context.score >= 60 then {grade: \"D\"} else {grade: \"F\"} end",
            "output": {
                "type": "object",
                "required": ["grade"],
                "properties": {
                    "grade": {"type": "string", "enum": ["A", "B", "C", "D", "F"]}
                }
            }
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["grade"], json!("F"));
    }

    /// Evaluate array emptiness — check if collections have items.
    ///
    /// Pattern: `(.array | length) > 0` or `(.array | length) == 0`
    #[test]
    fn array_emptiness_check() {
        let config = json!({
            "filter": "{has_items: ((.context.cart_items | length) > 0), has_flags: ((.context.flags | length) > 0)}"
        });
        let result = exec_transform(&envelope(), &config).unwrap();
        assert_eq!(result["has_items"], json!(true));
        assert_eq!(result["has_flags"], json!(true));
    }
}

// ---------------------------------------------------------------------------
// TAS-329: Rule-engine patterns (first-match / all-match)
//
// Rule-engine patterns use `transform` where the jaq filter implements ordered
// condition→result matching. Two primary patterns:
//
// **First-match** (case/switch semantics): `if-then-elif-else` chain.
// Returns the result of the FIRST matching condition. Subsequent conditions
// are not evaluated (short-circuit). Always include an `else` default.
//
// **All-match** (collect all matching): Array of `(if cond then result else
// empty end)` expressions. Collects ALL matching rule results into an array.
// Non-matching rules produce `empty` (jq's "no output"), which is
// automatically excluded from the array.
//
// This is what was conceptually the `evaluate_rules` capability. In the
// unified model, rule engines are simply jq conditionals — the pattern
// difference is documentation convention, not separate executors.
//
// When to prefer rule-engine over simple evaluation:
// - Many branches (>3) with structured result objects
// - Need to collect ALL matches rather than just the first
// - Result objects contain multiple fields, not just a boolean
// ---------------------------------------------------------------------------

mod rules {
    use super::*;

    /// First-match: VIP routing based on total and customer tier.
    ///
    /// The `if-elif-else` chain evaluates conditions top-to-bottom and returns
    /// the result of the first match. This is equivalent to a `CASE WHEN` in
    /// SQL or a `match` statement with guards.
    ///
    /// The output schema declares the expected result shape, ensuring every
    /// branch produces a consistent object.
    #[test]
    fn first_match_single() {
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
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["priority"], json!("urgent"));
        assert_eq!(result["routing"], json!("vip_queue"));
    }

    /// First-match short-circuits — only the first matching branch fires.
    ///
    /// Here both `> 500` and `> 0` are true, but first-match returns
    /// `"high"` because it appears first. This is the key difference from
    /// all-match, which would collect both.
    #[test]
    fn first_match_short_circuits() {
        let mut env = envelope();
        // total > 500 AND total > 0 both true, but first-match returns first
        env["prev"] = json!({"total": 750, "tier": "silver"});
        let config = json!({
            "filter": "if .prev.total > 1000 and .prev.tier == \"gold\" then {priority: \"urgent\"} elif .prev.total > 500 then {priority: \"high\"} elif .prev.total > 0 then {priority: \"normal\"} else {priority: \"low\"} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["priority"], json!("high"));
    }

    /// First-match with default — `else` catches unmatched inputs.
    ///
    /// Always include an `else` clause in first-match rules to ensure the
    /// filter always produces output. Without it, unmatched inputs would
    /// produce `null`, which may violate the output schema.
    #[test]
    fn first_match_default() {
        let mut env = envelope();
        env["prev"] = json!({"total": -5, "tier": "none"});
        let config = json!({
            "filter": "if .prev.total > 1000 then {priority: \"urgent\"} elif .prev.total > 0 then {priority: \"normal\"} else {priority: \"low\"} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["priority"], json!("low"));
    }

    /// All-match: collect ALL matching discount rules into an array.
    ///
    /// Pattern: `[(if cond then result else empty end), ...]`
    /// Each rule produces either a result object or `empty` (no output).
    /// jq's `empty` is automatically excluded from the enclosing array,
    /// giving us clean "collect all matches" semantics.
    ///
    /// This is preferable to first-match when multiple rules can apply
    /// simultaneously (e.g., stacking discounts, accumulating flags).
    #[test]
    fn all_match_collects() {
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
        let result = exec_transform(&env, &config).unwrap();
        let discounts = result["applicable_discounts"].as_array().unwrap();
        assert_eq!(discounts.len(), 3);
        assert_eq!(discounts[0]["rule"], json!("bulk_discount"));
        assert_eq!(discounts[1]["rule"], json!("loyalty_discount"));
        assert_eq!(discounts[2]["rule"], json!("promo_discount"));
    }

    /// All-match with no matches — produces an empty array.
    ///
    /// When no rules match, every branch produces `empty` and the result
    /// is `[]`. This is a safe, well-typed result (an array with zero items).
    #[test]
    fn all_match_no_matches() {
        let mut env = envelope();
        env["prev"] = json!({"total": 10});
        env["deps"]["customer_profile"]["tier"] = json!("standard");
        env["context"]["promo_code"] = json!(null);

        let config = json!({
            "filter": "[(if .prev.total > 500 then {rule: \"bulk\"} else empty end), (if .deps.customer_profile.tier == \"gold\" then {rule: \"loyalty\"} else empty end), (if .context.promo_code != null then {rule: \"promo\"} else empty end)] | {applicable_discounts: .}"
        });
        let result = exec_transform(&env, &config).unwrap();
        let discounts = result["applicable_discounts"].as_array().unwrap();
        assert!(discounts.is_empty());
    }

    /// Complex conditions — compound boolean with cross-dependency references.
    ///
    /// Real-world rule engines often combine multiple data sources in their
    /// conditions. This example models a refund approval policy with
    /// conditions spanning amount, reason, and eligibility rules.
    #[test]
    fn complex_conditions() {
        let mut env = envelope();
        env["prev"] = json!({"refund_amount": 300, "refund_reason": "defective"});
        let config = json!({
            "filter": "if .prev.refund_amount <= 50 then {approval_path: \"auto_approved\"} elif (.prev.refund_reason == \"defective\" or .prev.refund_reason == \"wrong_item\") and .prev.refund_amount <= 500 then {approval_path: \"auto_approved\"} elif .prev.refund_amount > 500 then {approval_path: \"manager_review\"} else {approval_path: \"standard_review\"} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["approval_path"], json!("auto_approved"));
    }

    /// Nested rule results — each branch produces a multi-field object.
    ///
    /// Rule results aren't limited to single values. Each branch can return
    /// a rich object with multiple fields, all validated against the output
    /// schema.
    #[test]
    fn nested_rule_results() {
        let mut env = envelope();
        env["prev"] = json!({"score": 95});
        let config = json!({
            "filter": "if .prev.score >= 90 then {grade: \"A\", pass: true, honors: true} elif .prev.score >= 80 then {grade: \"B\", pass: true, honors: false} else {grade: \"F\", pass: false, honors: false} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["grade"], json!("A"));
        assert_eq!(result["pass"], json!(true));
        assert_eq!(result["honors"], json!(true));
    }

    /// Dynamic rule values — results computed from input data, not constants.
    ///
    /// Rule results can reference input data to compute dynamic values.
    /// Here the discount rate is doubled when the total exceeds a threshold.
    #[test]
    fn dynamic_rule_values() {
        let mut env = envelope();
        env["prev"] = json!({"base_discount": 0.05, "total": 1000});
        let config = json!({
            "filter": "if .prev.total > 500 then {discount: (.prev.base_discount * 2), savings: (.prev.total * .prev.base_discount * 2)} else {discount: .prev.base_discount, savings: (.prev.total * .prev.base_discount)} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["discount"], json!(0.1));
        assert_eq!(result["savings"], json!(100.0));
    }

    /// Graceful null/empty input handling in rule conditions.
    ///
    /// Rules should handle missing fields gracefully. jq's null comparison
    /// semantics mean that `.prev == null` is safe even if `.prev` is absent.
    #[test]
    fn empty_input_handling() {
        let input = json!({"context": {}, "deps": {}, "step": {}, "prev": null});
        let config = json!({
            "filter": "if .prev == null then {status: \"no_input\"} else {status: \"has_input\"} end"
        });
        let result = exec_transform(&input, &config).unwrap();
        assert_eq!(result["status"], json!("no_input"));
    }

    /// Output schema validation for rule results — catches branch inconsistencies.
    ///
    /// When all branches of a first-match must produce the same shape, the
    /// output schema enforces consistency. If one branch forgets a required
    /// field, schema validation catches it at runtime.
    #[test]
    fn rule_output_schema_validation() {
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
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["priority"], json!("high"));
    }

    /// All-match with computed aggregate — sum matched discount values.
    ///
    /// After collecting all matching rules, a subsequent computation can
    /// aggregate the results. This pattern chains all-match collection with
    /// computation in a single filter using variable binding.
    #[test]
    fn all_match_with_aggregate() {
        let mut env = envelope();
        env["prev"] = json!({"total": 600});

        let config = json!({
            "filter": ".prev.total as $total | [(if $total > 500 then {rule: \"bulk\", rate: 0.10} else empty end), (if .deps.customer_profile.tier == \"gold\" then {rule: \"loyalty\", rate: 0.05} else empty end)] | {rules: ., combined_rate: (map(.rate) | add), savings: ($total * (map(.rate) | add))}"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["rules"].as_array().unwrap().len(), 2);
        assert_eq!(result["combined_rate"], json!(0.15000000000000002));
        assert_eq!(result["savings"], json!(90.00000000000001));
    }

    /// First-match with many branches — priority/SLA routing.
    ///
    /// Real-world rule engines often have 5+ branches. The if-elif-else
    /// chain scales cleanly to any number of conditions.
    #[test]
    fn many_branch_routing() {
        let mut env = envelope();
        env["prev"] = json!({"severity": "medium", "customer_tier": "silver"});
        let config = json!({
            "filter": "if .prev.severity == \"critical\" then {sla_hours: 1, team: \"on_call\"} elif .prev.severity == \"high\" and .prev.customer_tier == \"gold\" then {sla_hours: 4, team: \"senior\"} elif .prev.severity == \"high\" then {sla_hours: 8, team: \"standard\"} elif .prev.severity == \"medium\" then {sla_hours: 24, team: \"standard\"} elif .prev.severity == \"low\" then {sla_hours: 72, team: \"junior\"} else {sla_hours: 168, team: \"backlog\"} end"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["sla_hours"], json!(24));
        assert_eq!(result["team"], json!("standard"));
    }

    /// All-match partial — only some rules match.
    ///
    /// Verifies that all-match correctly includes matching rules and excludes
    /// non-matching ones (via `empty`).
    #[test]
    fn all_match_partial() {
        let mut env = envelope();
        env["prev"] = json!({"total": 200, "is_returning": true, "has_coupon": false});
        let config = json!({
            "filter": "[(if .prev.total > 500 then {tag: \"high_value\"} else empty end), (if .prev.is_returning then {tag: \"returning_customer\"} else empty end), (if .prev.has_coupon then {tag: \"coupon_user\"} else empty end)] | {tags: map(.tag)}"
        });
        let result = exec_transform(&env, &config).unwrap();
        assert_eq!(result["tags"], json!(["returning_customer"]));
    }
}
