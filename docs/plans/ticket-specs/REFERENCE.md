# Historical Ticket Specifications Reference

This document provides a reference to historical ticket specifications that shaped Tasker Core's architecture. Detailed specs have been archived from this directory (available in git history), with key decisions extracted to [ADRs](../decisions/) and progress consolidated in [CHRONOLOGY.md](../CHRONOLOGY.md).

For active ticket specs, see the remaining entries in this directory.

## Ticket Reference

### Foundation (2025-08 through 2025-10)

| Ticket | Title | Summary | ADR |
|--------|-------|---------|-----|
| TAS-29 | Observability & Benchmarking | OpenTelemetry integration, correlation IDs, SQL benchmarking | — |
| TAS-32 | Enqueued State Architecture | Step-level idempotency via "Enqueued" state | — |
| TAS-34 | OrchestrationExecutor Trait | Executor pool system replacing naive polling loops | — |
| TAS-37 | Task Finalizer Race Fix | Atomic claim SQL functions for finalization | — |
| TAS-40 | Worker Foundations | Rust WorkerSystem with FFI integration | — |
| TAS-41 | Pure Rust Worker | Standalone Rust worker with handler registry | — |
| TAS-42 | Ruby Binding Simplification | Simplified Ruby to pure business logic | — |
| TAS-43 | Event-Driven Task Claiming | PostgreSQL LISTEN/NOTIFY for task discovery | — |
| TAS-47 | Blog Post Migration | Migrated Rails examples to tasker-core with Ruby FFI | — |
| TAS-49 | DLQ & Lifecycle Management | Dead letter queue with staleness detection | — |
| TAS-50 | Configuration System | TOML-based hierarchical configuration | — |
| TAS-51 | Bounded MPSC Channels | All channels bounded, configuration-driven | ADR-002 |
| TAS-53 | DecisionPoint Steps | Dynamic workflow branching with convergence | — |
| TAS-54 | Ownership Removal | Audit-only processor UUID tracking | ADR-003 |
| TAS-56 | CI Stabilization | Pipeline reliability improvements | — |
| TAS-57 | Backoff Consolidation | Unified retry/backoff strategy | ADR-004 |
| TAS-58 | Rust Standards Compliance | Microsoft/Rust API Guidelines implementation | — |
| TAS-59 | Batch Processing | Cursor-based batch processing with checkpoints | — |
| TAS-60 | Configuration Bug Fix | Duplicate of TAS-50 | — |
| TAS-61 | Configuration v2 | Environment-based config architecture | — |
| TAS-63 | Codebase Analysis | Deep codebase analysis and learnings | — |
| TAS-64 | Retry E2E Testing | Retryability and resumability test suite | — |
| TAS-65 | Domain Events | Fire-and-forget domain event publication | — |
| TAS-67 | Dual Event System | Non-blocking dual-channel worker pattern | ADR-005 |
| TAS-69 | Worker Decomposition | Actor-based worker refactor | ADR-006 |
| TAS-71 | Profiling & Performance | Comprehensive profiling and performance analysis | — |
| TAS-72 | Python Worker | PyO3-based Python worker foundations | — |
| TAS-73 | Multi-Instance Cluster | Cluster testing, test feature flags | — |
| TAS-75 | Backpressure | Unified backpressure handling strategy | — |
| TAS-76 | Cross-Language Alignment | API alignment across Ruby, Python, Rust | — |
| TAS-78 | Circuit Breakers | Circuit breaker implementations | — |

### Architecture & Features (2025-11 through 2025-12)

| Ticket | Title | Summary | ADR |
|--------|-------|---------|-----|
| TAS-89 | Task Templates | Template-based task definition system | — |
| TAS-91 | Worker Event System | Worker event system implementation | — |
| TAS-92 | API Alignment | REST/gRPC API surface alignment | — |
| TAS-93 | Task Handler Cleanup | Vestigial task_handler concept removal | — |
| TAS-99 | Worker Polling | Polling contract standardization | — |
| TAS-100 | TypeScript Worker | Bun and Node.js TypeScript worker via FFI | ADR-007 |
| TAS-111 | cargo-make Standardization | Unified task runner configuration across workspace | — |
| TAS-112 | Handler Ergonomics | Cross-language handler composition pattern | ADR-008 |

### Abstractions & Security (2026-01)

| Ticket | Title | Summary | ADR |
|--------|-------|---------|-----|
| TAS-125 | Batchable Checkpoint | Checkpoint-yield pattern for batch processing | — |
| TAS-126 | Tasker Contrib | Contrib foundations and CLI plugin architecture | — |
| TAS-128 | PostgreSQL 18 Migration | Schema flattening, namespace refactoring | — |
| TAS-132 | PGMQ Message Groups | Queue-per-namespace + message groups analysis | — |
| TAS-133 | Messaging Abstraction | Provider-agnostic messaging strategy pattern | ADR-010 |
| TAS-134 | Atomic Counters | Hot-path stats converted to atomics | — |
| TAS-136 | Legacy Schema Cleanup | Drop vestigial Rails engine tables/columns | — |
| TAS-148 | Command Processor Refactor | Extract command types, reduce boilerplate | — |
| TAS-149 | Step State Refinement | Step state machine refinements | — |
| TAS-150 | API Security | JWT/API key authentication, permission enforcement | ADR-012 |
| TAS-154 | Task Identity Strategy | STRICT/CALLER_PROVIDED/ALWAYS_UNIQUE strategies | ADR-011 |
| TAS-156 | Redis Cache | Task template cache with graceful degradation | ADR-013 |
| TAS-157 | DB Pattern Optimizations | ResultProcessingContext, correlation ID joins | — |
| TAS-162 | Hot-Path Logging | Logging CPU optimization (5.15% → target 2-3%) | — |
| TAS-163 | Test Infrastructure | Test feature flag hierarchy | — |
| TAS-164 | Test Stabilization | Intermittent test fixes | — |
| TAS-166 | Benchmarks | Benchmark infrastructure and results | — |
| TAS-168 | Response Caching | Analytics endpoint caching with Moka | ADR-013 |
| TAS-170 | Release Management | Automated release system for 6 crates + 3 FFI | — |
| TAS-171 | gRPC Foundation | gRPC transport layer foundations | — |
| TAS-175 | gRPC Services | gRPC service implementations | — |
| TAS-176 | Client Library | REST + gRPC transport client | — |
| TAS-177 | gRPC Testing | REST/gRPC parity test infrastructure | — |
| TAS-187 | Worker Refinements | Worker architecture refinements | — |
| TAS-188 | API Refinements | API surface refinements | — |
| TAS-194 | Documentation | Architecture documentation improvements | — |
| TAS-205 | Example Applications | Standalone examples (Rails, FastAPI, Bun, Axum) | — |
| TAS-208 | CLI Templates | tasker-ctl template system | — |
| TAS-209 | CLI Remotes | tasker-ctl remote template repositories | — |
| TAS-281 | Minor Fix | Implementation fix | — |
| TAS-282 | Minor Fix | Implementation fix | — |

## Related Documentation

- [Architecture Decision Records](../decisions/) — Formal decision records (ADR-001 through ADR-013)
- [CHRONOLOGY](../CHRONOLOGY.md) — Development timeline and lessons learned
- [Principles](../principles/) — Core design principles
