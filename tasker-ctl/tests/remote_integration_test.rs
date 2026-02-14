//! Integration tests for remote repository template and config fetching (TAS-270).
//!
//! Uses `file://` URLs pointing to local bare git repos in temp directories,
//! so no network access is required.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the compiled tasker-ctl binary.
fn tasker_ctl_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tasker-ctl"))
}

/// Run tasker-ctl with the given args from a working directory.
fn run_tasker_ctl(work_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(tasker_ctl_bin())
        .args(args)
        .current_dir(work_dir)
        .env("HOME", work_dir.to_str().unwrap()) // Isolate cache directory
        .output()
        .expect("Failed to execute tasker-ctl")
}

/// Create a bare git repo from a directory of files.
/// Returns the path to the bare repo (suitable for file:// URL).
fn create_bare_git_repo(source_dir: &Path, bare_dir: &Path) {
    // Initialize a regular repo, add files, commit
    let work_dir = source_dir.parent().unwrap().join("_work_repo");
    fs::create_dir_all(&work_dir).unwrap();

    run_git(&work_dir, &["init"]);
    run_git(&work_dir, &["config", "user.email", "test@test.com"]);
    run_git(&work_dir, &["config", "user.name", "Test"]);

    // Copy source files into work repo
    copy_dir_recursive(source_dir, &work_dir);

    run_git(&work_dir, &["add", "."]);
    run_git(&work_dir, &["commit", "-m", "initial"]);

    // Clone to bare repo
    Command::new("git")
        .args(["clone", "--bare"])
        .arg(&work_dir)
        .arg(bare_dir)
        .output()
        .expect("Failed to create bare repo");

    // Clean up work dir
    fs::remove_dir_all(&work_dir).unwrap();
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command failed");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path).unwrap();
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

/// Create a plugin structure suitable for a remote repo.
fn create_remote_plugin_source(base: &Path, plugin_name: &str, language: &str) {
    let plugin_dir = base.join(plugin_name).join("tasker-cli-plugin");
    let template_dir = plugin_dir.join("templates").join("step_handler");
    fs::create_dir_all(&template_dir).unwrap();

    let manifest = format!(
        r#"[plugin]
name = "{plugin_name}"
version = "0.1.0"
description = "Remote test plugin for {language}"
language = "{language}"

[[templates]]
name = "step_handler"
path = "templates/step_handler"
description = "Generate a step handler"
"#
    );
    fs::write(plugin_dir.join("tasker-plugin.toml"), manifest).unwrap();

    let template_toml = r#"name = "step_handler"
description = "Generate a step handler"

[[parameters]]
name = "name"
description = "Handler name"
required = true

[[outputs]]
template = "handler.txt.tera"
filename = "{{ name | snake_case }}_handler.txt"
"#;
    fs::write(template_dir.join("template.toml"), template_toml).unwrap();
    fs::write(
        template_dir.join("handler.txt.tera"),
        "Handler: {{ name | pascal_case }}Handler\n",
    )
    .unwrap();
}

/// Create a .tasker-ctl.toml with a remote pointing to a file:// URL.
fn create_cli_config_with_remote(dir: &Path, name: &str, bare_repo: &Path) {
    let url = format!("file://{}", bare_repo.display());
    let config = format!(
        r#"[[remotes]]
name = "{name}"
url = "{url}"
git-ref = "main"
"#
    );
    fs::write(dir.join(".tasker-ctl.toml"), config).unwrap();
}

// ==========================================================================
// Config Parsing Tests (via unit tests in cli_config::loader)
// Config parsing is tested in the unit test module. Integration tests
// below focus on end-to-end CLI behavior.
// ==========================================================================

// ==========================================================================
// Remote Clone & Cache Tests
// ==========================================================================

#[test]
fn test_remote_clone_and_cache() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    // Create a plugin source and bare repo
    create_remote_plugin_source(&source, "test-remote-ruby", "ruby");
    create_bare_git_repo(&source, &bare);

    // Create config
    create_cli_config_with_remote(&work, "test-remote", &bare);

    // Run remote update to clone
    let output = run_tasker_ctl(&work, &["remote", "update", "test-remote"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "remote update failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Updated"),
        "Should show update success message"
    );

    // Verify cache directory was created
    let cache_dir = work
        .join(".cache")
        .join("tasker-ctl")
        .join("remotes")
        .join("test-remote");
    assert!(cache_dir.exists(), "Cache directory should exist");
    assert!(
        cache_dir.join(".tasker-last-fetch").exists(),
        "Last fetch timestamp should exist"
    );
}

#[test]
fn test_remote_list_shows_configured_remotes() {
    let temp = TempDir::new().unwrap();
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // Create minimal bare repo (just needs to exist for list)
    let source = temp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("README.md"), "test").unwrap();
    create_bare_git_repo(&source, &bare);

    create_cli_config_with_remote(&work, "my-remote", &bare);

    let output = run_tasker_ctl(&work, &["remote", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("my-remote"),
        "Should list configured remote"
    );
    assert!(
        stdout.contains("not cached"),
        "Should show not cached status"
    );
}

#[test]
fn test_remote_list_shows_cached_status_after_update() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    fs::write(source.join("README.md"), "test").unwrap();
    create_bare_git_repo(&source, &bare);
    create_cli_config_with_remote(&work, "cached-remote", &bare);

    // First update to populate cache
    run_tasker_ctl(&work, &["remote", "update", "cached-remote"]);

    // Now list should show cached
    let output = run_tasker_ctl(&work, &["remote", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("cached"),
        "Should show cached status: {}",
        stdout
    );
}

// ==========================================================================
// Remote Add/Remove Tests
// ==========================================================================

#[test]
fn test_remote_add_creates_config_entry() {
    let temp = TempDir::new().unwrap();
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // Start with empty config
    fs::write(work.join(".tasker-ctl.toml"), "").unwrap();

    let output = run_tasker_ctl(
        &work,
        &[
            "remote",
            "add",
            "new-remote",
            "https://github.com/example/repo.git",
        ],
    );
    assert!(
        output.status.success(),
        "remote add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify config was written
    let config_content = fs::read_to_string(work.join(".tasker-ctl.toml")).unwrap();
    assert!(
        config_content.contains("new-remote"),
        "Config should contain remote name"
    );
    assert!(
        config_content.contains("example/repo.git"),
        "Config should contain URL"
    );
}

#[test]
fn test_remote_remove_cleans_up() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    fs::write(source.join("README.md"), "test").unwrap();
    create_bare_git_repo(&source, &bare);
    create_cli_config_with_remote(&work, "removable", &bare);

    // Update to create cache
    run_tasker_ctl(&work, &["remote", "update", "removable"]);

    // Remove
    let output = run_tasker_ctl(&work, &["remote", "remove", "removable"]);
    assert!(
        output.status.success(),
        "remote remove failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify removed from config
    let config_content = fs::read_to_string(work.join(".tasker-ctl.toml")).unwrap();
    assert!(
        !config_content.contains("removable"),
        "Config should not contain removed remote"
    );

    // Verify cache removed
    let cache_dir = work
        .join(".cache")
        .join("tasker-ctl")
        .join("remotes")
        .join("removable");
    assert!(!cache_dir.exists(), "Cache directory should be removed");
}

// ==========================================================================
// Template from Remote Tests
// ==========================================================================

#[test]
fn test_template_list_from_remote() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    create_remote_plugin_source(&source, "remote-ruby", "ruby");
    create_bare_git_repo(&source, &bare);
    create_cli_config_with_remote(&work, "template-remote", &bare);

    let output = run_tasker_ctl(&work, &["template", "list", "--remote", "template-remote"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "template list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("step_handler"),
        "Should find step_handler template from remote: {}",
        stdout
    );
}

#[test]
fn test_template_generate_from_remote() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    let output_dir = temp.path().join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    create_remote_plugin_source(&source, "remote-ruby", "ruby");
    create_bare_git_repo(&source, &bare);
    create_cli_config_with_remote(&work, "gen-remote", &bare);

    let output = run_tasker_ctl(
        &work,
        &[
            "template",
            "generate",
            "step_handler",
            "--remote",
            "gen-remote",
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

    let generated_file = output_dir.join("process_payment_handler.txt");
    assert!(
        generated_file.exists(),
        "Generated file should exist: {}",
        generated_file.display()
    );

    let content = fs::read_to_string(&generated_file).unwrap();
    assert!(
        content.contains("ProcessPaymentHandler"),
        "Should contain PascalCase handler name"
    );
}

// ==========================================================================
// Auto-discovery Tests (Phase 5)
// ==========================================================================

#[test]
fn test_plugin_list_auto_discovers_remotes() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    create_remote_plugin_source(&source, "auto-ruby", "ruby");
    create_bare_git_repo(&source, &bare);
    create_cli_config_with_remote(&work, "auto-remote", &bare);

    // plugin list (no --remote flag) should auto-discover from configured remotes
    let output = run_tasker_ctl(&work, &["plugin", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "plugin list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("auto-ruby"),
        "Should auto-discover plugin from remote: {}",
        stdout
    );
}

// ==========================================================================
// Init Command Tests
// ==========================================================================

#[test]
fn test_init_creates_default_config() {
    let temp = TempDir::new().unwrap();
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    let output = run_tasker_ctl(&work, &["init"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Created .tasker-ctl.toml"),
        "Should show success message: {}",
        stdout
    );

    let config_path = work.join(".tasker-ctl.toml");
    assert!(config_path.exists(), ".tasker-ctl.toml should be created");

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("tasker-contrib"),
        "Should contain tasker-contrib remote: {}",
        content
    );
    assert!(
        content.contains("[[remotes]]"),
        "Should have remotes section: {}",
        content
    );
    assert!(
        content.contains("https://github.com/tasker-systems/tasker-contrib.git"),
        "Should contain tasker-contrib URL: {}",
        content
    );
}

#[test]
fn test_init_no_contrib_flag() {
    let temp = TempDir::new().unwrap();
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    let output = run_tasker_ctl(&work, &["init", "--no-contrib"]);
    assert!(
        output.status.success(),
        "init --no-contrib failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_path = work.join(".tasker-ctl.toml");
    assert!(config_path.exists(), ".tasker-ctl.toml should be created");

    let content = fs::read_to_string(&config_path).unwrap();
    // The remotes section should be commented out
    assert!(
        !content.contains("\n[[remotes]]"),
        "Should not have active remotes section: {}",
        content
    );
    assert!(
        content.contains("# [[remotes]]"),
        "Should have commented remotes section: {}",
        content
    );
}

#[test]
fn test_init_refuses_to_overwrite() {
    let temp = TempDir::new().unwrap();
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    // Create existing config with custom content
    let original_content = "# my custom config\nplugin-paths = [\"./my-plugins\"]\n";
    fs::write(work.join(".tasker-ctl.toml"), original_content).unwrap();

    let output = run_tasker_ctl(&work, &["init"]);
    assert!(
        !output.status.success(),
        "init should fail when config already exists"
    );

    // Verify original content preserved
    let content = fs::read_to_string(work.join(".tasker-ctl.toml")).unwrap();
    assert_eq!(
        content, original_content,
        "Original config should be preserved"
    );
}

// ==========================================================================
// Ad-hoc URL Tests
// ==========================================================================

#[test]
fn test_template_list_from_url() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let bare = temp.path().join("bare.git");
    let work = temp.path().join("work");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    create_remote_plugin_source(&source, "url-ruby", "ruby");
    create_bare_git_repo(&source, &bare);

    // Create minimal config (no remotes)
    fs::write(work.join(".tasker-ctl.toml"), "").unwrap();

    let url = format!("file://{}", bare.display());
    let output = run_tasker_ctl(&work, &["template", "list", "--url", &url]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "template list --url failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("step_handler"),
        "Should find template from ad-hoc URL: {}",
        stdout
    );
}
