//! MCP ServerHandler implementation for Tasker.
//!
//! Provides the MCP server with developer tooling tools (Tier 1, always available)
//! and profile management tools (available when profiles are configured):
//!
//! **Tier 1 — Developer Tooling (offline)**
//! - `template_validate` — Validate a task template for structural correctness
//! - `template_visualize` — Generate Mermaid flowchart diagram from a template
//! - `template_inspect` — Inspect template DAG structure and step details
//! - `template_generate` — Generate task template YAML from a structured spec
//! - `handler_generate` — Generate typed handler code for a template
//! - `schema_inspect` — Inspect result_schema field details per step
//! - `schema_compare` — Compare producer/consumer schema compatibility
//! - `schema_diff` — Detect field-level changes between two template versions
//! - `grammar_list` — List grammar categories and capabilities
//! - `capability_search` — Search capabilities by name or category
//! - `capability_inspect` — Inspect capability config schema and metadata
//! - `vocabulary_document` — Generate complete vocabulary documentation
//! - `composition_validate` — Validate a standalone composition spec
//! - `composition_explain` — Analyze and explain data flow through a composition spec
//!
//! **Profile Management (when profiles configured)**
//! - `connection_status` — Show profile health and available capabilities
//!
//! **Tier 2 — Connected Read-Only Tools (require live server)**
//! - `task_list` — List tasks with filtering by namespace/status
//! - `task_inspect` — Get task details with step breakdown
//! - `task_visualize` — Visualize task execution state as a Mermaid flowchart diagram
//! - `step_inspect` — Get step details including results and timing
//! - `step_audit` — Get SOC2-compliant audit trail for a step
//! - `dlq_list` — List dead letter queue entries with filtering
//! - `dlq_inspect` — Get detailed DLQ entry with investigation context
//! - `dlq_stats` — Get DLQ statistics aggregated by reason code
//! - `dlq_queue` — Get prioritized investigation queue
//! - `staleness_check` — Monitor task staleness with health annotations
//! - `analytics_performance` — Get system-wide performance metrics
//! - `analytics_bottlenecks` — Identify slow steps and tasks
//! - `system_health` — Get detailed component health status
//! - `system_config` — Get orchestration configuration (secrets redacted)
//! - `template_list_remote` — List templates registered on the server
//! - `template_inspect_remote` — Get template details from the server
//!
//! **Tier 3 — Write Tools with Confirmation (require live server)**
//! - `task_submit` — Submit a task for execution (preview → confirm)
//! - `task_cancel` — Cancel a task and all pending steps (preview → confirm)
//! - `step_retry` — Reset a failed step for retry (preview → confirm)
//! - `step_resolve` — Mark a step as manually resolved (preview → confirm)
//! - `step_complete` — Manually complete a step with result data (preview → confirm)
//! - `dlq_update` — Update DLQ entry investigation status (preview → confirm)

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::RwLock;

use tasker_client::{ProfileManager, UnifiedOrchestrationClient};
use tasker_sdk::operational::client_factory;

use crate::tier::{EnabledTiers, ToolTier};
use crate::tools::*;

/// Tasker MCP server handler with developer tooling and profile management.
#[derive(Debug, Clone)]
pub struct TaskerMcpServer {
    tool_router: ToolRouter<Self>,
    profile_manager: Arc<RwLock<ProfileManager>>,
    offline: bool,
    enabled_tiers: EnabledTiers,
}

impl Default for TaskerMcpServer {
    fn default() -> Self {
        let pm = ProfileManager::load().unwrap_or_else(|_| ProfileManager::offline());
        Self::with_profile_manager(pm, false, None)
    }
}

impl TaskerMcpServer {
    /// Create a server with profile management and optional tier override.
    ///
    /// When `tier_override` is `None`, tiers are resolved from the active profile's
    /// `tools` configuration (or default to all tiers when connected, tier1 when offline).
    /// When `Some`, the override takes precedence (used by `--tools` CLI flag).
    pub fn with_profile_manager(
        profile_manager: ProfileManager,
        offline: bool,
        tier_override: Option<EnabledTiers>,
    ) -> Self {
        let enabled_tiers = tier_override.unwrap_or_else(|| {
            let profile_tools = profile_manager
                .active_profile_metadata()
                .and_then(|m| m.tools.as_deref());
            EnabledTiers::resolve(offline, profile_tools)
        });

        let mut router = Self::tool_router();
        for tool_name in enabled_tiers.tools_to_remove() {
            router.remove_route(tool_name);
        }
        // Remove connection_status if fully offline (no profiles to show)
        if offline {
            router.remove_route("connection_status");
        }

        Self {
            tool_router: router,
            profile_manager: Arc::new(RwLock::new(profile_manager)),
            offline,
            enabled_tiers,
        }
    }

    /// Create a server in offline mode (Tier 1 developer tools only).
    pub fn offline() -> Self {
        Self::with_profile_manager(ProfileManager::offline(), true, None)
    }

    /// Create a server with no-arg constructor for backward compatibility in tests.
    pub fn new() -> Self {
        Self::offline()
    }

    /// Get a reference to the profile manager.
    pub fn profile_manager(&self) -> &Arc<RwLock<ProfileManager>> {
        &self.profile_manager
    }

    /// Get the enabled tiers for this server instance.
    pub fn enabled_tiers(&self) -> &EnabledTiers {
        &self.enabled_tiers
    }

    /// Resolve a connected orchestration client, returning both client and profile name.
    ///
    /// Returns an error JSON string if offline, profile not found, or connection fails.
    /// The profile name is needed by permission-aware error handlers.
    async fn resolve_client_with_profile(
        &self,
        profile: Option<&str>,
    ) -> Result<(UnifiedOrchestrationClient, String), String> {
        if self.offline {
            return Err(error_json(
                "offline_mode",
                "Running in offline mode. Connected tools require a live Tasker server. \
                 Start tasker-mcp with a profile configuration to enable connected tools.",
            ));
        }

        let pm = self.profile_manager.read().await;
        let profile_name = profile
            .unwrap_or_else(|| pm.active_profile_name())
            .to_string();

        let config = pm.get_config(&profile_name).ok_or_else(|| {
            let available = pm.list_profile_names().join(", ");
            error_json(
                "profile_not_found",
                &format!(
                    "Profile '{}' not found. Available profiles: [{}]. \
                     Use connection_status to see all profiles.",
                    profile_name, available
                ),
            )
        })?;

        let client = client_factory::build_orchestration_client(config)
            .await
            .map_err(|e| {
                error_json(
                    "connection_failed",
                    &format!(
                        "Failed to connect to profile '{}': {}. \
                         Use connection_status to check endpoint health.",
                        profile_name, e
                    ),
                )
            })?;

        Ok((client, profile_name))
    }

    /// Resolve a connected orchestration client for Tier 2 tools.
    ///
    /// Delegates to `resolve_client_with_profile` for backward compatibility.
    async fn resolve_client(
        &self,
        profile: Option<&str>,
    ) -> Result<UnifiedOrchestrationClient, String> {
        self.resolve_client_with_profile(profile)
            .await
            .map(|(client, _)| client)
    }

    /// Resolve a connected orchestration client for Tier 3 write tools.
    ///
    /// Enforces write-profile locking: if the caller requests a profile different from
    /// the active (launch) profile, the write is rejected. Reads can target any profile,
    /// but writes are locked to the launch profile for safety.
    async fn resolve_client_for_write(
        &self,
        profile: Option<&str>,
    ) -> Result<(UnifiedOrchestrationClient, String), String> {
        if let Some(requested) = profile {
            let pm = self.profile_manager.read().await;
            let active = pm.active_profile_name();
            if !active.is_empty() && requested != active {
                return Err(serde_json::json!({
                    "error": "write_profile_locked",
                    "message": format!(
                        "Write tools are locked to the launch profile '{}'. \
                         You requested '{}'. To write to a different environment, \
                         restart tasker-mcp with --profile {}.",
                        active, requested, requested
                    )
                })
                .to_string());
            }
        }
        self.resolve_client_with_profile(profile).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TaskerMcpServer {
    fn get_info(&self) -> ServerInfo {
        let tier1_text = "\
             Tasker is a workflow orchestration system. You help developers create and validate \
             task templates (workflow definitions) and generate typed handler code.\n\
             Workflow: template_generate → template_validate → handler_generate (unified types+handlers+tests)\n\
             handler_generate defaults to scaffold mode: handlers import generated types with typed returns.\n\
             When debugging: template_inspect → schema_inspect → schema_compare\n\
             For versioning: schema_diff compares before/after template YAML to detect breaking field changes";

        let has_tier2 = self.enabled_tiers.includes(ToolTier::Tier2);
        let has_tier3 = self.enabled_tiers.includes(ToolTier::Tier3);

        let instructions = if self.offline {
            format!(
                "{}\nRunning in OFFLINE mode — only developer tooling tools are available. \
                 Connect to a Tasker instance for task management tools.",
                tier1_text
            )
        } else if !has_tier2 && !has_tier3 {
            format!(
                "{}\nProfile management: connection_status to check environment health.\n\
                 This profile is configured for developer tooling only (tier1). \
                 No read or write tools are available.",
                tier1_text
            )
        } else if has_tier2 && !has_tier3 {
            format!(
                "{}\nProfile management: connection_status to check environment health.\n\
                 Read-only tools: task_list/task_inspect/task_visualize for task inspection, step_inspect/step_audit for step details, \
                 dlq_list/dlq_inspect/dlq_stats/dlq_queue/staleness_check for DLQ investigation, \
                 analytics_performance/analytics_bottlenecks for performance analysis, \
                 system_health/system_config for system status, \
                 template_list_remote/template_inspect_remote for server-side templates.\n\
                 Write tools are not enabled for this profile. \
                 All connected tools accept an optional 'profile' parameter to target a specific environment.",
                tier1_text
            )
        } else {
            format!(
                "{}\nProfile management: connection_status to check environment health.\n\
                 Read-only tools: task_list/task_inspect/task_visualize for task inspection, step_inspect/step_audit for step details, \
                 dlq_list/dlq_inspect/dlq_stats/dlq_queue/staleness_check for DLQ investigation, \
                 analytics_performance/analytics_bottlenecks for performance analysis, \
                 system_health/system_config for system status, \
                 template_list_remote/template_inspect_remote for server-side templates.\n\
                 Write tools (require confirm: true): task_submit, task_cancel, step_retry, step_resolve, \
                 step_complete, dlq_update. Always preview first (omit confirm), show preview to user, \
                 then call again with confirm: true after user approval.\n\
                 Write tools are locked to the launch profile. Read tools accept an optional 'profile' parameter \
                 to target any configured environment.",
                tier1_text
            )
        };

        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "tasker-mcp".to_string(),
                title: Some("Tasker MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "MCP server exposing Tasker developer tooling: template validation, \
                     code generation, schema inspection, and workflow analysis"
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(instructions),
        }
    }
}

#[tool_router(router = tool_router)]
impl TaskerMcpServer {
    // ── Tier 1: Developer Tooling (always available) ──

    /// Validate a task template YAML for structural correctness, dependency cycles,
    /// and best-practice warnings.
    #[tool(
        name = "template_validate",
        description = "Validate a task template YAML for structural correctness, dependency cycles, and best-practice warnings. Returns validation findings with severity levels (error/warning/info)."
    )]
    pub async fn template_validate(
        &self,
        Parameters(params): Parameters<TemplateValidateParams>,
    ) -> String {
        developer::template_validate(params)
    }

    /// Generate a Mermaid flowchart diagram from a task template, showing the step
    /// DAG with dependency edges and per-step details.
    #[tool(
        name = "template_visualize",
        description = "Generate a Mermaid flowchart diagram from a task template. Shows step DAG with dependency edges, node styling for step types (standard/decision), optional developer annotations, and a detail table with handler, schema, and retry information. Works entirely offline."
    )]
    pub async fn template_visualize(
        &self,
        Parameters(params): Parameters<TemplateVisualizeParams>,
    ) -> String {
        developer::template_visualize(params)
    }

    /// Inspect a task template's DAG structure: execution order, root/leaf steps,
    /// dependencies, and per-step details.
    #[tool(
        name = "template_inspect",
        description = "Inspect a task template's DAG structure: execution order, root/leaf steps, dependencies, and per-step details including handler callables and schema presence."
    )]
    pub async fn template_inspect(
        &self,
        Parameters(params): Parameters<TemplateInspectParams>,
    ) -> String {
        developer::template_inspect(params)
    }

    /// Generate a task template YAML from a structured specification with step
    /// definitions and output field types.
    #[tool(
        name = "template_generate",
        description = "Generate a task template YAML from a structured specification. Provide task name, namespace, steps with dependencies, and output field definitions. Returns valid template YAML."
    )]
    pub async fn template_generate(
        &self,
        Parameters(params): Parameters<TemplateGenerateParams>,
    ) -> String {
        developer::template_generate(params)
    }

    /// Generate typed handler code (types, handlers, tests) for a task template
    /// in the specified language.
    #[tool(
        name = "handler_generate",
        description = "Generate typed handler code for a task template. Returns types, handler scaffolds, and test files for the specified language (python, ruby, typescript, rust). By default uses scaffold mode where handlers import generated types."
    )]
    pub async fn handler_generate(
        &self,
        Parameters(params): Parameters<HandlerGenerateParams>,
    ) -> String {
        developer::handler_generate(params)
    }

    /// Inspect result_schema definitions across template steps with field-level
    /// detail including types, required status, and consumer relationships.
    #[tool(
        name = "schema_inspect",
        description = "Inspect result_schema definitions across template steps. Returns field-level detail including types, required status, and which downstream steps consume each step's output."
    )]
    pub async fn schema_inspect(
        &self,
        Parameters(params): Parameters<SchemaInspectParams>,
    ) -> String {
        developer::schema_inspect(params)
    }

    /// Compare the result_schema of a producer step against a consumer step
    /// to check compatibility.
    #[tool(
        name = "schema_compare",
        description = "Compare the result_schema of a producer step against a consumer step to check data contract compatibility. Reports missing required fields, type mismatches, and extra fields."
    )]
    pub async fn schema_compare(
        &self,
        Parameters(params): Parameters<SchemaCompareParams>,
    ) -> String {
        developer::schema_compare(params)
    }

    /// Diff two versions of a task template to detect field-level changes in
    /// result_schema definitions (additions, removals, type changes, required status).
    #[tool(
        name = "schema_diff",
        description = "Compare two versions of the same task template to detect field-level changes. Reports field additions, removals, type changes, and required/optional status changes with breaking-change analysis."
    )]
    pub async fn schema_diff(&self, Parameters(params): Parameters<SchemaDiffParams>) -> String {
        developer::schema_diff(params)
    }

    /// List all grammar categories with descriptions and associated capabilities.
    #[tool(
        name = "grammar_list",
        description = "List all grammar categories (transform, validate, assert, acquire, persist, emit) with descriptions and associated capabilities. Works offline."
    )]
    pub async fn grammar_list(&self) -> String {
        developer::grammar_list()
    }

    /// Search capabilities by name or category.
    #[tool(
        name = "capability_search",
        description = "Search grammar capabilities by name substring and/or category filter. Both parameters are optional — omit both to list all capabilities. Works offline."
    )]
    pub async fn capability_search(
        &self,
        Parameters(params): Parameters<CapabilitySearchParams>,
    ) -> String {
        developer::capability_search(params)
    }

    /// Inspect a capability's full configuration schema and metadata.
    #[tool(
        name = "capability_inspect",
        description = "Show full details for a capability: config_schema, mutation_profile, tags, and version. Use grammar_list first to see available capability names. Works offline."
    )]
    pub async fn capability_inspect(
        &self,
        Parameters(params): Parameters<CapabilityInspectParams>,
    ) -> String {
        developer::capability_inspect(params)
    }

    /// Generate complete vocabulary documentation.
    #[tool(
        name = "vocabulary_document",
        description = "Generate complete documentation for all registered grammar capabilities, organized by category with full config schemas. Works offline."
    )]
    pub async fn vocabulary_document(&self) -> String {
        developer::vocabulary_document()
    }

    /// Validate a standalone composition spec.
    #[tool(
        name = "composition_validate",
        description = "Validate a standalone CompositionSpec (YAML or JSON) for structural correctness: capability existence, config schemas, expression syntax, contract chaining, and checkpoint coverage. Works offline."
    )]
    pub async fn composition_validate(
        &self,
        Parameters(params): Parameters<CompositionValidateParams>,
    ) -> String {
        developer::composition_validate(params)
    }

    /// Explain data flow through a composition spec.
    #[tool(
        name = "composition_explain",
        description = "Analyze and explain data flow through a CompositionSpec. Shows how data threads through invocations via the envelope (.context, .deps, .prev, .step), which jaq expressions reference which fields, checkpoint placement, and output schemas. Optionally evaluates expressions against sample data for simulated execution. Works offline."
    )]
    pub async fn composition_explain(
        &self,
        Parameters(params): Parameters<CompositionExplainParams>,
    ) -> String {
        developer::composition_explain(params)
    }

    // ── Profile Management ──

    /// Show connection status of all configured Tasker profiles.
    #[tool(
        name = "connection_status",
        description = "Show connection status of all configured Tasker profiles. Returns profile names, endpoints, transport type, health status, and which profile is active. Use this to verify connectivity before running task management operations. Pass refresh=true to re-probe all endpoints."
    )]
    pub async fn connection_status(
        &self,
        Parameters(params): Parameters<ConnectionStatusParams>,
    ) -> String {
        if self.offline {
            return serde_json::json!({
                "mode": "offline",
                "message": "Running in offline mode. No profiles are configured. Only developer tooling tools (template_*, handler_*, schema_*) are available.",
                "profiles": []
            })
            .to_string();
        }

        let mut pm = self.profile_manager.write().await;

        if params.refresh.unwrap_or(false) {
            pm.probe_all_health().await;
        }

        let profiles = pm.list_profiles();
        let active = pm.active_profile_name().to_string();

        serde_json::to_string_pretty(&serde_json::json!({
            "mode": "connected",
            "active_profile": active,
            "profiles": profiles,
        }))
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    // ── Tier 2: Connected Read-Only Tools ──

    /// List tasks with optional filtering by namespace and status.
    #[tool(
        name = "task_list",
        description = "List tasks with optional filtering by namespace and status. Returns compact summaries with UUID, name, status, completion percentage, and health. Start here to find tasks for deeper inspection with task_inspect."
    )]
    pub async fn task_list(&self, Parameters(params): Parameters<TaskListParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::task_list(&client, params).await
    }

    /// Get detailed task information with step breakdown.
    #[tool(
        name = "task_inspect",
        description = "Get detailed task information including all steps with their status, attempt counts, and dependency satisfaction. Use task_uuid from task_list results. Follow up with step_inspect for individual step details or step_audit for audit trails."
    )]
    pub async fn task_inspect(&self, Parameters(params): Parameters<TaskInspectParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::task_inspect(&client, params).await
    }

    /// Visualize task execution state as a Mermaid flowchart diagram.
    #[tool(
        name = "task_visualize",
        description = "Visualize task execution state as a Mermaid flowchart diagram. Shows step DAG with nodes colored by execution status (completed/in-progress/pending/error/retrying), edge styling for dependency satisfaction, decision workflow paths (including untraversed branches), batch worker instances, DLQ status, and a detail table with timing, attempts, and error types. Requires a running server."
    )]
    pub async fn task_visualize(
        &self,
        Parameters(params): Parameters<TaskVisualizeParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::task_visualize(&client, params).await
    }

    /// Get detailed step information including results and timing.
    #[tool(
        name = "step_inspect",
        description = "Get detailed information about a specific workflow step including current state, results, attempt counts, retry eligibility, and dependency status. Requires both task_uuid and step_uuid from task_inspect results."
    )]
    pub async fn step_inspect(
        &self,
        Parameters(params): Parameters<StepInspectToolParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::step_inspect(&client, params).await
    }

    /// Get SOC2-compliant audit trail for a workflow step.
    #[tool(
        name = "step_audit",
        description = "Get the complete audit trail for a workflow step showing all state transitions, worker attribution, execution timing, and results. Ordered most-recent-first. Requires both task_uuid and step_uuid."
    )]
    pub async fn step_audit(&self, Parameters(params): Parameters<StepAuditParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::step_audit(&client, params).await
    }

    /// List dead letter queue entries with optional filtering.
    #[tool(
        name = "dlq_list",
        description = "List dead letter queue entries with optional filtering by resolution status. Returns summaries with task UUID, DLQ reason, resolution status, and age. Start here for DLQ investigation, then use dlq_inspect for details."
    )]
    pub async fn dlq_list(&self, Parameters(params): Parameters<DlqListToolParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::dlq_list(&client, params).await
    }

    /// Get detailed DLQ entry with investigation context.
    #[tool(
        name = "dlq_inspect",
        description = "Get detailed information about a specific DLQ entry including the full error context, original task details, and resolution history. Use the task_uuid from dlq_list results."
    )]
    pub async fn dlq_inspect(
        &self,
        Parameters(params): Parameters<DlqInspectToolParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::dlq_inspect(&client, params).await
    }

    /// Get DLQ statistics aggregated by reason code.
    #[tool(
        name = "dlq_stats",
        description = "Get dead letter queue statistics aggregated by reason code. Shows how many entries exist per failure reason, helping identify systemic issues. Use this for high-level DLQ health assessment."
    )]
    pub async fn dlq_stats(&self, Parameters(params): Parameters<DlqStatsToolParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::dlq_stats(&client, params).await
    }

    /// Get prioritized DLQ investigation queue.
    #[tool(
        name = "dlq_queue",
        description = "Get the prioritized investigation queue ranking DLQ entries by severity and age. Returns entries scored for triage priority. Use this to decide which DLQ entries to investigate first."
    )]
    pub async fn dlq_queue(&self, Parameters(params): Parameters<DlqQueueToolParams>) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::dlq_queue(&client, params).await
    }

    /// Monitor task staleness with health annotations.
    #[tool(
        name = "staleness_check",
        description = "Monitor task staleness with health annotations (healthy/warning/stale). Identifies tasks that may be stuck based on configurable time thresholds. Use this for proactive health monitoring."
    )]
    pub async fn staleness_check(
        &self,
        Parameters(params): Parameters<StalenessCheckParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::staleness_check(&client, params).await
    }

    /// Get system-wide performance metrics.
    #[tool(
        name = "analytics_performance",
        description = "Get system-wide performance metrics including task throughput, step execution times, and queue depths. Optionally filter by time window in hours. Use this for capacity planning and performance monitoring."
    )]
    pub async fn analytics_performance(
        &self,
        Parameters(params): Parameters<AnalyticsPerformanceParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::analytics_performance(&client, params).await
    }

    /// Identify slow steps and bottleneck tasks.
    #[tool(
        name = "analytics_bottlenecks",
        description = "Identify slow steps and bottleneck tasks in the system. Returns steps ranked by execution time with execution counts. Filter by minimum executions to focus on statistically significant bottlenecks."
    )]
    pub async fn analytics_bottlenecks(
        &self,
        Parameters(params): Parameters<AnalyticsBottlenecksParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::analytics_bottlenecks(&client, params).await
    }

    /// Get detailed component health status.
    #[tool(
        name = "system_health",
        description = "Get detailed health status of all system components including database pools, message queues, circuit breaker states, and cache connectivity. Use this to diagnose infrastructure issues."
    )]
    pub async fn system_health(
        &self,
        Parameters(params): Parameters<SystemHealthParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::system_health(&client, params).await
    }

    /// Get orchestration configuration (secrets redacted).
    #[tool(
        name = "system_config",
        description = "Get the orchestration system configuration with secrets redacted. Shows circuit breaker settings, pool sizes, messaging configuration, and feature flags. Use this to verify runtime configuration."
    )]
    pub async fn system_config(
        &self,
        Parameters(params): Parameters<SystemConfigParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::system_config(&client, params).await
    }

    /// List templates registered on the server.
    #[tool(
        name = "template_list_remote",
        description = "List task templates registered on the connected Tasker server. Optionally filter by namespace. Returns template names, versions, and step counts. Unlike template_inspect (which works on local YAML), this queries the live server."
    )]
    pub async fn template_list_remote(
        &self,
        Parameters(params): Parameters<TemplateListRemoteParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::template_list_remote(&client, params).await
    }

    /// Get template details from the server.
    #[tool(
        name = "template_inspect_remote",
        description = "Get detailed template information from the connected Tasker server including step definitions, handler callables, and schema details. Requires namespace, name, and version. Unlike template_inspect (local YAML), this queries the live server."
    )]
    pub async fn template_inspect_remote(
        &self,
        Parameters(params): Parameters<TemplateInspectRemoteParams>,
    ) -> String {
        let client = match self.resolve_client(params.profile.as_deref()).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        connected::template_inspect_remote(&client, params).await
    }

    // ── Tier 3: Write Tools with Confirmation ──

    /// Submit a task for execution against a registered template.
    #[tool(
        name = "task_submit",
        description = "Submit a task for execution against a registered template. Always preview first (omit confirm) to verify the template and context. Use task_inspect after submission to monitor progress."
    )]
    pub async fn task_submit(&self, Parameters(params): Parameters<TaskSubmitParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::task_submit(&client, &profile_name, params).await
    }

    /// Cancel a task and all pending/in-progress steps.
    #[tool(
        name = "task_cancel",
        description = "Cancel a task and all pending/in-progress steps. This is irreversible. Use task_inspect first to verify the target."
    )]
    pub async fn task_cancel(&self, Parameters(params): Parameters<TaskCancelParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::task_cancel(&client, &profile_name, params).await
    }

    /// Reset a failed step for retry by a worker.
    #[tool(
        name = "step_retry",
        description = "Reset a failed step for retry by a worker. Use after investigating via step_inspect and step_audit. The step must be in a failed state."
    )]
    pub async fn step_retry(&self, Parameters(params): Parameters<StepRetryParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::step_retry(&client, &profile_name, params).await
    }

    /// Mark a failed/blocked step as manually resolved without re-execution.
    #[tool(
        name = "step_resolve",
        description = "Mark a failed/blocked step as manually resolved without re-execution. Allows downstream steps to proceed."
    )]
    pub async fn step_resolve(&self, Parameters(params): Parameters<StepResolveParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::step_resolve(&client, &profile_name, params).await
    }

    /// Manually complete a step with specific result data.
    #[tool(
        name = "step_complete",
        description = "Manually complete a step with specific result data. Use when providing corrected data for downstream steps."
    )]
    pub async fn step_complete(
        &self,
        Parameters(params): Parameters<StepCompleteParams>,
    ) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::step_complete(&client, &profile_name, params).await
    }

    /// Update a DLQ entry's investigation status.
    #[tool(
        name = "dlq_update",
        description = "Update a DLQ entry's investigation status. Use after resolving the underlying step-level issue."
    )]
    pub async fn dlq_update(&self, Parameters(params): Parameters<DlqUpdateParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_for_write(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };
        write::dlq_update(&client, &profile_name, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Server-level integration tests ──
    // Tests that verify server construction, routing, offline mode, and profile resolution.
    // Pure tool logic tests live in tools/developer.rs, tools/connected.rs, tools/write.rs.

    #[test]
    fn test_server_info_offline() {
        let server = TaskerMcpServer::new();
        let info = server.get_info();

        assert_eq!(info.server_info.name, "tasker-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        let instructions = info.instructions.unwrap();
        assert!(instructions.contains("OFFLINE"));
        assert!(instructions.contains("template_generate"));
    }

    #[test]
    fn test_server_info_connected() {
        let pm = ProfileManager::offline(); // Empty but not in offline mode
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);
        let info = server.get_info();

        let instructions = info.instructions.unwrap();
        assert!(!instructions.contains("OFFLINE"));
        assert!(instructions.contains("connection_status"));
        assert!(instructions.contains("task_list"));
        assert!(instructions.contains("profile"));
    }

    #[test]
    fn test_server_uses_tasker_tooling() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = tasker_sdk::template_parser::parse_template_str(yaml).unwrap();
        assert_eq!(template.name, "codegen_test");

        let report = tasker_sdk::schema_inspector::inspect(&template);
        assert!(!report.steps.is_empty());
    }

    // ── Profile management routing tests ──

    #[tokio::test]
    async fn test_connection_status_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .connection_status(Parameters(ConnectionStatusParams { refresh: None }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["mode"], "offline");
        assert!(parsed["profiles"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_connection_status_with_profiles() {
        let toml_content = r#"
[profile.default]
description = "Local development"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);

        let result = server
            .connection_status(Parameters(ConnectionStatusParams { refresh: None }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["mode"], "connected");
        assert_eq!(parsed["active_profile"], "default");

        let profiles = parsed["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["name"], "default");
        assert_eq!(profiles[0]["description"], "Local development");
        assert_eq!(profiles[0]["is_active"], true);
    }

    // ── Offline mode routing tests ──
    // Verify that connected tools return offline_mode error through the server routing layer.

    #[tokio::test]
    async fn test_task_list_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .task_list(Parameters(TaskListParams {
                profile: None,
                namespace: None,
                status: None,
                limit: None,
                offset: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "offline_mode");
    }

    #[tokio::test]
    async fn test_task_inspect_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .task_inspect(Parameters(TaskInspectParams {
                profile: None,
                task_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "offline_mode");
    }

    #[tokio::test]
    async fn test_dlq_list_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .dlq_list(Parameters(DlqListToolParams {
                profile: None,
                resolution_status: None,
                limit: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "offline_mode");
    }

    #[tokio::test]
    async fn test_system_health_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .system_health(Parameters(SystemHealthParams { profile: None }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "offline_mode");
    }

    // ── Profile resolution error tests ──

    #[tokio::test]
    async fn test_task_list_profile_not_found() {
        let pm = ProfileManager::offline(); // No profiles loaded
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);
        let result = server
            .task_list(Parameters(TaskListParams {
                profile: Some("nonexistent".to_string()),
                namespace: None,
                status: None,
                limit: None,
                offset: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "profile_not_found");
        assert!(parsed["message"].as_str().unwrap().contains("nonexistent"));
    }

    #[tokio::test]
    async fn test_task_inspect_invalid_uuid() {
        let toml_content = r#"
[profile.default]
description = "Test"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);

        let result = server
            .task_inspect(Parameters(TaskInspectParams {
                profile: None,
                task_uuid: "not-a-uuid".to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "invalid_uuid");
    }

    // ── Tier filtering tests ──

    #[test]
    fn test_server_tier1_only_prunes_connected_tools() {
        let pm = ProfileManager::offline();
        let tiers = EnabledTiers::tier1_only();
        let server = TaskerMcpServer::with_profile_manager(pm, true, Some(tiers));

        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

        // Should have only Tier 1 tools (14), no connection_status (offline)
        assert_eq!(
            names.len(),
            14,
            "Expected 14 Tier 1 tools, got: {:?}",
            names
        );
        assert!(names.contains(&"template_validate"));
        assert!(names.contains(&"template_visualize"));
        assert!(names.contains(&"template_inspect"));
        assert!(names.contains(&"template_generate"));
        assert!(names.contains(&"handler_generate"));
        assert!(names.contains(&"schema_inspect"));
        assert!(names.contains(&"schema_compare"));
        assert!(names.contains(&"schema_diff"));
        // Verify connected tools are pruned
        assert!(!names.contains(&"task_list"));
        assert!(!names.contains(&"task_submit"));
        assert!(!names.contains(&"connection_status"));
    }

    #[test]
    fn test_server_tier1_tier2_prunes_write_tools() {
        let pm = ProfileManager::offline();
        let tiers = EnabledTiers::from_tier_strings(&["tier1".to_string(), "tier2".to_string()]);
        let server = TaskerMcpServer::with_profile_manager(pm, false, Some(tiers));

        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

        // 14 Tier 1 + 1 connection_status + 16 Tier 2 = 31
        assert_eq!(
            names.len(),
            31,
            "Expected 31 tools (T1+profile+T2), got: {:?}",
            names
        );
        assert!(names.contains(&"template_validate"));
        assert!(names.contains(&"task_list"));
        assert!(names.contains(&"connection_status"));
        // Verify write tools are pruned
        assert!(!names.contains(&"task_submit"));
        assert!(!names.contains(&"task_cancel"));
        assert!(!names.contains(&"dlq_update"));
    }

    #[test]
    fn test_server_all_tiers_connected() {
        let pm = ProfileManager::offline();
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);

        let tools = server.tool_router.list_all();
        // 14 T1 + 1 profile + 16 T2 + 6 T3 = 37
        assert_eq!(tools.len(), 37, "Expected all 37 tools");
    }

    #[tokio::test]
    async fn test_write_profile_locked() {
        let toml_content = r#"
[profile.default]
description = "Local"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.staging]
description = "Staging"
transport = "rest"

[profile.staging.orchestration]
base_url = "http://staging:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);

        // Try to write to "staging" when active is "default"
        let result = server
            .task_submit(Parameters(TaskSubmitParams {
                profile: Some("staging".to_string()),
                name: "test".to_string(),
                namespace: "default".to_string(),
                version: None,
                context: serde_json::json!({}),
                initiator: None,
                source_system: None,
                reason: None,
                priority: None,
                tags: vec![],
                confirm: false,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "write_profile_locked");
        assert!(parsed["message"].as_str().unwrap().contains("default"));
        assert!(parsed["message"].as_str().unwrap().contains("staging"));
    }

    #[test]
    fn test_server_info_adapts_to_tiers() {
        // Offline
        let server = TaskerMcpServer::offline();
        let info = server.get_info();
        let instr = info.instructions.unwrap();
        assert!(instr.contains("OFFLINE"));

        // Tier 1 + Tier 2 only (connected)
        let pm = ProfileManager::offline();
        let tiers = EnabledTiers::from_tier_strings(&["tier1".to_string(), "tier2".to_string()]);
        let server = TaskerMcpServer::with_profile_manager(pm, false, Some(tiers));
        let info = server.get_info();
        let instr = info.instructions.unwrap();
        assert!(instr.contains("Read-only tools"));
        assert!(instr.contains("Write tools are not enabled"));

        // All tiers (connected)
        let pm = ProfileManager::offline();
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);
        let info = server.get_info();
        let instr = info.instructions.unwrap();
        assert!(instr.contains("Write tools (require confirm: true)"));
        assert!(instr.contains("locked to the launch profile"));
    }

    #[test]
    fn test_tools_field_in_profile_config() {
        let toml_content = r#"
[profile.default]
description = "Dev with read-only"
transport = "rest"
tools = ["tier1", "tier2"]

[profile.default.orchestration]
base_url = "http://localhost:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let profile = file.profile.get("default").unwrap();
        assert_eq!(
            profile.tools.as_deref(),
            Some(&["tier1".to_string(), "tier2".to_string()][..])
        );
    }

    #[test]
    fn test_profile_tools_config_resolves_tiers() {
        let toml_content = r#"
[profile.default]
description = "Read-only profile"
transport = "rest"
tools = ["tier1", "tier2"]

[profile.default.orchestration]
base_url = "http://localhost:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        // Let the server resolve tiers from profile config (no override)
        let server = TaskerMcpServer::with_profile_manager(pm, false, None);

        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        // Should have T1 + profile + T2, no T3
        assert_eq!(
            names.len(),
            31,
            "Expected 31 tools from profile config, got: {:?}",
            names
        );
        assert!(!names.contains(&"task_submit"));
    }
}
