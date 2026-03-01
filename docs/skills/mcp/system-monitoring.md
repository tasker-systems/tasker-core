# Skill: System Monitoring with Connected MCP Tools

Use this skill when an SRE or platform engineer needs to **assess system health, monitor performance, or verify configuration** of a running Tasker instance.

## When to Apply

- Routine health checks during on-call shifts
- Verifying a deployment looks healthy
- Investigating performance degradation reports
- Checking configuration matches expectations
- Monitoring for task staleness or accumulation

## Agent Posture

You are a **systems diagnostician**. Present findings in a structured way that supports operational decisions. Highlight anomalies, suggest escalation paths, and provide context for metrics.

## Tool Sequence

```
connection_status → system_health → system_config → staleness_check → analytics_performance → analytics_bottlenecks
```

### Phase 1: Connectivity & Health

Start with `connection_status` (with `refresh: true` for fresh probes), then `system_health` for component-level detail. Key components to check:
- Database pool utilization
- Message queue depth
- Circuit breaker states (closed = healthy, open = failing)

### Phase 2: Configuration Verification

Use `system_config` to verify the running configuration. Secrets are redacted. Useful after deployments to confirm config changes took effect.

### Phase 3: Staleness Monitoring

Use `staleness_check` to detect tasks that are taking longer than expected. Health annotations:
- **healthy**: Within expected thresholds
- **warning**: Approaching staleness threshold
- **stale**: Exceeded threshold — investigate

### Phase 4: Performance Analysis

Use `analytics_performance` for throughput and latency metrics. Then `analytics_bottlenecks` to identify slow handlers.

## Escalation Paths

| Health Signal | Severity | Action |
|--------------|----------|--------|
| All components healthy, no staleness | Normal | Log and continue |
| Circuit breaker open | Warning | Check upstream dependency, may self-heal |
| Database pool exhaustion | High | Check connection leaks, increase pool size |
| Stale tasks accumulating | High | Check worker health, message queue depth |
| High DLQ accumulation | Critical | Transition to DLQ triage workflow |

## Anti-Patterns

- **Don't run analytics without health check first**: Performance numbers are meaningless if the system is unhealthy
- **Don't alarm on empty analytics**: A freshly deployed system will have zero metrics — that's normal
- **Don't share system_config output externally**: Even with redaction, it reveals infrastructure topology
