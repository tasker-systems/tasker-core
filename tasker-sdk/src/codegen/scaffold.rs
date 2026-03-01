//! Unified scaffold generation: types + handlers + tests with import wiring.
//!
//! Unlike the independent `generate_types`/`generate_handlers`/`generate_tests` functions,
//! `generate_scaffold` produces coordinated output where handlers **import** generated types
//! and use typed return values.

use std::collections::HashMap;

use askama::Template;

use super::handler::{extract_handlers, HandlerDef};
use super::schema::TypeDef;
use super::{CodegenError, TargetLanguage};
use tasker_shared::models::core::task_template::TaskTemplate;

/// Coordinated output from scaffold generation.
#[derive(Debug, Clone)]
pub struct ScaffoldOutput {
    /// Generated type definitions (same as `generate_types`)
    pub types: String,
    /// Handler scaffolds that import and use the generated types
    pub handlers: String,
    /// Test scaffolds with typed mock data
    pub tests: String,
    /// Handler registry bridge (Rust only — wraps plain functions as `StepHandler` trait objects)
    pub handler_registry: Option<String>,
}

/// Generate coordinated types + handlers + tests where handlers import generated types.
///
/// This produces the same type definitions as `generate_types`, but the handler and test
/// output includes proper import statements and typed return values/mock data.
pub fn generate_scaffold(
    template: &TaskTemplate,
    language: TargetLanguage,
    step_filter: Option<&str>,
) -> Result<ScaffoldOutput, CodegenError> {
    // 1. Extract handler IR
    let handlers = extract_handlers(template, step_filter);

    // 2. Extract type defs using unqualified step names.
    //
    // Unlike `generate_types` which prefixes step names with the namespace
    // (e.g., "codegen_tests_validate_order" → CodegenTestsValidateOrderResult),
    // scaffold uses unqualified names so that handler references match type names.
    // This is intentional: scaffold produces coordinated output where the types file,
    // handler file, and test file all use consistent naming.
    let mut all_types: Vec<TypeDef> = Vec::new();

    // Extract input types from template-level input_schema (skip when step-filtering)
    if step_filter.is_none() {
        if let Some(input_schema) = &template.input_schema {
            let input_types = super::schema::extract_input_types(&template.name, input_schema)
                .map_err(|e| CodegenError::SchemaExtraction {
                    step: format!("{} (input_schema)", template.name),
                    source: e,
                })?;
            all_types.extend(input_types);
        }
    }

    let mut type_defs: Vec<TypeDef> = Vec::new();
    for step in &template.steps {
        if let Some(filter) = step_filter {
            if step.name != filter {
                continue;
            }
        }
        if let Some(schema) = &step.result_schema {
            let extracted = super::schema::extract_types(&step.name, schema).map_err(|e| {
                CodegenError::SchemaExtraction {
                    step: step.name.clone(),
                    source: e,
                }
            })?;
            type_defs.extend(extracted.clone());
            all_types.extend(extracted);
        }
    }

    // 3. Render types using language-specific renderers
    let types = match language {
        TargetLanguage::Python => super::python::render(&all_types)?,
        TargetLanguage::Ruby => super::ruby::render(&all_types)?,
        TargetLanguage::TypeScript => super::typescript_zod::render(&all_types)?,
        TargetLanguage::Rust => super::rust_gen::render(&all_types)?,
    };

    let type_map: HashMap<String, &TypeDef> =
        type_defs.iter().map(|td| (td.name.clone(), td)).collect();

    // 4. Collect all type names imported by handlers
    let import_types: Vec<String> = handlers
        .iter()
        .filter_map(|h| h.result_type_name())
        .filter(|name| type_map.contains_key(name))
        .collect();

    // Also collect dependency result types
    let dep_import_types: Vec<String> = handlers
        .iter()
        .flat_map(|h| &h.dependencies)
        .filter_map(|dep| dep.result_type.as_ref())
        .filter(|name| type_map.contains_key(*name))
        .cloned()
        .collect();

    let mut all_imports: Vec<String> = import_types;
    all_imports.extend(dep_import_types);
    all_imports.sort();
    all_imports.dedup();

    // 5. Determine types_module_name per language
    let types_module_name = match language {
        TargetLanguage::Python => "models",
        TargetLanguage::Ruby => "models",
        TargetLanguage::TypeScript => "models",
        TargetLanguage::Rust => "models",
    };

    // 6. Determine file extension for the types module
    let types_file_ext = match language {
        TargetLanguage::Python => "py",
        TargetLanguage::Ruby => "rb",
        TargetLanguage::TypeScript => "ts",
        TargetLanguage::Rust => "rs",
    };

    // 7. Render scaffold handlers
    let handler_output = render_scaffold_handlers(
        &handlers,
        &type_map,
        &all_imports,
        types_module_name,
        types_file_ext,
        language,
    )?;

    // 8. Render scaffold tests
    let test_output = render_scaffold_tests(
        &handlers,
        &type_map,
        &all_imports,
        types_module_name,
        types_file_ext,
        language,
    )?;

    // 9. Render handler registry (Rust only)
    let handler_registry = if language == TargetLanguage::Rust {
        Some(render_scaffold_registry(
            &handlers,
            &template.namespace_name,
        )?)
    } else {
        None
    };

    Ok(ScaffoldOutput {
        types,
        handlers: handler_output,
        tests: test_output,
        handler_registry,
    })
}

// =========================================================================
// Askama template structs for scaffold handler rendering
// =========================================================================

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_python_handlers.py")]
struct ScaffoldPythonHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_ruby_handlers.rb")]
struct ScaffoldRubyHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    #[expect(dead_code, reason = "Ruby uses require_relative, not per-type imports")]
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_typescript_handlers.ts")]
struct ScaffoldTypeScriptHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_rust_handlers.rs")]
struct ScaffoldRustHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    #[expect(dead_code, reason = "template uses types_module_name")]
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_rust_handler_registry.rs")]
struct ScaffoldRustRegistryTemplate<'a> {
    handlers: &'a [HandlerDef],
    namespace: &'a str,
    registry_name: String,
}

// =========================================================================
// Askama template structs for scaffold test rendering
// =========================================================================

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_python_tests.py")]
struct ScaffoldPythonTestTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_ruby_tests.rb")]
struct ScaffoldRubyTestTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    #[expect(dead_code, reason = "Ruby uses require_relative, not per-type imports")]
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_typescript_tests.ts")]
struct ScaffoldTypeScriptTestTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    types_module_name: &'a str,
}

#[derive(Template, Debug)]
#[template(path = "codegen/scaffold_rust_tests.rs")]
struct ScaffoldRustTestTemplate<'a> {
    handlers: &'a [HandlerDef],
    type_map: &'a HashMap<String, &'a TypeDef>,
    import_types: &'a [String],
    #[expect(dead_code, reason = "template uses types_module_name")]
    types_module_name: &'a str,
}

// =========================================================================
// Render functions
// =========================================================================

fn render_scaffold_handlers(
    handlers: &[HandlerDef],
    type_map: &HashMap<String, &TypeDef>,
    import_types: &[String],
    types_module_name: &str,
    _types_file_ext: &str,
    language: TargetLanguage,
) -> Result<String, CodegenError> {
    let output = match language {
        TargetLanguage::Python => {
            let t = ScaffoldPythonHandlerTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::Ruby => {
            let t = ScaffoldRubyHandlerTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::TypeScript => {
            let t = ScaffoldTypeScriptHandlerTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::Rust => {
            let t = ScaffoldRustHandlerTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
    };

    output.map_err(|e| CodegenError::Rendering(e.to_string()))
}

fn render_scaffold_tests(
    handlers: &[HandlerDef],
    type_map: &HashMap<String, &TypeDef>,
    import_types: &[String],
    types_module_name: &str,
    _types_file_ext: &str,
    language: TargetLanguage,
) -> Result<String, CodegenError> {
    let output = match language {
        TargetLanguage::Python => {
            let t = ScaffoldPythonTestTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::Ruby => {
            let t = ScaffoldRubyTestTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::TypeScript => {
            let t = ScaffoldTypeScriptTestTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
        TargetLanguage::Rust => {
            let t = ScaffoldRustTestTemplate {
                handlers,
                type_map,
                import_types,
                types_module_name,
            };
            t.render()
        }
    };

    output.map_err(|e| CodegenError::Rendering(e.to_string()))
}

fn render_scaffold_registry(
    handlers: &[HandlerDef],
    namespace: &str,
) -> Result<String, CodegenError> {
    use heck::ToUpperCamelCase;
    let registry_name = format!("{}Registry", namespace.to_upper_camel_case());
    let t = ScaffoldRustRegistryTemplate {
        handlers,
        namespace,
        registry_name,
    };
    t.render()
        .map_err(|e| CodegenError::Rendering(e.to_string()))
}

// =========================================================================
// Helpers
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn codegen_test_template() -> TaskTemplate {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        TaskTemplate::from_yaml(yaml).expect("fixture should parse")
    }

    // ── Python scaffold ─────────────────────────────────────────────

    #[test]
    fn test_python_scaffold_imports_types() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Python, None).unwrap();

        // Handlers should import from models
        assert!(output.handlers.contains("from .models import"));
        assert!(output.handlers.contains("ValidateOrderResult"));
    }

    #[test]
    fn test_python_scaffold_typed_return() {
        let template = codegen_test_template();
        let output =
            generate_scaffold(&template, TargetLanguage::Python, Some("validate_order")).unwrap();

        // Handler should return typed constructor
        assert!(output.handlers.contains("ValidateOrderResult("));
        assert!(output.handlers.contains("-> ValidateOrderResult"));
    }

    #[test]
    fn test_python_scaffold_untyped_handler() {
        let template = codegen_test_template();
        let output =
            generate_scaffold(&template, TargetLanguage::Python, Some("process_payment")).unwrap();

        // No result_schema → still uses dict return
        assert!(output.handlers.contains("return {}"));
        // Should NOT have a return type annotation
        assert!(!output.handlers.contains("-> ProcessPaymentResult"));
    }

    #[test]
    fn test_python_scaffold_tests_use_types() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Python, None).unwrap();

        // Tests should import models
        assert!(output.tests.contains("from .models import"));
    }

    // ── Ruby scaffold ───────────────────────────────────────────────

    #[test]
    fn test_ruby_scaffold_requires_models() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Ruby, None).unwrap();

        assert!(output.handlers.contains("require_relative 'models'"));
    }

    #[test]
    fn test_ruby_scaffold_typed_return() {
        let template = codegen_test_template();
        let output =
            generate_scaffold(&template, TargetLanguage::Ruby, Some("validate_order")).unwrap();

        assert!(output.handlers.contains("ValidateOrderResult.new("));
    }

    // ── TypeScript scaffold ─────────────────────────────────────────

    #[test]
    fn test_typescript_scaffold_imports_types() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::TypeScript, None).unwrap();

        assert!(output.handlers.contains("import {"));
        assert!(output.handlers.contains("from './models'"));
    }

    #[test]
    fn test_typescript_scaffold_typed_return() {
        let template = codegen_test_template();
        let output = generate_scaffold(
            &template,
            TargetLanguage::TypeScript,
            Some("validate_order"),
        )
        .unwrap();

        assert!(output.handlers.contains(": ValidateOrderResult"));
    }

    // ── Rust scaffold ───────────────────────────────────────────────

    #[test]
    fn test_rust_scaffold_uses_types() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Rust, None).unwrap();

        // Pattern B: plain functions import from super::models
        assert!(output.handlers.contains("use super::models::{"));
        // Pattern B: has get_dependency helper
        assert!(output.handlers.contains("fn get_dependency<T"));
    }

    #[test]
    fn test_rust_scaffold_typed_return() {
        let template = codegen_test_template();
        let output =
            generate_scaffold(&template, TargetLanguage::Rust, Some("validate_order")).unwrap();

        // Pattern B: plain function with serde_json serialization
        assert!(output.handlers.contains("ValidateOrderResult {"));
        assert!(output.handlers.contains("serde_json::to_value(result)"));
        // Pattern B: plain function signature
        assert!(output.handlers.contains("pub fn validate_order("));
    }

    #[test]
    fn test_rust_scaffold_has_handler_registry() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Rust, None).unwrap();

        let registry = output
            .handler_registry
            .as_ref()
            .expect("Rust scaffold should produce handler_registry");
        assert!(registry.contains("impl StepHandlerRegistry"));
        assert!(registry.contains("FunctionHandler"));
        assert!(registry.contains("handlers::validate_order"));
    }

    #[test]
    fn test_non_rust_scaffold_has_no_handler_registry() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Python, None).unwrap();
        assert!(output.handler_registry.is_none());
    }

    // ── Cross-language: ScaffoldOutput has all three parts ──────────

    #[test]
    fn test_scaffold_output_has_all_parts() {
        let template = codegen_test_template();
        let output = generate_scaffold(&template, TargetLanguage::Python, None).unwrap();

        assert!(!output.types.is_empty());
        assert!(!output.handlers.is_empty());
        assert!(!output.tests.is_empty());
    }

    // ── Step filter works ───────────────────────────────────────────

    #[test]
    fn test_scaffold_step_filter() {
        let template = codegen_test_template();
        let output =
            generate_scaffold(&template, TargetLanguage::Python, Some("validate_order")).unwrap();

        // Only one handler should be generated
        assert!(output.handlers.contains("validate_order"));
        assert!(!output.handlers.contains("enrich_order"));
    }
}
