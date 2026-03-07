# TAS-317: Task Visualize Design

Live task execution state visualization as Mermaid diagrams with enriched summary API endpoint.

## Architecture Principles

1. **No presentation logic in orchestration server** — server returns enriched JSON only
2. **SDK returns structured data** — graph nodes/edges and table rows, not rendered strings
3. **SDK provides renderers** — Mermaid and detail table renderers consume structured data
4. **Callers own URLs** — resource paths are relative; callers prepend their known base URL
5. **No SVG** — Mermaid syntax only; SVG rendering is out of scope
6. **gRPC parity** — new endpoint maps to a distinct gRPC RPC
7. **Unified visualization architecture** — TAS-316 template visualization refactored to
   share the structured node/edge/renderer approach with task visualization

## Data Flow

```
Orchestration Server                 tasker-client → tasker-sdk → CLI / MCP
┌──────────────────────┐
│ GET /v1/tasks/{uuid} │
│     /summary         │──────►  tasker-client.get_task_summary()
│                      │                     │
│ SQL function:        │           tasker-sdk::visualization
│  get_task_summaries  │           ┌─────────┴──────────┐
│  (task + named_task  │           │  Structured Data   │
│   + steps + error    │           │  GraphNode[]       │
│   + DLQ status)      │           │  GraphEdge[]       │
│                      │           │  TableRow[]        │
└──────────────────────┘           └─────────┬──────────┘
                                             │
                                   ┌─────────┴──────────┐
                                   │  Renderers          │
                                   │  render_mermaid()   │
                                   │  render_detail_table│
                                   │    (base_url)       │
                                   └─────────────────────┘
```

tasker-client handles transport (REST or gRPC) transparently based on profile configuration.

## 1. Summary API Endpoint

### REST

```
GET /v1/tasks/{task_uuid}/summary
```

Permission: `tasks:read` (same as task detail).

### gRPC

```protobuf
rpc GetTaskSummary(GetTaskSummaryRequest) returns (GetTaskSummaryResponse);
```

### Response Shape: `TaskSummaryResponse`

```json
{
  "task": {
    "task_uuid": "...",
    "name": "order_processing",
    "namespace": "ecommerce",
    "version": "1.0.0",
    "status": "steps_in_process",
    "created_at": "2026-03-06T10:00:00Z",
    "updated_at": "2026-03-06T10:05:30Z",
    "completed_at": null,
    "initiator": "api",
    "source_system": "web",
    "reason": "New order #12345",
    "correlation_id": "...",
    "total_steps": 5,
    "pending_steps": 1,
    "in_progress_steps": 1,
    "completed_steps": 2,
    "failed_steps": 1,
    "completion_percentage": 40.0,
    "health_status": "recovering",
    "execution_status": "has_ready_steps",
    "recommended_action": "execute_ready_steps"
  },
  "template": {
    "steps": [
      {
        "name": "validate_order",
        "step_type": "standard",
        "handler": "ecommerce.ValidateOrderHandler",
        "dependencies": [],
        "retry": { "retryable": true, "max_attempts": 3, "backoff": "exponential" },
        "batch_config": null
      }
    ]
  },
  "steps": [
    {
      "step_uuid": "...",
      "name": "validate_order",
      "current_state": "complete",
      "created_at": "2026-03-06T10:00:01Z",
      "completed_at": "2026-03-06T10:00:05Z",
      "last_attempted_at": "2026-03-06T10:00:01Z",
      "attempts": 1,
      "max_attempts": 3,
      "dependencies_satisfied": true,
      "retry_eligible": false,
      "error": null,
      "results": null
    },
    {
      "step_uuid": "...",
      "name": "process_payment",
      "current_state": "error",
      "created_at": "2026-03-06T10:00:06Z",
      "completed_at": null,
      "last_attempted_at": "2026-03-06T10:02:00Z",
      "attempts": 3,
      "max_attempts": 3,
      "dependencies_satisfied": true,
      "retry_eligible": false,
      "error": {
        "error_type": "timeout",
        "retryable": true,
        "status_code": 504
      },
      "results": null
    }
  ],
  "dlq": {
    "in_dlq": false,
    "dlq_reason": null,
    "resolution_status": null
  },
  "links": {
    "task": "/v1/tasks/{task_uuid}",
    "steps": "/v1/tasks/{task_uuid}/workflow_steps",
    "dlq": "/v1/dlq/task/{task_uuid}"
  }
}
```

### Key Design Decisions

**No `results` or `context` fields** — these are verbose payloads. The summary includes
only what visualization needs. Full results are available via the step detail link.

**Error info is `StepExecutionError` shape minus message/backtrace/context** — same type,
just with only `error_type`, `retryable`, and `status_code` populated. We don't mutate the
type shape; we omit the verbose fields when serializing for summary.

**Template sourced from `named_tasks.configuration`** — parsed back into step definitions
to provide step_type, handler, dependencies, retry config, and batch_config. This is the
authoritative template structure, not dependent on any running worker.

**DLQ is task-level** — DLQ tracks task investigation status, not per-step. A single
`dlq` object shows whether the task is under investigation, with a link to the DLQ
resource for details.

**Links are relative resource paths** — `/v1/tasks/{uuid}`, not
`https://host:port/v1/tasks/{uuid}`. Callers prepend their known base URL.

### Database: SQL Function Strategy

Following the established pattern (e.g., `get_task_execution_context` /
`get_task_execution_contexts_batch`), we create a batch-first SQL function pair:

- `get_task_summaries(input_task_uuids uuid[])` — batch function, does all the work
- `get_task_summary(input_task_uuid uuid)` — convenience wrapper, calls batch with
  single-element array

The SQL function performs in a single call:

1. **Task metadata** — JOIN `tasks` → `named_tasks` → `task_namespaces` for name, version,
   namespace, status, timestamps, correlation_id
2. **Template configuration** — `named_tasks.configuration` JSONB column
3. **Step execution state** — reuses `get_step_readiness_status_batch()` internally for
   step states, retry info, dependency satisfaction
4. **Step error extraction** — `workflow_steps.results -> 'error'` JSONB extraction for
   `error_type`, `retryable`, `status_code` (no message/backtrace)
5. **DLQ status** — LEFT JOIN to `tasks_dlq WHERE resolution_status = 'pending'`

Returns a composite type with JSONB fields for template and steps arrays, avoiding
wide column explosion. The Rust model in `tasker-shared/src/models/orchestration/`
deserializes these JSONB fields into typed structs.

**Rust model location**: `tasker-shared/src/models/orchestration/task_summary.rs`

Following `TaskExecutionContext` pattern:

```rust
pub struct TaskSummary {
    // ... fields from SQL function
}

impl TaskSummary {
    pub async fn get_for_task(pool: &PgPool, task_uuid: Uuid) -> Result<Option<Self>, sqlx::Error>;
    pub async fn get_for_tasks(pool: &PgPool, task_uuids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error>;
}
```

## 2. SDK Structured Data Types — Unified Visualization Architecture

### Refactoring TAS-316

TAS-316's `visualize_template` currently returns pre-rendered Mermaid strings directly.
We refactor it to use the same structured node/edge architecture as task visualization:

1. **Extract shared types** into `tasker-sdk/src/visualization/types.rs` — `GraphNode`,
   `GraphEdge`, `GraphData`, `VisualCategory`, `TableRow`, `TableData`
2. **Extract shared renderers** into `tasker-sdk/src/visualization/render.rs` —
   `render_mermaid()`, `render_detail_table()`, `render_markdown()`
3. **Refactor `visualize_template`** to return structured data, with a convenience method
   that renders to the current output format for backward compatibility
4. **Add `visualize_task`** using the same types and renderers

Module structure after refactoring:

```
tasker-sdk/src/visualization/
├── mod.rs                  # Public API: visualize_template, visualize_task
├── types.rs                # Shared types: GraphNode, GraphEdge, VisualCategory, TableRow, etc.
├── render.rs               # Shared renderers: render_mermaid, render_detail_table, render_markdown
├── template_visualize.rs   # Template → structured data (refactored from mermaid.rs + detail_table.rs)
└── task_visualize.rs       # Task execution → structured data (new)
```

### Shared Types

```rust
// types.rs

/// DAG graph as structured nodes and edges.
#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,                    // step name (unique within task/template)
    pub label: String,                 // display label (name + state or annotation)
    pub visual_category: VisualCategory,
    pub node_shape: NodeShape,
    pub resource_path: Option<String>, // None for template viz, Some for task viz
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum VisualCategory {
    Completed,       // green  — complete, resolved_manually
    InProgress,      // blue   — in_progress, enqueued, enqueued_for_orchestration
    Pending,         // gray   — pending
    Error,           // red    — error, enqueued_as_error_for_orchestration, cancelled
    Retrying,        // yellow — waiting_for_retry
    Annotated,       // amber  — template annotations (TAS-316 compatibility)
    Untraversed,     // light gray — decision branch not taken at runtime
    Default,         // no special styling — template steps without execution state
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NodeShape {
    Rectangle,           // Standard steps
    Diamond,             // Decision steps
    Trapezoid,           // DeferredConvergence steps
    Parallelogram,       // Batchable steps
    Subroutine,          // BatchWorker steps
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_style: EdgeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EdgeStyle {
    Solid,      // dependency satisfied (or template-only edge)
    Dashed,     // dependency not yet satisfied
}

/// Table data as structured rows.
#[derive(Debug, Clone, Serialize)]
pub struct TableData {
    pub header: Option<TableHeader>,    // task-level info (None for template viz)
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableHeader {
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: Option<String>,         // None for template viz
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
    pub dependencies: Option<String>,   // Only for template viz (not needed for task viz)
    pub state: Option<String>,          // Only for task viz
    pub visual_category: Option<String>,// Only for task viz
    pub duration: Option<String>,       // Only for task viz
    pub attempts: Option<String>,       // Only for task viz
    pub error_type: Option<String>,     // Only for task viz
    pub schema_fields: Option<String>,  // Only for template viz
    pub retry_info: Option<String>,     // Only for template viz
    pub step_link: Option<String>,      // Only for task viz
}

/// Complete visualization output.
#[derive(Debug, Clone, Serialize)]
pub struct VisualizationOutput {
    pub graph: GraphData,
    pub table: TableData,
    pub warnings: Vec<String>,
}
```

### Input Types

```rust
// task_visualize.rs — input for task execution visualization

pub struct TaskVisualizationInput {
    pub task: TaskMetadata,
    pub template_steps: Vec<TemplateStepInfo>,
    pub execution_steps: Vec<StepExecutionInfo>,
    pub dlq: Option<DlqInfo>,
}

pub struct TaskMetadata {
    pub task_uuid: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub completion_percentage: f64,
    pub health_status: String,
}

pub struct TemplateStepInfo {
    pub name: String,
    pub step_type: String,
    pub handler: String,
    pub dependencies: Vec<String>,
    pub retryable: bool,
    pub max_attempts: u32,
}

pub struct StepExecutionInfo {
    pub step_uuid: String,
    pub name: String,
    pub current_state: String,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_attempted_at: Option<String>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub error_type: Option<String>,
    pub error_retryable: Option<bool>,
}

pub struct DlqInfo {
    pub in_dlq: bool,
    pub dlq_reason: Option<String>,
    pub resolution_status: Option<String>,
}
```

### Renderers (Shared)

```rust
// render.rs

/// Render structured graph data to Mermaid syntax.
pub fn render_mermaid(graph: &GraphData) -> String;

/// Render structured table data to markdown, with optional base_url for links.
pub fn render_detail_table(table: &TableData, base_url: Option<&str>) -> String;

/// Render full markdown document (fenced mermaid block + optional table).
pub fn render_markdown(
    title: &str,
    output: &VisualizationOutput,
    base_url: Option<&str>,
    graph_only: bool,
) -> String;
```

### Public API

```rust
// mod.rs — public API

/// Generate structured visualization data from a task template (offline).
/// Refactored from TAS-316 to use shared types.
pub fn visualize_template(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> VisualizationOutput;

/// Generate structured visualization data from live task execution state.
pub fn visualize_task(input: &TaskVisualizationInput) -> VisualizationOutput;

/// Backward-compatible convenience: render template visualization to strings.
/// Returns the same shape as TAS-316's original output for CLI/MCP callers
/// that haven't migrated to the structured API yet.
pub fn visualize_template_rendered(
    template: &TaskTemplate,
    annotations: &HashMap<String, String>,
    options: &VisualizeOptions,
) -> RenderedOutput {
    let output = visualize_template(template, annotations, options);
    let mermaid = render_mermaid(&output.graph);
    let detail_table = if options.graph_only { None } else {
        Some(render_detail_table(&output.table, None))
    };
    let markdown = render_markdown(&template.name, &output, None, options.graph_only);
    RenderedOutput { mermaid, detail_table, markdown, warnings: output.warnings }
}

/// Pre-rendered output for backward compatibility with TAS-316 callers.
pub struct RenderedOutput {
    pub mermaid: String,
    pub detail_table: Option<String>,
    pub markdown: String,
    pub warnings: Vec<String>,
}
```

## 3. Mermaid Rendering

### Node Shapes by Step Type

| NodeShape | Mermaid Syntax | Step Types |
|-----------|---------------|------------|
| Rectangle | `name["label"]` | Standard |
| Diamond | `name{"label"}` | Decision |
| Trapezoid | `name[/"label"/]` | DeferredConvergence |
| Parallelogram | `name[/"label"/]` | Batchable |
| Subroutine | `name[["label"]]` | BatchWorker |

Note: Batchable and DeferredConvergence share trapezoid shape — both are orchestration
coordination steps. They are distinguished by label text and color.

### Node Colors (classDef)

```mermaid
classDef completed fill:#d4edda,stroke:#28a745,color:#155724
classDef inProgress fill:#cce5ff,stroke:#0d6efd,color:#004085
classDef pending fill:#e9ecef,stroke:#6c757d,color:#495057
classDef error fill:#f8d7da,stroke:#dc3545,color:#721c24
classDef retrying fill:#fff3cd,stroke:#ffc107,color:#856404
classDef annotated fill:#fff3cd,stroke:#ffc107
classDef untraversed fill:#f8f9fa,stroke:#dee2e6,color:#adb5bd,stroke-dasharray:5 5
classDef default fill:#ffffff,stroke:#6c757d,color:#212529
```

### Edge Styling

- Solid arrow (`-->`) for satisfied dependencies or template-only edges
- Dotted arrow (`-.->`) for unsatisfied dependencies

### Node Labels

**Task visualization** — step name + state machine state:
```
validate_order["validate_order\ncomplete"]:::completed
process_payment["process_payment\nerror"]:::error
```

**Template visualization** — step name (+ annotation if present):
```
validate_order[validate_order]
process_payment["process_payment\n⚠ Not retry-safe"]:::annotated
```

State is always shown as text for accessibility — color is supplementary, not the sole
indicator.

### DLQ Annotation

When the task is in DLQ, a note is added at the graph level:

```mermaid
note right of _task_info: "DLQ: staleness_timeout (pending)"
```

### Decision Workflows: Full Possible Graph

For conditional workflows, the Mermaid graph shows **all possible paths** from the
template, including branches the decision step did not create at runtime:

- **Traversed branches**: colored by their execution state (completed, in_progress, etc.)
- **Untraversed branches**: rendered in `untraversed` style (light gray, dashed border)
- **Untraversed edges**: dashed lines from the decision step to untraversed branches

In the detail table, untraversed steps show state "untraversed" and visual category
"Untraversed" — they were defined in the template but not created for this execution.

This gives operators full context: "the decision could have gone here, but didn't."

### Dynamic Steps (Batch Workers)

Steps that exist in execution data but not in the template definition (dynamically created
batch worker instances like `process_csv_batch_001`) are included in the graph:

- Inferred as `BatchWorker` type when name matches `{worker_template}_NNN` pattern
- Dependencies come from execution data (the batchable parent step)
- Styled by their execution state like any other step

## 4. Detail Table Rendering

### Task Header Section

```markdown
# order_processing (ecommerce v1.0.0)

**Status**: steps_in_process | **Health**: recovering | **Progress**: 40% | **Elapsed**: 5m 30s
**Task**: [/v1/tasks/{uuid}](base_url/v1/tasks/{uuid})
```

When in DLQ:
```markdown
**DLQ**: staleness_timeout (pending) | [Investigate](base_url/v1/dlq/task/{uuid})
```

### Task Execution Step Table

| Step | State | Status | Type | Handler | Duration | Attempts | Error | Links |
|------|-------|--------|------|---------|----------|----------|-------|-------|
| validate_order | complete | Completed | standard | ecommerce.Validate | 4.0s | 1/3 | -- | [step](...) |
| process_payment | error | Error | standard | ecommerce.Payment | -- | 3/3 | timeout | [step](...) |
| enrich_order | in_progress | In Progress | standard | ecommerce.Enrich | running | 1/3 | -- | [step](...) |
| auto_approve | untraversed | Untraversed | standard | ecommerce.Approve | -- | -- | -- | -- |
| generate_report | pending | Pending | standard | ecommerce.Report | -- | 0/3 | -- | [step](...) |

### Template Step Table (TAS-316 refactored)

| Step | Type | Handler | Dependencies | Schema Fields | Retry |
|------|------|---------|--------------|---------------|-------|
| validate_order | Standard | ecommerce.Validate | -- | validated, order_total | -- |
| process_payment | Standard | ecommerce.Payment | validate_order | -- | 5x exponential |

Links column uses compact markdown. When `base_url` is provided to the renderer, links
become absolute: `[step](https://my-server.com/v1/tasks/{uuid}/workflow_steps/{step_uuid})`

## 5. State -> Visual Category Mapping

| Visual Category | Color | States |
|---|---|---|
| Completed | Green `#d4edda` | `complete`, `resolved_manually` |
| In Progress | Light blue `#cce5ff` | `in_progress`, `enqueued`, `enqueued_for_orchestration` |
| Pending | Light gray `#e9ecef` | `pending` |
| Error | Red `#f8d7da` | `error`, `enqueued_as_error_for_orchestration`, `cancelled` |
| Retrying | Yellow `#fff3cd` | `waiting_for_retry` |
| Untraversed | Very light gray `#f8f9fa` | Decision branches not created at runtime |
| Annotated | Amber `#fff3cd` | Template steps with developer annotations |
| Default | White `#ffffff` | Template steps without execution state |

## 6. CLI Integration

### Command

```
tasker-ctl task visualize <task_uuid> [--format mermaid|json|markdown] [--base-url <url>] [--output <file>] [--graph-only]
```

- `--format mermaid` (default): Mermaid syntax only
- `--format markdown`: Full markdown with table
- `--format json`: Structured `VisualizationOutput` as JSON
- `--base-url`: Prepended to resource paths in links
- `--output`: Write to file instead of stdout
- `--graph-only`: Omit detail table

### Implementation

Follows existing CLI pattern — client created from profile config via `ClientConfig`:

```rust
// In tasker-ctl/src/commands/task.rs
TaskCommands::Visualize { task_id, format, base_url, output, graph_only } => {
    let orchestration_config = OrchestrationApiConfig {
        base_url: config.orchestration.base_url.clone(),
        timeout_ms: config.orchestration.timeout_ms,
        max_retries: config.orchestration.max_retries,
        auth: config.orchestration.resolve_web_auth_config(),
    };
    let client = OrchestrationApiClient::new(orchestration_config)?;

    let summary = client.get_task_summary(task_uuid).await?;
    let input = TaskVisualizationInput::from(&summary);
    let viz = tasker_sdk::visualization::visualize_task(&input);
    // render based on format...
}
```

### Template Visualize Update

`tasker-ctl template visualize` and `tasker-mcp template_visualize` updated to call
`visualize_template_rendered()` (backward-compatible wrapper) or, if callers want
structured data, `visualize_template()` directly.

## 7. MCP Integration

### Tool: `task_visualize` (Tier 2 -- Connected)

```rust
#[tool(name = "task_visualize", description = "...")]
pub async fn task_visualize(&self, Parameters(params): Parameters<TaskVisualizeParams>) -> String {
    let client = match self.resolve_client(params.profile.as_deref()).await {
        Ok(c) => c,
        Err(e) => return e,
    };
    connected::task_visualize(&client, params).await
}
```

Parameters:
```rust
pub struct TaskVisualizeParams {
    pub task_uuid: String,
    pub profile: Option<String>,
    pub graph_only: Option<bool>,
    pub base_url: Option<String>,
}
```

Returns JSON with `graph` (structured data), `mermaid` (rendered), `detail_table`
(rendered), and `markdown` (rendered) fields.

## 8. gRPC Parity

### Proto Addition (tasks.proto)

```protobuf
rpc GetTaskSummary(GetTaskSummaryRequest) returns (GetTaskSummaryResponse);

message GetTaskSummaryRequest {
  string task_uuid = 1;
}

message GetTaskSummaryResponse {
  TaskSummaryData task = 1;
  TemplateSummary template = 2;
  repeated StepSummaryData steps = 3;
  DlqSummaryData dlq = 4;
  LinksSummary links = 5;
}

message TaskSummaryData {
  string task_uuid = 1;
  string name = 2;
  string namespace = 3;
  string version = 4;
  TaskState state = 5;
  google.protobuf.Timestamp created_at = 6;
  google.protobuf.Timestamp updated_at = 7;
  optional google.protobuf.Timestamp completed_at = 8;
  string initiator = 9;
  string source_system = 10;
  string reason = 11;
  string correlation_id = 12;
  int64 total_steps = 13;
  int64 completed_steps = 14;
  int64 failed_steps = 15;
  double completion_percentage = 16;
  string health_status = 17;
  string execution_status = 18;
  string recommended_action = 19;
}

message TemplateSummary {
  repeated TemplateStepSummary steps = 1;
}

message TemplateStepSummary {
  string name = 1;
  string step_type = 2;
  string handler = 3;
  repeated string dependencies = 4;
  bool retryable = 5;
  int32 max_attempts = 6;
}

message StepSummaryData {
  string step_uuid = 1;
  string name = 2;
  string current_state = 3;
  google.protobuf.Timestamp created_at = 4;
  optional google.protobuf.Timestamp completed_at = 5;
  optional google.protobuf.Timestamp last_attempted_at = 6;
  int32 attempts = 7;
  int32 max_attempts = 8;
  bool dependencies_satisfied = 9;
  bool retry_eligible = 10;
  optional StepErrorSummary error = 11;
}

message StepErrorSummary {
  optional string error_type = 1;
  bool retryable = 2;
  optional int32 status_code = 3;
}

message DlqSummaryData {
  bool in_dlq = 1;
  optional string dlq_reason = 2;
  optional string resolution_status = 3;
}

message LinksSummary {
  string task = 1;
  string steps = 2;
  string dlq = 3;
}
```

### tasker-client Addition

```rust
impl OrchestrationApiClient {
    pub async fn get_task_summary(&self, task_uuid: Uuid) -> TaskerResult<TaskSummaryResponse>;
}
```

Transport-agnostic: uses REST or gRPC based on client configuration.

## 9. TAS-316 Refactoring: Unified Visualization Architecture

TAS-316's `visualize_template` currently returns pre-rendered Mermaid strings via
`mermaid.rs` and `detail_table.rs`. We refactor it as part of TAS-317 to share the
structured node/edge architecture:

### What Changes

1. **`mermaid.rs`** -> absorbed into `template_visualize.rs` (builds `GraphData`) and
   `render.rs` (renders `GraphData` to Mermaid syntax)
2. **`detail_table.rs`** -> absorbed into `template_visualize.rs` (builds `TableData`)
   and `render.rs` (renders `TableData` to markdown)
3. **`mod.rs`** -> updated public API with both `visualize_template()` (structured) and
   `visualize_template_rendered()` (backward-compatible strings)
4. **New `types.rs`** -> shared `GraphNode`, `GraphEdge`, `VisualCategory`, `NodeShape`,
   `TableRow`, etc.
5. **New `render.rs`** -> shared renderers used by both template and task visualization
6. **New `task_visualize.rs`** -> task execution visualization

### What Stays the Same

- `VisualizeOptions` API (graph_only flag)
- All existing test assertions (via `visualize_template_rendered()`)
- CLI and MCP callers (updated to use rendered wrapper)
- Annotation support (template-only feature)

### Migration Path

1. Extract types into `types.rs`
2. Extract rendering into `render.rs`
3. Refactor `visualize_template` to produce structured data, add rendered wrapper
4. Verify all existing tests pass via rendered wrapper
5. Build `visualize_task` using same types and renderers
6. Update CLI/MCP callers to use structured data where beneficial

## 10. Testing Strategy

### Unit Tests (tasker-sdk)
- State -> VisualCategory mapping for all 10 step states + untraversed
- Node shape rendering for all 5 step types
- Edge styling (solid vs dashed)
- Detail table rendering with and without base_url
- Decision workflow: untraversed branches shown correctly
- Batch workflow: dynamic batch workers included
- DLQ annotation rendering
- Duration calculation edge cases
- Template visualization backward compatibility (rendered output matches)

### Integration Tests (tasker-sdk)
- Full visualization from fixture templates + mock execution data
- Diamond DAG, linear chain, decision workflow, batch workflow
- Completed, in-progress, and errored task scenarios

### Database Tests (tasker-shared)
- `get_task_summary` returns correct composite for known task
- `get_task_summaries` batch returns multiple results
- Template configuration correctly extracted from named_tasks
- Error info extracted without verbose fields
- DLQ status included when applicable
- Steps without errors have null error field

### API Tests (tasker-orchestration)
- Summary endpoint returns correct response shape
- gRPC parity with REST response
- Permission enforcement (tasks:read)

### MCP Tests (tasker-mcp)
- Tool count assertions updated for task_visualize
- Parameter validation
- Profile resolution
- JSON output shape

### CLI Tests (tasker-ctl)
- Format flag handling (mermaid, json, markdown)
- Base URL prepending
- Output file writing
