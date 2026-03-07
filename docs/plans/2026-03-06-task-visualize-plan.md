# TAS-317: Task Visualize Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Visualize live task execution state as Mermaid diagrams with an enriched summary API endpoint, refactoring TAS-316 template visualization to share a unified structured data architecture.

**Architecture:** Orchestration server returns enriched JSON via `GET /v1/tasks/{uuid}/summary` (backed by a SQL function). tasker-sdk produces structured `GraphData`/`TableData` from that response. Shared renderers convert structured data to Mermaid syntax and markdown tables. Callers (CLI, MCP) own URL construction via `base_url`.

**Tech Stack:** Rust, PostgreSQL (SQL functions), sqlx, Mermaid, protobuf (gRPC), clap (CLI), rmcp (MCP)

**Design Document:** `docs/plans/2026-03-06-task-visualize-design.md`

---

## Task 1: Extract Shared Visualization Types

Refactor TAS-316's visualization module to extract shared types that both template and task visualization will use.

**Files:**
- Create: `crates/tasker-sdk/src/visualization/types.rs`
- Modify: `crates/tasker-sdk/src/visualization/mod.rs`

**Step 1: Create `types.rs` with shared types**

Create `crates/tasker-sdk/src/visualization/types.rs` with:

```rust
//! Shared types for template and task visualization.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub visual_category: VisualCategory,
    pub node_shape: NodeShape,
    pub resource_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualCategory {
    Completed,
    InProgress,
    Pending,
    Error,
    Retrying,
    Annotated,
    Untraversed,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    Rectangle,
    Diamond,
    Trapezoid,
    Subroutine,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_style: EdgeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeStyle {
    Solid,
    Dashed,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableData {
    pub header: Option<TableHeader>,
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableHeader {
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: Option<String>,
    pub completion_percentage: Option<f64>,
    pub health_status: Option<String>,
    pub elapsed: Option<String>,
    pub dlq_status: Option<String>,
    pub task_link: Option<String>,
    pub dlq_link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableRow {
    pub name: String,
    pub step_type: String,
    pub handler: String,
    // Template-only columns
    pub dependencies: Option<String>,
    pub schema_fields: Option<String>,
    pub retry_info: Option<String>,
    // Task-only columns
    pub state: Option<String>,
    pub visual_category_label: Option<String>,
    pub duration: Option<String>,
    pub attempts: Option<String>,
    pub error_type: Option<String>,
    pub step_link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VisualizationOutput {
    pub graph: GraphData,
    pub table: TableData,
    pub warnings: Vec<String>,
}
```

**Step 2: Register module in `mod.rs`**

Add `pub mod types;` to `crates/tasker-sdk/src/visualization/mod.rs` and re-export the key types.

**Step 3: Verify compilation**

Run: `cargo check --all-features -p tasker-sdk`

**Step 4: Commit**

```
feat(TAS-317): extract shared visualization types into types.rs
```

---

## Task 2: Extract Shared Renderers

Move Mermaid rendering and detail table rendering into shared renderers that consume the structured types.

**Files:**
- Create: `crates/tasker-sdk/src/visualization/render.rs`
- Modify: `crates/tasker-sdk/src/visualization/mod.rs`

**Step 1: Write failing tests for `render_mermaid`**

In `render.rs`, write tests that build `GraphData` with known nodes/edges and assert the Mermaid output. Test:
- `classDef` lines for each `VisualCategory`
- Node shapes: Rectangle `["label"]`, Diamond `{"label"}`, Trapezoid `[/"label"/]`, Subroutine `[["label"]]`
- `:::category` class assignment on nodes
- Solid edges (`-->`) vs dashed edges (`-.->`)
- Deterministic edge ordering (sorted)

**Step 2: Run tests to verify they fail**

Run: `cargo test --features test-messaging -p tasker-sdk render`
Expected: compilation errors (functions don't exist yet)

**Step 3: Implement `render_mermaid`**

```rust
pub fn render_mermaid(graph: &GraphData) -> String
```

Logic:
- Emit `graph TD\n`
- Emit `classDef` lines for: completed, inProgress, pending, error, retrying, annotated, untraversed, default
- For each node, format using `node_shape` to determine Mermaid syntax, apply `:::category`
- For each edge, use `-->` (Solid) or `-.->` (Dashed)
- Sort edges for deterministic output

Reference existing patterns in `mermaid.rs` for `format_node` — this is the same logic generalized.

**Step 4: Run tests to verify they pass**

Run: `cargo test --features test-messaging -p tasker-sdk render`

**Step 5: Write failing tests for `render_detail_table`**

Test markdown table rendering with:
- Template-mode rows (dependencies, schema_fields, retry_info columns)
- Task-mode rows (state, duration, attempts, error_type, step_link columns)
- `base_url` prepending to links
- `None` base_url leaves resource paths relative

**Step 6: Implement `render_detail_table` and `render_markdown`**

```rust
pub fn render_detail_table(table: &TableData, base_url: Option<&str>) -> String
pub fn render_markdown(title: &str, output: &VisualizationOutput, base_url: Option<&str>, graph_only: bool) -> String
```

**Step 7: Run tests to verify they pass**

Run: `cargo test --features test-messaging -p tasker-sdk render`

**Step 8: Commit**

```
feat(TAS-317): add shared Mermaid and detail table renderers
```

---

## Task 3: Refactor Template Visualization to Structured Data

Refactor `visualize_template` to produce `VisualizationOutput` (structured data) and add a backward-compatible `visualize_template_rendered` wrapper.

**Files:**
- Create: `crates/tasker-sdk/src/visualization/template_visualize.rs`
- Modify: `crates/tasker-sdk/src/visualization/mod.rs`
- Delete (absorbed): `crates/tasker-sdk/src/visualization/mermaid.rs`, `crates/tasker-sdk/src/visualization/detail_table.rs`

**Step 1: Create `template_visualize.rs`**

Move the template → graph/table logic from `mermaid.rs` and `detail_table.rs` into a single function:

```rust
pub fn build_template_visualization(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
) -> VisualizationOutput
```

This produces `GraphData` and `TableData` using:
- `NodeShape` from `StepType` (Standard→Rectangle, Decision→Diamond, DeferredConvergence→Trapezoid, Batchable→Trapezoid, BatchWorker→Subroutine)
- `VisualCategory::Annotated` for annotated steps, `VisualCategory::Default` otherwise
- `EdgeStyle::Solid` for all template edges
- `TableRow` with template columns populated (dependencies, schema_fields, retry_info)
- Topological ordering for table rows (reuse existing `topological_order` logic)

**Step 2: Update `mod.rs` public API**

```rust
pub fn visualize_template(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> VisualizationOutput  // <-- now returns structured data

pub fn visualize_template_rendered(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> RenderedOutput  // <-- backward-compatible wrapper

pub struct RenderedOutput {
    pub mermaid: String,
    pub detail_table: Option<String>,
    pub markdown: String,
    pub warnings: Vec<String>,
}
```

`visualize_template_rendered` calls `visualize_template` then renders via `render_mermaid` and `render_detail_table`.

**Step 3: Remove `mermaid.rs` and `detail_table.rs`**

Their logic is now in `template_visualize.rs` and `render.rs`.

**Step 4: Run ALL existing tests**

Run: `cargo test --features test-messaging -p tasker-sdk visualization`

All existing tests in `mod.rs` (test_full_markdown_output, test_graph_only_mode, test_annotation_warning, test_diamond_dag_full_output, test_linear_chain_full_output) must pass via `visualize_template_rendered`. Tests from `mermaid.rs` and `detail_table.rs` are migrated to `template_visualize.rs`.

**Step 5: Update CLI caller**

In `crates/tasker-ctl/src/commands/template.rs`, update `visualize_template_command` to use `visualize_template_rendered` (just a function rename if the old `visualize_template` returned the same shape — but it now returns `VisualizationOutput`, so callers must switch to the rendered wrapper).

**Step 6: Update MCP caller**

In `crates/tasker-mcp/src/tools/developer.rs`, update `template_visualize` to use `visualize_template_rendered`.

**Step 7: Run full test suite for affected crates**

Run: `cargo test --features test-messaging -p tasker-sdk -p tasker-ctl -p tasker-mcp`

**Step 8: Commit**

```
refactor(TAS-317): unify template visualization with shared types and renderers
```

---

## Task 4: SQL Migration — `get_task_summaries` Function

Create the SQL function that powers the summary endpoint.

**Files:**
- Create: `migrations/20260306000001_task_summary_function.sql`

**Step 1: Write the migration**

Create `get_task_summaries(input_task_uuids uuid[])` function that returns:

| Column | Type | Source |
|--------|------|--------|
| task_uuid | uuid | tasks.task_uuid |
| named_task_uuid | uuid | tasks.named_task_uuid |
| task_name | text | named_tasks.name |
| task_version | text | named_tasks.version |
| namespace_name | text | task_namespaces.name |
| task_status | text | task_transitions (most_recent) |
| created_at | timestamptz | tasks.created_at |
| updated_at | timestamptz | tasks.updated_at |
| completed_at | timestamptz | tasks.completed_at |
| initiator | text | tasks.initiator |
| source_system | text | tasks.source_system |
| reason | text | tasks.reason |
| correlation_id | uuid | tasks.correlation_id |
| template_configuration | jsonb | named_tasks.configuration |
| step_summaries | jsonb | aggregated step data (see below) |
| execution_context | jsonb | aggregated counts and status |
| dlq_status | jsonb | tasks_dlq (pending entry if exists) |

`step_summaries` is a JSONB array built with `json_agg(json_build_object(...))` containing:
- step_uuid, name, current_state, created_at, completed_at, last_attempted_at
- attempts, max_attempts, dependencies_satisfied, retry_eligible
- error_type, error_retryable, error_status_code (extracted from `workflow_steps.results -> 'error'`)

`execution_context` is a JSONB object with: total_steps, pending_steps, in_progress_steps, completed_steps, failed_steps, completion_percentage, health_status, execution_status, recommended_action.

`dlq_status` is a JSONB object with: in_dlq (bool), dlq_reason, resolution_status — from LEFT JOIN to tasks_dlq.

Also create `get_task_summary(input_task_uuid uuid)` as a wrapper:

```sql
CREATE FUNCTION tasker.get_task_summary(input_task_uuid uuid)
RETURNS TABLE(...same columns...)
LANGUAGE sql STABLE
AS $$
  SELECT * FROM tasker.get_task_summaries(ARRAY[input_task_uuid]::uuid[]);
$$;
```

**Step 2: Run migration**

Run: `cargo make db-migrate`

**Step 3: Update SQLx cache**

Run: `DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test cargo sqlx prepare --workspace -- --all-targets --all-features`
Then: `git add .sqlx/`

**Step 4: Commit**

```
feat(TAS-317): add get_task_summary/get_task_summaries SQL functions
```

---

## Task 5: Rust Model — `TaskSummary` in tasker-shared

Create the Rust model that maps to the SQL function output.

**Files:**
- Create: `crates/tasker-shared/src/models/orchestration/task_summary.rs`
- Modify: `crates/tasker-shared/src/models/orchestration/mod.rs`

**Step 1: Write failing tests**

In `task_summary.rs`, write tests for:
- Deserialization of `StepSummaryData` from JSON (matching the JSONB shape from SQL)
- Deserialization of `DlqSummaryData` from JSON
- `VisualCategory::from_state()` mapping for all 10 step states + "untraversed"

**Step 2: Run tests to verify they fail**

Run: `cargo test --features test-messaging -p tasker-shared task_summary`

**Step 3: Implement the model**

```rust
//! Task summary — computed view via SQL function for visualization.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummaryRow {
    pub task_uuid: Uuid,
    pub named_task_uuid: Uuid,
    pub task_name: String,
    pub task_version: String,
    pub namespace_name: String,
    pub task_status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub initiator: String,
    pub source_system: String,
    pub reason: String,
    pub correlation_id: Uuid,
    pub template_configuration: Option<serde_json::Value>,
    pub step_summaries: serde_json::Value,     // JSONB array
    pub execution_context: serde_json::Value,  // JSONB object
    pub dlq_status: serde_json::Value,         // JSONB object
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSummaryData {
    pub step_uuid: String,
    pub name: String,
    pub current_state: String,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_attempted_at: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub dependencies_satisfied: bool,
    pub retry_eligible: bool,
    pub error_type: Option<String>,
    pub error_retryable: Option<bool>,
    pub error_status_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContextData {
    pub total_steps: i64,
    pub pending_steps: i64,
    pub in_progress_steps: i64,
    pub completed_steps: i64,
    pub failed_steps: i64,
    pub completion_percentage: f64,
    pub health_status: String,
    pub execution_status: String,
    pub recommended_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqSummaryData {
    pub in_dlq: bool,
    pub dlq_reason: Option<String>,
    pub resolution_status: Option<String>,
}

impl TaskSummaryRow {
    pub async fn get_for_task(pool: &PgPool, task_uuid: Uuid) -> Result<Option<Self>, sqlx::Error> {
        // calls get_task_summary(task_uuid)
    }

    pub async fn get_for_tasks(pool: &PgPool, task_uuids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
        // calls get_task_summaries(task_uuids)
    }

    pub fn step_summaries(&self) -> Vec<StepSummaryData> {
        serde_json::from_value(self.step_summaries.clone()).unwrap_or_default()
    }

    pub fn execution_context(&self) -> ExecutionContextData {
        serde_json::from_value(self.execution_context.clone()).unwrap_or(/* defaults */)
    }

    pub fn dlq(&self) -> DlqSummaryData {
        serde_json::from_value(self.dlq_status.clone()).unwrap_or(DlqSummaryData { in_dlq: false, dlq_reason: None, resolution_status: None })
    }
}
```

**Step 4: Register in `mod.rs`**

Add `pub mod task_summary;` and re-exports in `crates/tasker-shared/src/models/orchestration/mod.rs`.

**Step 5: Run tests**

Run: `cargo test --features test-messaging -p tasker-shared task_summary`

**Step 6: Commit**

```
feat(TAS-317): add TaskSummaryRow model in tasker-shared
```

---

## Task 6: API Response Type — `TaskSummaryResponse`

Create the API response type used by REST and gRPC handlers.

**Files:**
- Modify: `crates/tasker-shared/src/types/api/orchestration.rs`

**Step 1: Add response type**

Add to `crates/tasker-shared/src/types/api/orchestration.rs`:

```rust
/// Task summary response for visualization — enriched single-call endpoint.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct TaskSummaryResponse {
    pub task: TaskSummaryMetadata,
    pub template: TemplateSummary,
    pub steps: Vec<StepSummaryInfo>,
    pub dlq: DlqSummaryInfo,
    pub links: TaskSummaryLinks,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct TaskSummaryMetadata {
    pub task_uuid: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub initiator: String,
    pub source_system: String,
    pub reason: String,
    pub correlation_id: Uuid,
    pub total_steps: i64,
    pub pending_steps: i64,
    pub in_progress_steps: i64,
    pub completed_steps: i64,
    pub failed_steps: i64,
    pub completion_percentage: f64,
    pub health_status: String,
    pub execution_status: String,
    pub recommended_action: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct TemplateSummary {
    pub steps: Vec<TemplateStepSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct TemplateStepSummary {
    pub name: String,
    pub step_type: String,
    pub handler: String,
    pub dependencies: Vec<String>,
    pub retryable: bool,
    pub max_attempts: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct StepSummaryInfo {
    pub step_uuid: String,
    pub name: String,
    pub current_state: String,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_attempted_at: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub dependencies_satisfied: bool,
    pub retry_eligible: bool,
    pub error: Option<StepErrorSummary>,
    pub results: Option<serde_json::Value>,  // always None in summary
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct StepErrorSummary {
    pub error_type: Option<String>,
    pub retryable: bool,
    pub status_code: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct DlqSummaryInfo {
    pub in_dlq: bool,
    pub dlq_reason: Option<String>,
    pub resolution_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "web-api", derive(ToSchema))]
pub struct TaskSummaryLinks {
    pub task: String,
    pub steps: String,
    pub dlq: String,
}
```

**Step 2: Add `From<TaskSummaryRow>` conversion**

Implement `From<TaskSummaryRow> for TaskSummaryResponse` to convert from the DB model. This parses `template_configuration` JSONB back into `TemplateSummary` by extracting step definitions.

**Step 3: Verify compilation**

Run: `cargo check --all-features -p tasker-shared`

**Step 4: Commit**

```
feat(TAS-317): add TaskSummaryResponse API type
```

---

## Task 7: REST Endpoint — `GET /v1/tasks/{uuid}/summary`

Add the summary endpoint to the orchestration server.

**Files:**
- Modify: `crates/tasker-orchestration/src/web/routes.rs` (add route)
- Modify: `crates/tasker-orchestration/src/web/handlers/tasks.rs` (add handler)
- Modify: `crates/tasker-orchestration/src/services/task_query_service.rs` (add query method)

**Step 1: Add route**

In `routes.rs`, add under the task routes:
```rust
.route("/v1/tasks/{task_uuid}/summary", get(handlers::tasks::get_task_summary))
```

**Step 2: Add handler**

In `handlers/tasks.rs`:
```rust
pub async fn get_task_summary(
    State(state): State<AppState>,
    Path(task_uuid): Path<String>,
    // auth extraction...
) -> Result<Json<TaskSummaryResponse>, ApiError> {
    let uuid = Uuid::parse_str(&task_uuid)?;
    let summary = state.task_query_service.get_task_summary(uuid).await?;
    Ok(Json(summary))
}
```

**Step 3: Add query service method**

In `task_query_service.rs`:
```rust
pub async fn get_task_summary(&self, task_uuid: Uuid) -> TaskQueryResult<TaskSummaryResponse> {
    let row = TaskSummaryRow::get_for_task(&self.pool, task_uuid).await?
        .ok_or(TaskQueryError::NotFound)?;
    Ok(TaskSummaryResponse::from(row))
}
```

**Step 4: Verify compilation**

Run: `cargo check --all-features -p tasker-orchestration`

**Step 5: Commit**

```
feat(TAS-317): add GET /v1/tasks/{uuid}/summary REST endpoint
```

---

## Task 8: gRPC — `GetTaskSummary` RPC

Add gRPC parity for the summary endpoint.

**Files:**
- Modify: `proto/tasker/v1/tasks.proto` (add messages and RPC)
- Modify: `crates/tasker-orchestration/src/grpc/services/tasks.rs` (implement RPC)
- Modify: `crates/tasker-orchestration/src/grpc/conversions.rs` (add conversions)

**Step 1: Add proto definitions**

Add `GetTaskSummaryRequest`, `GetTaskSummaryResponse`, and supporting message types to `tasks.proto` as defined in the design doc section 8.

Add `rpc GetTaskSummary(GetTaskSummaryRequest) returns (GetTaskSummaryResponse);` to the `TaskService`.

**Step 2: Regenerate Rust proto code**

Run: `cargo build --all-features -p tasker-shared` (build.rs generates proto code)

**Step 3: Add conversion**

In `conversions.rs`, add `From<TaskSummaryResponse>` for the proto `GetTaskSummaryResponse`.

**Step 4: Implement RPC handler**

In `grpc/services/tasks.rs`:
```rust
async fn get_task_summary(
    &self,
    request: Request<GetTaskSummaryRequest>,
) -> Result<Response<GetTaskSummaryResponse>, Status> {
    // auth, parse UUID, call task_query_service.get_task_summary(), convert to proto
}
```

**Step 5: Verify compilation**

Run: `cargo check --all-features -p tasker-orchestration`

**Step 6: Commit**

```
feat(TAS-317): add GetTaskSummary gRPC RPC
```

---

## Task 9: tasker-client — `get_task_summary` Method

Add the client method for fetching task summaries.

**Files:**
- Modify: `crates/tasker-client/src/api_clients/orchestration_client.rs`

**Step 1: Add REST client method**

```rust
pub async fn get_task_summary(&self, task_uuid: Uuid) -> TaskerResult<TaskSummaryResponse> {
    let url = format!("{}/v1/tasks/{}/summary", self.base_url, task_uuid);
    // GET request, deserialize TaskSummaryResponse
}
```

**Step 2: Add gRPC client method**

In the gRPC client implementation (if separate), add the corresponding method using the proto-generated client.

**Step 3: Verify compilation**

Run: `cargo check --all-features -p tasker-client`

**Step 4: Commit**

```
feat(TAS-317): add get_task_summary to orchestration client
```

---

## Task 10: SDK — Task Visualization (`visualize_task`)

Build the core task visualization logic that converts `TaskSummaryResponse` data into structured `VisualizationOutput`.

**Files:**
- Create: `crates/tasker-sdk/src/visualization/task_visualize.rs`
- Modify: `crates/tasker-sdk/src/visualization/mod.rs`

**Step 1: Write failing tests**

In `task_visualize.rs`, write tests for:

1. **State mapping**: All 10 step states map to correct `VisualCategory`
2. **Node shapes**: Each `StepType` produces correct `NodeShape`
3. **Edge styling**: Satisfied deps = Solid, unsatisfied = Dashed
4. **Decision workflow**: Untraversed branches get `VisualCategory::Untraversed` and `EdgeStyle::Dashed`
5. **Batch workers**: Dynamic steps (in execution but not in template) included as `NodeShape::Subroutine`
6. **DLQ annotation**: Task-level DLQ info included in warnings/graph
7. **Resource paths**: Each node gets `/v1/tasks/{uuid}/workflow_steps/{step_uuid}` path
8. **Table rows**: Correct columns populated (state, visual_category_label, duration, attempts, error_type, step_link)
9. **Duration calculation**: completed_at - last_attempted_at formatted as human-readable

Use fixture-style test data, not live DB connections.

**Step 2: Run tests to verify they fail**

Run: `cargo test --features test-messaging -p tasker-sdk task_visualize`

**Step 3: Implement `visualize_task`**

```rust
pub fn visualize_task(input: &TaskVisualizationInput) -> VisualizationOutput
```

Logic:
1. Build a map of template step names → `TemplateStepInfo`
2. Build a map of execution step names → `StepExecutionInfo`
3. For each template step:
   - If execution data exists: color by state, include resource_path
   - If no execution data and step is a possible decision branch (parent is Decision type): mark as Untraversed
   - If no execution data otherwise: mark as Pending
4. For each execution step not in template (dynamic batch workers): infer as BatchWorker
5. Build edges from template dependencies. Mark satisfied/unsatisfied based on execution state.
6. Build table rows in topological order
7. Add DLQ annotation if present

**Step 4: Register in `mod.rs`**

Add `pub mod task_visualize;` and public API:
```rust
pub fn visualize_task(input: &TaskVisualizationInput) -> VisualizationOutput;
```

**Step 5: Run tests**

Run: `cargo test --features test-messaging -p tasker-sdk task_visualize`

**Step 6: Commit**

```
feat(TAS-317): implement task execution visualization in SDK
```

---

## Task 11: CLI — `task visualize` Command

Add the `task visualize` subcommand to tasker-ctl.

**Files:**
- Modify: `crates/tasker-ctl/src/main.rs` (add `Visualize` variant to `TaskCommands`)
- Modify: `crates/tasker-ctl/src/commands/task.rs` (add handler)

**Step 1: Add command definition**

In `main.rs`, add to `TaskCommands` enum (after existing variants around line 243):

```rust
/// Visualize task execution state as Mermaid diagram
Visualize {
    /// Task UUID
    #[arg(value_name = "UUID")]
    task_id: String,
    /// Output format: mermaid (default), markdown, json
    #[arg(short, long, default_value = "mermaid")]
    format: String,
    /// Base URL to prepend to resource links
    #[arg(long)]
    base_url: Option<String>,
    /// Write output to file instead of stdout
    #[arg(short, long)]
    output: Option<String>,
    /// Emit only the graph (no detail table)
    #[arg(long)]
    graph_only: bool,
},
```

**Step 2: Add handler**

In `commands/task.rs`, add the match arm:

```rust
TaskCommands::Visualize { task_id, format, base_url, output, graph_only } => {
    let task_uuid = Uuid::parse_str(&task_id).map_err(|e| {
        tasker_client::ClientError::InvalidInput(format!("Invalid UUID: {}", e))
    })?;

    output::dim(format!("Fetching task summary for: {}", task_id));

    let summary = client.get_task_summary(task_uuid).await.map_err(|e| {
        output::api_error("get task summary", &e, "task_visualize");
        e
    })?;

    let input = TaskVisualizationInput::from(&summary);
    let viz = tasker_sdk::visualization::visualize_task(&input);

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&viz).unwrap(),
        "markdown" => render_markdown(&summary.task.name, &viz, base_url.as_deref(), graph_only),
        _ => render_mermaid(&viz.graph),
    };

    // write to file or stdout (same pattern as template visualize)
}
```

**Step 3: Verify compilation**

Run: `cargo check --all-features -p tasker-ctl`

**Step 4: Commit**

```
feat(TAS-317): add tasker-ctl task visualize command
```

---

## Task 12: MCP — `task_visualize` Tier 2 Tool

Add the MCP tool for task visualization.

**Files:**
- Modify: `crates/tasker-mcp/src/tools/params.rs` (add params)
- Modify: `crates/tasker-mcp/src/tools/connected.rs` (add implementation)
- Modify: `crates/tasker-mcp/src/server.rs` (register tool, update docs)
- Modify: `crates/tasker-mcp/src/tier.rs` (add to TIER2_TOOLS)

**Step 1: Add params**

In `params.rs`:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskVisualizeParams {
    #[schemars(description = "UUID of the task to visualize")]
    pub task_uuid: String,
    #[schemars(description = "Optional profile name for multi-environment setups")]
    #[serde(default)]
    pub profile: Option<String>,
    #[schemars(description = "When true, returns only the Mermaid graph (no detail table)")]
    #[serde(default)]
    pub graph_only: Option<bool>,
    #[schemars(description = "Base URL to prepend to resource links in output")]
    #[serde(default)]
    pub base_url: Option<String>,
}
```

**Step 2: Add implementation**

In `connected.rs`:
```rust
pub async fn task_visualize(
    client: &UnifiedOrchestrationClient,
    params: TaskVisualizeParams,
) -> String {
    let task_uuid = match Uuid::parse_str(&params.task_uuid) {
        Ok(u) => u,
        Err(e) => return error_json("invalid_uuid", &format!("Invalid task_uuid: {}", e)),
    };

    let summary = match client.as_client().get_task_summary(task_uuid).await {
        Ok(s) => s,
        Err(e) => return error_json("api_error", &e.to_string()),
    };

    let input = TaskVisualizationInput::from(&summary);
    let viz = visualize_task(&input);
    let graph_only = params.graph_only.unwrap_or(false);
    let mermaid = render_mermaid(&viz.graph);
    let detail_table = if graph_only { None } else {
        Some(render_detail_table(&viz.table, params.base_url.as_deref()))
    };
    let markdown = render_markdown(&summary.task.name, &viz, params.base_url.as_deref(), graph_only);

    serde_json::to_string_pretty(&serde_json::json!({
        "graph": viz.graph,
        "mermaid": mermaid,
        "detail_table": detail_table,
        "markdown": markdown,
        "warnings": viz.warnings,
    }))
    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}
```

**Step 3: Register tool in server.rs**

Add tool method (following `task_inspect` pattern at line 452):
```rust
#[tool(
    name = "task_visualize",
    description = "Visualize task execution state as a Mermaid flowchart diagram. Shows step DAG with nodes colored by execution status (completed/in-progress/pending/error/retrying), edge styling for dependency satisfaction, decision workflow paths (including untraversed branches), batch worker instances, DLQ status, and a detail table with timing, attempts, and error types. Requires a running server."
)]
pub async fn task_visualize(&self, Parameters(params): Parameters<TaskVisualizeParams>) -> String {
    let client = match self.resolve_client(params.profile.as_deref()).await {
        Ok(c) => c,
        Err(e) => return e,
    };
    connected::task_visualize(&client, params).await
}
```

**Step 4: Add to TIER2_TOOLS**

In `tier.rs`, add `"task_visualize"` to the `TIER2_TOOLS` const array.

**Step 5: Update tool count assertions**

Update in `tier.rs`:
- `test_tier2_tools_count`: 15 → 16

Update in `server.rs`:
- `test_tier1_offline_mode`: tools count if affected
- Check `test_all_tools_registered` total: 30 → 31

Update in `tests/mcp_protocol_test.rs`:
- T1 count stays 8
- T1+profile+T2 count: 24 → 25

**Step 6: Update server docs and instruction text**

In `server.rs`, update the module-level doc comment and the instruction text strings that list Tier 2 tools to include `task_visualize`.

**Step 7: Verify compilation and tests**

Run: `cargo test --features test-messaging -p tasker-mcp`

**Step 8: Commit**

```
feat(TAS-317): add task_visualize MCP Tier 2 tool
```

---

## Task 13: Quality Checks and Final Verification

Run the full quality suite and fix any issues.

**Step 1: Clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: zero warnings

**Step 2: Format**

Run: `cargo fmt --check`

**Step 3: Full test suite**

Run: `cargo make test-no-infra` (unit tests without DB)
Run: `cargo test --features test-messaging -p tasker-sdk -p tasker-mcp -p tasker-ctl -p tasker-shared`

**Step 4: SQLx prepare**

Run: `DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test cargo sqlx prepare --workspace -- --all-targets --all-features`
Then: `git add .sqlx/`

**Step 5: Commit any fixes**

```
fix(TAS-317): quality check fixes
```

---

## Task 14: Integration Validation

Validate the full pipeline with real data.

**Step 1: Test with existing fixture templates**

Use the codegen_test_template, diamond workflow, and conditional approval fixtures to generate template visualizations via the refactored path. Compare output to TAS-316 results.

**Step 2: Test summary endpoint**

If services are running, test `GET /v1/tasks/{uuid}/summary` with a known task UUID.

**Step 3: Test CLI**

```bash
cargo run -p tasker-ctl -- task visualize <uuid> --format mermaid
cargo run -p tasker-ctl -- task visualize <uuid> --format markdown --base-url https://example.com
cargo run -p tasker-ctl -- task visualize <uuid> --format json
```

**Step 4: Commit**

```
test(TAS-317): integration validation of task visualization pipeline
```

---

## Dependency Graph

```
Task 1 (types) ──► Task 2 (renderers) ──► Task 3 (refactor template viz)
                                                     │
Task 4 (SQL) ──► Task 5 (Rust model) ──► Task 6 (API type) ──► Task 7 (REST) ──► Task 8 (gRPC)
                                                │
                                                └──► Task 9 (client) ──► Task 10 (SDK task viz)
                                                                               │
                                                                    ┌──────────┼──────────┐
                                                                    ▼          ▼          ▼
                                                              Task 11      Task 12    Task 13
                                                              (CLI)        (MCP)      (quality)
                                                                    └──────────┼──────────┘
                                                                               ▼
                                                                          Task 14
                                                                       (validation)
```

**Parallelizable groups:**
- Tasks 1-3 (SDK refactoring) can proceed independently from Tasks 4-6 (DB + API types)
- Tasks 7 and 8 (REST + gRPC) can be parallelized after Task 6
- Tasks 11 and 12 (CLI + MCP) can be parallelized after Task 10

---

## Key Files Quick Reference

| Component | Key Files |
|-----------|-----------|
| SDK types | `crates/tasker-sdk/src/visualization/types.rs` (new) |
| SDK renderers | `crates/tasker-sdk/src/visualization/render.rs` (new) |
| SDK template viz | `crates/tasker-sdk/src/visualization/template_visualize.rs` (new, replaces mermaid.rs + detail_table.rs) |
| SDK task viz | `crates/tasker-sdk/src/visualization/task_visualize.rs` (new) |
| SDK public API | `crates/tasker-sdk/src/visualization/mod.rs` |
| SQL migration | `migrations/20260306000001_task_summary_function.sql` (new) |
| Rust DB model | `crates/tasker-shared/src/models/orchestration/task_summary.rs` (new) |
| API response type | `crates/tasker-shared/src/types/api/orchestration.rs` |
| REST endpoint | `crates/tasker-orchestration/src/web/handlers/tasks.rs`, `routes.rs` |
| gRPC proto | `proto/tasker/v1/tasks.proto` |
| gRPC handler | `crates/tasker-orchestration/src/grpc/services/tasks.rs` |
| Client method | `crates/tasker-client/src/api_clients/orchestration_client.rs` |
| CLI command | `crates/tasker-ctl/src/main.rs`, `crates/tasker-ctl/src/commands/task.rs` |
| MCP tool | `crates/tasker-mcp/src/server.rs`, `crates/tasker-mcp/src/tools/connected.rs`, `crates/tasker-mcp/src/tools/params.rs` |
| MCP tiers | `crates/tasker-mcp/src/tier.rs` |
| MCP test assertions | `crates/tasker-mcp/tests/mcp_protocol_test.rs` |
