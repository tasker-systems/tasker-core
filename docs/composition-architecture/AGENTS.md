# Composition Architecture — Agent Guide

This document orients you to the composition architecture work in tasker-core. Read this before starting any session involving grammar capabilities, resource operations, runtime adapters, or composition worker infrastructure.

---

## What Is the Composition Architecture?

Tasker's **composition architecture** enables declarative, grammar-defined workflows that execute structured operations against external resources (databases, APIs, message buses) without custom handler code. It spans three crates with distinct responsibilities:

| Crate | Role | Depends On |
|-------|------|------------|
| **tasker-secure** | Resource identity, credentials, connection lifecycle. `ResourceHandle` trait, `ResourceRegistry`, `PostgresHandle`, `HttpHandle`, `SecretsProvider`. | — |
| **tasker-grammar** | Operation contracts, capability executors, composition engine. Defines `PersistableResource`, `AcquirableResource`, `EmittableResource` traits and the `OperationProvider` interface. All tests use `InMemoryOperations` — zero I/O. | tasker-secure (lightweight: `ResourceType`, `ResourceHandle` types only) |
| **tasker-runtime** | Adapters bridging grammar operations to secure handles. `ResourcePoolManager` for dynamic pool lifecycle. `RuntimeOperationProvider` wiring pool manager + adapters into the `OperationProvider` interface. `CompositionExecutionContext` for enriched executor context. | tasker-secure, tasker-grammar |

Two additional crates appear later:
- **tasker-rs** — The composition worker binary. Composes tasker-worker + tasker-runtime. Phase 4 work.
- **tasker-worker** — Existing step lifecycle crate. Receives a StepContext rename (TaskSequenceStep → StepContext) but otherwise unchanged by this architecture.

### The Core Insight

Grammar capability executors (persist, acquire, emit) own the **full action pipeline**: config parse → jaq expression eval → operation call → validate_success → result_shape. The actual I/O is a single call through an abstract trait (`PersistableResource`, etc.). In tests, that trait is backed by `InMemoryOperations`. In production, it's backed by an adapter wrapping a secure handle. The executor doesn't know the difference.

---

## Navigating the Documents

### Start Here

| Document | When to Read | What It Covers |
|----------|-------------|----------------|
| **[roadmap.md](roadmap.md)** | Every session | Phases, lanes, dependencies, what's done, what's next, ticket mapping |
| **This file** | Every session | Orientation, vocabulary, how to find things |

### Go Deeper When Needed

| Document | Read When | What It Covers |
|----------|-----------|----------------|
| [operation-shape-constraints.md](operation-shape-constraints.md) | Working on persist/acquire executors or adapters | What persist and acquire do and don't do. Four persist modes, declarative filter operators, nested relationship rules, HTTP equivalents |
| [dependency-diagrams.md](dependency-diagrams.md) | Planning work within a specific lane | Task-level dependency graphs for every lane in every phase |
| [resource-handle-traits-and-seams.md](../research/resource-handle-traits-and-seams.md) | Working on adapters, pool manager, or crate topology | Full architectural design: operation traits, adapter pattern, OperationProvider, ResourcePoolManager, worker segmentation, StepContext/CompositionExecutionContext split |

### Design Rationale (read for context, not instructions)

| Document | Content |
|----------|---------|
| [actions-traits-and-capabilities.md](../action-grammar/actions-traits-and-capabilities.md) | Original capability model and (action, resource, context) triple |
| [transform-revised-grammar.md](../action-grammar/transform-revised-grammar.md) | jaq-core integration, 6-capability consolidation |
| [grammar-trait-boundary.md](../action-grammar/grammar-trait-boundary.md) | Grammar trait system design |
| [composition-validation.md](../action-grammar/composition-validation.md) | JSON Schema contract chaining |
| [checkpoint-generalization.md](../action-grammar/checkpoint-generalization.md) | Checkpoint model for composition state |
| [virtual-handler-dispatch.md](../action-grammar/virtual-handler-dispatch.md) | Composition queues and worker segmentation |
| [security-and-secrets/](../research/security-and-secrets/) | SecretsProvider, ResourceRegistry, encryption, classification design |

### Historical (superseded)

| Document | Status |
|----------|--------|
| [implementation-phases.md](../action-grammar/implementation-phases.md) | Superseded by roadmap.md. Phase structure and ticket mapping no longer accurate. |

---

## Key Vocabulary

| Term | Meaning |
|------|---------|
| **Capability executor** | A component that executes one grammar capability (transform, validate, assert, persist, acquire, emit). Owns the full pipeline from config parse through result shaping. Lives in tasker-grammar. |
| **Operation trait** | `PersistableResource`, `AcquirableResource`, `EmittableResource` — the grammar's interface for resource I/O. Defined in tasker-grammar, implemented by adapters in tasker-runtime and by InMemoryOperations in tests. |
| **OperationProvider** | The seam between grammar and runtime. Executors call `context.operations.get_persistable("orders-db")` and get back `Arc<dyn PersistableResource>` without knowing whether it's in-memory or a live adapter. |
| **Adapter** | A type in tasker-runtime that wraps a tasker-secure handle and implements an operation trait. Example: `PostgresPersistAdapter` wraps `PostgresHandle` and implements `PersistableResource` by generating SQL. |
| **AdapterRegistry** | Maps `(ResourceType, operation)` to adapter factories. `standard()` registers all built-in adapters. |
| **ResourcePoolManager** | Wraps `ResourceRegistry` with dynamic lifecycle: lazy initialization, eviction, backpressure, connection budgets. |
| **RuntimeOperationProvider** | Implements `OperationProvider` by bridging ResourcePoolManager + AdapterRegistry. The production implementation of the seam. |
| **InMemoryOperations** | Grammar-level test double implementing all operation traits with fixture data (acquire) and capture lists (persist, emit). Lives in tasker-grammar. Distinct from InMemoryResourceHandle in tasker-secure (which is a handle-level test double). |
| **CompositionExecutionContext** | Enriched context for grammar executors. Contains StepContext + OperationProvider + checkpoint + classifier + composition envelope. Lives in tasker-runtime. Never crosses FFI. |
| **StepContext** | The DTO for domain handlers. Renamed from TaskSequenceStep. Crosses FFI. No resource handles, no grammar knowledge. |
| **Composition envelope** | The data context available to jaq expressions: `.context` (task input), `.deps` (dependency results), `.prev` (previous step output), `.step` (step config). |
| **GrammarActionResolver** | In tasker-rs. Resolves `grammar:*` callables into handlers that wrap the CompositionExecutor. Registered in ResolverChain at priority 15. |

---

## Figuring Out What's Done and What's Next

### Quick Status Check

```bash
# What's on this branch vs main?
git log main..HEAD --oneline

# What tests exist for composition work?
cargo test --package tasker-grammar --lib -- --list 2>/dev/null | head -30
cargo test --package tasker-secure --lib -- --list 2>/dev/null | head -30

# Does tasker-runtime exist yet?
ls crates/tasker-runtime/Cargo.toml 2>/dev/null && echo "exists" || echo "not yet scaffolded"

# Does tasker-rs exist yet?
ls crates/tasker-rs/Cargo.toml 2>/dev/null && echo "exists" || echo "not yet scaffolded"
```

### Check Linear for Current State

Use the Linear MCP tools to get the current ticket landscape:

```
list_issues(project: "Tasker Action Grammar", state: "In Progress")   # Active work
list_issues(project: "Tasker Action Grammar", state: "Backlog")       # What's available
list_milestones(project: "Tasker Action Grammar")                     # Phase progress
```

### What's Ready to Start?

Consult the [roadmap "What Can Start Now" section](roadmap.md#what-can-start-now). As a general rule:

- **Lane 1E** (TAS-335/336) — in progress via web agent
- **Lanes 2A, 2B, 2C** — ready for implementation. `tasker-runtime` scaffolded (TAS-373 ✅, PR #302). Type stubs and trait signatures in place — implement real behavior.
- **Lane 3C** (validation tooling) is now unblocked by 1D completion
- **Lane 3A** (TAS-370: StepContext rename) has zero dependencies
- **Lane 3D** (TAS-369: ConfigString, TAS-359/360: S3/S4) is fully independent

**Phase 1 core complete.** Phase 2 crate scaffolded. All lanes 2A-2D have type/trait stubs ready for implementation.

### Reading the Dependency Graph

The [roadmap eagle-eye diagram](roadmap.md#eagle-eye-view) shows all cross-phase dependencies. Key rules:

1. A lane is **ready** when all its incoming arrows point to completed work
2. Within a phase, lanes can proceed in parallel unless there's an arrow between them
3. Phase convergence points (1D, 2D, 3B, 4C) need all their inputs before they can complete

---

## Working on a Specific Ticket

### Before Starting

1. Read this file (you're doing that now)
2. Check the [roadmap](roadmap.md) for the ticket's lane, dependencies, and phase context
3. Read the relevant design doc (linked from the roadmap's lane description)
4. Check if the ticket has blocking dependencies in Linear

### Crate-Specific Guidance

**Working in tasker-grammar (Phase 1):**
- All tests use `InMemoryOperations` — zero I/O, zero infrastructure
- Expression evaluation uses jaq-core on `serde_json::Value`
- Run tests: `cargo test --package tasker-grammar --all-features`
- The grammar crate has NO dependency on tasker-worker, tasker-runtime, or any I/O crate

**Working in tasker-secure (already complete for Milestone 1):**
- Feature-gated modules: `postgres` (sqlx), `http` (reqwest), `sops` (rops), `test-utils`
- Run tests: `cargo test --package tasker-secure --all-features`
- 90+ tests covering secrets, resource types, handles, and registry

**Working in tasker-runtime (Phase 2+):**
- Depends on both tasker-secure and tasker-grammar
- Feature-gated like tasker-secure: `postgres`, `http`
- Adapter tests may need test infrastructure (mock HTTP server, test database)
- ResourcePoolManager tests can use InMemoryResourceHandle from tasker-secure's test-utils

**Working in tasker-worker (Lane 3A only):**
- The StepContext rename is a refactoring task touching many files
- Verify FFI alignment: tasker-py, tasker-rb, tasker-ts already use `StepContext`
- Run full test suite after rename: `cargo make test` (requires services)

**Working in tasker-rs (Phase 4):**
- Binary crate, not a library
- Composes tasker-worker + tasker-runtime
- Needs full service stack for E2E tests

### Commit Conventions

Follow the existing project conventions (see root CLAUDE.md):
- `feat(TAS-NNN):` for new functionality
- `test(TAS-NNN):` for test-only changes
- `refactor(TAS-NNN):` for structural changes
- `chore(TAS-NNN):` for cleanup and maintenance

---

## Design Constraints to Remember

These constraints are documented in detail in [operation-shape-constraints.md](operation-shape-constraints.md) but are important enough to highlight:

1. **persist operations are shape-constrained.** Four modes (insert/update/upsert/delete), identity declarations (PK/FK), one level of nested relationships. No raw SQL, no arbitrary WHERE clauses, no DDL.

2. **acquire operations are declarative.** Column/table selection with a fixed filter operator set. Joins via declared PK/FK relationships. No subqueries, no aggregation in the query, no CTEs. Complex analytical work happens in `transform` (jaq-core).

3. **The grammar makes predictable-shape things declarative.** If an operation can't be expressed as a structured declaration with predictable shape, it belongs in a domain handler.

4. **Each adapter knows exactly two things:** the grammar's operation contract and the specific handle's I/O protocol. Nothing about jaq expressions, composition context, checkpointing, or capability config.

5. **InMemoryOperations (grammar test double) is NOT InMemoryResourceHandle (secure test double).** They test at different abstraction levels. Grammar tests use InMemoryOperations. Runtime adapter tests use InMemoryResourceHandle.

---

## Linear Project Structure

All composition architecture tickets live in two Linear projects:

| Project | Scope | Milestones |
|---------|-------|------------|
| **Tasker Action Grammar** | Grammar capabilities, operation traits, runtime infrastructure, worker dispatch | Phase 1 (Grammar Foundations), Phase 2 (Runtime Infrastructure), Phase 3 (Integration & Tooling), Phase 4 (Worker Dispatch & Queues) |
| **Tasker Secure Foundations** | Credential resolution, resource handles, encryption, classification | Milestone 1 (S1+S2, complete), Milestone 1.5 (Integration), Milestone 2 (S3+S4) |

The tasker-runtime tickets live in Tasker Action Grammar because the runtime crate exists to serve grammar compositions, even though it wraps tasker-secure handles.


<claude-mem-context>
# Recent Activity

<!-- This section is auto-generated by claude-mem. Edit content outside the tags. -->

*No recent activity*
</claude-mem-context>