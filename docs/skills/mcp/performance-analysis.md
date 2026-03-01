# Skill: Performance Analysis with Connected MCP Tools

Use this skill when an analyst or engineer needs to **evaluate system throughput, identify bottlenecks, and inform capacity planning** using Tasker's connected MCP tools.

## When to Apply

- Weekly or monthly performance reviews
- Capacity planning for scaling decisions
- Identifying optimization targets in workflow execution
- Post-deployment performance validation
- Investigating throughput degradation

## Agent Posture

You are a **performance analyst**. Present data with context, highlight trends, and connect metrics to actionable optimization opportunities. Avoid raw data dumps — interpret the numbers.

## Tool Sequence

```
analytics_performance → analytics_bottlenecks → task_list (with filters)
```

### Phase 1: Performance Overview

Use `analytics_performance` for aggregate metrics:
- **Throughput**: Tasks completed per time period
- **Latency**: P50, P95, P99 completion times
- **Success rate**: Percentage of tasks completing without error
- **Step execution**: Average steps per task, parallelism ratio

### Phase 2: Bottleneck Identification

Use `analytics_bottlenecks` to find slow handlers:
- Steps ranked by execution time
- Handler callable names for code-level investigation
- Execution counts to distinguish rare slow paths from systemic slowness

Focus on steps with **high execution count AND high latency** — these are the best optimization targets.

### Phase 3: Filtered Task Analysis

Use `task_list` with filters to examine specific segments:
- **By namespace**: Compare performance across workflow types
- **By status**: Analyze error rates per namespace
- **With pagination**: Sample recent executions for trend analysis

## Interpretation Guidance

| Metric Pattern | Interpretation | Action |
|---------------|---------------|--------|
| High throughput, low latency | Healthy system | Document as baseline |
| High throughput, rising latency | Approaching capacity | Plan scaling |
| Low throughput, high latency | Performance degradation | Investigate bottleneck steps |
| High error rate, one namespace | Handler-specific issue | Focus on that namespace's handlers |
| High error rate, all namespaces | Infrastructure issue | Escalate to SRE (system_health) |

## Anti-Patterns

- **Don't optimize without data**: Run analytics before making performance changes
- **Don't optimize rare paths**: A step that runs once a day isn't a bottleneck even if it's slow
- **Don't compare across environments**: Staging metrics don't predict production performance
- **Don't ignore the parallelism ratio**: Low parallelism means workflows aren't leveraging DAG concurrency
