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

## Bridges to Tier 3

After diagnosis with these read-only tools, resolution requires Tier 3 mutation tools (future):
- **Retry**: Re-enqueue the failed step for another attempt
- **Resolve**: Mark the DLQ entry as investigated and resolved
- **Skip**: Mark the step as skipped to unblock downstream steps

Until Tier 3 tools are available, use `tasker-ctl` CLI or direct API calls for mutations.

## Anti-Patterns

- **Don't resolve entries without understanding root cause**: Blind resolution hides systemic problems
- **Don't investigate entries one-by-one when there's a pattern**: Use `dlq_stats` to identify clusters first
- **Don't ignore low-priority entries indefinitely**: Accumulated DLQ entries create noise and slow queries
