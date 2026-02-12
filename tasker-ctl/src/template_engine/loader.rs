//! Load `.tera` template files from a plugin's template directory.

use std::path::Path;

use tera::Tera;

/// Load all `.tera` files from a template directory into a Tera instance.
///
/// The templates are registered by their filename (e.g., `handler.rb.tera`).
pub(crate) fn load_templates_from_dir(template_dir: &Path) -> Result<Tera, LoaderError> {
    let glob_pattern = template_dir.join("*.tera").to_string_lossy().to_string();

    Tera::new(&glob_pattern).map_err(|e| LoaderError::Tera {
        dir: template_dir.to_path_buf(),
        source: e,
    })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum LoaderError {
    #[error("failed to load templates from {dir}: {source}")]
    Tera {
        dir: std::path::PathBuf,
        source: tera::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_templates() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("handler.rb.tera"),
            "class {{ name | pascal_case }}Handler\nend\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("handler_spec.rb.tera"),
            "describe {{ name | pascal_case }}Handler do\nend\n",
        )
        .unwrap();

        let tera = load_templates_from_dir(dir.path()).unwrap();
        let names: Vec<_> = tera.get_template_names().collect();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_load_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let tera = load_templates_from_dir(dir.path()).unwrap();
        assert_eq!(tera.get_template_names().count(), 0);
    }

    #[test]
    fn test_ignores_non_tera_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("handler.rb.tera"), "{{ name }}").unwrap();
        fs::write(dir.path().join("README.md"), "# Not a template").unwrap();
        fs::write(dir.path().join("template.toml"), "name = 'test'").unwrap();

        let tera = load_templates_from_dir(dir.path()).unwrap();
        assert_eq!(tera.get_template_names().count(), 1);
    }
}
