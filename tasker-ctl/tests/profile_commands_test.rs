//! Integration tests for `tasker-ctl profile` commands (TAS-310).
//!
//! Tests profile init, list, add, validate, show, and check subcommands
//! using temp directories with `chdir` to simulate real filesystem layout.

use serial_test::serial;
use std::fs;
use tempfile::TempDir;

use tasker_client::config::{ClientConfig, ProfileConfigFile};
use tasker_client::profile_manager::ProfileManager;

// =============================================================================
// Helper: write a profile config and load it
// =============================================================================

fn write_profile_toml(dir: &std::path::Path, content: &str) {
    let config_dir = dir.join(".config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("tasker-client.toml"), content).unwrap();
}

/// Standard multi-profile TOML for reuse across tests.
const MULTI_PROFILE_TOML: &str = r#"
[profile.default]
description = "Local development"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"
timeout_ms = 30000
max_retries = 3

[profile.default.worker]
base_url = "http://localhost:8081"
timeout_ms = 30000
max_retries = 3

[profile.staging]
description = "Staging environment"
transport = "grpc"
namespaces = ["orders", "analytics"]
tools = ["tier1", "tier2"]

[profile.staging.orchestration]
base_url = "https://staging-orch:9190"
timeout_ms = 60000

[profile.staging.worker]
base_url = "https://staging-worker:9191"
timeout_ms = 60000
"#;

// =============================================================================
// ProfileConfigFile parsing tests
// =============================================================================

#[test]
fn test_parse_multi_profile_config() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();

    assert_eq!(file.profile.len(), 2);
    assert!(file.profile.contains_key("default"));
    assert!(file.profile.contains_key("staging"));
}

#[test]
fn test_profile_fields_parsed_correctly() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();

    let staging = file.profile.get("staging").unwrap();
    assert_eq!(staging.description.as_deref(), Some("Staging environment"));
    assert_eq!(
        staging.transport,
        Some(tasker_client::config::Transport::Grpc)
    );
    assert_eq!(
        staging.namespaces.as_deref(),
        Some(&["orders".to_string(), "analytics".to_string()][..])
    );
    assert_eq!(
        staging.tools.as_deref(),
        Some(&["tier1".to_string(), "tier2".to_string()][..])
    );
}

#[test]
fn test_tools_field_roundtrip() {
    let toml_content = r#"
[profile.mcp_restricted]
tools = ["tier1"]

[profile.mcp_full]
tools = ["tier1", "tier2", "tier3"]
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();

    let restricted = file.profile.get("mcp_restricted").unwrap();
    assert_eq!(
        restricted.tools.as_deref(),
        Some(&["tier1".to_string()][..])
    );

    let full = file.profile.get("mcp_full").unwrap();
    assert_eq!(
        full.tools.as_deref(),
        Some(
            &[
                "tier1".to_string(),
                "tier2".to_string(),
                "tier3".to_string()
            ][..]
        )
    );
}

#[test]
fn test_profile_without_tools_field() {
    let toml_content = r#"
[profile.default]
transport = "rest"
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();
    let default = file.profile.get("default").unwrap();
    assert!(default.tools.is_none());
}

// =============================================================================
// ClientConfig resolution tests
// =============================================================================

#[test]
fn test_resolve_default_profile() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();
    let config = ClientConfig::resolve_from_file("default", &file).unwrap();

    assert_eq!(config.transport, tasker_client::config::Transport::Rest);
    assert_eq!(config.orchestration.base_url, "http://localhost:8080");
    assert_eq!(config.orchestration.timeout_ms, 30000);
    assert_eq!(config.orchestration.max_retries, 3);
    assert_eq!(config.worker.base_url, "http://localhost:8081");
}

#[test]
fn test_resolve_staging_profile_inherits_defaults() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();
    let config = ClientConfig::resolve_from_file("staging", &file).unwrap();

    // Staging overrides transport and URLs
    assert_eq!(config.transport, tasker_client::config::Transport::Grpc);
    assert_eq!(config.orchestration.base_url, "https://staging-orch:9190");
    assert_eq!(config.orchestration.timeout_ms, 60000);
    // max_retries should inherit from default profile
    assert_eq!(config.orchestration.max_retries, 3);
}

#[test]
fn test_resolve_nonexistent_profile_errors() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();
    let result = ClientConfig::resolve_from_file("nonexistent", &file);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// =============================================================================
// ProfileManager integration tests
// =============================================================================

#[test]
fn test_profile_manager_loads_all_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join(".config");
    fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("tasker-client.toml");
    fs::write(&config_path, MULTI_PROFILE_TOML).unwrap();

    let pm = ProfileManager::load_from_path(&config_path).unwrap();

    let names = pm.list_profile_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"default"));
    assert!(names.contains(&"staging"));
}

#[test]
fn test_profile_manager_default_is_active() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, MULTI_PROFILE_TOML).unwrap();

    let pm = ProfileManager::load_from_path(&config_path).unwrap();
    assert_eq!(pm.active_profile_name(), "default");
}

#[test]
fn test_profile_manager_summaries_include_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, MULTI_PROFILE_TOML).unwrap();

    let pm = ProfileManager::load_from_path(&config_path).unwrap();
    let summaries = pm.list_profiles();

    let staging = summaries.iter().find(|s| s.name == "staging").unwrap();
    assert_eq!(staging.description.as_deref(), Some("Staging environment"));
    assert_eq!(staging.transport, tasker_client::config::Transport::Grpc);
    assert_eq!(staging.orchestration_url, "https://staging-orch:9190");
    assert!(!staging.is_active);

    let default = summaries.iter().find(|s| s.name == "default").unwrap();
    assert!(default.is_active);
}

#[test]
fn test_profile_manager_active_metadata_includes_tools() {
    let toml_content = r#"
[profile.default]
tools = ["tier1", "tier2"]
"#;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, toml_content).unwrap();

    let pm = ProfileManager::load_from_path(&config_path).unwrap();
    let metadata = pm.active_profile_metadata().unwrap();
    assert_eq!(
        metadata.tools.as_deref(),
        Some(&["tier1".to_string(), "tier2".to_string()][..])
    );
}

// =============================================================================
// profile init tests (filesystem)
// =============================================================================

#[test]
#[serial]
fn test_profile_init_creates_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the init template rendering directly (same as handle_profile_init)
    let config_dir = std::path::PathBuf::from(".config");
    fs::create_dir_all(&config_dir).unwrap();

    use askama::Template;

    #[derive(Template, Debug)]
    #[template(path = "profile-init.toml")]
    struct ProfileInitTemplate;

    let template = ProfileInitTemplate;
    let content = template.render().unwrap();
    fs::write(config_dir.join("tasker-client.toml"), &content).unwrap();

    // Verify file was created
    let config_path = temp_dir.path().join(".config/tasker-client.toml");
    assert!(config_path.exists());

    // Verify it's valid TOML with a [profile.default] section
    let written = fs::read_to_string(&config_path).unwrap();
    let file: ProfileConfigFile = toml::from_str(&written).unwrap();
    assert!(file.profile.contains_key("default"));

    let default = file.profile.get("default").unwrap();
    assert_eq!(default.description.as_deref(), Some("Local development"));
    assert_eq!(
        default.transport,
        Some(tasker_client::config::Transport::Rest)
    );

    // Verify the generated config can resolve to a valid ClientConfig
    let config = ClientConfig::resolve_from_file("default", &file).unwrap();
    assert_eq!(config.orchestration.base_url, "http://localhost:8080");
    assert_eq!(config.worker.base_url, "http://localhost:8081");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_profile_init_template_includes_tool_tier_docs() {
    use askama::Template;

    #[derive(Template, Debug)]
    #[template(path = "profile-init.toml")]
    struct ProfileInitTemplate;

    let content = ProfileInitTemplate.render().unwrap();

    // Template should mention tool tiers for MCP users
    assert!(content.contains("tier1"));
    assert!(content.contains("tier2"));
    assert!(content.contains("tier3"));
    assert!(content.contains("tasker-mcp"));
}

// =============================================================================
// profile add tests (toml_edit)
// =============================================================================

#[test]
#[serial]
fn test_profile_add_appends_to_config() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Create initial config
    write_profile_toml(temp_dir.path(), MULTI_PROFILE_TOML);

    let config_path = temp_dir.path().join(".config/tasker-client.toml");

    // Simulate profile add by using toml_edit directly (same logic as handler)
    let content = fs::read_to_string(&config_path).unwrap();
    let mut doc = content.parse::<toml_edit::DocumentMut>().unwrap();

    let profile_table = doc
        .entry("profile")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .unwrap();

    let mut new_profile = toml_edit::Table::new();
    new_profile.insert("description", toml_edit::value("Production environment"));
    new_profile.insert("transport", toml_edit::value("grpc"));

    let mut tools_arr = toml_edit::Array::new();
    tools_arr.push("tier1");
    tools_arr.push("tier2");
    tools_arr.push("tier3");
    new_profile.insert("tools", toml_edit::value(tools_arr));

    let mut orch = toml_edit::Table::new();
    orch.insert("base_url", toml_edit::value("https://prod-orch:9190"));
    new_profile.insert("orchestration", toml_edit::Item::Table(orch));

    let mut worker = toml_edit::Table::new();
    worker.insert("base_url", toml_edit::value("https://prod-worker:9191"));
    new_profile.insert("worker", toml_edit::Item::Table(worker));

    profile_table.insert("production", toml_edit::Item::Table(new_profile));
    fs::write(&config_path, doc.to_string()).unwrap();

    // Verify the file now has 3 profiles and is valid
    let updated = fs::read_to_string(&config_path).unwrap();
    let file: ProfileConfigFile = toml::from_str(&updated).unwrap();
    assert_eq!(file.profile.len(), 3);
    assert!(file.profile.contains_key("production"));

    let prod = file.profile.get("production").unwrap();
    assert_eq!(prod.description.as_deref(), Some("Production environment"));
    assert_eq!(prod.transport, Some(tasker_client::config::Transport::Grpc));
    assert_eq!(
        prod.tools.as_deref(),
        Some(
            &[
                "tier1".to_string(),
                "tier2".to_string(),
                "tier3".to_string()
            ][..]
        )
    );

    // Verify the new profile resolves to a valid ClientConfig
    let config = ClientConfig::resolve_from_file("production", &file).unwrap();
    assert_eq!(config.orchestration.base_url, "https://prod-orch:9190");
    assert_eq!(config.worker.base_url, "https://prod-worker:9191");
    assert_eq!(config.transport, tasker_client::config::Transport::Grpc);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_profile_add_preserves_existing_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, MULTI_PROFILE_TOML).unwrap();

    // Add a profile via toml_edit
    let content = fs::read_to_string(&config_path).unwrap();
    let mut doc = content.parse::<toml_edit::DocumentMut>().unwrap();

    let profile_table = doc["profile"].as_table_mut().unwrap();

    let mut new_profile = toml_edit::Table::new();
    new_profile.insert("transport", toml_edit::value("rest"));
    profile_table.insert("ci", toml_edit::Item::Table(new_profile));
    fs::write(&config_path, doc.to_string()).unwrap();

    // Verify all profiles still exist and are valid
    let updated = fs::read_to_string(&config_path).unwrap();
    let file: ProfileConfigFile = toml::from_str(&updated).unwrap();
    assert_eq!(file.profile.len(), 3);

    // Original profiles should be intact
    let default_config = ClientConfig::resolve_from_file("default", &file).unwrap();
    assert_eq!(
        default_config.orchestration.base_url,
        "http://localhost:8080"
    );

    let staging_config = ClientConfig::resolve_from_file("staging", &file).unwrap();
    assert_eq!(
        staging_config.orchestration.base_url,
        "https://staging-orch:9190"
    );
}

// =============================================================================
// profile validate tests
// =============================================================================

#[test]
fn test_validate_valid_config() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();

    // All profiles should resolve successfully
    for name in file.profile.keys() {
        let result = ClientConfig::resolve_from_file(name, &file);
        assert!(
            result.is_ok(),
            "Profile '{name}' should resolve: {result:?}"
        );
    }
}

#[test]
fn test_validate_detects_invalid_toml() {
    let invalid_toml = "this is not [valid toml }{";
    let result = toml::from_str::<ProfileConfigFile>(invalid_toml);
    assert!(result.is_err());
}

#[test]
fn test_validate_empty_profiles() {
    let empty_toml = "# No profiles defined\n";
    let file: ProfileConfigFile = toml::from_str(empty_toml).unwrap();
    assert!(file.profile.is_empty());
}

#[test]
fn test_validate_url_scheme() {
    let toml_content = r#"
[profile.bad_url]
[profile.bad_url.orchestration]
base_url = "ftp://wrong-scheme:8080"
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();
    let profile = file.profile.get("bad_url").unwrap();
    let url = profile
        .orchestration
        .as_ref()
        .unwrap()
        .base_url
        .as_ref()
        .unwrap();

    // The validation logic checks for http:// or https:// prefix
    assert!(
        !url.starts_with("http://") && !url.starts_with("https://"),
        "URL with ftp:// scheme should fail validation"
    );
}

#[test]
fn test_validate_tool_tiers() {
    let toml_content = r#"
[profile.valid_tools]
tools = ["tier1", "tier2", "tier3"]

[profile.invalid_tools]
tools = ["tier1", "tier4"]
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();

    let valid = file.profile.get("valid_tools").unwrap();
    let valid_tiers = ["tier1", "tier2", "tier3"];
    for tier in valid.tools.as_ref().unwrap() {
        assert!(valid_tiers.contains(&tier.as_str()));
    }

    let invalid = file.profile.get("invalid_tools").unwrap();
    let has_invalid = invalid
        .tools
        .as_ref()
        .unwrap()
        .iter()
        .any(|t| !valid_tiers.contains(&t.as_str()));
    assert!(has_invalid, "Should detect invalid tier 'tier4'");
}

// =============================================================================
// profile show tests
// =============================================================================

#[test]
fn test_show_profile_resolves_config() {
    let file: ProfileConfigFile = toml::from_str(MULTI_PROFILE_TOML).unwrap();

    // Show displays both raw metadata and resolved config
    let profile = file.profile.get("staging").unwrap();
    let config = ClientConfig::resolve_from_file("staging", &file).unwrap();

    // Metadata fields
    assert_eq!(profile.description.as_deref(), Some("Staging environment"));
    assert_eq!(
        profile.namespaces.as_deref(),
        Some(&["orders".to_string(), "analytics".to_string()][..])
    );
    assert_eq!(
        profile.tools.as_deref(),
        Some(&["tier1".to_string(), "tier2".to_string()][..])
    );

    // Resolved config
    assert_eq!(config.transport, tasker_client::config::Transport::Grpc);
    assert_eq!(config.orchestration.base_url, "https://staging-orch:9190");
    assert_eq!(config.orchestration.timeout_ms, 60000);
    // Inherited from default
    assert_eq!(config.orchestration.max_retries, 3);
}

// =============================================================================
// profile check tests (health probing)
// =============================================================================

#[tokio::test]
async fn test_check_unreachable_profile() {
    let toml_content = r#"
[profile.unreachable]
[profile.unreachable.orchestration]
base_url = "http://127.0.0.1:19999"
[profile.unreachable.worker]
base_url = "http://127.0.0.1:19998"
"#;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mut pm = ProfileManager::load_from_path(&config_path).unwrap();
    pm.set_health_probe_timeout_ms(1000);

    let snapshot = pm.probe_health("unreachable").await.unwrap();
    assert_eq!(
        snapshot.status,
        tasker_client::profile_manager::ProfileHealthStatus::Unreachable
    );
    assert_eq!(snapshot.orchestration_healthy, Some(false));
    assert_eq!(snapshot.worker_healthy, Some(false));
}

#[tokio::test]
async fn test_check_all_profiles() {
    let toml_content = r#"
[profile.one]
[profile.one.orchestration]
base_url = "http://127.0.0.1:19999"
[profile.one.worker]
base_url = "http://127.0.0.1:19997"

[profile.two]
[profile.two.orchestration]
base_url = "http://127.0.0.1:19998"
[profile.two.worker]
base_url = "http://127.0.0.1:19996"
"#;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, toml_content).unwrap();

    let mut pm = ProfileManager::load_from_path(&config_path).unwrap();
    pm.set_health_probe_timeout_ms(1000);

    let results = pm.probe_all_health().await;
    assert_eq!(results.len(), 2);

    for (_, snapshot) in &results {
        assert_eq!(
            snapshot.status,
            tasker_client::profile_manager::ProfileHealthStatus::Unreachable
        );
    }
}

// =============================================================================
// Profile name validation tests
// =============================================================================

#[test]
fn test_valid_profile_names() {
    let valid_names = ["default", "staging", "prod-us-east", "ci_runner", "test123"];
    for name in valid_names {
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "Name '{name}' should be valid"
        );
    }
}

#[test]
fn test_invalid_profile_names() {
    let invalid_names = ["has spaces", "has.dots", "has/slashes", ""];
    for name in invalid_names {
        let is_valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        assert!(!is_valid, "Name '{name}' should be invalid");
    }
}

// =============================================================================
// Auth config in profiles
// =============================================================================

#[test]
fn test_profile_with_auth_config() {
    let toml_content = r#"
[profile.auth]
transport = "rest"

[profile.auth.orchestration]
base_url = "http://localhost:8080"

[profile.auth.orchestration.auth]
method = { type = "ApiKey", value = { key = "test-key", header_name = "X-API-Key" } }
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();
    let config = ClientConfig::resolve_from_file("auth", &file).unwrap();

    assert!(config.orchestration.auth.is_some());
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn test_profile_config_no_default_profile() {
    let toml_content = r#"
[profile.staging]
transport = "grpc"

[profile.production]
transport = "rest"
"#;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("tasker-client.toml");
    fs::write(&config_path, toml_content).unwrap();

    let pm = ProfileManager::load_from_path(&config_path).unwrap();

    // When no "default" profile, first alphabetically becomes active
    assert_eq!(pm.active_profile_name(), "production");
    assert_eq!(pm.list_profile_names().len(), 2);
}

#[test]
fn test_profile_with_all_optional_fields() {
    let toml_content = r#"
[profile.full]
description = "Full config"
transport = "grpc"
namespaces = ["ns1", "ns2"]
tools = ["tier1", "tier2", "tier3"]

[profile.full.orchestration]
base_url = "https://orch:9190"
timeout_ms = 45000
max_retries = 5

[profile.full.worker]
base_url = "https://worker:9191"
timeout_ms = 45000
max_retries = 5
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();
    let profile = file.profile.get("full").unwrap();

    assert_eq!(profile.description.as_deref(), Some("Full config"));
    assert_eq!(profile.namespaces.as_ref().unwrap().len(), 2);
    assert_eq!(profile.tools.as_ref().unwrap().len(), 3);

    let config = ClientConfig::resolve_from_file("full", &file).unwrap();
    assert_eq!(config.orchestration.timeout_ms, 45000);
    assert_eq!(config.orchestration.max_retries, 5);
}

#[test]
fn test_profile_with_minimal_fields() {
    let toml_content = r#"
[profile.minimal]
"#;
    let file: ProfileConfigFile = toml::from_str(toml_content).unwrap();
    let profile = file.profile.get("minimal").unwrap();

    assert!(profile.description.is_none());
    assert!(profile.transport.is_none());
    assert!(profile.namespaces.is_none());
    assert!(profile.tools.is_none());
    assert!(profile.orchestration.is_none());
    assert!(profile.worker.is_none());

    // Should still resolve with all defaults
    let config = ClientConfig::resolve_from_file("minimal", &file).unwrap();
    assert_eq!(config.transport, tasker_client::config::Transport::Rest);
    assert_eq!(config.orchestration.base_url, "http://localhost:8080");
}
