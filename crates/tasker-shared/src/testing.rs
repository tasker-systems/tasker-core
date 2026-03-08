use std::path::{Path, PathBuf};

/// Returns the workspace root directory by walking up from `CARGO_MANIFEST_DIR`
/// until finding a `Cargo.toml` that contains `[workspace]`.
///
/// This is robust regardless of crate depth within the workspace.
///
/// # Panics
///
/// Panics if `CARGO_MANIFEST_DIR` is not set or no workspace root is found.
pub fn workspace_root() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by cargo");
    let mut dir = Path::new(&manifest_dir);
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return dir.to_path_buf();
                }
            }
        }
        dir = dir
            .parent()
            .unwrap_or_else(|| panic!("workspace root not found from {manifest_dir}"));
    }
}

/// Returns a path relative to the workspace root.
///
/// Convenience wrapper for `workspace_root().join(relative_path)`.
pub fn workspace_path(relative_path: &str) -> PathBuf {
    workspace_root().join(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_root_finds_root() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("crates").is_dir());
        assert!(root.join("proto").is_dir());
    }

    #[test]
    fn test_workspace_path_resolves() {
        let path = workspace_path("tests/fixtures");
        assert!(path.exists());
    }
}
