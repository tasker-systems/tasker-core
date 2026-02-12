//! Tera-based template rendering engine with custom filter registration.

use std::collections::HashMap;
use std::path::Path;

use tera::{Context, Tera};

use super::filters;
use super::loader::load_templates_from_dir;
use super::metadata::{MetadataError, TemplateMetadata};

/// Template engine wrapping Tera with plugin-specific context.
#[derive(Debug)]
pub(crate) struct TemplateEngine {
    tera: Tera,
    metadata: TemplateMetadata,
}

/// A rendered output file ready to be written to disk.
#[derive(Debug)]
pub(crate) struct RenderedFile {
    /// Relative path for the output file.
    pub path: String,
    /// Rendered content.
    pub content: String,
}

impl TemplateEngine {
    /// Load a template engine from a template directory.
    pub fn load(template_dir: &Path) -> Result<Self, EngineError> {
        let metadata = TemplateMetadata::load(template_dir).map_err(EngineError::Metadata)?;
        let mut tera =
            load_templates_from_dir(template_dir).map_err(|e| EngineError::Load(e.to_string()))?;

        // Register custom filters
        tera.register_filter("snake_case", filters::snake_case);
        tera.register_filter("pascal_case", filters::pascal_case);
        tera.register_filter("camel_case", filters::camel_case);
        tera.register_filter("kebab_case", filters::kebab_case);

        Ok(Self { tera, metadata })
    }

    /// Get template metadata.
    pub fn metadata(&self) -> &TemplateMetadata {
        &self.metadata
    }

    /// Render all output files using the provided parameters.
    pub fn render(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Vec<RenderedFile>, EngineError> {
        // Build Tera context from params
        let mut context = Context::new();
        for (key, value) in params {
            context.insert(key, value);
        }

        // Fill in defaults for missing optional params
        for p in &self.metadata.parameters {
            if !params.contains_key(&p.name) {
                if let Some(default) = &p.default {
                    context.insert(&p.name, default);
                }
            }
        }

        let mut rendered = Vec::new();

        for output in &self.metadata.outputs {
            let content =
                self.tera
                    .render(&output.template, &context)
                    .map_err(|e| EngineError::Render {
                        template: output.template.clone(),
                        source: e,
                    })?;

            // Render the filename pattern (may contain Tera expressions)
            let filename = self.render_string(&output.filename, &context)?;

            let path = match &output.subdir {
                Some(subdir) => {
                    let rendered_subdir = self.render_string(subdir, &context)?;
                    format!("{rendered_subdir}/{filename}")
                }
                None => filename,
            };

            rendered.push(RenderedFile { path, content });
        }

        Ok(rendered)
    }

    fn render_string(&self, template_str: &str, context: &Context) -> Result<String, EngineError> {
        // Use a cloned Tera instance so custom filters are available for inline rendering
        let mut inline = self.tera.clone();
        inline
            .add_raw_template("__inline__", template_str)
            .map_err(|e| EngineError::Render {
                template: template_str.to_string(),
                source: e,
            })?;
        inline
            .render("__inline__", context)
            .map_err(|e| EngineError::Render {
                template: template_str.to_string(),
                source: e,
            })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum EngineError {
    #[error("metadata error: {0}")]
    Metadata(#[from] MetadataError),
    #[error("template load error: {0}")]
    Load(String),
    #[error("render error for '{template}': {source}")]
    Render {
        template: String,
        source: tera::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_template_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();

        let metadata = r#"
name = "step-handler"
description = "Generate a Tasker step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[parameters]]
name = "handler_type"
description = "Handler type"
required = false
default = "generic"

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"

[[outputs]]
template = "handler_spec.rb.tera"
filename = "{{ name | snake_case }}_handler_spec.rb"
subdir = "spec"
"#;
        fs::write(dir.path().join("template.toml"), metadata).unwrap();

        let handler_template = r#"# frozen_string_literal: true

class {{ name | pascal_case }}Handler < Tasker::StepHandler
  # Handler type: {{ handler_type }}

  def process(context)
    # TODO: Implement {{ name | snake_case }} logic
  end
end
"#;
        fs::write(dir.path().join("handler.rb.tera"), handler_template).unwrap();

        let spec_template = r#"# frozen_string_literal: true

RSpec.describe {{ name | pascal_case }}Handler do
  describe '#process' do
    it 'processes {{ name | snake_case }} step' do
      # TODO: Add test
    end
  end
end
"#;
        fs::write(dir.path().join("handler_spec.rb.tera"), spec_template).unwrap();

        dir
    }

    #[test]
    fn test_engine_load() {
        let dir = setup_template_dir();
        let engine = TemplateEngine::load(dir.path()).unwrap();
        assert_eq!(engine.metadata().name, "step-handler");
        assert_eq!(engine.metadata().parameters.len(), 2);
    }

    #[test]
    fn test_engine_render() {
        let dir = setup_template_dir();
        let engine = TemplateEngine::load(dir.path()).unwrap();

        let mut params = HashMap::new();
        params.insert("name".to_string(), "ProcessPayment".to_string());

        let rendered = engine.render(&params).unwrap();
        assert_eq!(rendered.len(), 2);

        // Check handler file
        assert_eq!(rendered[0].path, "process_payment_handler.rb");
        assert!(rendered[0].content.contains("class ProcessPaymentHandler"));
        assert!(rendered[0].content.contains("Handler type: generic")); // default applied

        // Check spec file
        assert_eq!(rendered[1].path, "spec/process_payment_handler_spec.rb");
        assert!(rendered[1]
            .content
            .contains("RSpec.describe ProcessPaymentHandler"));
    }

    #[test]
    fn test_engine_render_with_explicit_params() {
        let dir = setup_template_dir();
        let engine = TemplateEngine::load(dir.path()).unwrap();

        let mut params = HashMap::new();
        params.insert("name".to_string(), "SendEmail".to_string());
        params.insert("handler_type".to_string(), "api".to_string());

        let rendered = engine.render(&params).unwrap();
        assert!(rendered[0].content.contains("Handler type: api"));
    }

    #[test]
    fn test_engine_missing_template_file() {
        let dir = tempfile::tempdir().unwrap();
        let metadata = r#"
name = "broken"
description = "Missing template file"
[[outputs]]
template = "nonexistent.tera"
filename = "out.txt"
"#;
        fs::write(dir.path().join("template.toml"), metadata).unwrap();

        let engine = TemplateEngine::load(dir.path()).unwrap();
        let result = engine.render(&HashMap::new());
        assert!(result.is_err());
    }
}
