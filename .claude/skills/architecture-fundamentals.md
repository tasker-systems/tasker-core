# Skill: Architecture Fundamentals

## When to Use

Use this skill when understanding the system architecture, working with actors, state machines, event systems, or the overall orchestration-worker pattern in tasker-core.

## Core Architecture Pattern

PostgreSQL-backed orchestration with provider-agnostic messaging (PGMQ default, RabbitMQ optional). Rust handles orchestration via lightweight actors; workers process steps via push notifications or polling.

**Separation of Concerns**:
- **Orchestration**: State management, coordination, step discovery, result processing
- **Workers**: Step execution, handler dispatch, domain events

## Workspace Crates

```
tasker-pgmq          # PGMQ wrapper with notification support
tasker-client        # API client library (REST + gRPC transport)
tasker-ctl           # CLI binary (config generate/validate, dry-run releases)
tasker-orchestration # Core orchestration logic (actors, services)
tasker-shared        # Shared types, traits, utilities, state machines
tasker-worker        # Worker implementation (handler dispatch, FFI)
workers/python       # Python FFI bindings (maturin/pyo3)
workers/ruby         # Ruby FFI bindings (magnus)
workers/rust         # Rust worker implementation
workers/typescript   # TypeScript FFI bindings (Bun/Node/Deno)
```

Crate-level documentation in `tasker-orchestration/AGENTS.md` and `tasker-worker/AGENTS.md`.

## Actor Pattern

### Orchestration Actors (4 core)

| Actor | Responsibility |
|-------|---------------|
| **TaskRequestActor** | Task initialization, step discovery |
| **ResultProcessorActor** | Step result processing, next-step triggering |
| **StepEnqueuerActor** | Batch step enqueueing to worker queues |
| **TaskFinalizerActor** | Task completion, finalization |

### Worker Actors (5 specialized)

| Actor | Responsibility |
|-------|---------------|
| **StepExecutorActor** | Step execution coordination |
| **FFICompletionActor** | FFI completion handling |
| **TemplateCacheActor** | Template cache management |
| **DomainEventActor** | Event dispatching |
| **WorkerStatusActor** | Status and health |

Each actor handles specific message types via bounded MPSC channels, enabling testability and clear ownership.

## State Machines

### Task State Machine (12 states)

**Flow**: `Pending -> Initializing -> EnqueuingSteps -> StepsInProcess -> EvaluatingResults -> Complete/Error`

State categories:
- **Initial**: Pending, Initializing
- **Active**: EnqueuingSteps, StepsInProcess, EvaluatingResults
- **Waiting**: WaitingForDependencies, WaitingForRetry, BlockedByFailures
- **Terminal**: Complete, Error, Cancelled, ResolvedManually

### Step State Machine (9 states)

**Flow**: `Pending -> Enqueued -> InProgress -> EnqueuedForOrchestration -> Complete`

Error path: `InProgress -> EnqueuedAsErrorForOrchestration -> WaitingForRetry -> Pending (retry)`

### State Machine Guarantees

- All transitions are **atomic** (compare-and-swap at database level)
- All transitions are **audited** (full history in transitions table)
- All transitions are **validated** (state guards prevent invalid transitions)
- Processor UUID tracked for audit, NOT enforced for ownership (TAS-54 discovery)

## Event System

### Event-Driven Communication

- **pg_notify**: PostgreSQL LISTEN/NOTIFY for real-time coordination
- **PGMQ/RabbitMQ**: Provider-agnostic message queues for worker communication
- **Domain events**: Published after state transitions committed
- **MPSC channels**: Internal actor communication (always bounded, configured via TOML)

### Deployment Modes

| Mode | Primary | Fallback | Best For |
|------|---------|----------|----------|
| **Hybrid** | pg_notify events | Polling | Production (recommended) |
| **EventDrivenOnly** | pg_notify events | None | Low-latency requirements |
| **PollingOnly** | Polling | N/A | Restricted networks |

## Worker Architecture

Dual-channel system:
- **Dispatch channel**: Routes steps to handlers via `HandlerDispatchService`
- **Completion channel**: Returns results to orchestration

Key components:
- `HandlerDispatchService`: Semaphore-bounded parallel execution
- `FfiDispatchChannel`: Pull-based polling for Ruby/Python FFI
- Handler resolution via resolver chain (ExplicitMapping -> Custom -> ClassLookup)

## Defense in Depth (4 Layers)

| Layer | Mechanism | Purpose |
|-------|-----------|---------|
| 1. Database Atomicity | Unique constraints, row locks, CAS | Prevent duplicate records |
| 2. State Machine Guards | Current state validation | Prevent invalid transitions |
| 3. Transaction Boundaries | All-or-nothing semantics | Prevent partial state |
| 4. Application Filtering | State-based deduplication | Idempotent processing |

**Key insight from TAS-54**: Processor UUID ownership enforcement was removed because layers 1-4 already prevent corruption, and enforcement blocked crash recovery.

## Bounded Resources (Tenet #10)

- Every MPSC channel is bounded (no `unbounded_channel()`)
- Channel sizes configured via TOML, not hard-coded
- Semaphores limit concurrent handler execution
- Circuit breakers protect downstream services

## Performance Targets

- 10-100x faster dependency resolution vs PostgreSQL functions
- <1ms overhead per step coordination
- >10k events/sec cross-language processing
- Zero race conditions via atomic claiming

## References

- Actor pattern: `docs/architecture/actors.md`
- State machines: `docs/architecture/states-and-lifecycles.md`
- Events: `docs/architecture/events-and-commands.md`
- Worker events: `docs/architecture/worker-event-systems.md`
- Crate architecture: `docs/architecture/crate-architecture.md`
- Backpressure: `docs/architecture/backpressure-architecture.md`
- Circuit breakers: `docs/architecture/circuit-breakers.md`
- Crate-level: `tasker-orchestration/AGENTS.md`, `tasker-worker/AGENTS.md`
