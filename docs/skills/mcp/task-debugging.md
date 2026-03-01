# Skill: Task Debugging with Connected MCP Tools

Use this skill when a developer needs to **investigate task failures, stuck workflows, or unexpected step results** using Tasker's connected MCP tools.

## When to Apply

- Developer says a task is "stuck", "failed", or "not completing"
- Developer wants to see what happened with a specific task or step
- Developer needs to trace execution through a multi-step workflow
- Developer wants to understand retry behavior or step ordering

## Agent Posture

You are a **diagnostic assistant**. Guide the developer through a structured investigation flow, starting broad (task list) and narrowing to the specific failure point (step audit). Don't skip steps — the investigation context builds progressively.

## Tool Sequence

```
connection_status → task_list → task_inspect → step_inspect → step_audit
```

### Phase 1: Verify Connectivity

Always start with `connection_status` to confirm the profile is connected and healthy. If offline or unreachable, resolve connectivity first.

### Phase 2: Find the Task

Use `task_list` with appropriate filters:
- **Namespace filter**: When the developer knows which workflow type
- **Status filter**: `error` for failures, `pending`/`initializing` for stuck tasks
- **Limit**: Start with 10-20 to avoid overwhelming output

### Phase 3: Inspect the Task

Use `task_inspect` with the task UUID. Look for:
- **Completion percentage**: How far did it get?
- **Step statuses**: Which steps completed, which failed?
- **Dependencies**: Is a step waiting on a failed upstream step?

### Phase 4: Drill into Steps

Use `step_inspect` on the failed or stuck step. Key data:
- **Handler callable**: Which code ran?
- **Results**: What did it return (or what error)?
- **Retry count**: Did it exhaust retries?
- **Timing**: How long did execution take?

### Phase 5: Audit Trail

Use `step_audit` for the complete state transition history. Useful for:
- Understanding retry timing and backoff behavior
- Identifying which worker processed each attempt
- Correlating with external logs via timestamps

## Decision Tree

| Symptom | Investigation Path |
|---------|-------------------|
| Task stuck in `pending` | `task_inspect` → check if template is registered (`template_list_remote`) |
| Task stuck in `steps_in_process` | `task_inspect` → find pending steps → `step_inspect` for handler errors |
| Step in `error` state | `step_inspect` → check retry count vs max_attempts → `step_audit` for retry history |
| Step completed but wrong results | `step_inspect` → examine results JSON → check handler logic |
| Task in `error` state | `task_inspect` → find failed steps → DLQ tools for investigation context |

## Phase 6: Remediation with Write Tools

After identifying the root cause, use Tier 3 write tools to resolve the issue. All write tools use a **preview → confirm** pattern.

| Symptom | Resolution Tool | Action |
|---------|----------------|--------|
| Step failed, transient cause fixed | `step_retry` | Reset step for re-execution by a worker |
| Step failed, work done out-of-band | `step_resolve` | Mark as resolved to unblock downstream steps |
| Step failed, you have correct data | `step_complete` | Provide result data for downstream consumers |
| Task should not continue | `task_cancel` | Cancel task and all pending steps (irreversible) |

Always preview first (omit `confirm`), verify the action is correct, then execute with `confirm: true`.

## Anti-Patterns

- **Don't guess UUIDs**: Always get them from `task_list` or `task_inspect` output
- **Don't skip connectivity check**: Connected tool failures are confusing if the real issue is connectivity
- **Don't jump to step_audit first**: The task-level view provides essential context for interpreting step-level data
