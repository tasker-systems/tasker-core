//! Custom Tera filters for case conversion.

use std::collections::HashMap;

use heck::{ToKebabCase, ToLowerCamelCase, ToPascalCase, ToSnakeCase};
use tera::{Result, Value};

pub(crate) fn snake_case(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("snake_case filter expects a string"))?;
    Ok(Value::String(s.to_snake_case()))
}

pub(crate) fn pascal_case(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("pascal_case filter expects a string"))?;
    Ok(Value::String(s.to_pascal_case()))
}

pub(crate) fn camel_case(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("camel_case filter expects a string"))?;
    Ok(Value::String(s.to_lower_camel_case()))
}

pub(crate) fn kebab_case(value: &Value, _args: &HashMap<String, Value>) -> Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("kebab_case filter expects a string"))?;
    Ok(Value::String(s.to_kebab_case()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(filter: fn(&Value, &HashMap<String, Value>) -> Result<Value>, input: &str) -> String {
        let val = Value::String(input.to_string());
        let args = HashMap::new();
        filter(&val, &args).unwrap().as_str().unwrap().to_string()
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(apply(snake_case, "ProcessPayment"), "process_payment");
        assert_eq!(apply(snake_case, "process-payment"), "process_payment");
        assert_eq!(apply(snake_case, "already_snake"), "already_snake");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(apply(pascal_case, "process_payment"), "ProcessPayment");
        assert_eq!(apply(pascal_case, "process-payment"), "ProcessPayment");
        assert_eq!(apply(pascal_case, "ProcessPayment"), "ProcessPayment");
    }

    #[test]
    fn test_camel_case() {
        assert_eq!(apply(camel_case, "ProcessPayment"), "processPayment");
        assert_eq!(apply(camel_case, "process_payment"), "processPayment");
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(apply(kebab_case, "ProcessPayment"), "process-payment");
        assert_eq!(apply(kebab_case, "process_payment"), "process-payment");
    }

    #[test]
    fn test_filter_rejects_non_string() {
        let val = Value::Number(42.into());
        let args = HashMap::new();
        assert!(snake_case(&val, &args).is_err());
    }
}
