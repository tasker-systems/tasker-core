# Distributed Benchmarking Strategy

**Status**: 🎯 Framework Complete | Implementation In Progress
**Last Updated**: 2025-10-08

---

## Overview

Complete benchmarking infrastructure for measuring distributed system performance across all components.

## Benchmark Suite Structure

### ✅ Implemented

#### 1. **API Task Creation** (`crates/tasker-client/benches/task_initialization.rs`)

**Status**: ✅ **COMPLETE** - Fully implemented and tested

**Measures**:

- HTTP request → task initialized latency
- Task record creation in PostgreSQL
- Initial step discovery from template
- Response generation and serialization

**Results** (2025-10-08):

```
Linear (3 steps):   17.7ms  (Target: < 50ms)  ✅ 3x better than target
Diamond (4 steps):  20.8ms  (Target: < 75ms)  ✅ 3.6x better than target
```

**Run Command**:

```bash
cargo bench --package tasker-client --features benchmarks
```

#### 2. **SQL Function Performance** (`crates/tasker-shared/benches/sql_functions.rs`)

**Status**: ✅ **COMPLETE** - Fully implemented (Phase 5.2)

**Measures**:

- 6 critical PostgreSQL function benchmarks
- Intelligent stratified sampling (5-10 diverse samples per function)
- EXPLAIN ANALYZE query plan analysis (run once per function)

**Results** (from Phase 5.2):

```
Task discovery:            1.75-2.93ms  (O(1) scaling!)
Step readiness:            440-603µs    (37% variance captured)
State transitions:         ~380µs       (±5% variance)
Task execution context:    448-559µs
Step dependencies:         332-343µs
Query plan buffer hit:     100%         (all functions)
```

**Run Command**:

```bash
DATABASE_URL="postgresql://tasker:tasker@localhost:5432/tasker_rust_test" \
cargo bench --package tasker-shared --features benchmarks sql_functions
```

### 🚧 Placeholders (Ready for Implementation)

#### 3. **Worker Processing Cycle** (`crates/tasker-worker/benches/worker_execution.rs`)

**Status**: 🚧 Skeleton created - needs implementation

**Measures**:

- **Claim**: PGMQ read + atomic claim
- **Execute**: Handler execution (framework overhead)
- **Submit**: Result serialization + HTTP submit
- **Total**: Complete worker cycle

**Targets**:

- Claim: < 20ms
- Execute (noop): < 10ms
- Submit: < 30ms
- **Total overhead**: < 60ms

**Implementation Requirements**:

- Pre-enqueued steps in namespace queues
- Worker client with breakdown metrics
- Multiple handler types (noop, calculation, database)
- Accurate timestamp collection for each phase

**Run Command** (when implemented):

```bash
cargo bench --package tasker-worker --features benchmarks worker_execution
```

#### 4. **Event Propagation** (`crates/tasker-shared/benches/event_propagation.rs`)

**Status**: 🚧 Skeleton created - needs implementation

**Measures**:

- PostgreSQL LISTEN/NOTIFY latency
- PGMQ `pgmq_send_with_notify` overhead
- Event system framework overhead

**Targets**:

- p50: < 5ms
- p95: < 10ms
- p99: < 20ms

**Implementation Requirements**:

- PostgreSQL LISTEN connection setup
- PGMQ notification channel configuration
- Concurrent listener with timestamp correlation
- Accurate cross-thread time measurement

**Run Command** (when implemented):

```bash
cargo bench --package tasker-shared --features benchmarks event_propagation
```

#### 5. **Step Enqueueing** (`crates/tasker-orchestration/benches/step_enqueueing.rs`)

**Status**: 🚧 Skeleton created - needs implementation

**Measures**:

- Ready step discovery (SQL query time)
- Queue publishing (PGMQ write time)
- Notification overhead (LISTEN/NOTIFY)
- Total orchestration coordination

**Targets**:

- 3-step workflow: < 50ms
- 10-step workflow: < 100ms
- 50-step workflow: < 500ms

**Implementation Requirements**:

- Pre-created tasks with dependency chains
- Orchestration client with result processing trigger
- Queue polling to detect enqueued steps
- Breakdown metrics (discovery, publish, notify)

**Challenge**: Triggering step discovery without full workflow execution

**Run Command** (when implemented):

```bash
cargo bench --package tasker-orchestration --features benchmarks step_enqueueing
```

#### 6. **Handler Overhead** (`crates/tasker-worker/benches/handler_overhead.rs`)

**Status**: 🚧 Skeleton created - needs implementation

**Measures**:

- Pure Rust handler (baseline - direct call)
- Rust handler via framework (dispatch overhead)
- Ruby handler via FFI (FFI boundary cost)

**Targets**:

- Pure Rust: < 1µs (baseline)
- Via Framework: < 1ms
- Ruby FFI: < 5ms

**Implementation Requirements**:

- Noop handler implementations (Rust + Ruby)
- Direct function call benchmarks
- Framework dispatch overhead measurement
- FFI bridge overhead measurement

**Run Command** (when implemented):

```bash
cargo bench --package tasker-worker --features benchmarks handler_overhead
```

#### 7. **End-to-End Latency** (`tests/benches/e2e_latency.rs`)

**Status**: 🚧 Skeleton created - needs implementation

**Measures**:

- Complete workflow execution (API → Task Complete)
- All system components (API, DB, Queue, Worker, Events)
- Real network overhead
- Different workflow patterns

**Targets**:

- Linear (3 steps): < 500ms p99
- Diamond (4 steps): < 800ms p99
- Tree (7 steps): < 1500ms p99

**Implementation Requirements**:

- All Docker Compose services running
- Orchestration client for task creation
- Polling mechanism for completion detection
- Multiple workflow templates
- Timeout handling for stuck workflows

**Special Considerations**:

- **SLOW by design**: Measures real workflow execution (seconds)
- Fewer samples (sample_size=10 vs 50 default)
- Higher variance expected (network + system state)
- Focus on regression detection, not absolute numbers

**Run Command** (when implemented):

```bash
# Requires all Docker services running
docker-compose -f docker/docker-compose.test.yml up -d

cargo bench --test e2e_latency
```

---

## Benchmark Output Logging Strategy

### Current State

**Implemented**:

- Criterion default output (terminal + HTML reports)
- Custom health check banners in benchmarks
- EXPLAIN ANALYZE output in SQL benchmarks
- Inline result commentary

**Location**: Results saved to `target/criterion/`

### Proposed Consistent Structure

#### 1. **Standard Output Format**

All benchmarks should follow this pattern:

```
═══════════════════════════════════════════════════════════════════════════════
🔍 VERIFYING PREREQUISITES
═══════════════════════════════════════════════════════════════════════════════
✅ All prerequisites met
═══════════════════════════════════════════════════════════════════════════════

Benchmarking <category>/<test_name>
...
<category>/<test_name>   time: [X.XX ms Y.YY ms Z.ZZ ms]

═══════════════════════════════════════════════════════════════════════════════
📊 BENCHMARK RESULTS: <CATEGORY NAME>
═══════════════════════════════════════════════════════════════════════════════

Performance Summary:
  • Test 1: X.XX ms  (Target: < YY ms)  ✅ Status
  • Test 2: X.XX ms  (Target: < YY ms)  ⚠️  Status

Key Findings:
  • Finding 1
  • Finding 2

═══════════════════════════════════════════════════════════════════════════════
```

#### 2. **Structured Log Files**

Proposal: Create `tmp/benchmarks/` directory with dated output:

```
tmp/benchmarks/
├── 2025-10-08-task-initialization.log
├── 2025-10-08-sql-functions.log
├── 2025-10-08-worker-execution.log
├── ...
└── latest/
    ├── task-initialization.log -> ../2025-10-08-task-initialization.log
    └── summary.md
```

**Log Format** (example):

```markdown
# Benchmark Run: task_initialization
Date: 2025-10-08 14:23:45 UTC
Commit: abc123def456
Environment: Docker Compose Test

## Prerequisites
- [x] Orchestration service healthy (http://localhost:8080)
- [x] Worker service healthy (http://localhost:8081)

## Results

### Linear Workflow (3 steps)
- Mean: 17.748 ms
- Std Dev: 0.624 ms
- Min: 17.081 ms
- Max: 18.507 ms
- Target: < 50 ms
- Status: ✅ PASS (3.0x better than target)
- Outliers: 2/20 (10%)

### Diamond Workflow (4 steps)
- Mean: 20.805 ms
- Std Dev: 0.741 ms
- Min: 19.949 ms
- Max: 21.633 ms
- Target: < 75 ms
- Status: ✅ PASS (3.6x better than target)
- Outliers: 0/20 (0%)

## Summary
✅ All tests passed
🎯 Average performance: 3.3x better than targets
```

#### 3. **Baseline Comparison Format**

For tracking performance over time:

```markdown
# Performance Baseline Comparison
Baseline: main branch (2025-10-01)
Current: feature/benchmarks (2025-10-08)

| Benchmark | Baseline | Current | Change | Status |
|-----------|----------|---------|--------|--------|
| task_init/linear | 18.2ms | 17.7ms | -2.7% | ✅ Improved |
| task_init/diamond | 21.1ms | 20.8ms | -1.4% | ✅ Improved |
| sql/task_discovery | 2.91ms | 2.93ms | +0.7% | ✅ Stable |
```

#### 4. **CI Integration Format**

For GitHub Actions / CI output:

```json
{
  "benchmark_suite": "task_initialization",
  "timestamp": "2025-10-08T14:23:45Z",
  "commit": "abc123def456",
  "results": [
    {
      "name": "linear_3_steps",
      "mean_ms": 17.748,
      "std_dev_ms": 0.624,
      "target_ms": 50,
      "status": "pass",
      "performance_ratio": 3.0
    }
  ],
  "summary": {
    "total_tests": 2,
    "passed": 2,
    "failed": 0,
    "warnings": 0
  }
}
```

---

## Running All Benchmarks

### Quick Reference

```bash
# 1. Start Docker services
docker-compose -f docker/docker-compose.test.yml up -d

# 2. Run individual benchmarks
cargo bench --package tasker-client --features benchmarks     # Task initialization
cargo bench --package tasker-shared --features benchmarks     # SQL + Events
cargo bench --package tasker-worker --features benchmarks     # Worker + Handlers
cargo bench --package tasker-orchestration --features benchmarks  # Step enqueueing
cargo bench --test e2e_latency                                # End-to-end

# 3. Run ALL benchmarks (when all implemented)
cargo bench --all-features
```

### Environment Variables

```bash
# Required for SQL benchmarks
export DATABASE_URL="postgresql://tasker:tasker@localhost:5432/tasker_rust_test"

# Optional: Skip health checks (CI)
export TASKER_TEST_SKIP_HEALTH_CHECK="true"

# Optional: Custom service URLs
export TASKER_TEST_ORCHESTRATION_URL="http://localhost:9080"
export TASKER_TEST_WORKER_URL="http://localhost:9081"
```

---

## Performance Targets Summary

| Category | Component | Metric | Target | Status |
|----------|-----------|--------|--------|--------|
| **API** | Task Creation (3 steps) | p99 | < 50ms | ✅ 17.7ms |
| **API** | Task Creation (4 steps) | p99 | < 75ms | ✅ 20.8ms |
| **SQL** | Task Discovery | mean | < 3ms | ✅ 1.75-2.93ms |
| **SQL** | Step Readiness | mean | < 1ms | ✅ 440-603µs |
| **Worker** | Total Overhead | mean | < 60ms | 🚧 TBD |
| **Worker** | FFI Overhead | mean | < 5ms | 🚧 TBD |
| **Events** | Notify Latency | p95 | < 10ms | 🚧 TBD |
| **Orchestration** | Step Enqueueing (3 steps) | mean | < 50ms | 🚧 TBD |
| **E2E** | Complete Workflow (3 steps) | p99 | < 500ms | 🚧 TBD |

---

## Next Steps

### Immediate (Current Session)

1. ✅ Create all benchmark skeletons
2. 🎯 Design consistent logging structure
3. Decide on implementation priorities

### Short Term

1. Implement worker execution benchmark
2. Implement event propagation benchmark
3. Create benchmark output logging utilities

### Medium Term

1. Implement step enqueueing benchmark
2. Implement handler overhead benchmark
3. Implement E2E latency benchmark

### Long Term

1. CI integration with baseline tracking
2. Performance regression detection
3. Automated benchmark reports
4. Historical performance trending

---

## Documentation

- **Full Plan**: [phase-5.4-distributed-benchmarks-plan.md](./phase-5.4-distributed-benchmarks-plan.md)
- **SQL Benchmarks**: [benchmarking-guide.md](./benchmarking-guide.md)
