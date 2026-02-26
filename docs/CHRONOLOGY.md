# Tasker Core Development Chronology

**Last Updated**: 2026-02-26
**Status**: Active

This document captures the major architectural decisions, feature milestones, and lessons learned during Tasker Core development. Use it to understand *why* things are the way they are.

---

## Timeline Overview

### 2025-08: Foundation

| Category | What Happened |
|----------|---------------|
| Foundation | Axum web API established as the HTTP layer |
| Observability | OpenTelemetry integration + correlation IDs + benchmarking infrastructure |

**Key outcome**: Core web infrastructure and observability foundation in place.

### 2025-10: Core Architecture

| Category | What Happened |
|----------|---------------|
| Bug Fix | Task finalizer race condition eliminated through atomic state transitions |
| Architecture | Enhanced state machines with 12 task states and 8 step states |
| Architecture | Event-driven task claiming via PostgreSQL LISTEN/NOTIFY |
| Architecture | Actor-based lifecycle components introduced (4 orchestration actors) |
| Feature | Dead Letter Queue (DLQ) system for stuck task investigation |
| Resilience | Bounded MPSC channels mandated everywhere |
| Feature | Dynamic workflows and decision points |

**Key outcomes**:

- Actor pattern established as core architectural approach
- Event-driven + polling hybrid pattern defined
- All channels bounded, backpressure everywhere

### 2025-10: The Ownership Enforcement Breakthrough

| Category | What Happened |
|----------|---------------|
| **Breakthrough** | Processor UUID ownership enforcement **removed** |

This was a pivotal moment. Analysis proved that:

1. Ownership enforcement was redundant (four protection layers already sufficient)
2. Ownership enforcement *prevented* automatic recovery after crashes
3. Tracking for audit (without enforcement) provides full visibility

> **Lesson learned**: "Processor UUID ownership was redundant protection with harmful side effects."

See [Defense in Depth](./principles/defense-in-depth.md) for the full protection model.

### 2025-11: Batch Processing

| Category | What Happened |
|----------|---------------|
| Feature | Batch processing with cursor-based workers |

**Key outcome**: Large dataset processing via paginated batch workers with cursor state.

### 2025-12: Worker Architecture & Cross-Language

| Category | What Happened |
|----------|---------------|
| Architecture | Distributed event system (durable/fast/broadcast modes) |
| Architecture | Rust worker dual-event system (dispatch + completion channels) |
| Refactor | Worker actor-service decomposition (1,575 LOC → 5 focused actors) |
| Workers | Python worker via PyO3 FFI |
| Resilience | Backpressure and circuit breakers unified |
| API | Cross-language API alignment initiative |
| Workers | TypeScript worker via FFI (FFI chosen over WASM) |
| Tooling | cargo-make standardization across workspace |
| Research | Handler ergonomics analysis (composition pattern identified) |

**Key outcomes**:

- Worker architecture mirrors orchestration's actor pattern
- FFI chosen over WASM for pragmatic reasons
- Cross-language API consistency established
- Composition over inheritance identified as target pattern

### 2026-01: Abstractions & Security

| Category | What Happened |
|----------|---------------|
| Architecture | Messaging strategy pattern abstraction (PGMQ/RabbitMQ provider-agnostic) |
| Architecture | Task identity strategy pattern (STRICT, CALLER_PROVIDED, ALWAYS_UNIQUE) |
| Architecture | Command processor refactor — extracted command types, reduced boilerplate (TAS-148) |
| Security | Permission enforcement boundary defined (enforce, don't manage identity) |
| Architecture | Cache-aside pattern with graceful degradation (Redis/Moka/NoOp) |
| Performance | Hot-path logging optimization (5.15% → target 2-3% CPU) |
| Testing | gRPC test infrastructure with REST/gRPC parity validation (TAS-177) |
| Feature | Database pattern optimizations scoped (ResultProcessingContext, TaskFinalizationContext) |

**Key outcomes**:

- Provider-agnostic messaging enables zero-code migration between PGMQ and RabbitMQ
- Identity strategy gives each use case appropriate deduplication semantics
- Security model cleanly separates enforcement (Tasker) from identity management (external)
- Cache never becomes a failure point — graceful degradation to NoOp

### 2026-02: TypeScript FFI & Tooling

| Category | What Happened |
|----------|---------------|
| Architecture | napi-rs replaces koffi for TypeScript FFI — eliminates TAS-283 trailing input bugs |
| Workers | TypeScript DSL handler examples and parity tests (TAS-294) |
| Workers | Handler ergonomics harmonization completed across Ruby, Python, TypeScript (TAS-112) |
| Tooling | cargo-make standardization across workspace (TAS-111) |
| Tooling | `tasker-ctl` plugin architecture and remote template system (TAS-126/TAS-208/TAS-270) |
| Documentation | Architecture documentation curated for mdbook (TAS-218) |
| Cleanup | Vestigial `task_handler` concept removed from codebase (TAS-93) |
| Planning | PostgreSQL 18 migration and schema flattening scoped (TAS-128) |
| Planning | Standalone example applications designed (TAS-205) |
| Planning | Automated release management system designed (TAS-170) |

**Key outcomes**:

- TypeScript worker reaches parity with Ruby (magnus) and Python (pyo3) via napi-rs
- Single N-API binary serves Bun, Node.js, and Deno — eliminates multi-runtime FFI layer
- `tasker-ctl` evolves into extensible developer tool with plugin discovery and template generation
- Ticket specs consolidated into ADRs and chronology; directory cleaned to active specs only

---

## Architectural Decisions

### Actor Pattern Adoption

**Context**: Monolithic command processors were growing unwieldy (1,500+ LOC files).

**Decision**: Adopt lightweight actor pattern with message-passing:

- 4 orchestration actors (TaskRequest, ResultProcessor, StepEnqueuer, TaskFinalizer)
- 5 worker actors (StepExecutor, FFICompletion, TemplateCache, DomainEvent, WorkerStatus)

**Outcome**: ~92% reduction in per-file complexity, clear ownership boundaries, improved testability.

### Ownership Enforcement Removal

**Context**: Processor UUID was being used to enforce "ownership" of tasks during processing.

**Discovery**: When analyzing race conditions, we found:

1. Four protection layers (database, state machine, transaction, application) already prevent corruption
2. Ownership enforcement blocked recovery when orchestrator crashed and restarted with new UUID
3. No data corruption occurred in 377 tests without ownership enforcement

**Decision**: Remove enforcement, keep tracking for audit.

**Outcome**: Tasks auto-recover after crashes; audit trails preserved; zero data corruption.

### FFI Over WASM

**Context**: TypeScript worker needed Rust integration. WASM seemed "pure" but FFI was proven.

**Analysis**:

- WASM: No production PostgreSQL client, single-threaded, WASI immaturity
- FFI: Proven in Ruby (Magnus) and Python (PyO3), identical polling contract

**Decision**: Use FFI for all language workers, reserve WASM for future serverless handlers.

**Outcome**: Pattern consistency across Ruby/Python/TypeScript; single Rust codebase serves all.

### Composition Over Inheritance

**Context**: Handler capabilities (API, Decision, Batchable) were growing complex.

**Discovery**: Batchable handlers already used mixin pattern successfully.

**Decision**: Migrate all handlers to composition pattern:

```
Not: class Handler < API
But: class Handler < Base; include API, include Decision, include Batchable
```

**Outcome**: Selective capability inclusion, clear separation of concerns, easier testing.

### Messaging Strategy Pattern

**Context**: PGMQ was hard-coded as the messaging backend. RabbitMQ support needed without code changes.

**Decision**: Enum-based provider abstraction with zero-cost dispatch. `MessageNotification` enum handles the signal-vs-payload divide between PGMQ and RabbitMQ. Dual command variants (`*FromMessage`/`*FromMessageEvent`) enable provider-agnostic routing.

**Outcome**: Zero-code migration between providers via configuration. Community providers (SQS, Kafka) extensible via `tasker-contrib`.

### Task Identity Strategy

**Context**: Different use cases have fundamentally different deduplication needs.

**Decision**: Three strategies (STRICT hash, CALLER_PROVIDED key, ALWAYS_UNIQUE uuid) per template, with per-request override. 409 Conflict on duplicates (security-conscious).

**Outcome**: Each use case gets appropriate identity semantics without workarounds.

### Permission Enforcement Boundary

**Context**: Tasker needed auth without becoming an identity provider.

**Decision**: Enforce permissions at API boundary; delegate identity management to external providers (JWT/JWKS). Health endpoints always public.

**Outcome**: Integrates with any identity provider. No user management, no password storage.

### napi-rs Over koffi

**Context**: koffi + C FFI approach had unfixable trailing input bugs (TAS-283) in TypeScript worker.

**Decision**: Replace koffi with napi-rs. Single `.node` binary serves Bun, Node.js, and Deno via N-API. Eliminates JSON serialization, C string marshalling, and manual memory management at FFI boundary.

**Outcome**: TypeScript FFI parity with Ruby (magnus) and Python (pyo3). Entire runtime abstraction layer deleted.

---

## Lessons Learned

### Defense in Depth (from Ownership Enforcement Removal)

> "Find the minimal set of protections that prevents corruption. Additional layers that prevent recovery are worse than none."

The four-layer protection model (database, state machine, transaction, application) is sufficient. Don't add protections that block recovery.

### Parallel Execution Reveals Bugs (from Worker Dual-Event System)

> "Parallel execution changed probability distributions of state combinations, transforming a latent SQL precedence bug into a discoverable one."

Heisenbugs become Bohrbugs when you stress the system. True parallel execution surfaced bugs that sequential execution never showed.

### Maturity Over Purity (from FFI Over WASM)

> "FFI wins over WASM for pragmatic reasons - WASI networking immature."

Production readiness matters more than architectural purity. Choose proven technology for core paths; experiment on edges.

### One Obvious Way (from Composition Over Inheritance)

> "Batchable already uses mixin pattern - this is the TARGET architecture."

Look for patterns that emerged naturally. If one handler type already works well, that's likely the right pattern for all.

### Gaps Surface During Migration (from Worker Actor Decomposition)

> "Moving from monolithic to actor-based revealed three gaps: domain events not dispatched, errors silently swallowed, namespace sharing lost."

Refactoring is discovery. The act of decomposition reveals hidden assumptions and undocumented behaviors.

### Enum Dispatch Over Trait Objects (from Messaging + Cache Abstractions)

> "When the provider set is small and known at compile time, enum dispatch is zero-cost and provides exhaustive matching. Save trait objects for truly open extension points."

Both the messaging abstraction (ADR-010) and cache provider (ADR-013) use enum dispatch rather than `Box<dyn Trait>`. The compile-time exhaustive match catches missing provider implementations; trait objects would defer this to runtime.

### Enforce, Don't Manage (from Permission Boundary)

> "Tasker enforces permissions but doesn't manage identity. External systems handle the hard problems of authentication; Tasker handles the easy problem of checking claims."

Avoiding identity management keeps Tasker focused. The permission vocabulary maps 1:1 to API operations, preventing the abstraction mismatch that plagues generic RBAC systems.

### Graceful Degradation as Default (from Cache Architecture)

> "Optional infrastructure should degrade to no-op, never to failure. If Redis is down, log a warning and continue — PostgreSQL is always the source of truth."

This principle extended from caching to other optional components. The system should always start and serve requests; optional infrastructure improves performance but never gates correctness.

---

## Pre-Alpha Philosophy

Throughout development, the pre-alpha status enabled:

1. **Breaking changes encouraged**: Architecture correctness over backward compatibility
2. **Rapid iteration**: Learn from real implementation, correct course quickly
3. **Pattern discovery**: Let good patterns emerge, then standardize
4. **Technical debt avoidance**: Fix things properly rather than adding workarounds

This freedom is temporary. Once stable, these patterns become the foundation.

---

## Related Documentation

- [Tasker Core Tenets](./principles/tasker-core-tenets.md) - The 10 principles that emerged
- [Defense in Depth](./principles/defense-in-depth.md) - Protection model from ownership enforcement removal
- [Composition Over Inheritance](./principles/composition-over-inheritance.md) - Handler composition pattern
- [Cross-Language Consistency](./principles/cross-language-consistency.md) - Cross-language API philosophy
- [Architecture Decision Records](./decisions/) - Formal decision records (ADR-001 through ADR-013)
- [Ticket Specifications](./ticket-specs/) - Active specs (recent tickets only; historical specs archived in git)
