//! `tasker-ctl profile` commands: manage `.config/tasker-client.toml` profiles (TAS-310).

use std::path::PathBuf;

use askama::Template;
use tasker_client::config::{ClientConfig, ProfileConfigFile, Transport};
use tasker_client::profile_manager::ProfileManager;
use tasker_client::{ClientError, ClientResult};

use crate::output;
use crate::ProfileCommands;

/// Askama template for generating `.config/tasker-client.toml`.
#[derive(Template, Debug)]
#[template(path = "profile-init.toml")]
struct ProfileInitTemplate;

pub(crate) async fn handle_profile_command(cmd: ProfileCommands) -> ClientResult<()> {
    match cmd {
        ProfileCommands::Init { force } => handle_profile_init(force).await,
        ProfileCommands::List => handle_profile_list().await,
        ProfileCommands::Add {
            name,
            description,
            transport,
            orchestration_url,
            worker_url,
            tools,
        } => {
            handle_profile_add(
                &name,
                description.as_deref(),
                &transport,
                &orchestration_url,
                &worker_url,
                tools.as_deref(),
            )
            .await
        }
        ProfileCommands::Validate => handle_profile_validate().await,
        ProfileCommands::Show { name } => handle_profile_show(&name).await,
        ProfileCommands::Check { name, all } => handle_profile_check(name.as_deref(), all).await,
    }
}

// =============================================================================
// profile init
// =============================================================================

async fn handle_profile_init(force: bool) -> ClientResult<()> {
    let config_dir = PathBuf::from(".config");
    let config_path = config_dir.join("tasker-client.toml");

    if config_path.exists() && !force {
        output::warning(format!("{} already exists.", config_path.display()));
        output::hint("Use --force to overwrite, or edit the file directly.");
        return Err(ClientError::ConfigError(format!(
            "{} already exists",
            config_path.display()
        )));
    }

    // Ensure .config/ directory exists
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| {
            ClientError::ConfigError(format!("Failed to create .config/ directory: {e}"))
        })?;
    }

    let template = ProfileInitTemplate;
    let content = template
        .render()
        .map_err(|e| ClientError::ConfigError(format!("Template rendering failed: {e}")))?;

    std::fs::write(&config_path, content).map_err(|e| {
        ClientError::ConfigError(format!("Failed to write {}: {e}", config_path.display()))
    })?;

    output::success(format!("Created {}", config_path.display()));
    output::blank();
    output::hint("Next steps:");
    output::plain("  tasker-ctl profile list                  # See your profiles");
    output::plain("  tasker-ctl profile add staging \\");
    output::plain("    --orchestration-url http://staging:8080 # Add an environment");
    output::plain("  tasker-ctl profile check                 # Test connectivity");

    Ok(())
}

// =============================================================================
// profile list
// =============================================================================

async fn handle_profile_list() -> ClientResult<()> {
    let path = find_profile_config_path()?;
    let pm = ProfileManager::load_from_path(&path)?;
    let summaries = pm.list_profiles();

    if summaries.is_empty() {
        output::warning("No profiles found.");
        output::hint("Run `tasker-ctl profile init` to create a profile config file.");
        return Ok(());
    }

    output::header(format!("Profiles ({})", path.display()));
    output::blank();

    for summary in &summaries {
        let active_marker = if summary.is_active { " *" } else { "" };
        output::plain(format!("  {}{active_marker}", summary.name,));
        if let Some(ref desc) = summary.description {
            output::dim(format!("    {desc}"));
        }
        output::label("    Transport", summary.transport);
        output::label("    Orchestration", &summary.orchestration_url);
        output::label("    Worker", &summary.worker_url);
        if let Some(ref ns) = summary.namespaces {
            output::label("    Namespaces", ns.join(", "));
        }
        output::label("    Health", summary.health_status);
        output::blank();
    }

    output::dim("  * = active profile");

    Ok(())
}

// =============================================================================
// profile add
// =============================================================================

async fn handle_profile_add(
    name: &str,
    description: Option<&str>,
    transport: &str,
    orchestration_url: &str,
    worker_url: &str,
    tools: Option<&[String]>,
) -> ClientResult<()> {
    // Validate transport
    let _transport: Transport = transport.parse().map_err(|_| {
        ClientError::ConfigError(format!(
            "Invalid transport '{transport}': expected 'rest' or 'grpc'"
        ))
    })?;

    // Validate profile name
    validate_profile_name(name)?;

    // Validate tool tiers if provided
    if let Some(tiers) = tools {
        for tier in tiers {
            if !["tier1", "tier2", "tier3"].contains(&tier.as_str()) {
                return Err(ClientError::ConfigError(format!(
                    "Invalid tool tier '{tier}': expected tier1, tier2, or tier3"
                )));
            }
        }
    }

    let path = find_profile_config_path()?;

    // Read the existing file content
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ClientError::ConfigError(format!("Failed to read {}: {e}", path.display())))?;

    // Parse with toml_edit to preserve formatting
    let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| {
        ClientError::ConfigError(format!("Failed to parse {}: {e}", path.display()))
    })?;

    // Check if profile already exists
    let profile_table = doc
        .entry("profile")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let profile_table = profile_table
        .as_table_mut()
        .ok_or_else(|| ClientError::ConfigError("[profile] is not a table".to_string()))?;

    if profile_table.contains_key(name) {
        return Err(ClientError::ConfigError(format!(
            "Profile '{name}' already exists in {}. Edit the file directly to modify it.",
            path.display()
        )));
    }

    // Build the new profile table
    let mut profile = toml_edit::Table::new();
    if let Some(desc) = description {
        profile.insert("description", toml_edit::value(desc));
    }
    profile.insert("transport", toml_edit::value(transport));

    if let Some(tiers) = tools {
        let mut arr = toml_edit::Array::new();
        for t in tiers {
            arr.push(t.as_str());
        }
        profile.insert("tools", toml_edit::value(arr));
    }

    // Orchestration sub-table
    let mut orch = toml_edit::Table::new();
    orch.insert("base_url", toml_edit::value(orchestration_url));
    profile.insert("orchestration", toml_edit::Item::Table(orch));

    // Worker sub-table
    let mut worker = toml_edit::Table::new();
    worker.insert("base_url", toml_edit::value(worker_url));
    profile.insert("worker", toml_edit::Item::Table(worker));

    profile_table.insert(name, toml_edit::Item::Table(profile));

    std::fs::write(&path, doc.to_string()).map_err(|e| {
        ClientError::ConfigError(format!("Failed to write {}: {e}", path.display()))
    })?;

    output::success(format!("Added profile '{name}' to {}", path.display()));
    output::blank();
    output::hint("Verify with:");
    output::plain(format!("  tasker-ctl profile show {name}"));
    output::plain(format!("  tasker-ctl profile check {name}"));

    Ok(())
}

// =============================================================================
// profile validate
// =============================================================================

async fn handle_profile_validate() -> ClientResult<()> {
    let path = find_profile_config_path()?;

    output::header(format!("Validating {}", path.display()));
    output::blank();

    // Step 1: TOML parsing
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ClientError::ConfigError(format!("Failed to read {}: {e}", path.display())))?;

    let profile_file: ProfileConfigFile = match toml::from_str(&content) {
        Ok(pf) => {
            output::status_icon(true, "TOML syntax valid");
            pf
        }
        Err(e) => {
            output::status_icon(false, format!("TOML parse error: {e}"));
            return Err(ClientError::ConfigError(format!("Invalid TOML: {e}")));
        }
    };

    // Step 2: Check for profiles
    if profile_file.profile.is_empty() {
        output::status_icon(false, "No [profile.*] sections found");
        return Err(ClientError::ConfigError("No profiles defined".to_string()));
    }
    output::status_icon(
        true,
        format!("{} profile(s) found", profile_file.profile.len()),
    );

    // Step 3: Validate each profile
    let mut errors = Vec::new();

    for (name, profile) in &profile_file.profile {
        let mut profile_ok = true;

        // Validate transport if specified
        if let Some(t) = profile.transport {
            output::status_icon(true, format!("[{name}] transport = {t}"));
        }

        // Validate orchestration URL
        if let Some(ref orch) = profile.orchestration {
            if let Some(ref url) = orch.base_url {
                if url.starts_with("http://") || url.starts_with("https://") {
                    output::status_icon(true, format!("[{name}] orchestration.base_url = {url}"));
                } else {
                    output::status_icon(
                        false,
                        format!(
                            "[{name}] orchestration.base_url must start with http:// or https://"
                        ),
                    );
                    profile_ok = false;
                }
            }
        }

        // Validate worker URL
        if let Some(ref worker) = profile.worker {
            if let Some(ref url) = worker.base_url {
                if url.starts_with("http://") || url.starts_with("https://") {
                    output::status_icon(true, format!("[{name}] worker.base_url = {url}"));
                } else {
                    output::status_icon(
                        false,
                        format!("[{name}] worker.base_url must start with http:// or https://"),
                    );
                    profile_ok = false;
                }
            }
        }

        // Validate tool tiers if specified
        if let Some(ref tools) = profile.tools {
            let valid_tiers = ["tier1", "tier2", "tier3"];
            for tier in tools {
                if !valid_tiers.contains(&tier.as_str()) {
                    output::status_icon(
                        false,
                        format!(
                            "[{name}] unknown tool tier '{tier}' (expected: tier1, tier2, tier3)"
                        ),
                    );
                    profile_ok = false;
                }
            }
            if profile_ok {
                output::status_icon(true, format!("[{name}] tools = {tools:?}"));
            }
        }

        // Try to resolve to ClientConfig
        match ClientConfig::resolve_from_file(name, &profile_file) {
            Ok(_) => {
                output::status_icon(true, format!("[{name}] resolves to valid ClientConfig"));
            }
            Err(e) => {
                output::status_icon(false, format!("[{name}] resolution error: {e}"));
                profile_ok = false;
            }
        }

        if !profile_ok {
            errors.push(name.clone());
        }
    }

    output::blank();

    if errors.is_empty() {
        output::success("All profiles are valid.");
        Ok(())
    } else {
        output::error(format!("Validation failed for: {}", errors.join(", ")));
        Err(ClientError::ConfigError(format!(
            "Validation errors in profiles: {}",
            errors.join(", ")
        )))
    }
}

// =============================================================================
// profile show
// =============================================================================

async fn handle_profile_show(name: &str) -> ClientResult<()> {
    let path = find_profile_config_path()?;
    let profile_file = ClientConfig::load_profile_file(&path)?;

    let profile = profile_file.profile.get(name).ok_or_else(|| {
        let available: Vec<&str> = profile_file.profile.keys().map(|s| s.as_str()).collect();
        ClientError::ConfigError(format!(
            "Profile '{name}' not found. Available: {}",
            available.join(", ")
        ))
    })?;

    let config = ClientConfig::resolve_from_file(name, &profile_file)?;

    output::header(format!("Profile: {name}"));
    output::blank();

    if let Some(ref desc) = profile.description {
        output::label("  Description", desc);
    }
    output::label("  Transport", config.transport);
    output::blank();

    output::plain("  Orchestration:");
    output::label("    URL", &config.orchestration.base_url);
    output::label(
        "    Timeout",
        format!("{}ms", config.orchestration.timeout_ms),
    );
    output::label("    Max retries", config.orchestration.max_retries);
    if config.orchestration.auth.is_some() {
        output::label("    Auth", "configured");
    }
    output::blank();

    output::plain("  Worker:");
    output::label("    URL", &config.worker.base_url);
    output::label("    Timeout", format!("{}ms", config.worker.timeout_ms));
    output::label("    Max retries", config.worker.max_retries);
    if config.worker.auth.is_some() {
        output::label("    Auth", "configured");
    }

    if let Some(ref ns) = profile.namespaces {
        output::blank();
        output::label("  Namespaces", ns.join(", "));
    }

    if let Some(ref tools) = profile.tools {
        output::blank();
        output::label("  MCP tool tiers", tools.join(", "));
    }

    Ok(())
}

// =============================================================================
// profile check
// =============================================================================

async fn handle_profile_check(name: Option<&str>, all: bool) -> ClientResult<()> {
    let path = find_profile_config_path()?;
    let mut pm = ProfileManager::load_from_path(&path)?;
    pm.set_health_probe_timeout_ms(5000);

    if all {
        output::header("Checking all profiles...");
        output::blank();

        let results = pm.probe_all_health().await;

        for (profile_name, snapshot) in &results {
            let healthy = matches!(
                snapshot.status,
                tasker_client::profile_manager::ProfileHealthStatus::Healthy
            );
            let degraded = matches!(
                snapshot.status,
                tasker_client::profile_manager::ProfileHealthStatus::Degraded
            );

            if healthy {
                output::status_icon(true, format!("{profile_name}: healthy"));
            } else if degraded {
                output::warning(format!("  {profile_name}: degraded"));
            } else {
                output::status_icon(false, format!("{profile_name}: {}", snapshot.status));
            }

            if let Some(orch) = snapshot.orchestration_healthy {
                output::status_icon(
                    orch,
                    format!(
                        "  orchestration: {}",
                        if orch { "reachable" } else { "unreachable" }
                    ),
                );
            }
            if let Some(worker) = snapshot.worker_healthy {
                output::status_icon(
                    worker,
                    format!(
                        "  worker: {}",
                        if worker { "reachable" } else { "unreachable" }
                    ),
                );
            }
            output::blank();
        }

        let healthy_count = results
            .iter()
            .filter(|(_, s)| {
                matches!(
                    s.status,
                    tasker_client::profile_manager::ProfileHealthStatus::Healthy
                )
            })
            .count();

        output::dim(format!(
            "{}/{} profiles healthy",
            healthy_count,
            results.len()
        ));

        Ok(())
    } else {
        let profile_name = match name {
            Some(n) => n.to_string(),
            None => pm.active_profile_name().to_string(),
        };

        output::header(format!("Checking profile '{profile_name}'..."));
        output::blank();

        let snapshot = pm.probe_health(&profile_name).await?;

        if let Some(orch) = snapshot.orchestration_healthy {
            output::status_icon(
                orch,
                format!(
                    "Orchestration: {}",
                    if orch { "reachable" } else { "unreachable" }
                ),
            );
        }
        if let Some(worker) = snapshot.worker_healthy {
            output::status_icon(
                worker,
                format!(
                    "Worker: {}",
                    if worker { "reachable" } else { "unreachable" }
                ),
            );
        }

        output::blank();

        match snapshot.status {
            tasker_client::profile_manager::ProfileHealthStatus::Healthy => {
                output::success(format!("Profile '{profile_name}' is healthy."));
            }
            tasker_client::profile_manager::ProfileHealthStatus::Degraded => {
                output::warning(format!(
                    "Profile '{profile_name}' is degraded (partial connectivity)."
                ));
            }
            _ => {
                output::error(format!("Profile '{profile_name}' is unreachable."));
            }
        }

        Ok(())
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn find_profile_config_path() -> ClientResult<PathBuf> {
    ClientConfig::find_profile_config_file().ok_or_else(|| {
        ClientError::ConfigError(
            "No profile config file found. Run `tasker-ctl profile init` to create one."
                .to_string(),
        )
    })
}

fn validate_profile_name(name: &str) -> ClientResult<()> {
    if name.is_empty() {
        return Err(ClientError::ConfigError(
            "Profile name cannot be empty".to_string(),
        ));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ClientError::ConfigError(format!(
            "Profile name '{name}' contains invalid characters. Use alphanumeric, hyphens, or underscores."
        )));
    }

    Ok(())
}
