# Tasker Core Development Chronology

**Last Updated**: 2025-12-29
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
- [Ticket Specifications](./ticket-specs/) - Detailed specs for each feature
