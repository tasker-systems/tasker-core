//! Standard capability vocabulary for the Tasker grammar system.
//!
//! This module provides the built-in capability declarations that are available
//! to all composition-based workflows. The [`standard_capability_registry`]
//! function returns the complete set of 6 built-in capabilities: transform,
//! validate, assert, persist, acquire, and emit.
//!
//! # Usage
//!
//! ```rust
//! use tasker_grammar::standard_capability_registry;
//!
//! let registry = standard_capability_registry();
//! assert!(registry.contains_key("transform"));
//! assert!(registry.contains_key("persist"));
//! ```

use std::collections::HashMap;

use serde_json::json;

use crate::types::{CapabilityDeclaration, GrammarCategoryKind, MutationProfile};

/// Returns a [`HashMap`] containing all 6 built-in capability declarations.
///
/// The standard capabilities are:
///
/// | Name      | Category  | Mutation      |
/// |-----------|-----------|---------------|
/// | transform | Transform | NonMutating   |
/// | validate  | Validate  | NonMutating   |
/// | assert    | Assert    | NonMutating   |
/// | acquire   | Acquire   | NonMutating   |
/// | persist   | Persist   | Mutating      |
/// | emit      | Emit      | Mutating      |
///
/// All capabilities are versioned at `"1.0.0"`.
pub fn standard_capability_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut registry = HashMap::new();

    registry.insert(
        "transform".to_owned(),
        CapabilityDeclaration {
            name: "transform".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Pure data transformation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["output", "filter"],
                "properties": {
                    "output": { "type": "object" },
                    "filter": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "validate".to_owned(),
        CapabilityDeclaration {
            name: "validate".to_owned(),
            grammar_category: GrammarCategoryKind::Validate,
            description: "JSON Schema validation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["schema"],
                "properties": {
                    "schema": { "type": "object" },
                    "on_failure": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "assert".to_owned(),
        CapabilityDeclaration {
            name: "assert".to_owned(),
            grammar_category: GrammarCategoryKind::Assert,
            description: "Execution gate".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["filter", "error"],
                "properties": {
                    "filter": { "type": "string" },
                    "error": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "acquire".to_owned(),
        CapabilityDeclaration {
            name: "acquire".to_owned(),
            grammar_category: GrammarCategoryKind::Acquire,
            description: "Fetch data".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": {},
                    "params": {},
                    "constraints": { "type": "object" },
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "persist".to_owned(),
        CapabilityDeclaration {
            name: "persist".to_owned(),
            grammar_category: GrammarCategoryKind::Persist,
            description: "Write state".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource", "data"],
                "properties": {
                    "resource": {},
                    "data": {},
                    "mode": { "type": "string" },
                    "identity": { "type": "object" },
                    "constraints": { "type": "object" },
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "emit".to_owned(),
        CapabilityDeclaration {
            name: "emit".to_owned(),
            grammar_category: GrammarCategoryKind::Emit,
            description: "Fire domain events".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["event_name", "payload"],
                "properties": {
                    "event_name": { "type": "string" },
                    "event_version": { "type": "string" },
                    "resource": { "type": "string" },
                    "payload": {},
                    "condition": { "type": "string" },
                    "metadata": { "type": "object" },
                    "result_shape": { "type": "object" },
                    "validate_success": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_contains_all_six_capabilities() {
        let registry = standard_capability_registry();
        for name in &[
            "transform",
            "validate",
            "assert",
            "acquire",
            "persist",
            "emit",
        ] {
            assert!(
                registry.contains_key(*name),
                "standard registry should contain capability '{name}'"
            );
        }
        assert_eq!(
            registry.len(),
            6,
            "standard registry should contain exactly 6 capabilities"
        );
    }

    #[test]
    fn capabilities_have_correct_categories() {
        let registry = standard_capability_registry();

        assert_eq!(
            registry["transform"].grammar_category,
            GrammarCategoryKind::Transform
        );
        assert_eq!(
            registry["validate"].grammar_category,
            GrammarCategoryKind::Validate
        );
        assert_eq!(
            registry["assert"].grammar_category,
            GrammarCategoryKind::Assert
        );
        assert_eq!(
            registry["acquire"].grammar_category,
            GrammarCategoryKind::Acquire
        );
        assert_eq!(
            registry["persist"].grammar_category,
            GrammarCategoryKind::Persist
        );
        assert_eq!(registry["emit"].grammar_category, GrammarCategoryKind::Emit);
    }

    #[test]
    fn mutating_capabilities_have_correct_profiles() {
        let registry = standard_capability_registry();

        // Non-mutating
        for name in &["transform", "validate", "assert", "acquire"] {
            assert_eq!(
                registry[*name].mutation_profile,
                MutationProfile::NonMutating,
                "'{name}' should be NonMutating"
            );
        }

        // Mutating with idempotency key support
        for name in &["persist", "emit"] {
            assert_eq!(
                registry[*name].mutation_profile,
                MutationProfile::Mutating {
                    supports_idempotency_key: true
                },
                "'{name}' should be Mutating with idempotency key support"
            );
        }
    }

    #[test]
    fn capabilities_have_config_schemas_with_required_fields() {
        let registry = standard_capability_registry();

        let required_fields: &[(&str, &[&str])] = &[
            ("transform", &["output", "filter"]),
            ("validate", &["schema"]),
            ("assert", &["filter", "error"]),
            ("acquire", &["resource"]),
            ("persist", &["resource", "data"]),
            ("emit", &["event_name", "payload"]),
        ];

        for (name, expected_required) in required_fields {
            let decl = &registry[*name];
            let required = decl.config_schema["required"]
                .as_array()
                .unwrap_or_else(|| panic!("'{name}' config_schema should have 'required' array"));

            for field in *expected_required {
                assert!(
                    required.iter().any(|v| v.as_str() == Some(field)),
                    "'{name}' config schema should require field '{field}'"
                );
            }
        }
    }
}
