# MCP Operational Exercise: Connected Tool Workflows

This exercise demonstrates how different roles use Tasker's connected MCP tools for operational tasks. Each scenario follows a realistic investigation flow that you can replicate against a running Tasker instance.

## Prerequisites

- `tasker-mcp` installed and configured with a profile (see [MCP Setup](./setup.md))
- A running Tasker orchestration server with some task history
- An MCP client (Claude Code, Claude Desktop, mcphost, etc.)

## Scenario 1: Software Engineer — Debugging a Stuck Task

**Context**: A developer reports their workflow isn't completing. You need to find the task, identify which step is stuck, and understand what happened.

### Step 1: Verify Connectivity

**Prompt**: "Check if we're connected to the Tasker server"

**Tool**: `connection_status`
**Parameters**: `{}`
**Expected**: Mode is "connected", at least one profile shows healthy status.
**Next**: If healthy, proceed. If unhealthy, check server status before continuing.

### Step 2: Find the Task

**Prompt**: "List recent tasks in the analytics namespace that aren't complete"

**Tool**: `task_list`
**Parameters**: `{ "namespace": "analytics", "status": "error", "limit": 10 }`
**Expected**: A list of task summaries with UUIDs, names, status, and completion percentages.
**Next**: Identify the stuck task by name and UUID.

### Step 3: Inspect the Task

**Prompt**: "Show me the details of task {uuid}"

**Tool**: `task_inspect`
**Parameters**: `{ "task_uuid": "<uuid-from-step-2>" }`
**Expected**: Full task details including all steps, their statuses, and dependencies.
**Next**: Identify which step is in error or stuck state.

### Step 4: Inspect the Failed Step

**Prompt**: "What happened with step {id}?"

**Tool**: `step_inspect`
**Parameters**: `{ "task_uuid": "<task-uuid>", "step_uuid": "<step-uuid-from-task-inspect>" }`
**Expected**: Step details including handler callable, execution results, timing, and retry count.
**Next**: Check if the error is transient (retry-related) or permanent.

### Step 5: Review the Audit Trail

**Prompt**: "Show me the audit history for that step"

**Tool**: `step_audit`
**Parameters**: `{ "task_uuid": "<task-uuid>", "step_uuid": "<same-step-uuid>" }`
**Expected**: SOC2-compliant audit trail showing state transitions with timestamps and worker attribution.
**Next**: Use the audit trail to understand the sequence of events leading to failure.

## Scenario 2: SRE — System Health Assessment

**Context**: You're doing a routine health check during an on-call shift.

### Step 1: Check Connectivity

**Tool**: `connection_status`
**Parameters**: `{ "refresh": true }`
**Expected**: Fresh health probes for all configured profiles.

### Step 2: Detailed Health Check

**Tool**: `system_health`
**Parameters**: `{}`
**Expected**: Component-level health including database pools, message queues, and circuit breaker states.

### Step 3: Review Configuration

**Tool**: `system_config`
**Parameters**: `{}`
**Expected**: Running configuration with secrets redacted. Useful for verifying deployment configuration matches expectations.

### Step 4: Check for Stale Tasks

**Tool**: `staleness_check`
**Parameters**: `{}`
**Expected**: Staleness monitoring with healthy/warning/stale annotations based on threshold percentages.

### Step 5: Review Performance Metrics

**Tool**: `analytics_performance`
**Parameters**: `{}`
**Expected**: System-wide throughput, latency percentiles, and success rates.

### Step 6: Identify Bottlenecks

**Tool**: `analytics_bottlenecks`
**Parameters**: `{}`
**Expected**: Slow steps ranked by execution time, with handler names and execution counts.

## Scenario 3: Technical Ops — DLQ Triage

**Context**: The monitoring dashboard shows DLQ entries accumulating. Time to investigate.

### Step 1: Get the Big Picture

**Tool**: `dlq_stats`
**Parameters**: `{}`
**Expected**: DLQ entry counts aggregated by reason code (retry_exhaustion, permanent_failure, timeout, etc.).
**Next**: Identify which failure reason is most prevalent.

### Step 2: Prioritize Investigation

**Tool**: `dlq_queue`
**Parameters**: `{}`
**Expected**: Entries ranked by severity score, showing which failures need attention first.
**Next**: Pick the highest-priority entry.

### Step 3: List Entries for Context

**Tool**: `dlq_list`
**Parameters**: `{}`
**Expected**: DLQ entries with task references, timestamps, and resolution status.
**Next**: Correlate with the prioritized queue.

### Step 4: Deep-Dive on a Specific Entry

**Tool**: `dlq_inspect`
**Parameters**: `{ "task_uuid": "<task-uuid-from-dlq-entry>" }`
**Expected**: Full DLQ entry with error message, stack trace snapshot, task/step context, and retry history.
**Next**: Determine if the failure is systemic (infrastructure) or task-specific (bad input).

### Step 5: Cross-Reference with Task

**Tool**: `task_inspect`
**Parameters**: `{ "task_uuid": "<uuid-from-dlq-entry>" }`
**Expected**: Full task context to understand what the task was trying to do.

### Step 6: Check Step Details

**Tool**: `step_inspect`
**Parameters**: `{ "task_uuid": "<task-uuid>", "step_uuid": "<step-uuid-from-task>" }`
**Expected**: Handler configuration, input data, and execution results to identify root cause.

## Scenario 4: Analytics — Performance Review

**Context**: Weekly performance review — analyze throughput and identify optimization targets.

### Step 1: Performance Overview

**Tool**: `analytics_performance`
**Parameters**: `{}`
**Expected**: Aggregate metrics: total tasks processed, average completion time, success rate, throughput.

### Step 2: Bottleneck Identification

**Tool**: `analytics_bottlenecks`
**Parameters**: `{}`
**Expected**: Steps ranked by execution time with handler names and execution counts. Focus on steps with high execution count AND high latency.

### Step 3: Filter by Namespace

**Tool**: `task_list`
**Parameters**: `{ "namespace": "production", "status": "complete", "limit": 50 }`
**Expected**: Recent completed tasks for trend analysis.

## Scenario 5: Technical Ops — DLQ Remediation with Write Tools

**Context**: You've triaged DLQ entries (Scenario 3) and identified a step that failed due to a transient infrastructure issue that's now resolved. Time to fix it.

All write tools use a **preview → confirm** pattern. The first call (without `confirm`) shows what will happen. The second call (with `confirm: true`) executes the action.

### Step 1: Preview a Step Retry

**Prompt**: "Reset the failed step for retry — the database connection issue is resolved"

**Tool**: `step_retry`
**Parameters**: `{ "task_uuid": "<uuid>", "step_uuid": "<uuid>", "reason": "Database connection restored", "reset_by": "operator@example.com" }`
**Expected**: Preview showing current step state, attempt count, and what will happen. Includes `"status": "preview"` and `"instruction": "Call this tool again with confirm: true..."`.
**Next**: Review the preview, then confirm.

### Step 2: Execute the Retry

**Tool**: `step_retry`
**Parameters**: `{ "task_uuid": "<uuid>", "step_uuid": "<uuid>", "reason": "Database connection restored", "reset_by": "operator@example.com", "confirm": true }`
**Expected**: `"status": "executed"` — step is reset to pending and will be picked up by a worker.
**Next**: Use `task_inspect` to monitor progress.

### Step 3: Record the DLQ Resolution

**Prompt**: "Update the DLQ entry to record our investigation"

**Tool**: `dlq_update`
**Parameters**: `{ "dlq_entry_uuid": "<uuid>", "resolution_status": "manually_resolved", "resolution_notes": "Transient DB connection failure. Root cause: connection pool exhaustion during deployment. Step retried successfully after pool recovery.", "resolved_by": "operator@example.com" }`
**Expected**: Preview first (omit `confirm`), then execute with `confirm: true`.

### Alternative: Manual Step Completion

When you have the correct result data but the handler can't run:

**Tool**: `step_complete`
**Parameters**: `{ "task_uuid": "<uuid>", "step_uuid": "<uuid>", "result": { "processed": true, "output_url": "https://..." }, "reason": "Data obtained from backup system", "completed_by": "operator@example.com", "confirm": true }`
**Expected**: Step marked complete with provided result data. Downstream dependent steps are unblocked.

### Alternative: Submit a New Task

**Tool**: `task_submit`
**Parameters**: `{ "name": "retry_failed_batch", "namespace": "data_processing", "context": { "batch_id": "2026-03-01" }, "initiator": "operator@example.com" }`
**Expected**: Preview shows template details and context shape. Confirm to create and execute the task.

## Cross-Persona: Incident Response Flow

When an incident triggers, multiple roles collaborate using these tools in sequence:

1. **SRE** runs `system_health` → identifies degraded component
2. **SRE** runs `staleness_check` → finds stale tasks piling up
3. **Ops** runs `dlq_stats` → confirms DLQ spike correlates with degraded component
4. **Ops** runs `dlq_queue` → identifies affected task patterns
5. **Engineer** runs `task_inspect` on affected tasks → understands the failure mode
6. **Engineer** runs `step_audit` → traces the sequence of events to root cause
7. **Ops** runs `step_retry` on affected steps → resets them for re-execution after fix
8. **Ops** runs `dlq_update` → records investigation notes and resolution status
9. **SRE** runs `analytics_performance` post-fix → confirms recovery

Each handoff passes context (UUIDs, step IDs, error patterns) from one tool's output to the next tool's input.
