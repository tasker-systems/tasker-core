//! MCP ServerHandler implementation for Tasker.
//!
//! Provides the MCP server with developer tooling tools (Tier 1, always available)
//! and profile management tools (available when profiles are configured):
//!
//! **Tier 1 — Developer Tooling (offline)**
//! - `template_validate` — Validate a task template for structural correctness
//! - `template_inspect` — Inspect template DAG structure and step details
//! - `template_generate` — Generate task template YAML from a structured spec
//! - `handler_generate` — Generate typed handler code for a template
//! - `schema_inspect` — Inspect result_schema field details per step
//! - `schema_compare` — Compare producer/consumer schema compatibility
//! - `schema_diff` — Detect field-level changes between two template versions
//!
//! **Profile Management (when profiles configured)**
//! - `connection_status` — Show profile health and available capabilities
//!
//! **Tier 2 — Connected Read-Only Tools (require live server)**
//! - `task_list` — List tasks with filtering by namespace/status
//! - `task_inspect` — Get task details with step breakdown
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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::RwLock;
use uuid::Uuid;

use tasker_client::{OrchestrationClient, ProfileManager, UnifiedOrchestrationClient};
use tasker_sdk::codegen::{self, TargetLanguage};
use tasker_sdk::operational::client_factory;
use tasker_sdk::operational::confirmation::{build_preview, handle_api_error, ConfirmationPhase};
use tasker_sdk::operational::responses::{
    BottleneckFilter, BottleneckReport, DlqSummary, HealthReport, PerformanceReport, StepSummary,
    TaskDetail, TaskSummary,
};
use tasker_sdk::schema_comparator;
use tasker_sdk::schema_diff;
use tasker_sdk::schema_inspector;
use tasker_sdk::template_generator;
use tasker_sdk::template_parser::parse_template_str;
use tasker_sdk::template_validator;

use crate::tools::*;

/// Tasker MCP server handler with developer tooling and profile management.
#[derive(Debug, Clone)]
pub struct TaskerMcpServer {
    tool_router: ToolRouter<Self>,
    profile_manager: Arc<RwLock<ProfileManager>>,
    offline: bool,
}

impl Default for TaskerMcpServer {
    fn default() -> Self {
        let pm = ProfileManager::load().unwrap_or_else(|_| ProfileManager::offline());
        Self::with_profile_manager(pm, false)
    }
}

impl TaskerMcpServer {
    /// Create a server with profile management.
    pub fn with_profile_manager(profile_manager: ProfileManager, offline: bool) -> Self {
        Self {
            tool_router: Self::tool_router(),
            profile_manager: Arc::new(RwLock::new(profile_manager)),
            offline,
        }
    }

    /// Create a server in offline mode (Tier 1 developer tools only).
    pub fn offline() -> Self {
        Self::with_profile_manager(ProfileManager::offline(), true)
    }

    /// Create a server with no-arg constructor for backward compatibility in tests.
    pub fn new() -> Self {
        Self::offline()
    }

    /// Get a reference to the profile manager.
    pub fn profile_manager(&self) -> &Arc<RwLock<ProfileManager>> {
        &self.profile_manager
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
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TaskerMcpServer {
    fn get_info(&self) -> ServerInfo {
        let instructions = if self.offline {
            "Tasker is a workflow orchestration system. Running in OFFLINE mode — \
             only developer tooling tools are available (template validation, code generation, \
             schema inspection). Connect to a Tasker instance for task management tools.\n\
             Workflow: template_generate → template_validate → handler_generate (unified types+handlers+tests)\n\
             handler_generate defaults to scaffold mode: handlers import generated types with typed returns.\n\
             When debugging: template_inspect → schema_inspect → schema_compare\n\
             For versioning: schema_diff compares before/after template YAML to detect breaking field changes"
                .to_string()
        } else {
            "Tasker is a workflow orchestration system. You help developers create and validate \
             task templates (workflow definitions) and generate typed handler code.\n\
             Workflow: template_generate → template_validate → handler_generate (unified types+handlers+tests)\n\
             handler_generate defaults to scaffold mode: handlers import generated types with typed returns.\n\
             When debugging: template_inspect → schema_inspect → schema_compare\n\
             For versioning: schema_diff compares before/after template YAML to detect breaking field changes\n\
             Profile management: connection_status to check environment health.\n\
             Read-only tools: task_list/task_inspect for task inspection, step_inspect/step_audit for step details, \
             dlq_list/dlq_inspect/dlq_stats/dlq_queue/staleness_check for DLQ investigation, \
             analytics_performance/analytics_bottlenecks for performance analysis, \
             system_health/system_config for system status, \
             template_list_remote/template_inspect_remote for server-side templates.\n\
             Write tools (require confirm: true): task_submit, task_cancel, step_retry, step_resolve, \
             step_complete, dlq_update. Always preview first (omit confirm), show preview to user, \
             then call again with confirm: true after user approval.\n\
             All connected tools accept an optional 'profile' parameter to target a specific environment."
                .to_string()
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
        match parse_template_str(&params.template_yaml) {
            Ok(template) => {
                let report = template_validator::validate(&template);
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("yaml_parse_error", &e.to_string()),
        }
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
        match parse_template_str(&params.template_yaml) {
            Ok(template) => {
                let schema_report = schema_inspector::inspect(&template);

                // Build dependency maps
                let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
                for step in &template.steps {
                    for dep in &step.dependencies {
                        dependents.entry(dep.as_str()).or_default().push(&step.name);
                    }
                }

                let root_steps: Vec<String> = template
                    .steps
                    .iter()
                    .filter(|s| s.dependencies.is_empty())
                    .map(|s| s.name.clone())
                    .collect();

                let depended_on: HashSet<&str> = template
                    .steps
                    .iter()
                    .flat_map(|s| s.dependencies.iter().map(|d| d.as_str()))
                    .collect();
                let leaf_steps: Vec<String> = template
                    .steps
                    .iter()
                    .filter(|s| !depended_on.contains(s.name.as_str()))
                    .map(|s| s.name.clone())
                    .collect();

                // Topological sort for execution order
                let execution_order = topological_sort(&template);

                let steps: Vec<StepInspection> = template
                    .steps
                    .iter()
                    .map(|step| {
                        let schema_info = schema_report.steps.iter().find(|s| s.name == step.name);
                        StepInspection {
                            name: step.name.clone(),
                            description: step.description.clone(),
                            handler_callable: step.handler.callable.clone(),
                            dependencies: step.dependencies.clone(),
                            dependents: dependents
                                .get(step.name.as_str())
                                .map(|d| d.iter().map(|s| s.to_string()).collect())
                                .unwrap_or_default(),
                            has_result_schema: schema_info
                                .map(|s| s.has_result_schema)
                                .unwrap_or(false),
                            result_field_count: schema_info.and_then(|s| s.property_count),
                        }
                    })
                    .collect();

                let response = TemplateInspectResponse {
                    name: template.name.clone(),
                    namespace: template.namespace_name.clone(),
                    version: template.version.clone(),
                    description: template.description.clone(),
                    step_count: template.steps.len(),
                    has_input_schema: template.input_schema.is_some(),
                    execution_order,
                    root_steps,
                    leaf_steps,
                    steps,
                };

                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("yaml_parse_error", &e.to_string()),
        }
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
        let spec: tasker_sdk::template_generator::TemplateSpec = params.into();
        match template_generator::generate_yaml(&spec) {
            Ok(yaml) => yaml,
            Err(e) => error_json("generation_error", &e.to_string()),
        }
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
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        let language: TargetLanguage = match params.language.parse() {
            Ok(l) => l,
            Err(e) => return error_json("invalid_language", &e.to_string()),
        };

        let step_filter = params.step_filter.as_deref();
        let use_scaffold = params.scaffold.unwrap_or(true);

        if use_scaffold {
            let scaffold_output =
                match codegen::scaffold::generate_scaffold(&template, language, step_filter) {
                    Ok(o) => o,
                    Err(e) => return error_json("codegen_error", &e.to_string()),
                };

            let response = HandlerGenerateResponse {
                language: language.to_string(),
                types: scaffold_output.types,
                handlers: scaffold_output.handlers,
                tests: scaffold_output.tests,
                handler_registry: scaffold_output.handler_registry,
            };

            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        } else {
            let types = match codegen::generate_types(&template, language, step_filter) {
                Ok(t) => t,
                Err(e) => return error_json("codegen_error", &format!("types: {e}")),
            };

            let handlers = match codegen::generate_handlers(&template, language, step_filter) {
                Ok(h) => h,
                Err(e) => return error_json("codegen_error", &format!("handlers: {e}")),
            };

            let tests = match codegen::generate_tests(&template, language, step_filter) {
                Ok(t) => t,
                Err(e) => return error_json("codegen_error", &format!("tests: {e}")),
            };

            let response = HandlerGenerateResponse {
                language: language.to_string(),
                types,
                handlers,
                tests,
                handler_registry: None,
            };

            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
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
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        // Build consumed_by map
        let mut consumed_by: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &template.steps {
            for dep in &step.dependencies {
                consumed_by
                    .entry(dep.as_str())
                    .or_default()
                    .push(&step.name);
            }
        }

        let steps: Vec<StepSchemaDetail> = template
            .steps
            .iter()
            .filter(|s| {
                params
                    .step_filter
                    .as_ref()
                    .is_none_or(|filter| s.name == *filter)
            })
            .map(|step| {
                let fields = step
                    .result_schema
                    .as_ref()
                    .and_then(|schema| codegen::schema::extract_types(&step.name, schema).ok())
                    .map(|type_defs| {
                        // Get the root type (last in dependency order)
                        type_defs
                            .last()
                            .map(|td| {
                                td.fields
                                    .iter()
                                    .map(|f| FieldDetail {
                                        name: f.name.clone(),
                                        field_type: f.field_type.json_schema_type(),
                                        required: f.required,
                                        description: f.description.clone(),
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                StepSchemaDetail {
                    name: step.name.clone(),
                    has_result_schema: step.result_schema.is_some(),
                    fields,
                    consumed_by: consumed_by
                        .get(step.name.as_str())
                        .map(|c| c.iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();

        let response = SchemaInspectResponse {
            template_name: template.name.clone(),
            has_input_schema: template.input_schema.is_some(),
            steps,
        };

        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
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
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        let producer = template
            .steps
            .iter()
            .find(|s| s.name == params.producer_step);
        let consumer = template
            .steps
            .iter()
            .find(|s| s.name == params.consumer_step);

        let Some(producer) = producer else {
            return error_json(
                "step_not_found",
                &format!("Producer step '{}' not found", params.producer_step),
            );
        };
        let Some(consumer) = consumer else {
            return error_json(
                "step_not_found",
                &format!("Consumer step '{}' not found", params.consumer_step),
            );
        };

        let empty_schema = serde_json::json!({"type": "object"});
        let producer_schema = producer.result_schema.as_ref().unwrap_or(&empty_schema);
        let consumer_schema = consumer.result_schema.as_ref().unwrap_or(&empty_schema);

        let report = schema_comparator::compare_schemas(
            &params.producer_step,
            producer_schema,
            &params.consumer_step,
            consumer_schema,
        );

        serde_json::to_string_pretty(&report)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    /// Diff two versions of a task template to detect field-level changes in
    /// result_schema definitions (additions, removals, type changes, required status).
    #[tool(
        name = "schema_diff",
        description = "Compare two versions of the same task template to detect field-level changes. Reports field additions, removals, type changes, and required/optional status changes with breaking-change analysis."
    )]
    pub async fn schema_diff(&self, Parameters(params): Parameters<SchemaDiffParams>) -> String {
        let before = match parse_template_str(&params.before_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &format!("before_yaml: {e}")),
        };
        let after = match parse_template_str(&params.after_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &format!("after_yaml: {e}")),
        };

        let report = schema_diff::diff_templates(&before, &after, params.step_filter.as_deref());

        serde_json::to_string_pretty(&report)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
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

        let limit = params.limit.unwrap_or(20).min(100);
        let offset = params.offset.unwrap_or(0);

        match client
            .as_client()
            .list_tasks(
                limit,
                offset,
                params.namespace.as_deref(),
                params.status.as_deref(),
            )
            .await
        {
            Ok(response) => {
                let summaries: Vec<TaskSummary> =
                    response.tasks.iter().map(TaskSummary::from).collect();
                serde_json::to_string_pretty(&serde_json::json!({
                    "tasks": summaries,
                    "total_count": response.pagination.total_count,
                    "limit": limit,
                    "offset": offset,
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };

        let task = match client.as_client().get_task(task_uuid).await {
            Ok(t) => t,
            Err(e) => return error_json("api_error", &e.to_string()),
        };

        let steps = match client.as_client().list_task_steps(task_uuid).await {
            Ok(s) => s,
            Err(e) => {
                return error_json("api_error", &format!("Task found but steps failed: {}", e))
            }
        };

        let detail = TaskDetail {
            task: serde_json::to_value(&task)
                .unwrap_or_else(|_| serde_json::json!({"error": "serialization_failed"})),
            steps: steps.iter().map(StepSummary::from).collect(),
        };

        serde_json::to_string_pretty(&detail)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
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

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };
        let step_uuid = match Uuid::parse_str(&params.step_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
        };

        match client.as_client().get_step(task_uuid, step_uuid).await {
            Ok(step) => serde_json::to_string_pretty(&step)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };
        let step_uuid = match Uuid::parse_str(&params.step_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
        };

        match client
            .as_client()
            .get_step_audit_history(task_uuid, step_uuid)
            .await
        {
            Ok(audit) => serde_json::to_string_pretty(&audit)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let dlq_params = {
            let status = params
                .resolution_status
                .as_deref()
                .and_then(|s| tasker_sdk::operational::enums::parse_dlq_resolution_status(s).ok());
            Some(tasker_shared::models::orchestration::DlqListParams {
                resolution_status: status,
                limit: params.limit.unwrap_or(20),
                offset: 0,
            })
        };

        match client.list_dlq_entries(dlq_params.as_ref()).await {
            Ok(entries) => {
                let summaries: Vec<DlqSummary> = entries.iter().map(DlqSummary::from).collect();
                serde_json::to_string_pretty(&serde_json::json!({
                    "entries": summaries,
                    "count": summaries.len(),
                }))
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };

        match client.get_dlq_entry(task_uuid).await {
            Ok(entry) => serde_json::to_string_pretty(&serde_json::json!({
                "dlq_entry_uuid": entry.dlq_entry_uuid.to_string(),
                "task_uuid": entry.task_uuid.to_string(),
                "original_state": entry.original_state,
                "dlq_reason": format!("{:?}", entry.dlq_reason),
                "dlq_timestamp": entry.dlq_timestamp.to_string(),
                "resolution_status": format!("{:?}", entry.resolution_status),
                "resolution_timestamp": entry.resolution_timestamp.map(|t| t.to_string()),
                "resolution_notes": entry.resolution_notes,
                "resolved_by": entry.resolved_by,
                "task_snapshot": entry.task_snapshot,
                "metadata": entry.metadata,
                "created_at": entry.created_at.to_string(),
                "updated_at": entry.updated_at.to_string(),
            }))
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client.get_dlq_stats().await {
            Ok(stats) => serde_json::to_string_pretty(&stats)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client.get_investigation_queue(params.limit).await {
            Ok(queue) => serde_json::to_string_pretty(&queue)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client.get_staleness_monitoring(params.limit).await {
            Ok(monitoring) => serde_json::to_string_pretty(&monitoring)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let query = params
            .hours
            .map(|h| tasker_shared::types::api::orchestration::MetricsQuery { hours: Some(h) });

        match client.get_performance_metrics(query.as_ref()).await {
            Ok(metrics) => {
                let report = PerformanceReport {
                    metrics: serde_json::to_value(&metrics)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    period_hours: params.hours,
                };
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        let query = if params.limit.is_some() || params.min_executions.is_some() {
            Some(tasker_shared::types::api::orchestration::BottleneckQuery {
                limit: params.limit,
                min_executions: params.min_executions,
            })
        } else {
            None
        };

        match client.get_bottlenecks(query.as_ref()).await {
            Ok(analysis) => {
                let report = BottleneckReport {
                    analysis: serde_json::to_value(&analysis)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    filter: BottleneckFilter {
                        limit: params.limit,
                        min_executions: params.min_executions,
                    },
                };
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client.as_client().get_detailed_health().await {
            Ok(health) => {
                let report = HealthReport {
                    overall_status: health.status.clone(),
                    timestamp: health.timestamp.clone(),
                    components: serde_json::to_value(&health.checks)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    system_info: serde_json::to_value(&health.info)
                        .unwrap_or_else(|_| serde_json::json!({})),
                };
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client.get_config().await {
            Ok(config) => serde_json::to_string_pretty(&config)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client
            .as_client()
            .list_templates(params.namespace.as_deref())
            .await
        {
            Ok(templates) => serde_json::to_string_pretty(&templates)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
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

        match client
            .as_client()
            .get_template(&params.namespace, &params.name, &params.version)
            .await
        {
            Ok(template) => serde_json::to_string_pretty(&template)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
            Err(e) => error_json("api_error", &e.to_string()),
        }
    }

    // ── Tier 3: Write Tools with Confirmation ──

    /// Submit a task for execution against a registered template.
    #[tool(
        name = "task_submit",
        description = "Submit a task for execution against a registered template. Always preview first (omit confirm) to verify the template and context. Use task_inspect after submission to monitor progress."
    )]
    pub async fn task_submit(&self, Parameters(params): Parameters<TaskSubmitParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let version = params.version.as_deref().unwrap_or("0.1.0");

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => {
                let preview = build_preview(
                    "task_submit",
                    &format!(
                        "Submit task '{}' in namespace '{}' version '{}'",
                        params.name, params.namespace, version
                    ),
                    serde_json::json!({
                        "name": params.name,
                        "namespace": params.namespace,
                        "version": version,
                        "context_keys": params.context.as_object()
                            .map(|o| o.keys().cloned().collect::<Vec<_>>())
                            .unwrap_or_default(),
                        "initiator": params.initiator.as_deref().unwrap_or("mcp-agent"),
                        "source_system": params.source_system.as_deref().unwrap_or("tasker-mcp"),
                        "tags": params.tags,
                        "priority": params.priority,
                    }),
                );
                serde_json::to_string_pretty(&preview)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            ConfirmationPhase::Execute => {
                let request = tasker_shared::models::core::task_request::TaskRequest::builder()
                    .name(params.name)
                    .namespace(params.namespace)
                    .version(version.to_string())
                    .context(params.context)
                    .initiator(params.initiator.unwrap_or_else(|| "mcp-agent".to_string()))
                    .source_system(
                        params
                            .source_system
                            .unwrap_or_else(|| "tasker-mcp".to_string()),
                    )
                    .reason(
                        params
                            .reason
                            .unwrap_or_else(|| "Submitted via MCP".to_string()),
                    )
                    .tags(params.tags)
                    .maybe_priority(params.priority)
                    .build();

                match client.as_client().create_task(request).await {
                    Ok(response) => serde_json::to_string_pretty(&serde_json::json!({
                        "status": "executed",
                        "action": "task_submit",
                        "result": response,
                    }))
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                    Err(e) => handle_api_error(&e.to_string(), "task_submit", &profile_name),
                }
            }
        }
    }

    /// Cancel a task and all pending/in-progress steps.
    #[tool(
        name = "task_cancel",
        description = "Cancel a task and all pending/in-progress steps. This is irreversible. Use task_inspect first to verify the target."
    )]
    pub async fn task_cancel(&self, Parameters(params): Parameters<TaskCancelParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => match client.as_client().get_task(task_uuid).await {
                Ok(task) => {
                    let preview = build_preview(
                        "task_cancel",
                        &format!("Cancel task '{}'", params.task_uuid),
                        serde_json::json!({
                            "task_uuid": params.task_uuid,
                            "current_state": serde_json::to_value(&task)
                                .unwrap_or_else(|_| serde_json::json!({})),
                        }),
                    );
                    serde_json::to_string_pretty(&preview)
                        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                }
                Err(e) => handle_api_error(&e.to_string(), "task_cancel", &profile_name),
            },
            ConfirmationPhase::Execute => match client.as_client().cancel_task(task_uuid).await {
                Ok(()) => serde_json::json!({
                    "status": "executed",
                    "action": "task_cancel",
                    "task_uuid": params.task_uuid,
                    "message": "Task cancelled successfully."
                })
                .to_string(),
                Err(e) => handle_api_error(&e.to_string(), "task_cancel", &profile_name),
            },
        }
    }

    /// Reset a failed step for retry by a worker.
    #[tool(
        name = "step_retry",
        description = "Reset a failed step for retry by a worker. Use after investigating via step_inspect and step_audit. The step must be in a failed state."
    )]
    pub async fn step_retry(&self, Parameters(params): Parameters<StepRetryParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };
        let step_uuid = match Uuid::parse_str(&params.step_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
        };

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => {
                match client.as_client().get_step(task_uuid, step_uuid).await {
                    Ok(step) => {
                        let preview = build_preview(
                            "step_retry",
                            &format!(
                                "Reset step '{}' for retry on task '{}'",
                                params.step_uuid, params.task_uuid
                            ),
                            serde_json::json!({
                                "task_uuid": params.task_uuid,
                                "step_uuid": params.step_uuid,
                                "current_step": serde_json::to_value(&step)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                                "reason": params.reason,
                                "reset_by": params.reset_by,
                            }),
                        );
                        serde_json::to_string_pretty(&preview)
                            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                    }
                    Err(e) => handle_api_error(&e.to_string(), "step_retry", &profile_name),
                }
            }
            ConfirmationPhase::Execute => {
                let action =
                    tasker_shared::types::api::orchestration::StepManualAction::ResetForRetry {
                        reason: params.reason,
                        reset_by: params.reset_by,
                    };

                match client
                    .as_client()
                    .resolve_step_manually(task_uuid, step_uuid, action)
                    .await
                {
                    Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                        "status": "executed",
                        "action": "step_retry",
                        "result": step,
                    }))
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                    Err(e) => handle_api_error(&e.to_string(), "step_retry", &profile_name),
                }
            }
        }
    }

    /// Mark a failed/blocked step as manually resolved without re-execution.
    #[tool(
        name = "step_resolve",
        description = "Mark a failed/blocked step as manually resolved without re-execution. Allows downstream steps to proceed."
    )]
    pub async fn step_resolve(&self, Parameters(params): Parameters<StepResolveParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };
        let step_uuid = match Uuid::parse_str(&params.step_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
        };

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => {
                match client.as_client().get_step(task_uuid, step_uuid).await {
                    Ok(step) => {
                        let preview = build_preview(
                            "step_resolve",
                            &format!(
                                "Mark step '{}' as manually resolved on task '{}'",
                                params.step_uuid, params.task_uuid
                            ),
                            serde_json::json!({
                                "task_uuid": params.task_uuid,
                                "step_uuid": params.step_uuid,
                                "current_step": serde_json::to_value(&step)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                                "reason": params.reason,
                                "resolved_by": params.resolved_by,
                            }),
                        );
                        serde_json::to_string_pretty(&preview)
                            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                    }
                    Err(e) => handle_api_error(&e.to_string(), "step_resolve", &profile_name),
                }
            }
            ConfirmationPhase::Execute => {
                let action =
                    tasker_shared::types::api::orchestration::StepManualAction::ResolveManually {
                        reason: params.reason,
                        resolved_by: params.resolved_by,
                    };

                match client
                    .as_client()
                    .resolve_step_manually(task_uuid, step_uuid, action)
                    .await
                {
                    Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                        "status": "executed",
                        "action": "step_resolve",
                        "result": step,
                    }))
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                    Err(e) => handle_api_error(&e.to_string(), "step_resolve", &profile_name),
                }
            }
        }
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
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let task_uuid = match Uuid::parse_str(&params.task_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
        };
        let step_uuid = match Uuid::parse_str(&params.step_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid step_uuid: {}", e)),
        };

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => {
                match client.as_client().get_step(task_uuid, step_uuid).await {
                    Ok(step) => {
                        let preview = build_preview(
                            "step_complete",
                            &format!(
                                "Manually complete step '{}' on task '{}'",
                                params.step_uuid, params.task_uuid
                            ),
                            serde_json::json!({
                                "task_uuid": params.task_uuid,
                                "step_uuid": params.step_uuid,
                                "current_step": serde_json::to_value(&step)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                                "result_data": params.result,
                                "reason": params.reason,
                                "completed_by": params.completed_by,
                            }),
                        );
                        serde_json::to_string_pretty(&preview)
                            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
                    }
                    Err(e) => handle_api_error(&e.to_string(), "step_complete", &profile_name),
                }
            }
            ConfirmationPhase::Execute => {
                let completion_data =
                    tasker_shared::types::api::orchestration::ManualCompletionData {
                        result: params.result,
                        metadata: params.metadata,
                    };
                let action =
                    tasker_shared::types::api::orchestration::StepManualAction::CompleteManually {
                        completion_data,
                        reason: params.reason,
                        completed_by: params.completed_by,
                    };

                match client
                    .as_client()
                    .resolve_step_manually(task_uuid, step_uuid, action)
                    .await
                {
                    Ok(step) => serde_json::to_string_pretty(&serde_json::json!({
                        "status": "executed",
                        "action": "step_complete",
                        "result": step,
                    }))
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string())),
                    Err(e) => handle_api_error(&e.to_string(), "step_complete", &profile_name),
                }
            }
        }
    }

    /// Update a DLQ entry's investigation status.
    #[tool(
        name = "dlq_update",
        description = "Update a DLQ entry's investigation status. Use after resolving the underlying step-level issue."
    )]
    pub async fn dlq_update(&self, Parameters(params): Parameters<DlqUpdateParams>) -> String {
        let (client, profile_name) = match self
            .resolve_client_with_profile(params.profile.as_deref())
            .await
        {
            Ok(c) => c,
            Err(e) => return e,
        };

        let dlq_entry_uuid = match Uuid::parse_str(&params.dlq_entry_uuid) {
            Ok(u) => u,
            Err(e) => return error_json("invalid_uuid", &format!("Invalid dlq_entry_uuid: {}", e)),
        };

        match ConfirmationPhase::from_flag(params.confirm) {
            ConfirmationPhase::Preview => {
                let preview = build_preview(
                    "dlq_update",
                    &format!("Update DLQ entry '{}'", params.dlq_entry_uuid),
                    serde_json::json!({
                        "dlq_entry_uuid": params.dlq_entry_uuid,
                        "resolution_status": params.resolution_status,
                        "resolution_notes": params.resolution_notes,
                        "resolved_by": params.resolved_by,
                        "has_metadata": params.metadata.is_some(),
                    }),
                );
                serde_json::to_string_pretty(&preview)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            ConfirmationPhase::Execute => {
                let resolution_status = params
                    .resolution_status
                    .as_deref()
                    .map(tasker_sdk::operational::enums::parse_dlq_resolution_status)
                    .transpose()
                    .map_err(|e| error_json("invalid_resolution_status", &e));

                let resolution_status = match resolution_status {
                    Ok(s) => s,
                    Err(e) => return e,
                };

                let update = tasker_shared::models::orchestration::DlqInvestigationUpdate {
                    resolution_status,
                    resolution_notes: params.resolution_notes,
                    resolved_by: params.resolved_by,
                    metadata: params.metadata,
                };

                match client
                    .update_dlq_investigation(dlq_entry_uuid, update)
                    .await
                {
                    Ok(()) => serde_json::json!({
                        "status": "executed",
                        "action": "dlq_update",
                        "dlq_entry_uuid": params.dlq_entry_uuid,
                        "message": "DLQ entry updated successfully."
                    })
                    .to_string(),
                    Err(e) => handle_api_error(&e.to_string(), "dlq_update", &profile_name),
                }
            }
        }
    }
}

/// Build a structured error JSON string that LLMs can parse.
fn error_json(error_code: &str, message: &str) -> String {
    serde_json::json!({
        "error": error_code,
        "message": message,
        "valid": false
    })
    .to_string()
}

/// Simple topological sort via Kahn's algorithm.
fn topological_sort(
    template: &tasker_shared::models::core::task_template::TaskTemplate,
) -> Vec<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &template.steps {
        in_degree.entry(&step.name).or_insert(0);
        for dep in &step.dependencies {
            adj.entry(dep.as_str()).or_default().push(&step.name);
            *in_degree.entry(&step.name).or_insert(0) += 1;
        }
    }

    let mut queue: std::collections::VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();

    // Sort the initial queue for deterministic output
    let mut sorted_queue: Vec<&str> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next_batch = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_batch.push(neighbor);
                    }
                }
            }
            next_batch.sort();
            queue.extend(next_batch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let server = TaskerMcpServer::with_profile_manager(pm, false);
        let info = server.get_info();

        let instructions = info.instructions.unwrap();
        assert!(!instructions.contains("OFFLINE"));
        assert!(instructions.contains("connection_status"));
        assert!(instructions.contains("task_list"));
        assert!(instructions.contains("profile"));
    }

    #[test]
    fn test_server_uses_tasker_tooling() {
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = tasker_sdk::template_parser::parse_template_str(yaml).unwrap();
        assert_eq!(template.name, "codegen_test");

        let report = tasker_sdk::schema_inspector::inspect(&template);
        assert!(!report.steps.is_empty());
    }

    #[tokio::test]
    async fn test_template_validate_valid() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 5);
    }

    #[tokio::test]
    async fn test_template_validate_cycle() {
        let server = TaskerMcpServer::new();
        let yaml = r#"
name: cycle_test
namespace_name: test
version: "1.0.0"
steps:
  - name: a
    handler:
      callable: test.a
    depends_on: [b]
  - name: b
    handler:
      callable: test.b
    depends_on: [a]
"#;
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["has_cycles"], true);
    }

    #[tokio::test]
    async fn test_template_validate_invalid_yaml() {
        let server = TaskerMcpServer::new();
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: "not: [valid: yaml".to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "yaml_parse_error");
    }

    #[tokio::test]
    async fn test_template_inspect() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .template_inspect(Parameters(TemplateInspectParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "codegen_test");
        assert_eq!(parsed["step_count"], 5);
        assert!(parsed["execution_order"].as_array().unwrap().len() == 5);
        assert!(parsed["root_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "validate_order"));
    }

    #[tokio::test]
    async fn test_template_generate() {
        let server = TaskerMcpServer::new();
        let result = server
            .template_generate(Parameters(TemplateGenerateParams {
                name: "test_task".into(),
                namespace: "ns".into(),
                version: None,
                description: Some("Test".into()),
                steps: vec![StepSpecParam {
                    name: "step_one".into(),
                    description: None,
                    handler: None,
                    depends_on: vec![],
                    outputs: vec![FieldSpecParam {
                        name: "result".into(),
                        field_type: "string".into(),
                        required: true,
                        description: None,
                    }],
                }],
            }))
            .await;
        assert!(result.contains("test_task"));
        assert!(result.contains("ns.step_one"));
    }

    #[tokio::test]
    async fn test_handler_generate() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: Some("validate_order".into()),
                scaffold: Some(true),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        // Scaffold mode: handlers import types
        assert!(parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[tokio::test]
    async fn test_handler_generate_no_scaffold() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: Some("validate_order".into()),
                scaffold: Some(false),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        // Non-scaffold mode: no type imports in handlers
        assert!(!parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[tokio::test]
    async fn test_handler_generate_invalid_language() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "cobol".into(),
                step_filter: None,
                scaffold: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "invalid_language");
    }

    #[tokio::test]
    async fn test_schema_inspect() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_inspect(Parameters(SchemaInspectParams {
                template_yaml: yaml.to_string(),
                step_filter: Some("validate_order".into()),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["template_name"], "codegen_test");
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert!(steps[0]["has_result_schema"].as_bool().unwrap());
        assert!(!steps[0]["fields"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_schema_compare() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_compare(Parameters(SchemaCompareParams {
                template_yaml: yaml.to_string(),
                producer_step: "validate_order".into(),
                consumer_step: "enrich_order".into(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["compatibility"].is_string());
        assert!(parsed["findings"].is_array());
    }

    #[tokio::test]
    async fn test_schema_compare_step_not_found() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_compare(Parameters(SchemaCompareParams {
                template_yaml: yaml.to_string(),
                producer_step: "nonexistent".into(),
                consumer_step: "enrich_order".into(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "step_not_found");
    }

    #[tokio::test]
    async fn test_schema_diff() {
        let server = TaskerMcpServer::new();
        let before_yaml = r#"
name: diff_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      required: [id, name]
      properties:
        id:
          type: string
        name:
          type: string
"#;
        let after_yaml = r#"
name: diff_test
namespace_name: test
version: "2.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      required: [id]
      properties:
        id:
          type: string
        email:
          type: string
"#;
        let result = server
            .schema_diff(Parameters(SchemaDiffParams {
                before_yaml: before_yaml.to_string(),
                after_yaml: after_yaml.to_string(),
                step_filter: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["compatibility"], "incompatible");
        let diffs = parsed["step_diffs"].as_array().unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["status"], "modified");

        let findings = diffs[0]["findings"].as_array().unwrap();
        // Should detect: FIELD_ADDED (email), FIELD_REMOVED (name, required→breaking),
        // REQUIRED_TO_OPTIONAL (name was required, removed is separate)
        assert!(findings.iter().any(|f| f["code"] == "FIELD_ADDED"));
        assert!(findings
            .iter()
            .any(|f| f["code"] == "FIELD_REMOVED" && f["breaking"] == true));
    }

    #[tokio::test]
    async fn test_content_publishing_template_validates() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 7);
    }

    #[tokio::test]
    async fn test_content_publishing_template_inspect() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .template_inspect(Parameters(TemplateInspectParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "publish_article");
        assert_eq!(parsed["step_count"], 7);

        let root_steps = parsed["root_steps"].as_array().unwrap();
        assert_eq!(root_steps.len(), 1);
        assert_eq!(root_steps[0], "validate_content");

        let leaf_steps = parsed["leaf_steps"].as_array().unwrap();
        assert_eq!(leaf_steps.len(), 1);
        assert_eq!(leaf_steps[0], "update_analytics");
    }

    #[tokio::test]
    async fn test_content_publishing_handler_generate() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: None,
                scaffold: Some(true),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        assert!(parsed["tests"].as_str().unwrap().contains("def test_"));
    }

    // ── Profile management tool tests ──

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

    // ── Tier 2 tool tests ──

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

    #[tokio::test]
    async fn test_task_list_profile_not_found() {
        let pm = ProfileManager::offline(); // No profiles loaded
        let server = TaskerMcpServer::with_profile_manager(pm, false);
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
        let server = TaskerMcpServer::with_profile_manager(pm, false);

        let result = server
            .task_inspect(Parameters(TaskInspectParams {
                profile: None,
                task_uuid: "not-a-uuid".to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "invalid_uuid");
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
        let server = TaskerMcpServer::with_profile_manager(pm, false);

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
}
