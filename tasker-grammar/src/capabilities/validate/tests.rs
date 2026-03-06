use serde_json::json;

use crate::types::{CapabilityError, CapabilityExecutor, ExecutionContext};

use super::ValidateExecutor;

fn executor() -> ValidateExecutor {
    ValidateExecutor::new()
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
            "amount": 99.99,
        },
        "deps": {},
        "step": {
            "name": "validate_input",
            "attempt": 1
        },
        "prev": null
    })
}

/// Envelope with `.prev` set to a concrete value.
fn envelope_with_prev(prev: serde_json::Value) -> serde_json::Value {
    let mut env = envelope();
    env["prev"] = prev;
    env
}

// ---------------------------------------------------------------------------
// Basic validation — valid input passes through
// ---------------------------------------------------------------------------

#[test]
fn valid_object_passes_through_unchanged() {
    let input = envelope_with_prev(json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"},
                "email": {"type": "string"}
            }
        }
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(
        result,
        json!({
            "name": "Alice",
            "age": 30,
            "email": "alice@example.com"
        })
    );
}

#[test]
fn valid_array_passes_through() {
    let input = envelope_with_prev(json!([1, 2, 3]));

    let config = json!({
        "schema": {
            "type": "array",
            "items": {"type": "integer"}
        }
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn valid_scalar_passes_through() {
    let input = envelope_with_prev(json!("hello"));

    let config = json!({
        "schema": {"type": "string"}
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!("hello"));
}

// ---------------------------------------------------------------------------
// Context fallback — validates .context when .prev is null
// ---------------------------------------------------------------------------

#[test]
fn validates_context_when_prev_is_null() {
    let input = envelope(); // prev is null, context has order_id, customer_email, amount

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["order_id"],
            "properties": {
                "order_id": {"type": "string"},
                "customer_email": {"type": "string"},
                "amount": {"type": "number"}
            }
        }
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["order_id"], json!("ORD-001"));
    assert_eq!(result["amount"], json!(99.99));
}

// ---------------------------------------------------------------------------
// Invalid input — structured error with field-level details
// ---------------------------------------------------------------------------

#[test]
fn invalid_input_produces_structured_error() {
    let input = envelope_with_prev(json!({
        "name": 42,
        "age": "not a number"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        }
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    match &err {
        CapabilityError::InputValidation(msg) => {
            assert!(
                msg.contains("expected type"),
                "should mention type mismatch: {msg}"
            );
        }
        other => panic!("expected InputValidation, got: {other:?}"),
    }
}

#[test]
fn missing_required_field_reports_field_name() {
    let input = envelope_with_prev(json!({
        "name": "Alice"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        }
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    match &err {
        CapabilityError::InputValidation(msg) => {
            assert!(
                msg.contains("age"),
                "should mention missing field 'age': {msg}"
            );
            assert!(msg.contains("required"), "should mention 'required': {msg}");
        }
        other => panic!("expected InputValidation, got: {other:?}"),
    }
}

#[test]
fn error_does_not_leak_actual_values() {
    let input = envelope_with_prev(json!({
        "ssn": "123-45-6789",
        "name": 42
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ssn": {"type": "string"},
                "name": {"type": "string"}
            }
        }
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    let msg = err.to_string();
    // The SSN value must NOT appear in error messages
    assert!(
        !msg.contains("123-45-6789"),
        "error must not leak PII: {msg}"
    );
    // The numeric value that failed should not appear as the raw value
    assert!(
        !msg.contains("\"42\""),
        "error should not embed raw value: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Coercion — type coercion before validation
// ---------------------------------------------------------------------------

#[test]
fn coerce_string_to_number() {
    let input = envelope_with_prev(json!({
        "amount": "123.45",
        "quantity": "7"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "amount": {"type": "number"},
                "quantity": {"type": "number"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["amount"], json!(123.45));
    assert_eq!(result["quantity"], json!(7.0));
}

#[test]
fn coerce_string_to_integer() {
    let input = envelope_with_prev(json!({
        "count": "42"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "count": {"type": "integer"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["count"], json!(42));
}

#[test]
fn coerce_string_to_boolean() {
    let input = envelope_with_prev(json!({
        "active": "true",
        "deleted": "false"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "active": {"type": "boolean"},
                "deleted": {"type": "boolean"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["active"], json!(true));
    assert_eq!(result["deleted"], json!(false));
}

#[test]
fn coerce_number_to_string() {
    let input = envelope_with_prev(json!({
        "code": 12345
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "code": {"type": "string"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["code"], json!("12345"));
}

#[test]
fn coerce_boolean_to_string() {
    let input = envelope_with_prev(json!({
        "flag": true
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "flag": {"type": "string"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["flag"], json!("true"));
}

#[test]
fn coerce_nested_objects() {
    let input = envelope_with_prev(json!({
        "payment": {
            "amount": "99.99",
            "approved": "true"
        }
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "payment": {
                    "type": "object",
                    "properties": {
                        "amount": {"type": "number"},
                        "approved": {"type": "boolean"}
                    }
                }
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["payment"]["amount"], json!(99.99));
    assert_eq!(result["payment"]["approved"], json!(true));
}

#[test]
fn coerce_array_items() {
    let input = envelope_with_prev(json!({
        "quantities": ["1", "2", "3"]
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "quantities": {
                    "type": "array",
                    "items": {"type": "integer"}
                }
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["quantities"], json!([1, 2, 3]));
}

#[test]
fn coerce_non_coercible_string_leaves_value_unchanged() {
    let input = envelope_with_prev(json!({
        "amount": "not-a-number"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "amount": {"type": "number"}
            }
        },
        "coerce": true
    });

    // Should fail validation since "not-a-number" can't be coerced
    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::InputValidation(_)));
}

#[test]
fn no_coercion_by_default() {
    let input = envelope_with_prev(json!({
        "amount": "123"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "amount": {"type": "number"}
            }
        }
        // coerce not specified — defaults to false
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::InputValidation(_)));
}

// ---------------------------------------------------------------------------
// filter_extra — strip undeclared fields
// ---------------------------------------------------------------------------

#[test]
fn filter_extra_removes_undeclared_fields() {
    let input = envelope_with_prev(json!({
        "name": "Alice",
        "age": 30,
        "internal_id": "secret-123",
        "debug_info": {"trace": "abc"}
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        },
        "filter_extra": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!({"name": "Alice", "age": 30}));
    assert!(result.get("internal_id").is_none());
    assert!(result.get("debug_info").is_none());
}

#[test]
fn filter_extra_recurses_into_nested_objects() {
    let input = envelope_with_prev(json!({
        "user": {
            "name": "Alice",
            "password_hash": "abc123"
        }
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            }
        },
        "filter_extra": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["user"], json!({"name": "Alice"}));
    assert!(result["user"].get("password_hash").is_none());
}

#[test]
fn filter_extra_recurses_into_array_items() {
    let input = envelope_with_prev(json!({
        "items": [
            {"sku": "A1", "secret": "x"},
            {"sku": "B2", "secret": "y"}
        ]
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "sku": {"type": "string"}
                        }
                    }
                }
            }
        },
        "filter_extra": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["items"][0], json!({"sku": "A1"}));
    assert_eq!(result["items"][1], json!({"sku": "B2"}));
}

#[test]
fn without_filter_extra_keeps_all_fields() {
    let input = envelope_with_prev(json!({
        "name": "Alice",
        "extra_field": "kept"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        }
        // filter_extra not specified — defaults to false
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["extra_field"], json!("kept"));
}

// ---------------------------------------------------------------------------
// on_failure modes
// ---------------------------------------------------------------------------

#[test]
fn on_failure_error_is_default() {
    let input = envelope_with_prev(json!({"age": "not a number"}));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["age"],
            "properties": {
                "age": {"type": "integer"}
            }
        }
        // on_failure not specified — defaults to "error"
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::InputValidation(_)));
}

#[test]
fn on_failure_error_explicit() {
    let input = envelope_with_prev(json!({"age": "not a number"}));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "age": {"type": "integer"}
            }
        },
        "on_failure": "error"
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::InputValidation(_)));
}

#[test]
fn on_failure_warn_passes_data_with_warnings() {
    let input = envelope_with_prev(json!({
        "name": "Alice",
        "age": "not a number"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        },
        "on_failure": "warn"
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    // Original data is preserved
    assert_eq!(result["name"], json!("Alice"));
    assert_eq!(result["age"], json!("not a number"));
    // Warnings are attached
    let warnings = result["_validation_warnings"].as_array().unwrap();
    assert!(!warnings.is_empty());
    let warning_text = warnings[0].as_str().unwrap();
    assert!(
        warning_text.contains("expected type"),
        "warning should describe the error: {warning_text}"
    );
}

#[test]
fn on_failure_skip_passes_invalid_data_through() {
    let input = envelope_with_prev(json!({
        "name": "Alice",
        "age": "not a number"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        },
        "on_failure": "skip"
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    // Data passes through unchanged
    assert_eq!(
        result,
        json!({
            "name": "Alice",
            "age": "not a number"
        })
    );
    // No warnings attached
    assert!(result.get("_validation_warnings").is_none());
}

#[test]
fn on_failure_warn_valid_data_has_no_warnings() {
    let input = envelope_with_prev(json!({"name": "Alice"}));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        },
        "on_failure": "warn"
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!({"name": "Alice"}));
    assert!(result.get("_validation_warnings").is_none());
}

#[test]
fn on_failure_invalid_value_rejected() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "schema": {"type": "object"},
        "on_failure": "panic"
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    match &err {
        CapabilityError::ConfigValidation(msg) => {
            assert!(
                msg.contains("on_failure"),
                "should mention on_failure: {msg}"
            );
        }
        other => panic!("expected ConfigValidation, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Config validation errors
// ---------------------------------------------------------------------------

#[test]
fn missing_schema_in_config() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "coerce": true
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    match &err {
        CapabilityError::ConfigValidation(msg) => {
            assert!(msg.contains("schema"), "should mention 'schema': {msg}");
        }
        other => panic!("expected ConfigValidation, got: {other:?}"),
    }
}

#[test]
fn invalid_json_schema_in_config() {
    let input = envelope_with_prev(json!({"x": 1}));

    let config = json!({
        "schema": {
            "type": "not-a-real-type"
        }
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

// ---------------------------------------------------------------------------
// Coercion + filter_extra combined
// ---------------------------------------------------------------------------

#[test]
fn coercion_and_filter_extra_combined() {
    let input = envelope_with_prev(json!({
        "amount": "99.99",
        "active": "true",
        "internal_debug": "should_be_removed",
        "extra": 42
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["amount", "active"],
            "properties": {
                "amount": {"type": "number"},
                "active": {"type": "boolean"}
            }
        },
        "coerce": true,
        "filter_extra": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!({"amount": 99.99, "active": true}));
}

// ---------------------------------------------------------------------------
// Capability name
// ---------------------------------------------------------------------------

#[test]
fn capability_name_is_validate() {
    assert_eq!(executor().capability_name(), "validate");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_object_validates_against_empty_schema() {
    let input = envelope_with_prev(json!({}));

    let config = json!({
        "schema": {"type": "object"}
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, json!({}));
}

#[test]
fn null_context_and_null_prev() {
    let input = json!({
        "context": null,
        "deps": {},
        "step": {},
        "prev": null
    });

    let config = json!({
        "schema": {"type": "null"}
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn complex_schema_with_nested_required() {
    let input = envelope_with_prev(json!({
        "order": {
            "id": "ORD-001",
            "items": [
                {"sku": "A1", "qty": 2, "price": 25.0},
                {"sku": "B2", "qty": 1, "price": 50.0}
            ],
            "total": 100.0
        }
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["order"],
            "properties": {
                "order": {
                    "type": "object",
                    "required": ["id", "items", "total"],
                    "properties": {
                        "id": {"type": "string"},
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["sku", "qty", "price"],
                                "properties": {
                                    "sku": {"type": "string"},
                                    "qty": {"type": "integer"},
                                    "price": {"type": "number"}
                                }
                            }
                        },
                        "total": {"type": "number"}
                    }
                }
            }
        }
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["order"]["id"], json!("ORD-001"));
    assert_eq!(result["order"]["total"], json!(100.0));
}

#[test]
fn multiple_validation_errors_joined() {
    let input = envelope_with_prev(json!({
        "name": 42,
        "age": "not-a-number"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "required": ["name", "age", "email"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"},
                "email": {"type": "string"}
            }
        }
    });

    let err = executor().execute(&input, &config, &ctx()).unwrap_err();
    match &err {
        CapabilityError::InputValidation(msg) => {
            // Multiple errors should be joined with "; "
            assert!(
                msg.contains("; "),
                "multiple errors should be joined: {msg}"
            );
        }
        other => panic!("expected InputValidation, got: {other:?}"),
    }
}

#[test]
fn default_impl() {
    // ValidateExecutor implements Default
    let exec = ValidateExecutor;
    assert_eq!(exec.capability_name(), "validate");
}

use serde_json::Value;

// ---------------------------------------------------------------------------
// Date format coercion — coerce: true with format: "date-time" or "date"
// ---------------------------------------------------------------------------

#[test]
fn coerce_iso_date_only_to_datetime() {
    let input = envelope_with_prev(json!({
        "created_at": "2026-03-05"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "created_at": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["created_at"], json!("2026-03-05T00:00:00Z"));
}

#[test]
fn coerce_iso_datetime_without_timezone() {
    let input = envelope_with_prev(json!({
        "timestamp": "2026-03-05T14:30:00"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "timestamp": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["timestamp"], json!("2026-03-05T14:30:00Z"));
}

#[test]
fn coerce_rfc3339_passthrough() {
    let input = envelope_with_prev(json!({
        "ts": "2026-03-05T14:30:00Z"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ts": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["ts"], json!("2026-03-05T14:30:00Z"));
}

#[test]
fn coerce_us_date_format_to_datetime() {
    let input = envelope_with_prev(json!({
        "date": "03/05/2026"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "date": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["date"], json!("2026-03-05T00:00:00Z"));
}

#[test]
fn coerce_european_date_format_to_datetime() {
    let input = envelope_with_prev(json!({
        "date": "05.03.2026"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "date": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["date"], json!("2026-03-05T00:00:00Z"));
}

#[test]
fn coerce_epoch_string_to_datetime() {
    let input = envelope_with_prev(json!({
        "ts": "1709683200"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ts": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["ts"], json!("2024-03-06T00:00:00Z"));
}

#[test]
fn coerce_non_date_string_left_unchanged_for_datetime() {
    let input = envelope_with_prev(json!({
        "ts": "not-a-date"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ts": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    // Coercion can't parse it, so it stays as-is. Whether this then
    // fails validation depends on whether jsonschema enforces format.
    let result = executor().execute(&input, &config, &ctx());
    // The value should either pass through or fail validation —
    // the key point is coercion doesn't panic
    match result {
        Ok(v) => assert_eq!(v["ts"], json!("not-a-date")),
        Err(CapabilityError::InputValidation(_)) => {} // also acceptable
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn coerce_us_format_to_date() {
    let input = envelope_with_prev(json!({
        "dob": "03/05/2026"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "dob": {"type": "string", "format": "date"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["dob"], json!("2026-03-05"));
}

#[test]
fn coerce_european_format_to_date() {
    let input = envelope_with_prev(json!({
        "dob": "05.03.2026"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "dob": {"type": "string", "format": "date"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["dob"], json!("2026-03-05"));
}

#[test]
fn coerce_rfc3339_to_date_extracts_date() {
    let input = envelope_with_prev(json!({
        "dob": "2026-03-05T14:30:00Z"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "dob": {"type": "string", "format": "date"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["dob"], json!("2026-03-05"));
}

#[test]
fn coerce_date_passthrough() {
    let input = envelope_with_prev(json!({
        "dob": "2026-03-05"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "dob": {"type": "string", "format": "date"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["dob"], json!("2026-03-05"));
}

#[test]
fn coerce_datetime_nested_in_object() {
    let input = envelope_with_prev(json!({
        "event": {
            "start": "03/05/2026",
            "end": "2026-03-06T18:00:00"
        }
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "event": {
                    "type": "object",
                    "properties": {
                        "start": {"type": "string", "format": "date-time"},
                        "end": {"type": "string", "format": "date-time"}
                    }
                }
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["event"]["start"], json!("2026-03-05T00:00:00Z"));
    assert_eq!(result["event"]["end"], json!("2026-03-06T18:00:00Z"));
}

#[test]
fn coerce_datetime_in_array_items() {
    let input = envelope_with_prev(json!({
        "timestamps": ["2026-03-05", "03/06/2026", "2026-03-07T10:00:00Z"]
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "timestamps": {
                    "type": "array",
                    "items": {"type": "string", "format": "date-time"}
                }
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    let ts = result["timestamps"].as_array().unwrap();
    assert_eq!(ts[0], json!("2026-03-05T00:00:00Z"));
    assert_eq!(ts[1], json!("2026-03-06T00:00:00Z"));
    assert_eq!(ts[2], json!("2026-03-07T10:00:00Z"));
}

#[test]
fn no_date_coercion_without_coerce_flag() {
    let input = envelope_with_prev(json!({
        "ts": "03/05/2026"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ts": {"type": "string", "format": "date-time"}
            }
        }
        // coerce not set — no coercion
    });

    // Without coerce, the US-format date is left as-is
    let result = executor().execute(&input, &config, &ctx());
    match result {
        Ok(v) => assert_eq!(v["ts"], json!("03/05/2026")),
        Err(CapabilityError::InputValidation(_)) => {} // format validation may reject it
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn coerce_datetime_with_fractional_seconds() {
    let input = envelope_with_prev(json!({
        "ts": "2026-03-05T14:30:00.123"
    }));

    let config = json!({
        "schema": {
            "type": "object",
            "properties": {
                "ts": {"type": "string", "format": "date-time"}
            }
        },
        "coerce": true
    });

    let result = executor().execute(&input, &config, &ctx()).unwrap();
    assert_eq!(result["ts"], json!("2026-03-05T14:30:00.123Z"));
}
