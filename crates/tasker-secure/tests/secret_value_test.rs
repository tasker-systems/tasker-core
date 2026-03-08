use tasker_secure::SecretValue;

#[test]
fn display_is_redacted() {
    let val = SecretValue::new("super-secret-password");
    assert_eq!(format!("{val}"), "[REDACTED]");
}

#[test]
fn debug_is_redacted() {
    let val = SecretValue::new("super-secret-password");
    let debug_output = format!("{val:?}");
    assert!(!debug_output.contains("super-secret-password"));
    assert!(debug_output.contains("REDACTED"));
}

#[test]
fn expose_secret_returns_actual_value() {
    let val = SecretValue::new("super-secret-password");
    assert_eq!(val.expose_secret(), "super-secret-password");
}

#[test]
fn secret_value_from_string() {
    let val = SecretValue::from(String::from("from-owned-string"));
    assert_eq!(val.expose_secret(), "from-owned-string");
}

#[test]
fn secret_value_debug_never_leaks_in_struct() {
    let val = SecretValue::new("my-api-key");
    let debug = format!("{val:?}");
    assert!(
        !debug.contains("my-api-key"),
        "Debug output must never contain the secret value"
    );
}
