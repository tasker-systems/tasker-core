//! Askama template structs for rendering handler scaffolds and test files.
//!
//! Each language has a handler template and a test template, rendered from
//! `HandlerDef` IR extracted by `handler::extract_handlers()`.

use askama::Template;

use super::handler::HandlerDef;
use super::{CodegenError, TargetLanguage};

// =========================================================================
// Handler templates
// =========================================================================

#[derive(Template, Debug)]
#[template(path = "codegen/python_handlers.py")]
struct PythonHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
}

#[derive(Template, Debug)]
#[template(path = "codegen/ruby_handlers.rb")]
struct RubyHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
}

#[derive(Template, Debug)]
#[template(path = "codegen/typescript_handlers.ts")]
struct TypeScriptHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
}

#[derive(Template, Debug)]
#[template(path = "codegen/rust_handlers.rs")]
struct RustHandlerTemplate<'a> {
    handlers: &'a [HandlerDef],
}

/// Render handler scaffolds for the given language.
pub fn render_handlers(
    handlers: &[HandlerDef],
    _template_name: &str,
    language: TargetLanguage,
) -> Result<String, CodegenError> {
    let output = match language {
        TargetLanguage::Python => {
            let t = PythonHandlerTemplate { handlers };
            t.render()
        }
        TargetLanguage::Ruby => {
            let t = RubyHandlerTemplate { handlers };
            t.render()
        }
        TargetLanguage::TypeScript => {
            let t = TypeScriptHandlerTemplate { handlers };
            t.render()
        }
        TargetLanguage::Rust => {
            let t = RustHandlerTemplate { handlers };
            t.render()
        }
    };

    output.map_err(|e| CodegenError::Rendering(e.to_string()))
}

// =========================================================================
// Test templates (placeholder — wired in commit 4)
// =========================================================================

/// Render test scaffolds for the given language.
pub fn render_tests(
    _handlers: &[HandlerDef],
    _template_name: &str,
    _language: TargetLanguage,
) -> Result<String, CodegenError> {
    // Will be implemented in commit 4 with test Askama templates
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::handler::{extract_handlers, DependencyRef, StubField};
    use crate::codegen::schema::FieldType;
    use tasker_shared::models::core::task_template::TaskTemplate;

    fn codegen_test_template() -> TaskTemplate {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        TaskTemplate::from_yaml(yaml).expect("fixture should parse")
    }

    // ── Python ──────────────────────────────────────────────────────

    #[test]
    fn test_python_handler_no_dependencies() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("validate_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Python).unwrap();

        assert!(output.contains("@step_handler(\"codegen_tests.validate_order\")"));
        assert!(output.contains("def validate_order(context):"));
        assert!(output.contains("\"validated\": False"));
    }

    #[test]
    fn test_python_handler_with_typed_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("enrich_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Python).unwrap();

        assert!(output.contains("@depends_on(validate_order_result=\"validate_order\")"));
        assert!(output.contains("def enrich_order(validate_order_result, context):"));
        assert!(output.contains("# validate_order_result: ValidateOrderResult (typed)"));
    }

    #[test]
    fn test_python_handler_multiple_handlers() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Python).unwrap();

        assert!(output.contains("def validate_order(context):"));
        assert!(output.contains("def enrich_order("));
        assert!(output.contains("def process_payment("));
        assert!(output.contains("def generate_report("));
    }

    // ── Ruby ────────────────────────────────────────────────────────

    #[test]
    fn test_ruby_handler_no_dependencies() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("validate_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Ruby).unwrap();

        assert!(output.contains("ValidateOrderHandler = step_handler("));
        assert!(output.contains("'codegen_tests.validate_order'"));
        assert!(output.contains(") do |context:|"));
    }

    #[test]
    fn test_ruby_handler_with_typed_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("enrich_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Ruby).unwrap();

        assert!(output.contains("depends_on: {"));
        assert!(output.contains("validate_order_result: 'validate_order'"));
        assert!(output.contains(") do |validate_order_result:, context:|"));
    }

    // ── TypeScript ──────────────────────────────────────────────────

    #[test]
    fn test_typescript_handler_no_dependencies() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("validate_order"));
        let output =
            render_handlers(&handlers, "codegen_test", TargetLanguage::TypeScript).unwrap();

        assert!(output.contains("export const ValidateOrderHandler = defineHandler("));
        assert!(output.contains("'codegen_tests.validate_order'"));
        assert!(output.contains("async ({ context }) => {"));
    }

    #[test]
    fn test_typescript_handler_with_typed_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("enrich_order"));
        let output =
            render_handlers(&handlers, "codegen_test", TargetLanguage::TypeScript).unwrap();

        assert!(output.contains("depends: {"));
        assert!(output.contains("validateOrderResult: 'validate_order'"));
        assert!(output.contains("async ({ validateOrderResult, context }) => {"));
    }

    // ── Rust ────────────────────────────────────────────────────────

    #[test]
    fn test_rust_handler_no_dependencies() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("validate_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Rust).unwrap();

        assert!(output.contains("pub struct ValidateOrderHandler {"));
        assert!(output.contains("impl RustStepHandler for ValidateOrderHandler {"));
        assert!(output.contains("\"codegen_tests.validate_order\""));
    }

    #[test]
    fn test_rust_handler_with_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("enrich_order"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Rust).unwrap();

        assert!(output.contains("pub struct EnrichOrderHandler {"));
        assert!(output.contains("// Dependency: validate_order"));
    }

    // ── Untyped handler ─────────────────────────────────────────────

    #[test]
    fn test_python_untyped_handler() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("process_payment"));
        let output = render_handlers(&handlers, "codegen_test", TargetLanguage::Python).unwrap();

        assert!(output.contains("def process_payment(validate_order_result, context):"));
        assert!(output.contains("return {}"));
    }
}
