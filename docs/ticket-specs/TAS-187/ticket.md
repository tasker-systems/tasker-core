# Public API Visibility Audit

**Date**: 2026-02-02 (Updated from 2026-01-31 initial audit)

**Status**: Detailed Analysis Complete - Phased Implementation Plan Ready

**Scope**: All workspace crates in tasker-core (10 crates)

---

## Executive Summary

The tasker-core workspace has **~4,444 unrestricted `pub` items** across 10 crates, with only **31 `pub(crate)` items** total. This represents a near-total absence of visibility discipline. The vast majority of internal implementation details are publicly exposed, creating an oversized and unclear public API surface.

**Key finding**: An estimated **937-1,067 items (~21-24%) are candidates for restriction** to `pub(crate)` or private visibility. The biggest opportunities are in `tasker-worker` (~250-260 items), `tasker-shared` (~170-210 items), `tasker-orchestration` (~186 items), and `workers/rust` (~150 items).

**Changes since initial audit (2026-01-31)**:

- `tasker-cli` was extracted from `tasker-client` as a new binary crate (TAS-63). As a leaf binary with zero external consumers, all 31 of its pub items can become `pub(crate)`.
- TAS-63 refactoring added/modified 308 files, expanding test infrastructure and adding coverage tooling. The pub item counts have shifted accordingly.
- The workspace now has 10 crates (was 9).

---

## Workspace Dependency Graph

```
Layer 0 (Foundation):     tasker-pgmq  (no workspace deps)
                              |
Layer 1 (Core):          tasker-shared  (depends on: tasker-pgmq)
                          /    |     \        \
Layer 2 (APIs):   tasker-client  tasker-orchestration
                      |    \            |
Layer 3 (Binaries): tasker-cli  tasker-worker
                                 /    |    \      \
Layer 4 (Lang):           python  ruby  rust  typescript
```

### Reverse Dependency Map (who uses this crate)

| Crate | Used By | Consumer Count |
| -- | -- | -- |
| tasker-pgmq | tasker-shared, tasker-orchestration, tasker-worker (test) | 3 |
| tasker-shared | tasker-client, tasker-orchestration, tasker-worker, all 4 language workers, root tests | 9 |
| tasker-client | tasker-cli, tasker-worker, root E2E tests | 3 |
| tasker-orchestration | root tests only (2 files: actor_test_harness, lifecycle_test_manager) | 1 (tests only) |
| tasker-worker | workers/python, workers/ruby, workers/rust, workers/typescript | 4 |
| tasker-cli | **NONE** -- confirmed leaf binary crate | 0 |

---

## Current State: Pub Item Inventory

| Crate | pub struct | pub enum | pub fn | pub async fn | pub trait | pub const | pub type | pub mod | pub static | **pub(crate)** | **Total pub** |
| -- | -- | -- | -- | -- | -- | -- | -- | -- | -- | -- | -- |
| tasker-pgmq | 15 | 4 | 56 | 35 | 2 | 0 | 1 | 8 | 0 | **0** | **121** |
| tasker-client | 19 | 5 | 53 | 61 | 2 | 2 | 1 | 7 | 0 | **0** | **~150** |
| tasker-cli | 6 | 8 | 0 | 7 | 0 | 0 | 0 | 10 | 0 | **15** | **31** |
| tasker-shared | 423 | 85 | 1,122 | 447 | 19 | 106 | 45 | 183 | 93 | **2** | **2,523** |
| tasker-orchestration | 157 | 30 | 248 | 211 | 5 | 23 | 6 | 109 | 0 | **4** | **680** |
| tasker-worker | 126 | 19 | 313 | 121 | 15 | 1 | 4 | 61 | 0 | **10** | **599** |
| workers/rust | 93 | 1 | 57 | -- | 1 | 0 | 0 | 29 | 1 | **0** | **200** |
| workers/python | 1 | 1 | 35 | -- | 0 | 0 | 1 | 0 | 1 | **0** | **39** |
| workers/ruby | 4 | 0 | 43 | -- | 0 | 0 | 0 | 0 | 2 | **0** | **49** |
| workers/typescript | 12 | 1 | 38 | -- | 0 | 0 | 0 | 0 | 1 | **0** | **52** |
| **TOTAL** | **856** | **154** | **1,965** | **882** | **44** | **132** | **58** | **407** | **98** | **31** | **~4,444** |

**pub(crate) usage rate: 0.69%** (31 out of 4,475 total pub items)

---

## Crate-by-Crate Analysis

### 1. tasker-shared (2,523 pub items - 57% of workspace total)

**Role**: Core shared library used by every other crate. Highest-impact target.

#### lib.rs Re-exports (~50 items that MUST stay pub)

All 25 top-level modules are declared `pub mod` in lib.rs. The explicit `pub use` items from lib.rs represent the intended primary API surface:

```rust
pub use constants::{events as system_events, status_groups, system, ExecutionStatus, HealthStatus, ...};
pub use database::{AnalyticsMetrics, DependencyLevel, FunctionRegistry, SqlFunctionExecutor, ...};
pub use event_system::deployment::{DeploymentMode, ...};
pub use event_system::event_driven::{EventDrivenSystem, EventSystemFactory, ...};
pub use errors::{TaskerError, TaskerResult, OrchestrationError, ...};
pub use messaging::{PgmqClient, StepExecutionResult, StepMessage, ...};
pub use registry::TaskTemplateRegistry;
pub use system_context::SystemContext;
pub use types::base::{HandlerMetadata, StepEventPayload, ...};
```

#### Cross-Crate Import Analysis

**Most-imported modules** (by external crate count):

1. `errors` (TaskerError, TaskerResult) -- ALL 9 dependents
2. `system_context` (SystemContext) -- orchestration, worker, tests (30+ files each)
3. `models` -- ALL dependents (TaskRequest, Task, WorkflowStep most frequent)
4. `messaging` -- 6 dependents (StepExecutionResult most frequent)
5. `types` -- 6 dependents (TaskSequenceStep, API response types)
6. `config` -- orchestration, worker, cli
7. `events` -- 7 dependents (DomainEvent, EventPublisher)
8. `state_machine` -- orchestration, worker, client, tests
9. `metrics` -- orchestration, worker
10. `database` -- orchestration, worker, tests

**Modules with ZERO external imports** (strongest restriction candidates):

- `scopes/` -- no external imports found
- `utils/` -- no external imports found
- `validation/` -- no external imports found
- `constants/` internals -- only used via root re-exports
- `event_system/` internals -- only deployment types used externally

#### High-Impact Restriction Candidates

| Target | Pub Items | Restriction Strategy | Rationale |
| -- | -- | -- | -- |
| `config/tasker.rs` nested structs | ~50 | `pub(crate)` for deeply nested config types | Only top-level config structs accessed externally |
| `metrics/*.rs` (93 OnceLock statics) | ~93 | `pub(crate)` if accessor fns cover usage | Metric handles are implementation details |
| `state_machine/` guards, actions | ~25 | `pub(crate)` for internal SM types | Only 2 items already pub(crate) |
| `messaging/service/types.rs` | ~15 | `pub(crate)` for internal message types | Internal provider plumbing |
| `scopes/` | ~5 | `pub(crate)` for SQL fragment helpers | Zero external imports |
| `types/web.rs` | ~16 | Feature-gate behind `web-api` | Only used by web layer |
| `utils/`, `validation/` | ~10 | `pub(crate)` | Zero external imports |

**Estimated restrictable: ~170-210 items (7-8% of crate's pub items)**

Note: This crate's restriction potential is lower than the original estimate because most items are genuinely consumed across the workspace. The ~50 explicit lib.rs re-exports represent only 2% of the 2,523 pub items, but the module-path-accessible items are heavily used.

---

### 2. tasker-orchestration (680 pub items)

**Role**: Orchestration server. Consumed only by workspace root tests (2 files).

#### lib.rs Re-exports

```rust
pub use tasker_shared::{ConfigManager, ConfigResult, ConfigurationError, TaskerError, TaskerResult};
#[cfg(feature = "web-api")]
pub use web::{create_app, state::AppState};
```

**Top-level modules**: `actors`, `api_common`, `grpc` (gated), `health`, `orchestration`, `services` (gated), `web` (gated)

#### Cross-Crate Import Analysis

**Only 2 external files import from this crate** (both workspace root test helpers):

- `tests/common/actor_test_harness.rs`: `actors::ActorRegistry`
- `tests/common/lifecycle_test_manager.rs`: `orchestration::lifecycle::*`

This crate is almost entirely consumed through its binary (server) and web API, not as a library dependency.

#### Module-by-Module Breakdown

| Module | Pub Items | Restriction Candidate? |
| -- | -- | -- |
| `orchestration/lifecycle/` (all sub-modules) | ~158 | YES - internal decomposition |
| `web/` (handlers, middleware, extractors) | 84 | YES - only route setup needed pub |
| `services/` | 75 | PARTIAL - used by web + gRPC handlers |
| `health/` | 53 | PARTIAL - some types used by tests |
| `grpc/` | 40 | YES - internal gRPC plumbing |
| `actors/` | 31 | PARTIAL - ActorRegistry needed by tests |
| `orchestration/event_systems/` | 31 | YES - internal event coordination |
| `orchestration/channels/` | 24 | YES - internal channel plumbing |
| `orchestration/commands/` | 23 | YES - internal command handling |
| `api_common/` | 11 | YES - shared API internals |

#### High Priority Restriction: Lifecycle Internals (~89 items)

The `orchestration/lifecycle/` sub-modules contain decomposed internals that are fully internal:

- `result_processing/` (5 internal files: message_handler, metadata_processor, processing_context, state_transition_handler, task_coordinator) -- ~27 items
- `task_finalization/` (5 internal components) -- ~22 items
- `task_initialization/` (5 internal components) -- ~21 items
- `step_enqueuer_services/` (types, batch_processor, task_processor, state_handlers, summary) -- ~23 items

**Estimated restrictable: ~186 items (27% of crate's pub items)**

---

### 3. tasker-worker (599 pub items, 10 existing pub(crate))

**Role**: Worker framework. Used by all 4 language workers.

#### lib.rs Re-exports (MUST stay pub)

```rust
pub use batch_processing::BatchAggregationScenario;
pub use bootstrap::{WorkerBootstrap, WorkerBootstrapConfig, WorkerSystemHandle, WorkerSystemStatus};
pub use error::{Result, WorkerError};
pub use health::WorkerHealthStatus;
pub use worker::{WorkerCore, WorkerCoreStatus};
pub use handler_capabilities::{APICapable, BatchableCapable, DecisionCapable, ...};
```

#### Cross-Crate Import Analysis (what language workers actually use)

**All 4 workers**: `WorkerBootstrap`, `WorkerSystemHandle`, `WorkerSystemStatus`
**FFI workers (Python, Ruby, TS)**: `FfiDispatchChannel`, `FfiDispatchMetrics`, `FfiStepEvent`
**Rust worker**: `StepHandler`, `StepHandlerRegistry`, `HandlerDispatchService`, `EventRouter`, `DomainEventCallback`, `BatchWorkerContext`, capability traits
**Ruby only**: `web::WorkerWebState`

#### Existing pub(crate) Pattern (10 items - good precedent)

```
pub(crate) struct OrchestrationResultSender
pub(crate) struct EventDrivenMessageProcessor
pub(crate) struct WorkerFallbackPoller
pub(crate) struct WorkerQueueListener
pub(crate) struct StepClaim
pub(crate) struct RequestIdMiddleware
+ 4 pub(crate) channel inner fields
```

#### Modules with ZERO External Imports

| Module | Pub Items | Notes |
| -- | -- | -- |
| `worker::actors/` | ~46 | Actor structs, messages, traits - all internal plumbing |
| `worker::services/` (except checkpoint) | ~33 | Internal service decomposition |
| `worker::channels/` | ~24 | Internal channel wrappers |
| `worker::worker_queues/` | ~24 | Internal queue management |
| `worker::event_subscriber/` | ~16 | Internal event subscription |
| `worker::task_template_manager/` | ~19 | Internal template management |
| `worker::command_processor/` | ~5 | Internal command processing |
| `worker::hydration/` | ~7 | Internal message hydration |
| `worker::event_driven_processor/` | ~7 | Internal message processor |
| `web/handlers/`, `web/middleware/` | ~19 | Route handlers, middleware |
| `worker::domain_event_commands/` | ~2 | Internal commands |

**Estimated restrictable: ~250-260 items (40-43% of crate's pub items)**

---

### 4. tasker-pgmq (121 pub items)

**Role**: Foundation PGMQ wrapper. Used by tasker-shared, tasker-orchestration, tasker-worker.

#### External Usage (only ~12 items imported externally)

- `PgmqClient` -- used by 3 crates
- `PgmqNotifyConfig` -- used by tasker-shared (3 files)
- `PgmqNotifyEvent`, `PgmqNotifyListener`, `PgmqEventHandler` -- used by tasker-shared
- `PgmqNotifyError`, `MessagingError` -- used by tasker-shared (From impls)
- `MessageReadyEvent` -- used by tasker-shared (From impl)
- `types::QueueMetrics` -- used by tasker-shared (field type)
- `error::Result` -- used by tasker-shared

#### Modules with ZERO External Imports

| Module | Items | Action |
| -- | -- | -- |
| `channel_metrics` | 17 | Entire module to `pub(crate)` |
| `emitter` (DbEmitter, NoopEmitter, PgmqNotifyEmitter) | 8 | `pub(crate)` -- marked "legacy" |
| `events::QueueCreatedEvent`, `BatchReadyEvent` | 6 | `pub(crate)` |
| `client::PgmqNotifyClient`, `PgmqNotifyClientFactory` | ~5 | `pub(crate)` |
| `ClientStatus` | 1 | `pub(crate)` |

**Estimated restrictable: ~40-50 items (35-40%)**

---

### 5. tasker-client (~150 pub items)

**Role**: Client library for external API interaction. Used by tasker-cli, tasker-worker, root tests.

#### lib.rs Re-exports (well-structured, ~20 items)

All core types re-exported: `OrchestrationApiClient`, `OrchestrationApiConfig`, `WorkerApiClient`, `WorkerApiConfig`, `ClientConfig`, `Transport`, `ClientError`, `ClientResult`, and gRPC variants.

#### Restriction Candidates

| Target | Items | Rationale |
| -- | -- | -- |
| `grpc_clients::conversions` (34 pub fns) | 34 | Module already private; functions only used within grpc_clients |
| `ProfileConfigFile`, `ProfileConfig`, `ProfileEndpointConfig` | 3 | Internal deserialization structs, not re-exported |
| `ClientConfig` default URL constants | 2 | Internal defaults |
| Auth config helper methods | ~6 | Internal builder pattern methods |

**Estimated restrictable: ~45-55 items (30-35%)**

---

### 6. tasker-cli (31 pub items - NEW crate)

**Role**: CLI binary. Extracted from tasker-client in TAS-63. **LEAF crate with zero external consumers.**

As a binary-only crate with no lib.rs, every `pub` item can safely become `pub(crate)`. Already has 15 `pub(crate)` items.

| Category | Items | Action |
| -- | -- | -- |
| Clap CLI struct + command enums | 9 | `pub(crate)` |
| Command handler async fns | 7 | `pub(crate)` |
| Doc template structs | 5 | `pub(crate)` |
| Module declarations | 10 | `pub(crate) mod` |

**Estimated restrictable: 31 items (100%)**

---

### 7. workers/rust (200 pub items)

**Role**: Rust worker demonstration/implementation. Leaf crate (no dependents except integration tests).

#### Key Finding: 79 step handler structs are all pub

Nearly all step handler implementations (`EcommerceHandler`, `DataPipelineHandler`, etc.) are pub despite being registered into the registry at construction time and only accessed by name at runtime. Only `PaymentEventPublisher` and `ProcessPaymentHandler` are used by the integration test.

| Category | Restrictable Items | Notes |
| -- | -- | -- |
| Step handler structs | ~77 | 79 total minus 2 used by integration test |
| Handler sub-modules | ~22 | `pub(crate) mod` instead of `pub mod` |
| Data structs (DTOs) | ~7 | CartItem, SalesRecord, etc. |
| Event subscribers | ~5 | Internal to crate |
| Bootstrap helpers | ~2 | Internal config variants |
| Metrics/logging internals | ~17 | Internal subscriber methods |
| GLOBAL_EVENT_SYSTEM static | 1 | Internal only |

**Estimated restrictable: ~150 items (75%)**

---

### 8. FFI Workers (python: 39, ruby: 49, typescript: 52)

**Role**: FFI boundary crates. Items with framework attributes (#[pyfunction], magnus::function!, #[no_mangle]) MUST stay pub.

**Key observation**: All three FFI crates already use **private modules** (`mod` not `pub mod`) in their lib.rs, so their `pub` items are effectively crate-scoped. Marking internal helpers as `pub(crate)` would clarify intent.

| Crate | Must stay pub (FFI) | Could restrict | Reduction |
| -- | -- | -- | -- |
| workers/python | ~24 (#[pyfunction]) | ~16 (bridge, errors, conversions) | 41% |
| workers/ruby | ~33 (magnus functions) | ~19 (bridge, event handler, conversions) | 39% |
| workers/typescript | ~22 (#[no_mangle]) | ~30 (bridge internals, DTOs, conversions) | 58% |

**Combined estimated restrictable: ~65 items**

---

## Implementation Strategy

### Phase 1: Zero-Risk Binary Crates (Start Here)

**Target**: Leaf binary crates with zero external consumers.

1. **tasker-cli**: Convert all 31 `pub` items to `pub(crate)` or private
   - 9 Clap CLI structs/enums
   - 7 command handler functions
   - 5 doc template structs
   - 10 module declarations to `pub(crate) mod`
   - **Estimated: 31 items, ZERO break risk**

2. **workers/rust**: Restrict step handler implementations
   - 77 step handler structs to `pub(crate)` (keep PaymentEventPublisher, ProcessPaymentHandler pub for integration test)
   - 22 handler sub-modules to `pub(crate) mod`
   - Internal subscriber/bootstrap/metrics types
   - **Estimated: ~150 items, very low risk (only integration test to verify)**

**Phase 1 Total: ~181 items**

### Phase 2: Internal Plumbing in Server Crates

**Target**: Internal implementation modules with zero external imports.

1. **tasker-orchestration lifecycle internals** (~89 items)
   - `lifecycle/result_processing/` internal components (5 files)
   - `lifecycle/task_finalization/` internal components (5 files)
   - `lifecycle/task_initialization/` internal components (5 files)
   - `lifecycle/step_enqueuer_services/` internal components
   - Convert `pub mod` to `pub(crate) mod` for these sub-modules

2. **tasker-orchestration web/gRPC internals** (~68 items)
   - Web handler functions, middleware, extractors
   - gRPC service implementations, interceptors, conversions

3. **tasker-worker internal modules** (~200 items)
   - `worker::actors/` (46 items) -- actor structs, messages, traits
   - `worker::services/` except CheckpointService (33 items)
   - `worker::channels/` (24 items)
   - `worker::worker_queues/` (24 items)
   - `worker::event_subscriber/` (16 items)
   - `worker::hydration/`, `command_processor/`, `event_driven_processor/` (~19 items)
   - `web/handlers/`, `web/middleware/` (~19 items)

**Phase 2 Total: ~357 items**

### Phase 3: Foundation Crate Internals

**Target**: Internal modules in tasker-pgmq and tasker-client.

1. **tasker-pgmq** (~40-50 items)
   - `channel_metrics` module (17 items) -- zero external imports
   - `emitter` module (8 items) -- marked "legacy", zero external imports
   - Unused event types, client variants, error constructors

2. **tasker-client** (~45-55 items)
   - `grpc_clients::conversions` (34 pub fns) -- already in private module
   - Profile config internal structs (3)
   - Auth config helper methods (~6)
   - Default URL constants (2)

**Phase 3 Total: ~85-105 items**

### Phase 4: tasker-shared Core Internals

**Target**: The largest crate's internal implementation details.

1. **Metrics statics** (~93 OnceLock items)
   - Verify accessor functions exist for all metrics
   - If so, restrict raw statics to `pub(crate)`

2. **Config nested structs** (~50 items in `config/tasker.rs`)
   - Restrict deeply nested deserialization structs not re-exported from config/mod.rs

3. **State machine internals** (~25 items)
   - Extend existing `pub(crate)` pattern (2 items) to guards, actions

4. **Zero-import modules** (~15 items)
   - `scopes/` helper functions
   - `utils/` helpers
   - `validation/` helpers

5. **Feature-gating** (~16 items)
   - Move `types/web.rs` types behind `web-api` feature gate

**Phase 4 Total: ~170-210 items**

### Phase 5: FFI Worker Cleanup

**Target**: Clarify intent in FFI boundary crates.

1. **workers/python**: Restrict 16 internal items (bridge, errors, conversions)
2. **workers/ruby**: Restrict 19 internal items (bridge handle, event handler, conversions)
3. **workers/typescript**: Restrict 30 internal items (bridge, DTOs, conversions)

Note: These crates already use private modules, so this is primarily intent clarification.

**Phase 5 Total: ~65 items**

### Phase 6: Module-Level Visibility

**Target**: Change `pub mod` to `pub(crate) mod` where entire modules are internal.

Priority modules for `pub(crate) mod`:

- `tasker-orchestration::orchestration::hydration`
- `tasker-orchestration::orchestration::lifecycle::result_processing`
- `tasker-orchestration::orchestration::lifecycle::task_finalization`
- `tasker-orchestration::orchestration::lifecycle::task_initialization`
- `tasker-orchestration::api_common`
- `tasker-worker::worker::actors`
- `tasker-worker::worker::services` (except checkpoint re-export)
- `tasker-worker::worker::worker_queues`
- `tasker-worker::worker::hydration`

Making a module `pub(crate)` automatically restricts all its contents from external access, even if individual items remain `pub` within the module. This is the most impactful single change pattern.

---

## Verification Approach

For each phase of changes:

1. **Compile check**: `cargo check --all-features` (catches any broken imports)
2. **Test suite**: `cargo test --all-features` (catches runtime issues)
3. **Rustdoc generation**: `cargo doc --all-features --no-deps` (verify public API documentation is clean)
4. **Cross-crate validation**: Search for `use <crate>::<restricted_item>` patterns to confirm no external usage

The compiler will catch most issues immediately since restricting visibility from `pub` to `pub(crate)` will produce compilation errors if any external crate references the item.

---

## Summary of Estimated Impact

| Phase | Crates Affected | Items Restricted | Risk Level |
| -- | -- | -- | -- |
| Phase 1 | cli, workers/rust | ~181 | Zero / Very Low |
| Phase 2 | orchestration, worker | ~357 | Low |
| Phase 3 | pgmq, client | ~85-105 | Low-Medium |
| Phase 4 | shared (internal modules) | ~170-210 | Medium |
| Phase 5 | FFI workers (python, ruby, ts) | ~65 | Very Low |
| Phase 6 | Module-level changes | cascading | Medium |
| **Total** | **All 10 crates** | **~858-918** | |

This would reduce the unrestricted public API surface from **~4,444 to ~3,526-3,586 items** (~20-21% reduction), with many of the remaining items being intentional public API.

---

## Existing pub(crate) Patterns (Reference)

These 31 existing `pub(crate)` items establish the project's pattern:

### tasker-orchestration (4 items)

- `pub(crate) struct PgmqMessageResolver`
- `pub(crate) enum CommandOutcome`
- `pub(crate) mod command_outcome`, `pub(crate) mod pgmq_message_resolver`

### tasker-worker (10 items)

- `pub(crate) struct OrchestrationResultSender`
- `pub(crate) struct EventDrivenMessageProcessor`
- `pub(crate) struct WorkerFallbackPoller`
- `pub(crate) struct WorkerQueueListener`
- `pub(crate) struct StepClaim`
- `pub(crate) struct RequestIdMiddleware`
- 4x `pub(crate)` channel inner fields (mpsc::Sender/Receiver)

### tasker-shared (2 items)

- `pub(crate) struct PublishTransitionEventAction`
- `pub(crate) struct TriggerStepDiscoveryAction`

### tasker-cli (15 items)

- 9x `pub(crate)` functions in `commands/docs.rs`
- 6x feature-gated `pub(crate)` items

## Metadata

- URL: [https://linear.app/tasker-systems/issue/TAS-187/public-api-visibility-audit](https://linear.app/tasker-systems/issue/TAS-187/public-api-visibility-audit)
- Identifier: TAS-187
- Status: In Progress
- Priority: Low
- Assignee: Pete Taylor
- Labels: Chore
- Project: [Tasker Core Rust](https://linear.app/tasker-systems/project/tasker-core-rust-9b5a1c23b7b1). Alpha version of the Tasker Core in Rust
- Created: 2026-01-31T02:12:16.327Z
- Updated: 2026-02-02T15:22:05.610Z
