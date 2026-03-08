//! Plugin discovery via smart 2-level path scanning.
//!
//! For each configured path, checks three locations:
//! 1. The path itself (contains `tasker-plugin.toml`)
//! 2. Immediate subdirectories (`<path>/rails/tasker-plugin.toml`)
//! 3. Nested plugin directories (`<path>/rails/tasker-cli-plugin/tasker-plugin.toml`)

use std::path::{Path, PathBuf};

const MANIFEST_FILENAME: &str = "tasker-plugin.toml";
const PLUGIN_SUBDIR: &str = "tasker-cli-plugin";

/// Discover all directories containing a `tasker-plugin.toml` under the given search paths.
pub(crate) fn discover_plugin_dirs(search_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();

    for base in search_paths {
        if !base.is_dir() {
            tracing::debug!(?base, "Plugin search path does not exist, skipping");
            continue;
        }

        // Level 0: path itself
        if has_manifest(base) {
            found.push(base.clone());
            continue;
        }

        // Scan immediate subdirectories
        let entries = match std::fs::read_dir(base) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::debug!(?base, error = %e, "Cannot read plugin search path");
                continue;
            }
        };

        for entry in entries.filter_map(Result::ok) {
            let subdir = entry.path();
            if !subdir.is_dir() {
                continue;
            }

            // Level 1: immediate subdir (e.g., rails/tasker-plugin.toml)
            if has_manifest(&subdir) {
                found.push(subdir.clone());
                continue;
            }

            // Level 2: nested plugin subdir (e.g., rails/tasker-cli-plugin/tasker-plugin.toml)
            let nested = subdir.join(PLUGIN_SUBDIR);
            if nested.is_dir() && has_manifest(&nested) {
                found.push(nested);
            }
        }
    }

    found
}

fn has_manifest(dir: &Path) -> bool {
    dir.join(MANIFEST_FILENAME).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_manifest(dir: &Path, name: &str) {
        let content = format!(
            r#"
[plugin]
name = "{name}"
description = "Test plugin"
version = "0.1.0"
language = "rust"
"#
        );
        fs::write(dir.join(MANIFEST_FILENAME), content).unwrap();
    }

    #[test]
    fn test_discover_root_manifest() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), "root-plugin");

        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], dir.path());
    }

    #[test]
    fn test_discover_immediate_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let rails = dir.path().join("rails");
        fs::create_dir(&rails).unwrap();
        write_manifest(&rails, "rails");

        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], rails);
    }

    #[test]
    fn test_discover_nested_plugin_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("rails").join("tasker-cli-plugin");
        fs::create_dir_all(&nested).unwrap();
        write_manifest(&nested, "rails");

        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], nested);
    }

    #[test]
    fn test_discover_multiple_plugins() {
        let dir = tempfile::tempdir().unwrap();

        // Plugin at immediate subdir level
        let rails = dir.path().join("rails");
        fs::create_dir(&rails).unwrap();
        write_manifest(&rails, "rails");

        // Plugin at nested level
        let django_nested = dir.path().join("django").join("tasker-cli-plugin");
        fs::create_dir_all(&django_nested).unwrap();
        write_manifest(&django_nested, "django");

        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_discover_nonexistent_path() {
        let found = discover_plugin_dirs(&[PathBuf::from("/nonexistent/path")]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_skips_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create a file (not a directory) in the search path
        fs::write(dir.path().join("not-a-plugin.txt"), "hello").unwrap();
        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_multiple_search_paths() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();

        let plugin1 = dir1.path().join("plugin-a");
        fs::create_dir(&plugin1).unwrap();
        write_manifest(&plugin1, "plugin-a");

        let plugin2 = dir2.path().join("plugin-b");
        fs::create_dir(&plugin2).unwrap();
        write_manifest(&plugin2, "plugin-b");

        let found = discover_plugin_dirs(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert_eq!(found.len(), 2);
    }
}
