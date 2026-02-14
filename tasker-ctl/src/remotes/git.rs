//! Git operations for remote repository cloning and fetching.
//!
//! Uses the `git` CLI via `std::process::Command` for reliability:
//! - Full support for shallow clones, ref checkout, and credential helpers
//! - The `gix` crate's checkout API is still incomplete for arbitrary refs
//! - `git` is a reasonable dependency for a tool that manages git remotes

use std::path::Path;
use std::process::Command;

use super::RemoteError;

/// Shallow-clone a repository to a target directory, checking out a specific ref.
pub(crate) fn shallow_clone(url: &str, git_ref: &str, target: &Path) -> Result<(), RemoteError> {
    let output = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            git_ref,
            "--single-branch",
            url,
        ])
        .arg(target)
        .output()
        .map_err(|e| RemoteError::GitError {
            url: url.to_string(),
            source: Box::new(e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RemoteError::GitError {
            url: url.to_string(),
            source: stderr.to_string().into(),
        });
    }

    Ok(())
}

/// Fetch latest changes and checkout a ref in an existing cloned repository.
pub(crate) fn fetch_and_checkout(repo_dir: &Path, git_ref: &str) -> Result<(), RemoteError> {
    let url = repo_dir.display().to_string();

    // Fetch from origin with the specific ref
    let output = Command::new("git")
        .args(["fetch", "--depth", "1", "origin", git_ref])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| RemoteError::GitError {
            url: url.clone(),
            source: Box::new(e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RemoteError::GitError {
            url: url.clone(),
            source: format!("fetch failed: {}", stderr).into(),
        });
    }

    // Checkout FETCH_HEAD to update the working tree
    let output = Command::new("git")
        .args(["checkout", "FETCH_HEAD"])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| RemoteError::GitError {
            url: url.clone(),
            source: Box::new(e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RemoteError::GitError {
            url,
            source: format!("checkout failed: {}", stderr).into(),
        });
    }

    Ok(())
}

/// Check if git is available on the system.
pub(crate) fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}
