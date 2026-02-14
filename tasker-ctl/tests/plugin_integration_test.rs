//! Integration tests for the plugin discovery and template generation pipeline.
//!
//! Creates realistic plugin directory structures in temp directories and tests
//! the full pipeline: discovery → manifest parsing → template rendering → file output.
//!
//! Self-contained — no dependency on tasker-contrib being on disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Template entry: (name, description, files).
type TemplateEntry<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

/// Get the path to the compiled tasker-ctl binary.
fn tasker_ctl_bin() -> PathBuf {
    // In integration tests, CARGO_BIN_EXE_<name> gives the path to the binary
    PathBuf::from(env!("CARGO_BIN_EXE_tasker-ctl"))
}

/// Run tasker-ctl with the given args from a working directory that has a .tasker-ctl.toml.
fn run_tasker_ctl(work_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(tasker_ctl_bin())
        .args(args)
        .current_dir(work_dir)
        .output()
        .expect("Failed to execute tasker-ctl")
}

/// Create a minimal but complete plugin structure for testing.
fn create_test_plugin(
    base_dir: &Path,
    name: &str,
    language: &str,
    framework: Option<&str>,
    templates: &[TemplateEntry<'_>],
) -> PathBuf {
    let plugin_dir = base_dir.join(name).join("tasker-cli-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    // Build manifest
    let mut manifest = format!(
        r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "Test plugin for {language}"
language = "{language}"
"#
    );
    if let Some(fw) = framework {
        manifest.push_str(&format!("framework = \"{fw}\"\n"));
    }

    for (tmpl_name, tmpl_desc, _) in templates {
        manifest.push_str(&format!(
            r#"
[[templates]]
name = "{tmpl_name}"
path = "templates/{tmpl_name}"
description = "{tmpl_desc}"
"#
        ));
    }

    fs::write(plugin_dir.join("tasker-plugin.toml"), manifest).unwrap();

    // Create template directories with content
    for (tmpl_name, _, files) in templates {
        let tmpl_dir = plugin_dir.join("templates").join(tmpl_name);
        fs::create_dir_all(&tmpl_dir).unwrap();

        for (filename, content) in *files {
            fs::write(tmpl_dir.join(filename), content).unwrap();
        }
    }

    plugin_dir
}

/// Create a .tasker-ctl.toml in the given directory.
fn create_cli_config(dir: &Path, plugin_paths: &[&str]) {
    let paths: Vec<String> = plugin_paths.iter().map(|p| format!("\"{p}\"")).collect();
    let config = format!("plugin-paths = [{}]\n", paths.join(", "));
    fs::write(dir.join(".tasker-ctl.toml"), config).unwrap();
}

// ==========================================================================
// Plugin Discovery Tests
// ==========================================================================

#[test]
fn test_plugin_list_discovers_plugins_with_correct_schema() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        Some("rails"),
        &[("step_handler", "Generate a step handler", &[])],
    );

    create_test_plugin(
        &plugins_dir,
        "test-python",
        "python",
        None,
        &[("step_handler", "Generate a step handler", &[])],
    );

    // Create CLI config pointing at the plugins directory
    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(temp.path(), &["plugin", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "plugin list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("test-ruby"), "Should find ruby plugin");
    assert!(stdout.contains("test-python"), "Should find python plugin");
}

#[test]
fn test_plugin_validate_with_correct_manifest() {
    let temp = TempDir::new().unwrap();

    let template_toml = r#"name = "step_handler"
description = "Test step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"
"#;

    let handler_tera = r#"class {{ name | pascal_case }}Handler
end
"#;

    create_test_plugin(
        temp.path(),
        "valid-plugin",
        "ruby",
        Some("rails"),
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.rb.tera", handler_tera),
            ],
        )],
    );

    let plugin_dir = temp.path().join("valid-plugin").join("tasker-cli-plugin");

    let output = run_tasker_ctl(
        temp.path(),
        &["plugin", "validate", plugin_dir.to_str().unwrap()],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "plugin validate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("valid-plugin"),
        "Should show plugin name in validation output"
    );
}

#[test]
fn test_three_level_discovery() {
    let temp = TempDir::new().unwrap();
    let contrib_dir = temp.path().join("contrib");
    fs::create_dir_all(&contrib_dir).unwrap();

    // Level 2: lang/tasker-cli-plugin/tasker-plugin.toml
    create_test_plugin(
        &contrib_dir,
        "rails",
        "ruby",
        Some("rails"),
        &[("step_handler", "Generate a step handler", &[])],
    );

    create_test_plugin(
        &contrib_dir,
        "python",
        "python",
        None,
        &[("step_handler", "Generate a step handler", &[])],
    );

    create_test_plugin(
        &contrib_dir,
        "typescript",
        "typescript",
        None,
        &[("step_handler", "Generate a step handler", &[])],
    );

    // Create CLI config pointing at the contrib root
    create_cli_config(temp.path(), &[contrib_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(temp.path(), &["plugin", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "plugin list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // All three should be discovered via level-2 scanning
    assert!(stdout.contains("rails"), "Should find rails plugin");
    assert!(stdout.contains("python"), "Should find python plugin");
    assert!(
        stdout.contains("typescript"),
        "Should find typescript plugin"
    );
}

// ==========================================================================
// Template Listing Tests
// ==========================================================================

#[test]
fn test_template_list_shows_all_templates() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        Some("rails"),
        &[
            ("step_handler", "Generate a step handler", &[]),
            ("task_template", "Generate a task definition", &[]),
        ],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(temp.path(), &["template", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("step_handler"),
        "Should list step_handler template"
    );
    assert!(
        stdout.contains("task_template"),
        "Should list task_template template"
    );
}

#[test]
fn test_template_list_filters_by_language() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        Some("rails"),
        &[("step_handler", "Generate a ruby step handler", &[])],
    );

    create_test_plugin(
        &plugins_dir,
        "test-python",
        "python",
        None,
        &[("step_handler", "Generate a python step handler", &[])],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(temp.path(), &["template", "list", "--language", "ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("ruby"), "Should show ruby templates");
}

// ==========================================================================
// Template Generation Tests
// ==========================================================================

#[test]
fn test_template_generate_produces_output_files() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[parameters]]
name = "module_name"
description = "Module namespace"
required = false
default = "Handlers"

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"
"#;

    let handler_tera = r#"# frozen_string_literal: true

module {{ module_name }}
  class {{ name | pascal_case }}Handler
    def call(context)
      # Handler logic for {{ name | snake_case }}
    end
  end
end
"#;

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        Some("rails"),
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.rb.tera", handler_tera),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(
        temp.path(),
        &[
            "template",
            "generate",
            "step_handler",
            "--language",
            "ruby",
            "--param",
            "name=ProcessPayment",
            "--output",
            output_dir.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "template generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the output file was created with correct name
    let generated_file = output_dir.join("process_payment_handler.rb");
    assert!(
        generated_file.exists(),
        "Generated file should exist at: {}",
        generated_file.display()
    );

    // Verify file content has correct class name and module
    let content = fs::read_to_string(&generated_file).unwrap();
    assert!(
        content.contains("ProcessPaymentHandler"),
        "Should contain PascalCase class name"
    );
    assert!(
        content.contains("module Handlers"),
        "Should contain default module name"
    );
    assert!(
        content.contains("process_payment"),
        "Should contain snake_case handler reference"
    );
}

#[test]
fn test_template_generate_with_explicit_module_name() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[parameters]]
name = "module_name"
description = "Module namespace"
required = false
default = "Handlers"

[[outputs]]
template = "handler.py.tera"
filename = "{{ name | snake_case }}_handler.py"
"#;

    let handler_tera = r#"from tasker_core import StepHandler

class {{ name | pascal_case }}Handler(StepHandler):
    handler_name = "{{ name | snake_case }}"

    def call(self, context):
        pass
"#;

    create_test_plugin(
        &plugins_dir,
        "test-python",
        "python",
        None,
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.py.tera", handler_tera),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(
        temp.path(),
        &[
            "template",
            "generate",
            "step_handler",
            "--language",
            "python",
            "--param",
            "name=ValidateInput",
            "--param",
            "module_name=my_app.handlers",
            "--output",
            output_dir.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "template generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated_file = output_dir.join("validate_input_handler.py");
    assert!(generated_file.exists());

    let content = fs::read_to_string(&generated_file).unwrap();
    assert!(content.contains("ValidateInputHandler"));
    assert!(content.contains("validate_input"));
}

#[test]
fn test_template_generate_with_subdirectory_outputs() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler with test"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.ts.tera"
filename = "{{ name | snake_case }}-handler.ts"

[[outputs]]
template = "handler.test.ts.tera"
filename = "{{ name | snake_case }}-handler.test.ts"
subdir = "__tests__"
"#;

    let handler_tera = r#"export class {{ name | pascal_case }}Handler {
  async call(context: any) {}
}
"#;

    let test_tera = r#"import { {{ name | pascal_case }}Handler } from '../{{ name | snake_case }}-handler';

describe('{{ name | pascal_case }}Handler', () => {
  it('works', () => {});
});
"#;

    create_test_plugin(
        &plugins_dir,
        "test-ts",
        "typescript",
        None,
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.ts.tera", handler_tera),
                ("handler.test.ts.tera", test_tera),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(
        temp.path(),
        &[
            "template",
            "generate",
            "step_handler",
            "--language",
            "typescript",
            "--param",
            "name=SendEmail",
            "--output",
            output_dir.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "template generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Handler file in root output
    let handler_file = output_dir.join("send_email-handler.ts");
    assert!(handler_file.exists(), "Handler file should exist");

    // Test file in __tests__ subdirectory
    let test_file = output_dir
        .join("__tests__")
        .join("send_email-handler.test.ts");
    assert!(
        test_file.exists(),
        "Test file should exist in __tests__ subdirectory"
    );

    let test_content = fs::read_to_string(&test_file).unwrap();
    assert!(test_content.contains("SendEmailHandler"));
}

// ==========================================================================
// Template Info Tests
// ==========================================================================

#[test]
fn test_template_info_shows_parameters() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[parameters]]
name = "module_name"
description = "Module namespace"
required = false
default = "Handlers"

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"
"#;

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        Some("rails"),
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.rb.tera", "{{ name }}"),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    let output = run_tasker_ctl(
        temp.path(),
        &["template", "info", "step_handler", "--plugin", "test-ruby"],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("name"), "Should show 'name' parameter");
    assert!(
        stdout.contains("module_name"),
        "Should show 'module_name' parameter"
    );
}

// ==========================================================================
// Error Handling Tests
// ==========================================================================

#[test]
fn test_template_generate_fails_on_missing_required_param() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"
"#;

    create_test_plugin(
        &plugins_dir,
        "test-ruby",
        "ruby",
        None,
        &[(
            "step_handler",
            "Generate a step handler",
            &[
                ("template.toml", template_toml),
                ("handler.rb.tera", "{{ name }}"),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    // Don't pass the required 'name' parameter
    let output = run_tasker_ctl(
        temp.path(),
        &[
            "template",
            "generate",
            "step_handler",
            "--language",
            "ruby",
            "--output",
            output_dir.to_str().unwrap(),
        ],
    );

    assert!(
        !output.status.success(),
        "Should fail when required param is missing"
    );
}

#[test]
fn test_plugin_validate_fails_for_invalid_manifest() {
    let temp = TempDir::new().unwrap();
    let plugin_dir = temp.path().join("bad-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    // Write invalid manifest (old schema format)
    fs::write(
        plugin_dir.join("tasker-plugin.toml"),
        r#"
[plugin]
name = "bad"
languages = ["ruby"]

[templates]
step-handler = { path = "templates/step_handler" }
"#,
    )
    .unwrap();

    let output = run_tasker_ctl(
        temp.path(),
        &["plugin", "validate", plugin_dir.to_str().unwrap()],
    );

    assert!(
        !output.status.success(),
        "Should fail validation for manifest with old schema"
    );
}

// ==========================================================================
// Multi-plugin Template Selection Tests
// ==========================================================================

#[test]
fn test_template_generate_selects_by_plugin_name() {
    let temp = TempDir::new().unwrap();
    let plugins_dir = temp.path().join("plugins");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let ruby_template_toml = r#"name = "step_handler"
description = "Ruby step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"
"#;

    let python_template_toml = r#"name = "step_handler"
description = "Python step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.py.tera"
filename = "{{ name | snake_case }}_handler.py"
"#;

    create_test_plugin(
        &plugins_dir,
        "ruby-plugin",
        "ruby",
        None,
        &[(
            "step_handler",
            "Ruby step handler",
            &[
                ("template.toml", ruby_template_toml),
                (
                    "handler.rb.tera",
                    "class {{ name | pascal_case }}Handler; end",
                ),
            ],
        )],
    );

    create_test_plugin(
        &plugins_dir,
        "python-plugin",
        "python",
        None,
        &[(
            "step_handler",
            "Python step handler",
            &[
                ("template.toml", python_template_toml),
                (
                    "handler.py.tera",
                    "class {{ name | pascal_case }}Handler: pass",
                ),
            ],
        )],
    );

    create_cli_config(temp.path(), &[plugins_dir.to_str().unwrap()]);

    // Generate using explicit plugin selection
    let output = run_tasker_ctl(
        temp.path(),
        &[
            "template",
            "generate",
            "step_handler",
            "--plugin",
            "python-plugin",
            "--param",
            "name=FetchData",
            "--output",
            output_dir.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "template generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should generate Python file, not Ruby
    let py_file = output_dir.join("fetch_data_handler.py");
    assert!(py_file.exists(), "Should generate Python handler file");

    let rb_file = output_dir.join("fetch_data_handler.rb");
    assert!(!rb_file.exists(), "Should NOT generate Ruby handler file");
}
