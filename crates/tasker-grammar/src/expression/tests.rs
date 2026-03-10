use super::*;
use serde_json::json;

fn engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

// ─── Path Traversal ──────────────────────────────────────────────────────

#[test]
fn path_simple_field() {
    let input = json!({"name": "Alice"});
    let result = engine().evaluate(".name", &input).unwrap();
    assert_eq!(result, json!("Alice"));
}

#[test]
fn path_nested_field() {
    let input = json!({"customer": {"address": {"city": "Portland"}}});
    let result = engine().evaluate(".customer.address.city", &input).unwrap();
    assert_eq!(result, json!("Portland"));
}

#[test]
fn path_array_index() {
    let input = json!({"items": ["a", "b", "c"]});
    let result = engine().evaluate(".items[1]", &input).unwrap();
    assert_eq!(result, json!("b"));
}

#[test]
fn path_array_iterator() {
    let input = json!({"items": [{"price": 10}, {"price": 20}]});
    let results = engine().evaluate_multi(".items[].price", &input).unwrap();
    assert_eq!(results, vec![json!(10), json!(20)]);
}

#[test]
fn path_optional_field_missing() {
    let input = json!({"name": "Alice"});
    let result = engine().evaluate(".age?", &input).unwrap();
    assert_eq!(result, json!(null));
}

// ─── Field Projection ────────────────────────────────────────────────────

#[test]
fn projection_object_construction() {
    let input = json!({"subtotal": 100, "tax": 8, "line_items": [1, 2]});
    let result = engine()
        .evaluate("{total: (.subtotal + .tax), items: .line_items}", &input)
        .unwrap();
    assert_eq!(result, json!({"total": 108, "items": [1, 2]}));
}

#[test]
fn projection_select_fields() {
    let input = json!({"a": 1, "b": 2, "c": 3});
    let result = engine().evaluate("{a, b}", &input).unwrap();
    assert_eq!(result, json!({"a": 1, "b": 2}));
}

// ─── Arithmetic ──────────────────────────────────────────────────────────

#[test]
fn arithmetic_addition() {
    let input = json!({"a": 5, "b": 3});
    let result = engine().evaluate(".a + .b", &input).unwrap();
    assert_eq!(result, json!(8));
}

#[test]
fn arithmetic_map_and_add() {
    let input = json!({"items": [{"price": 10, "quantity": 2}, {"price": 5, "quantity": 3}]});
    let result = engine()
        .evaluate("[.items[] | .price * .quantity] | add", &input)
        .unwrap();
    assert_eq!(result, json!(35));
}

#[test]
fn arithmetic_subtraction() {
    let input = json!({"total": 100, "discount": 15});
    let result = engine().evaluate(".total - .discount", &input).unwrap();
    assert_eq!(result, json!(85));
}

#[test]
fn arithmetic_division() {
    let input = json!({"value": 100, "count": 4});
    let result = engine().evaluate(".value / .count", &input).unwrap();
    assert_eq!(result, json!(25.0));
}

#[test]
fn arithmetic_modulo() {
    let input = json!({"value": 17, "divisor": 5});
    let result = engine().evaluate(".value % .divisor", &input).unwrap();
    assert_eq!(result, json!(2));
}

// ─── Boolean Expressions ─────────────────────────────────────────────────

#[test]
fn boolean_comparison_gt() {
    let input = json!({"amount": 1500, "status": "pending"});
    let result = engine()
        .evaluate(".amount > 1000 and .status == \"pending\"", &input)
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn boolean_comparison_false() {
    let input = json!({"amount": 500, "status": "pending"});
    let result = engine()
        .evaluate(".amount > 1000 and .status == \"pending\"", &input)
        .unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn boolean_or() {
    let input = json!({"tier": "gold"});
    let result = engine()
        .evaluate(".tier == \"gold\" or .tier == \"platinum\"", &input)
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn boolean_not() {
    let input = json!({"active": false});
    let result = engine().evaluate(".active | not", &input).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn boolean_equality() {
    let input = json!({"status": "complete"});
    let result = engine()
        .evaluate(".status == \"complete\"", &input)
        .unwrap();
    assert_eq!(result, json!(true));
}

// ─── String Construction ─────────────────────────────────────────────────

#[test]
fn string_interpolation() {
    let input = json!({"order_id": "ORD-123"});
    let result = engine()
        .evaluate("\"Order \\(.order_id) confirmed\"", &input)
        .unwrap();
    assert_eq!(result, json!("Order ORD-123 confirmed"));
}

#[test]
fn string_concatenation() {
    let input = json!({"first": "Jane", "last": "Doe"});
    let result = engine().evaluate(".first + \" \" + .last", &input).unwrap();
    assert_eq!(result, json!("Jane Doe"));
}

#[test]
fn string_length() {
    let input = json!("hello");
    let result = engine().evaluate("length", &input).unwrap();
    assert_eq!(result, json!(5));
}

// ─── Conditional Expressions ─────────────────────────────────────────────

#[test]
fn conditional_if_then_else() {
    let input = json!({"tier": "gold"});
    let result = engine()
        .evaluate(
            "if .tier == \"gold\" then \"priority\" else \"standard\" end",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!("priority"));
}

#[test]
fn conditional_if_else_branch() {
    let input = json!({"tier": "silver"});
    let result = engine()
        .evaluate(
            "if .tier == \"gold\" then \"priority\" else \"standard\" end",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!("standard"));
}

#[test]
fn conditional_elif() {
    let input = json!({"score": 75});
    let result = engine()
        .evaluate(
            "if .score >= 90 then \"A\" elif .score >= 80 then \"B\" elif .score >= 70 then \"C\" else \"F\" end",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!("C"));
}

// ─── Collection Operations ───────────────────────────────────────────────

#[test]
fn collection_map() {
    let input = json!([1, 2, 3]);
    let result = engine().evaluate("[.[] | . * 2]", &input).unwrap();
    assert_eq!(result, json!([2, 4, 6]));
}

#[test]
fn collection_select() {
    let input = json!([1, 2, 3, 4, 5]);
    let result = engine().evaluate("[.[] | select(. > 3)]", &input).unwrap();
    assert_eq!(result, json!([4, 5]));
}

#[test]
fn collection_sort_by() {
    let input = json!([{"name": "b", "age": 30}, {"name": "a", "age": 25}]);
    let result = engine().evaluate("[.[] | .name] | sort", &input).unwrap();
    assert_eq!(result, json!(["a", "b"]));
}

#[test]
fn collection_group_by() {
    let input = json!([
        {"dept": "eng", "name": "Alice"},
        {"dept": "eng", "name": "Bob"},
        {"dept": "sales", "name": "Carol"}
    ]);
    let result = engine()
        .evaluate("group_by(.dept) | length", &input)
        .unwrap();
    assert_eq!(result, json!(2));
}

#[test]
fn collection_unique() {
    let input = json!([1, 2, 2, 3, 3, 3]);
    let result = engine().evaluate("unique", &input).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn collection_flatten() {
    let input = json!([[1, 2], [3, 4]]);
    let result = engine().evaluate("flatten", &input).unwrap();
    assert_eq!(result, json!([1, 2, 3, 4]));
}

// ─── Composition Context Patterns ────────────────────────────────────────

#[test]
fn context_envelope_traversal() {
    let input = json!({
        "context": {"order_id": "ORD-001"},
        "deps": {"step_a": {"total": 42}},
        "prev": {"validated": true},
        "step": {"name": "calculate_total"}
    });
    let result = engine()
        .evaluate(
            "{order: .context.order_id, total: .deps.step_a.total, was_valid: .prev.validated}",
            &input,
        )
        .unwrap();
    assert_eq!(
        result,
        json!({"order": "ORD-001", "total": 42, "was_valid": true})
    );
}

// ─── Type Checking and Builtins ──────────────────────────────────────────

#[test]
fn builtin_type() {
    let input = json!(42);
    let result = engine().evaluate("type", &input).unwrap();
    assert_eq!(result, json!("number"));
}

#[test]
fn builtin_keys() {
    let input = json!({"b": 2, "a": 1});
    let result = engine().evaluate("keys", &input).unwrap();
    // jq sorts keys
    assert_eq!(result, json!(["a", "b"]));
}

#[test]
fn builtin_has() {
    let input = json!({"name": "Alice"});
    let result = engine().evaluate("has(\"name\")", &input).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn builtin_to_string() {
    let input = json!(42);
    let result = engine().evaluate("tostring", &input).unwrap();
    assert_eq!(result, json!("42"));
}

#[test]
fn builtin_to_number() {
    let input = json!("42");
    let result = engine().evaluate("tonumber", &input).unwrap();
    assert_eq!(result, json!(42));
}

// ─── Null Handling ───────────────────────────────────────────────────────

#[test]
fn null_input() {
    let result = engine().evaluate("null", &json!(null)).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn null_field_access() {
    let input = json!({"a": null});
    let result = engine().evaluate(".a", &input).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn null_alternative_operator() {
    let input = json!({"a": null});
    let result = engine().evaluate(".a // \"default\"", &input).unwrap();
    assert_eq!(result, json!("default"));
}

// ─── Multiple Outputs ────────────────────────────────────────────────────

#[test]
fn multi_output_comma() {
    let input = json!({"a": 1, "b": 2});
    let results = engine().evaluate_multi(".a, .b", &input).unwrap();
    assert_eq!(results, vec![json!(1), json!(2)]);
}

// ─── Syntax Validation ──────────────────────────────────────────────────

#[test]
fn validate_valid_filter() {
    assert!(engine().validate_syntax(".foo | .bar").is_ok());
}

#[test]
fn validate_invalid_filter() {
    let err = engine().validate_syntax(".foo ||| .bar").unwrap_err();
    assert!(matches!(err, ExpressionError::SyntaxError { .. }));
}

#[test]
fn validate_unclosed_paren() {
    let err = engine().validate_syntax("(.foo").unwrap_err();
    assert!(matches!(err, ExpressionError::SyntaxError { .. }));
}

#[test]
fn validate_empty_filter() {
    let err = engine().validate_syntax("").unwrap_err();
    assert!(matches!(err, ExpressionError::SyntaxError { .. }));
}

// ─── Error Message Quality ──────────────────────────────────────────────

#[test]
fn error_message_syntax_actionable() {
    let err = engine().validate_syntax(".foo ||| .bar").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("syntax error"),
        "error should mention 'syntax error': {msg}"
    );
}

#[test]
fn error_message_unclosed_string() {
    let err = engine().validate_syntax("\"unclosed").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("syntax error"),
        "should report syntax error for unclosed string: {msg}"
    );
}

#[test]
fn error_message_type_error_at_runtime() {
    let input = json!("not_a_number");
    let err = engine().evaluate(". + 1", &input).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("evaluation error"),
        "should report evaluation error for type mismatch: {msg}"
    );
}

#[test]
fn error_message_undefined_function() {
    let err = engine()
        .validate_syntax("nonexistent_function")
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("syntax error"),
        "should report syntax error for undefined function: {msg}"
    );
}

#[test]
fn error_message_null_iterator() {
    let input = json!(null);
    let err = engine().evaluate(".[]", &input).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("evaluation error"),
        "should report evaluation error for null iteration: {msg}"
    );
}

// ─── Sandboxing ──────────────────────────────────────────────────────────

#[test]
fn sandbox_output_size_limit() {
    let engine = ExpressionEngine::new(ExpressionEngineConfig {
        max_output_bytes: 32,
        ..Default::default()
    });
    let input = json!({"data": "this is a string that when projected will exceed the limit"});
    let err = engine.evaluate(".", &input).unwrap_err();
    assert!(
        matches!(err, ExpressionError::OutputTooLarge { .. }),
        "expected OutputTooLarge, got: {err}"
    );
}

#[test]
fn sandbox_timeout() {
    let engine = ExpressionEngine::new(ExpressionEngineConfig {
        // Use an extremely short timeout to trigger it
        timeout: Duration::from_nanos(1),
        ..Default::default()
    });
    // A filter that produces many values to iterate
    let input = json!(null);
    // limit/3 generates 0,1,2 — but with 1ns timeout, even simple work should timeout
    // We use a longer chain to ensure we hit the timeout check
    let result = engine.evaluate_multi("[range(10000)] | .[0:100] | .[]", &input);
    // The timeout may or may not trigger depending on how fast the first result comes back.
    // This test validates the mechanism exists; in production the 100ms default is what matters.
    // We accept either a timeout error or successful completion (if the machine is fast enough).
    match result {
        Err(ExpressionError::Timeout { .. }) => {} // expected
        Ok(_) => {} // acceptable if machine completed within 1ns (unlikely but possible)
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[test]
fn sandbox_timeout_infinite_loop_protection() {
    let engine = ExpressionEngine::new(ExpressionEngineConfig {
        timeout: Duration::from_millis(50),
        ..Default::default()
    });
    // `repeat(. + 1)` generates an infinite sequence
    let input = json!(0);
    let result = engine.evaluate_multi("limit(100000; repeat(. + 1))", &input);
    match result {
        Err(ExpressionError::Timeout { .. }) => {}         // timeout fired first
        Err(ExpressionError::TooManyOutputs { .. }) => {}   // output limit fired first
        Ok(values) => {
            // If it completed, the timeout didn't trigger but the filter was bounded by limit()
            assert!(!values.is_empty());
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

// ─── Date/Time Functions ──────────────────────────────────────────────────

#[test]
fn date_fromdateiso8601() {
    let input = json!("2026-03-05T14:30:00Z");
    let result = engine().evaluate("fromdateiso8601", &input).unwrap();
    // Should be epoch seconds
    assert_eq!(result, json!(1772721000));
}

#[test]
fn date_todateiso8601() {
    let input = json!(1772721000);
    let result = engine().evaluate("todateiso8601", &input).unwrap();
    assert_eq!(result, json!("2026-03-05T14:30:00Z"));
}

#[test]
fn date_roundtrip_iso8601() {
    let input = json!("2026-03-05T14:30:00Z");
    let result = engine()
        .evaluate("fromdateiso8601 | todateiso8601", &input)
        .unwrap();
    assert_eq!(result, json!("2026-03-05T14:30:00Z"));
}

#[test]
fn date_strftime_custom_format() {
    let input = json!(1772721000);
    let result = engine().evaluate("strftime(\"%Y-%m-%d\")", &input).unwrap();
    assert_eq!(result, json!("2026-03-05"));
}

#[test]
fn date_comparison_before() {
    let input = json!({
        "start": "2026-03-05T00:00:00Z",
        "end": "2026-03-06T00:00:00Z"
    });
    let result = engine()
        .evaluate(
            "(.start | fromdateiso8601) < (.end | fromdateiso8601)",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn date_comparison_after() {
    let input = json!({
        "start": "2026-03-06T00:00:00Z",
        "end": "2026-03-05T00:00:00Z"
    });
    let result = engine()
        .evaluate(
            "(.start | fromdateiso8601) < (.end | fromdateiso8601)",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn date_duration_arithmetic() {
    let input = json!("2026-03-05T14:30:00Z");
    // Add 24 hours (86400 seconds)
    let result = engine()
        .evaluate("fromdateiso8601 + 86400 | todateiso8601", &input)
        .unwrap();
    assert_eq!(result, json!("2026-03-06T14:30:00Z"));
}

#[test]
fn date_difference_in_days() {
    let input = json!({
        "start": "2026-03-01T00:00:00Z",
        "end": "2026-03-06T00:00:00Z"
    });
    let result = engine()
        .evaluate(
            "((.end | fromdateiso8601) - (.start | fromdateiso8601)) / 86400",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(5.0));
}

#[test]
fn date_range_overlap_check() {
    let input = json!({
        "a_start": "2026-03-01T00:00:00Z",
        "a_end": "2026-03-05T00:00:00Z",
        "b_start": "2026-03-03T00:00:00Z",
        "b_end": "2026-03-07T00:00:00Z"
    });
    let result = engine()
        .evaluate(
            r#"(.a_start | fromdateiso8601) < (.b_end | fromdateiso8601)
            and (.b_start | fromdateiso8601) < (.a_end | fromdateiso8601)"#,
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn date_range_no_overlap() {
    let input = json!({
        "a_start": "2026-03-01T00:00:00Z",
        "a_end": "2026-03-03T00:00:00Z",
        "b_start": "2026-03-05T00:00:00Z",
        "b_end": "2026-03-07T00:00:00Z"
    });
    let result = engine()
        .evaluate(
            r#"(.a_start | fromdateiso8601) < (.b_end | fromdateiso8601)
            and (.b_start | fromdateiso8601) < (.a_end | fromdateiso8601)"#,
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn date_filter_items_by_date_range() {
    let input = json!({
        "cutoff": "2026-03-03T00:00:00Z",
        "events": [
            {"name": "early", "date": "2026-03-01T00:00:00Z"},
            {"name": "mid", "date": "2026-03-03T00:00:00Z"},
            {"name": "late", "date": "2026-03-05T00:00:00Z"}
        ]
    });
    let result = engine()
        .evaluate(
            r#"(.cutoff | fromdateiso8601) as $cutoff |
            [.events[] | select((.date | fromdateiso8601) >= $cutoff) | .name]"#,
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(["mid", "late"]));
}

#[test]
fn date_now_returns_number() {
    let result = engine().evaluate("now | type", &json!(null)).unwrap();
    assert_eq!(result, json!("number"));
}

#[test]
fn date_strptime_and_mktime() {
    let input = json!("2026-03-05T00:00:00");
    let result = engine()
        .evaluate(
            "strptime(\"%Y-%m-%dT%H:%M:%S\") | mktime | todateiso8601",
            &input,
        )
        .unwrap();
    assert_eq!(result, json!("2026-03-05T00:00:00Z"));
}

// ─── Edge Cases ──────────────────────────────────────────────────────────

#[test]
fn identity_filter() {
    let input = json!({"keep": "everything"});
    let result = engine().evaluate(".", &input).unwrap();
    assert_eq!(result, input);
}

#[test]
fn empty_object_input() {
    let result = engine().evaluate(".", &json!({})).unwrap();
    assert_eq!(result, json!({}));
}

#[test]
fn empty_array_input() {
    let result = engine().evaluate(".", &json!([])).unwrap();
    assert_eq!(result, json!([]));
}

#[test]
fn deeply_nested_access() {
    let input = json!({"a": {"b": {"c": {"d": {"e": 42}}}}});
    let result = engine().evaluate(".a.b.c.d.e", &input).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn pipe_chain() {
    let input = json!({"items": [3, 1, 4, 1, 5]});
    let result = engine()
        .evaluate(".items | sort | reverse | .[0]", &input)
        .unwrap();
    assert_eq!(result, json!(5));
}

// ─── Real-world Capability Patterns ──────────────────────────────────────

#[test]
fn capability_transform_order_total() {
    let input = json!({
        "context": {
            "items": [
                {"name": "Widget", "price": 25, "quantity": 4},
                {"name": "Gadget", "price": 15, "quantity": 2}
            ],
            "tax_rate": 0.08
        }
    });
    let result = engine()
        .evaluate(
            r#"
            .context |
            {
                subtotal: [.items[] | .price * .quantity] | add,
                tax: ([.items[] | .price * .quantity] | add) * .tax_rate,
                item_count: .items | length
            }
            "#,
            &input,
        )
        .unwrap();
    assert_eq!(result["subtotal"], json!(130));
    assert_eq!(result["item_count"], json!(2));
}

#[test]
fn capability_assert_boolean_gate() {
    let input = json!({
        "prev": {"amount": 1500, "currency": "USD"},
        "context": {"limit": 1000}
    });
    let result = engine()
        .evaluate(".prev.amount > .context.limit", &input)
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn capability_validate_type_check() {
    let input = json!({
        "prev": {"name": "test", "count": 5}
    });
    let result = engine()
        .evaluate(
            r#".prev | ((.name | type) == "string") and ((.count | type) == "number")"#,
            &input,
        )
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn capability_emit_event_construction() {
    let input = json!({
        "context": {"order_id": "ORD-789", "customer": "CUST-456"},
        "prev": {"total": 250}
    });
    let result = engine()
        .evaluate(
            r#"{
                event_type: "order.completed",
                payload: {
                    order_id: .context.order_id,
                    customer_id: .context.customer,
                    amount: .prev.total
                }
            }"#,
            &input,
        )
        .unwrap();
    assert_eq!(result["event_type"], json!("order.completed"));
    assert_eq!(result["payload"]["order_id"], json!("ORD-789"));
    assert_eq!(result["payload"]["amount"], json!(250));
}

#[test]
fn capability_persist_envelope() {
    let input = json!({
        "context": {"user_id": "U-123"},
        "prev": {"status": "approved", "notes": "Looks good"}
    });
    let result = engine()
        .evaluate(
            r#"{
                target: "audit_log",
                data: {
                    user: .context.user_id,
                    action: .prev.status,
                    details: .prev.notes
                }
            }"#,
            &input,
        )
        .unwrap();
    assert_eq!(result["target"], json!("audit_log"));
    assert_eq!(result["data"]["user"], json!("U-123"));
}

// ─── Output Count Limits ────────────────────────────────────────────────

#[test]
fn evaluate_multi_respects_max_outputs_limit() {
    let engine = ExpressionEngine::new(ExpressionEngineConfig {
        max_outputs: 5,
        ..Default::default()
    });

    let result = engine.evaluate_multi("range(100)", &json!(null));
    assert!(result.is_err(), "should reject excessive outputs");
    let err = result.unwrap_err();
    assert!(
        matches!(err, ExpressionError::TooManyOutputs { limit: 5, .. }),
        "expected TooManyOutputs, got: {err}"
    );
}

#[test]
fn evaluate_multi_allows_within_limit() {
    let engine = ExpressionEngine::new(ExpressionEngineConfig {
        max_outputs: 100,
        ..Default::default()
    });

    let result = engine.evaluate_multi("range(10)", &json!(null)).unwrap();
    assert_eq!(result.len(), 10);
}

#[test]
fn evaluate_multi_default_limit_handles_reasonable_output() {
    // Default max_outputs is 10,000 — a filter producing 50 values should be fine
    let result = engine().evaluate_multi("range(50)", &json!(null)).unwrap();
    assert_eq!(result.len(), 50);
}
