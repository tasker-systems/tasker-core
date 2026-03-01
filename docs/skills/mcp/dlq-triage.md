# Skill: DLQ Triage with Connected MCP Tools

Use this skill when a technical operator needs to **investigate dead letter queue entries, understand failure patterns, and prepare resolution actions** using Tasker's connected MCP tools.

## When to Apply

- DLQ monitoring dashboard shows accumulating entries
- Alerts fire on DLQ entry count or rate
- Post-incident review of task failures
- Routine DLQ hygiene and cleanup triage

## Agent Posture

You are a **triage coordinator**. Help the operator systematically work through DLQ entries from overview to detail, identifying patterns and root causes. Distinguish between systemic issues (affecting many tasks) and one-off failures.

## Tool Sequence

```
dlq_stats → dlq_queue → dlq_list → dlq_inspect → task_inspect → step_inspect → step_audit
```

### Phase 1: Statistical Overview

Use `dlq_stats` to understand the landscape. Key dimensions:
- **By reason code**: retry_exhaustion, permanent_failure, timeout, etc.
- **Volume**: Is this a spike or steady accumulation?

### Phase 2: Prioritized Queue

Use `dlq_queue` to get entries ranked by severity. The investigation queue considers:
- Recency (newer failures may indicate active problems)
- Impact (failures in high-priority tasks)
- Pattern (repeated failures in the same handler)

### Phase 3: Entry List & Inspection

Use `dlq_list` for a broader view, then `dlq_inspect` on specific entries. The inspect view provides:
- Error message and context
- Task/step references for cross-referencing
- Retry history showing what was already attempted

### Phase 4: Cross-Reference

Use `task_inspect` and `step_inspect` to understand the original task context. This helps distinguish:
- **Bad input**: The task's context data caused the failure
- **Handler bug**: The handler code has a defect
- **Infrastructure**: External dependency was unavailable

## Triage Decision Tree by Reason Code

| Reason Code | Likely Cause | Investigation |
|-------------|-------------|---------------|
| `retry_exhaustion` | Transient failure that didn't recover | Check handler retry config, external dependency health |
| `permanent_failure` | Non-retryable error in handler | Check handler code, validate input data |
| `timeout` | Step exceeded timeout_seconds | Check handler performance, increase timeout if justified |
| `dependency_failure` | Upstream step failed | Investigate the upstream step first |

## Phase 5: Resolution with Tier 3 Write Tools

After diagnosis, use write tools to remediate. All write tools use a two-phase **preview → confirm** pattern — call without `confirm` to see what will happen, then call with `confirm: true` to execute.

### Remediation Actions

| Action | Tool | When to Use |
|--------|------|-------------|
| Retry a failed step | `step_retry` | Root cause resolved (e.g., dependency restored), step can succeed on re-execution |
| Manually resolve a step | `step_resolve` | Step's work was completed out-of-band, or step is non-critical and can be bypassed |
| Manually complete with data | `step_complete` | You have the correct result data from another source; provides it to downstream steps |
| Update DLQ investigation | `dlq_update` | Record resolution status, notes, and operator identity after fixing the underlying issue |

### Typical Resolution Flow

```
dlq_inspect → task_inspect → step_inspect (diagnose)
  → step_retry / step_resolve / step_complete (fix the step)
  → dlq_update (record resolution with notes)
```

Always fix at the **step level first** (retry, resolve, or complete), then update the **DLQ entry** to record the investigation outcome. The DLQ entry tracks the investigation; the actual fix happens at the step level.

## Anti-Patterns

- **Don't resolve entries without understanding root cause**: Blind resolution hides systemic problems
- **Don't investigate entries one-by-one when there's a pattern**: Use `dlq_stats` to identify clusters first
- **Don't ignore low-priority entries indefinitely**: Accumulated DLQ entries create noise and slow queries
