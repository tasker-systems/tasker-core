# Coverage Analysis: tasker-orchestration

**Current Coverage**: 48.34% line (20,048 / 41,474), 43.77% function (2,524 / 5,766) — measured Jan 31, 2026 — 1,044 tests
**Baseline Coverage**: 31.60% line (11,790 / 37,306), 28.57% function (1,509 / 5,282)
**Target**: 55%
**Gap**: 6.66 percentage points remaining (was 23.4 pp at baseline)
**Progress**: +16.74 pp line coverage, +619 tests added (183 unit + 41 integration + 35 refactoring-phase + 71 quick-win/pipeline + 38 staleness/backoff + 22 DB integration + 128 additional unit/DB tests + 28 event system tests + 28 viable-step/decision-point/batch-processing DB tests + 45 gRPC layer unit tests)

---

## Summary

The `tasker-orchestration` crate is the core orchestration engine containing actors, lifecycle management, result processing, web handlers, and gRPC services. At 31.6% line coverage it is substantially below the 55% target. The largest gaps are concentrated in four areas: (1) the orchestration core infrastructure (event systems, bootstrap, state management) at 0-10% coverage comprising 2,261 uncovered lines, (2) the result processing pipeline at 8-21% coverage with 589 uncovered lines, (3) the entire gRPC service layer at 0% coverage with 741 uncovered lines, and (4) the actor system including the critical command processor at 18-51% coverage with 556 uncovered lines. Closing these gaps through targeted integration and unit tests would add approximately 18.5 percentage points of coverage, bringing the crate to roughly 50%, with the remaining gap addressable through medium-priority modules.

## Uncovered Files (0% Coverage)

These 23 files have zero test coverage. Together they represent 1,771 coverable lines.

| File | Lines (coverable) | Description |
|------|-------------------|-------------|
| ~~`orchestration/event_systems/orchestration_event_system.rs`~~ | ~~370~~ | **Now ~47%** — 13 unit + 15 integration tests added. Moved to Lowest Coverage section. |
| ~~`orchestration/error_handling_service.rs`~~ | ~~217~~ | **Now 41.97%** — Moved to Lowest Coverage section. 9 pure unit tests added for action/result types. |
| ~~`orchestration/orchestration_queues/fallback_poller.rs`~~ | ~~208~~ | **Now 32.3%** — 7 pure unit tests added for config/stats types. Moved to Lowest Coverage section. |
| ~~`grpc/services/dlq.rs`~~ | ~~381~~ | **Now 83.73%** — 26 unit tests for conversion helpers. Moved to above-target. |
| `grpc/server.rs` | 121 | gRPC server bootstrap: service registration, reflection, health checking, lifecycle management. |
| ~~`grpc/services/analytics.rs`~~ | ~~194~~ | **Now 75.77%** — 11 unit tests for error mapping + system health conversion. Above target. |
| ~~`grpc/conversions.rs`~~ | ~~271~~ | **Now 100%** — 28 proto-to-domain conversion tests. |
| `grpc/services/tasks.rs` | 82 | gRPC task service: CRUD operations, task creation from requests, streaming. Requires service infrastructure. |
| ~~`grpc/interceptors/auth.rs`~~ | ~~91~~ | **Now 54.95%** — 6 unit tests for disabled auth path, clone, debug. Moved to Lowest Coverage. |
| ~~`orchestration/errors.rs`~~ | ~~167~~ | **Now 94.01%** — 10 From trait conversion tests. Above target. |
| `web/extractors.rs` | 47 | Custom Axum extractors: database connection pool selection, worker claims, request ID. |
| ~~`grpc/services/templates.rs`~~ | ~~72~~ | **Now 44.44%** — 4 unit tests for error mapping. Moved to Lowest Coverage. |
| `grpc/services/config.rs` | 39 | gRPC config service: runtime configuration endpoints. Requires orchestration context. |
| `grpc/services/steps.rs` | 33 | gRPC step service: step query and management endpoints. Requires service infrastructure. |
| ~~`web/middleware/operational_state.rs`~~ | ~~64~~ | **Now 60.94%** — Above target. |
| `web/middleware/mod.rs` | 22 | Middleware module re-exports and composition. |
| `orchestration/orchestration_queues/events.rs` | 14 | Queue event types for orchestration queue notifications. |
| `grpc/state.rs` | 12 | gRPC shared state container holding service references. |
| `grpc/services/health.rs` | 11 | gRPC health check service (liveness/readiness). Requires service infrastructure. |
| `bin/server.rs` | 58 | Server binary entry point: CLI args, config loading, server startup. |
| `bin/generate_openapi.rs` | 6 | OpenAPI spec generation binary. |
| `orchestration/lifecycle/batch_processing/mod.rs` | 3 | Module re-export for batch processing. |
| `orchestration/lifecycle/decision_point/mod.rs` | 3 | Module re-export for decision point. |

## Lowest Coverage Files (Above 0% but Below 55%)

| File | Lines (coverable) | Covered | Coverage % | Description |
|------|-------------------|---------|-----------|-------------|
| ~~`orchestration/hydration/step_result_hydrator.rs`~~ | ~~230~~ | ~~186~~ | **80.87%** | 10 DB integration tests added. Above target. |
| ~~`orchestration/viable_step_discovery.rs`~~ | ~~602~~ | ~~485~~ | **80.56%** | DB integration tests added. Well above target. |
| ~~`orchestration/lifecycle/result_processing/message_handler.rs`~~ | ~~314~~ | ~~186~~ | **59.24%** | Routes step result messages to appropriate handlers. Above target. |
| ~~`orchestration/lifecycle/batch_processing/service.rs`~~ | ~~319~~ | ~~254~~ | **79.62%** | DB integration tests added. Well above target. |
| `orchestration/bootstrap.rs` | 444 | 43 | 9.68% | Unified orchestration system bootstrap across all deployment modes with lifecycle management. |
| ~~`orchestration/hydration/finalization_hydrator.rs`~~ | ~~198~~ | ~~189~~ | **95.45%** | Inline tests now measured. Well above target. |
| `orchestration/lifecycle/result_processing/metadata_processor.rs` | 80 | 65 | **81.11%** | 6 pipeline integration tests added. Now above target. |
| ~~`orchestration/hydration/task_request_hydrator.rs`~~ | ~~186~~ | ~~177~~ | **95.16%** | Inline tests now measured. Well above target. |
| `orchestration/lifecycle/result_processing/state_transition_handler.rs` | 162 | 91 | **56.17%** | 10 pipeline integration tests added. Above target. |
| ~~`orchestration/lifecycle/decision_point/service.rs`~~ | ~~314~~ | ~~247~~ | **78.66%** | DB integration tests added. Well above target. |
| ~~`orchestration/state_manager.rs`~~ | ~~442~~ | ~~77~~ | **Deleted** | Removed — 11/14 methods were dead code. 3 used methods inlined into callers. |
| `actors/command_processor_actor.rs` | ~200 | — | — | Reduced from 574 coverable lines. Business logic extracted to `commands/service.rs`. |
| ~~`orchestration/staleness_detector.rs`~~ | ~~500~~ | ~~322~~ | **64.40%** | 20 pure unit tests added. Above target. |
| `orchestration/lifecycle/result_processing/task_coordinator.rs` | 108 | 81 | **75.21%** | 4 pipeline integration tests added. Above target. |
| `orchestration/core.rs` | 278 | 120 | **43.2%** | 11 tests added for OrchestrationCoreStatus. Command-pattern bootstrap, channel setup, health monitoring. |
| ~~`orchestration/lifecycle/task_finalization/state_handlers.rs`~~ | ~~270~~ | ~~225~~ | **83.33%** | 4 DB integration tests added. Above target. |
| ~~`health/db_status.rs`~~ | ~~126~~ | ~~99~~ | **78.6%** | 8 tests added including real DB evaluation. Above target. |
| `orchestration/event_systems/task_readiness_event_system.rs` | 86 | 41 | **47.7%** | 7 DB tests added. Task readiness event system. |
| ~~`orchestration/event_systems/orchestration_event_system.rs`~~ | ~~461~~ | ~~296~~ | **64.21%** | 13 unit + 15 integration tests added. Above target. |
| ~~`orchestration/lifecycle/step_result_processor.rs`~~ | ~~143~~ | ~~96~~ | **67.13%** | Above target. |
| `web/state.rs` | 145 | 49 | 33.79% | Web application state: database pools, services, circuit breaker references. |
| ~~`orchestration/backoff_calculator.rs`~~ | ~~535~~ | ~~499~~ | **93.27%** | 18 new + 7 converted tests. Well above target. |
| `orchestration/orchestration_queues/listener.rs` | 404 | 186 | **46.0%** | 7 tests added for config/stats. PGMQ queue listener: message dispatch, statistics tracking. |
| `orchestration/event_systems/unified_event_coordinator.rs` | 395 | 208 | **52.7%** | 9 tests added for config and health reports. Unified coordinator. |
| ~~`orchestration/lifecycle/task_request_processor.rs`~~ | ~~119~~ | ~~71~~ | **59.6%** | 8 tests added. Above target. |
| ~~`orchestration/lifecycle/task_finalization/event_publisher.rs`~~ | ~~143~~ | ~~116~~ | **81.12%** | Above target. |
| ~~`orchestration/lifecycle/step_enqueuer_services/state_handlers.rs`~~ | ~~164~~ | ~~104~~ | **63.2%** | 5 tests added. Above target. |
| ~~`orchestration/task_readiness/fallback_poller.rs`~~ | ~~145~~ | ~~86~~ | **59.3%** | 5 DB tests added. Above target. |
| ~~`orchestration/lifecycle/task_finalization/completion_handler.rs`~~ | ~~405~~ | ~~341~~ | **84.20%** | 8 DB integration tests added. Above target. |
| ~~`orchestration/event_systems/orchestration_statistics.rs`~~ | ~~172~~ | ~~0~~ | **81.37%** | 12 pure unit tests added. Now above target. |
| ~~`orchestration/error_classifier.rs`~~ | ~~804~~ | ~~727~~ | **90.42%** | 25 pure unit tests added. Well above target. |
| ~~`services/template_query_service.rs`~~ | ~~171~~ | ~~153~~ | **89.47%** | Above target. |

---

## Gap Analysis by Priority

### Critical Priority -- Production Correctness Risk

These modules form the core orchestration pipeline. Bugs here cause task corruption, stuck workflows, or data loss.

**1. Result Processing Pipeline (589 uncovered lines, 8-21% coverage)**

| Module | Coverage | Uncovered Lines | Risk |
|--------|----------|-----------------|------|
| `message_handler.rs` | 8.1% | 249 | Routes all step results; unverified routing logic can misroute completions |
| `state_transition_handler.rs` | 14.2% | 139 | Handles EnqueuedForOrchestration race condition fix (TAS-41); untested means race could regress |
| `task_coordinator.rs` | 21.3% | 85 | Finalization coordination; incorrect idempotency logic risks duplicate finalization |
| `metadata_processor.rs` | 13.8% | 69 | Orchestration metadata extraction; silent failures cause missing execution data |
| `step_result_processor.rs` | 31.9% | 47 | Delegation layer to result processing; thin but critical path |

**Test approach**: Integration tests with database fixtures. Create tasks with multiple steps in various states, submit step results via the message handler, and verify correct state transitions and finalization triggering. Mock the decision point and batch processing actors. The state transition handler needs specific tests for EnqueuedForOrchestration steps to verify the TAS-41 race condition fix.

**Estimated coverage impact**: +1.6 percentage points (approximately 589 lines / 37,306 total)

**2. ~~State Management (442 uncovered lines at 17.4% coverage)~~ — RESOLVED**

~~The `state_manager.rs` coordinates SQL function intelligence with state machine transitions.~~

**Resolution**: `state_manager.rs` was deleted (1,297 lines). Investigation revealed 11 of 14 methods were dead code. The 3 used methods were inlined into their callers (`StepEnqueuer`, `StateInitializer`). The 20 tests that tested dead code paths were also removed. This eliminated 442 coverable lines from the denominator.

**Estimated coverage impact**: +1.2 percentage points (denominator reduction)

**3. Viable Step Discovery (342 lines at 7.3% coverage)**

SQL-driven step readiness discovery is the engine for determining which workflow steps can execute. At 7.3% coverage, the dependency resolution, circuit breaker integration, and state machine verification are essentially untested.

**Test approach**: Integration tests with multi-step workflow fixtures at various dependency levels. Test circuit breaker bypass logic, dependency level calculation, and state machine verification after SQL function returns.

**Estimated coverage impact**: +0.9 percentage points

**4. Error Handling Service (217 lines at 0% coverage)**

Bridges error classification with actual step state transitions and backoff scheduling. This module determines whether a failed step retries, permanently fails, or enters waiting state. Zero coverage means the retry/failure decision logic is completely unverified.

**Test approach**: Unit tests with mock state machine and error classifier. Test each ErrorHandlingAction path: permanent failure, retry with backoff, retry limit exceeded, and no-action-needed cases. Verify backoff timing calculations.

**Estimated coverage impact**: +0.6 percentage points

**5. Command Processor Actor / Command Processing Service — RESTRUCTURED**

The command processor has been decomposed into three components:

| Component | Lines | Testability |
|-----------|-------|-------------|
| `command_processor_actor.rs` | 366 | Thin routing + stats — `execute_with_stats` testable |
| `commands/service.rs` | ~340 | **Testable with InMemoryMessagingService** |
| `commands/pgmq_message_resolver.rs` | ~115 | Error path testable, success path needs PGMQ |

The `CommandProcessingService` in `commands/service.rs` now has three explicit lifecycle flows. Flow 2 (FromMessage) methods can be tested with `InMemoryMessagingService` — no PGMQ or RabbitMQ needed. Flow 3 error paths (provider rejection) are also testable with the in-memory provider.

**Test approach**: Construct `CommandProcessingService` with `MessagingProvider::new_in_memory()`. Test Flow 2 methods with constructed `QueuedMessage` values. Test Flow 3 error paths (provider rejects `supports_fetch_by_message_id`). Test `execute_with_stats` with mock futures for success/error counting. Test `health_check()` with constructed `HealthStatusCaches`.

**Estimated coverage impact**: +1.3 percentage points (now achievable without messaging infrastructure for most paths)

### High Priority -- Quality and Reliability Risk

**6. Bootstrap System (444 lines at 9.7% coverage)**

The unified orchestration bootstrap initializes the entire system for all deployment modes (standalone, Docker, test). At 9.7% coverage, most initialization paths, lifecycle management (start/stop), and error handling during startup are untested.

**Test approach**: Integration tests for each deployment mode initialization. Test graceful shutdown sequencing. Test error handling when dependent services are unavailable. The bootstrap is complex because it conditionally initializes web, gRPC, and event systems based on feature flags.

**Estimated coverage impact**: +1.1 percentage points

**7. Orchestration Event System (370 lines, 0% → ~47% coverage) — PARTIALLY ADDRESSED**

The queue-level event system implementation coordinates PGMQ listener and fallback poller with the EventDrivenSystem interface. **28 tests added** (13 unit + 15 integration) covering:

- **Unit tests** (in-module `#[cfg(test)]`): `fire_and_forget_command` success/closed-channel paths, `process_orchestration_notification` for all notification variants (StepResult, TaskRequest, TaskFinalization, Unknown, ConnectionError, Reconnected, StepResultWithPayload valid/invalid, timestamp updates)
- **Integration tests** (`tests/services/event_system_tests.rs`): Construction/getters, health_check when not running, config values, component_statistics, process_event for all event types, multiple/mixed events, stop lifecycle, closed channel error paths

**Remaining uncovered (~53%)**: `start()` deployment mode logic (~120 lines), `setup_listener_and_spawn_loop()` (~55 lines), `setup_fallback_poller()` (~15 lines), `listener_config()`/`poller_config()` (~35 lines), `health_check()` running paths (~110 lines), `send_command_and_await()` error paths (~50 lines). These require real messaging infrastructure (PGMQ listener, fallback poller) or a multi-threaded runtime (the `statistics()` method uses `block_in_place` which is incompatible with `sqlx::test`'s current-thread runtime).

**Estimated remaining coverage impact**: +0.5 percentage points (from messaging infrastructure tests)

**8. Fallback Poller (208 lines at 0% coverage)**

The orchestration queue fallback poller is the safety net ensuring no messages are missed. Zero coverage means the reliability mechanism itself is unverified.

**Test approach**: Integration tests with messaging infrastructure. Seed queue with messages, verify poller picks them up. Test age threshold filtering, batch size limits, and circuit breaker interaction.

**Estimated coverage impact**: +0.6 percentage points

**9. Staleness Detector (274 lines at 21.2% coverage)**

Background service that detects and transitions stale tasks. Uses SQL functions and integrates with OpenTelemetry metrics. The detection logic, batch worker checkpoint health checks (TAS-59), and dry-run mode need verification.

**Test approach**: Integration tests creating tasks in stale states. Verify detection and state transition via SQL function. Test batch worker checkpoint filtering. Test dry-run mode produces correct output without side effects.

**Estimated coverage impact**: +0.6 percentage points

**10. gRPC Service Layer (741 lines across 13 files, all 0% coverage)**

The entire gRPC layer has zero coverage. This includes 7 service implementations (tasks, steps, templates, analytics, DLQ, config, health), auth interceptors, type conversions, server setup, and shared state.

| gRPC Module | Uncovered Lines |
|-------------|-----------------|
| `services/dlq.rs` | 166 |
| `server.rs` | 121 |
| `services/analytics.rs` | 88 |
| `conversions.rs` | 87 |
| `services/tasks.rs` | 82 |
| `interceptors/auth.rs` | 57 |
| `services/templates.rs` | 45 |
| `services/config.rs` | 39 |
| `services/steps.rs` | 33 |
| `state.rs` | 12 |
| `services/health.rs` | 11 |

**Test approach**: The gRPC services delegate to the same shared `services/` layer that the REST handlers use (which are already at 65-96% coverage). There are two viable strategies:

- *Conversion tests*: Unit tests for `conversions.rs` (proto-to-domain and domain-to-proto) and auth interceptor logic. These are pure functions and can be tested without infrastructure.
- *Integration tests*: Spin up a tonic test server, send gRPC requests, verify responses match expectations. Existing REST/gRPC parity tests (`cargo make test-grpc-parity`) could be extended.

**Estimated coverage impact**: +2.0 percentage points

**11. Decision Point Service (255 lines at 16.1% coverage)**

Dynamic workflow step creation from decision outcomes. Creates new steps, edges, and manages cycle detection. Low coverage means the DAG modification logic is largely unverified.

**Test approach**: Integration tests with template fixtures containing decision points. Test each outcome type: step creation, edge creation, cycle detection rejection. Verify transactional safety (rollback on failure).

**Estimated coverage impact**: +0.6 percentage points

**12. Batch Processing Service (203 lines at 8.9% coverage)**

Dynamic batch worker instance creation (TAS-59). Analyzes datasets, generates cursor configurations, creates worker instances. Similar architecture to decision point but for batch parallelism.

**Test approach**: Integration tests with batch-configured templates. Test NoBatches and CreateBatches paths. Verify convergence step creation when dependencies intersect. Test cursor configuration generation.

**Estimated coverage impact**: +0.5 percentage points

### Medium Priority -- Infrastructure and Observability

**13. Orchestration Queues Listener (330 lines at 33.9% coverage)**

PGMQ queue listener with LISTEN/NOTIFY integration. The message dispatch and statistics tracking have partial coverage. The inline test module provides some baseline.

**Test approach**: Extend existing inline tests to cover message dispatch for each queue type. Test statistics tracking (messages received, errors, latency). Test reconnection logic on listener failures.

**Estimated coverage impact**: +0.6 percentage points

**14. Unified Event Coordinator (300 lines at 35.3% coverage)**

Manages multiple event systems across deployment modes. Partially covered but the mode coordination and startup sequencing need more tests.

**Test approach**: Integration tests for coordinator lifecycle: startup, shutdown, mode transitions. Test error handling when individual event systems fail.

**Estimated coverage impact**: +0.5 percentage points

**15. Task Finalization Pipeline (completion_handler + state_handlers + event_publisher)**

| Module | Coverage | Uncovered Lines |
|--------|----------|-----------------|
| `completion_handler.rs` | 44.4% | 184 |
| `state_handlers.rs` | 25.9% | 160 |
| `event_publisher.rs` | 37.8% | 89 |

These modules handle the final phase of task processing. The completion handler evaluates whether all steps are done; state handlers manage per-state finalization logic; event publisher emits completion events.

**Test approach**: Integration tests exercising each finalization path: all-steps-complete (success), partial failure (some steps errored), in-progress (more steps to enqueue). Test event publishing for each outcome type.

**Estimated coverage impact**: +1.2 percentage points

**16. Backoff Calculator (213 lines at 33.8% coverage)**

Exponential backoff with jitter. Has inline tests but many edge cases untested. The `BackoffContext` integration and configuration-driven behavior need more coverage.

**Test approach**: Unit tests for edge cases: max retry exceeded, zero delay, very large retry counts, configuration boundary values. The pure-function nature makes this straightforward.

**Estimated coverage impact**: +0.4 percentage points

**17. Error Classifier (531 lines at 49.2% coverage)**

Error classification is half-covered. The uncovered portions likely include less common error patterns and edge cases in the pattern matching logic.

**Test approach**: Add unit tests for underrepresented error patterns. Test classification of rare error types. The existing inline test infrastructure should make this easy to extend.

**Estimated coverage impact**: +0.7 percentage points

**18. Hydration Layer (step_result + finalization + task_request hydrators)**

| Module | Coverage | Uncovered Lines |
|--------|----------|-----------------|
| `step_result_hydrator.rs` | 2.0% | 146 |
| `finalization_hydrator.rs` | 11.1% | 56 |
| `task_request_hydrator.rs` | 14.0% | 43 |

These modules convert lightweight queue messages into rich domain objects via database lookups. All three are very low coverage.

**Test approach**: Integration tests with seeded database records. Create workflow steps with stored results, hydrate from mock PGMQ messages, verify correct output. Test error paths: missing step, invalid JSON, null results column.

**Estimated coverage impact**: +0.7 percentage points

### Lower Priority -- Completeness Items

**19. Web Infrastructure (extractors, middleware, state)**

| Module | Coverage | Uncovered Lines |
|--------|----------|-----------------|
| `web/extractors.rs` | 0% | 47 |
| `web/middleware/operational_state.rs` | 0% | 28 |
| `web/middleware/mod.rs` | 0% | 22 |
| `web/state.rs` | 33.8% | 96 |

The web extractors and operational state middleware are at 0% but are exercised implicitly by web handler tests (which are at 65-96%). Adding direct tests would improve coverage metrics but provides lower marginal value since the paths are already exercised.

**Test approach**: Unit tests for extractors (pool selection logic). Test operational state middleware responses during maintenance/degraded mode. These are relatively small modules.

**Estimated coverage impact**: +0.5 percentage points

**20. Actor Wrappers (batch_processing, result_processor, task_finalizer, step_enqueuer, task_request)**

The actor wrapper modules are at 43-52% coverage. They are thin wrappers that implement the `Handler` trait and delegate to lifecycle services. The uncovered portions are primarily the `handle` method implementations which require the full actor system to be running.

**Test approach**: These are best tested through integration tests that exercise the full actor pipeline. Individual actor tests would require significant mocking infrastructure for limited value.

**Estimated coverage impact**: +0.3 percentage points

**21. Binary Entry Points (server.rs, generate_openapi.rs)**

These are CLI entry points that parse arguments and start the server. Testing them directly requires starting the full server, which is better covered by E2E tests.

**Estimated coverage impact**: +0.2 percentage points

---

## Recommended Test Plan

### Phase 1: Critical Path (Target: +7.0 pp, reaching approximately 38.6%)

Focus on production correctness. All modules here handle task/step state and affect data integrity.

| Action | Files | Test Type | Est. Lines | Est. pp |
|--------|-------|-----------|-----------|---------|
| Result processing pipeline tests | message_handler, state_transition_handler, task_coordinator, metadata_processor | Integration (DB fixtures) | 542 | +1.5 |
| ~~State manager integration tests~~ | ~~state_manager.rs~~ | — | — | **Resolved**: file deleted, +1.2 pp from denominator reduction |
| Viable step discovery tests | viable_step_discovery.rs | Integration (DB) + Unit (pure fns) | 317 | +0.9 |
| Error handling service unit tests | error_handling_service.rs | Unit (mock state machine) | 217 | +0.6 |
| Command processing service tests | commands/service.rs | Unit (InMemoryMessagingService) + Integration | 340 | +1.3 |
| Error type conversion tests | errors.rs | Unit | 54 | +0.1 |
| Core orchestration tests | core.rs | Integration | 167 | +0.4 |
| Decision point service tests | decision_point/service.rs | Integration (DB) | 214 | +0.6 |
| Batch processing service tests | batch_processing/service.rs | Integration (DB) | 185 | +0.5 |

### Phase 2: Reliability and API (Target: +6.5 pp, reaching approximately 45.1%)

Focus on infrastructure reliability and API coverage.

| Action | Files | Test Type | Est. Lines | Est. pp |
|--------|-------|-----------|-----------|---------|
| Bootstrap lifecycle tests | bootstrap.rs | Integration | 401 | +1.1 |
| gRPC conversion unit tests | grpc/conversions.rs | Unit | 87 | +0.2 |
| gRPC auth interceptor tests | grpc/interceptors/auth.rs | Unit | 57 | +0.2 |
| gRPC service integration tests | grpc/services/*.rs | Integration (tonic) | 464 | +1.2 |
| gRPC server/state tests | grpc/server.rs, grpc/state.rs | Integration | 133 | +0.4 |
| Staleness detector tests | staleness_detector.rs | Integration (DB) | 216 | +0.6 |
| Task finalization pipeline tests | completion_handler, state_handlers, event_publisher | Integration (DB) | 433 | +1.2 |
| ~~Event system~~ + fallback poller | ~~orchestration_event_system~~ (partially done, ~47%), fallback_poller | Integration (messaging) | ~290 | +0.8 |

### Phase 3: Depth and Completeness (Target: +4.5 pp, reaching approximately 49.6%)

Extend existing coverage and close remaining gaps.

| Action | Files | Test Type | Est. Lines | Est. pp |
|--------|-------|-----------|-----------|---------|
| Hydration layer tests | step_result_hydrator, finalization_hydrator, task_request_hydrator | Integration (DB) | 245 | +0.7 |
| Backoff calculator edge cases | backoff_calculator.rs | Unit | 141 | +0.4 |
| Error classifier extensions | error_classifier.rs | Unit | 270 | +0.7 |
| Queue listener extensions | orchestration_queues/listener.rs | Integration | 218 | +0.6 |
| Unified event coordinator | unified_event_coordinator.rs | Integration | 194 | +0.5 |
| Task request processor | task_request_processor.rs | Integration | 76 | +0.2 |
| Step enqueuer services | batch_processor, state_handlers | Integration | 164 | +0.4 |
| Fallback poller (task readiness) | task_readiness/fallback_poller.rs | Integration | 76 | +0.2 |
| Web infrastructure | extractors, operational_state, state | Unit/Integration | 193 | +0.5 |
| Health modules | db_status, status_evaluator | Unit/Integration | 123 | +0.3 |

### Phase 4: Final Push to 55% (Target: +5.4 pp)

Extend coverage on already-partially-covered modules, targeting the 55-85% range files to bring them to 90%+ and picking up remaining gaps.

| Action | Est. pp |
|--------|---------|
| Deepen web handler tests (dlq, tasks) | +1.0 |
| Extend step enqueuer tests | +0.8 |
| Actor wrapper handle() method tests | +0.5 |
| Template query service extensions | +0.4 |
| System events module extensions | +0.5 |
| Task initialization module extensions | +0.3 |
| Health service edge cases | +0.4 |
| Miscellaneous small modules | +0.5 |
| Binary entry point smoke tests | +0.2 |
| Event system statistics coverage | +0.4 |
| Remaining health/channel modules | +0.4 |

---

## Estimated Impact

| Phase | Focus | Estimated Lines Added | Coverage Delta | Cumulative Coverage |
|-------|-------|-----------------------|----------------|---------------------|
| Baseline | -- | -- | -- | 31.6% |
| **Completed** | **Unit tests + integration tests** | **1,228 integration + inline unit** | **+4.10 pp** | **35.70%** |
| **Completed** | **Refactoring (denominator reduction + testability)** | **~2,700 lines removed/restructured** | **+~1.88 pp** | **37.58%** |
| **Completed** | **Quick-win unit + result processing pipeline** | **71 tests (45 pure unit + 26 integration)** | **+3.42 pp** | **41.0%** |
| **Completed** | **Staleness detector + backoff calculator** | **38 tests (pure unit)** | **+0.64 pp** | **41.64%** |
| **Completed** | **DB integration: hydration + finalization pipeline** | **22 tests (#[sqlx::test])** | **+1.03 pp** | **42.67%** |
| **Completed** | **Broad unit + DB tests across 12 gap files** | **128 tests** | **+2.21 pp** | **44.88%** |
| **Completed** | **Event system unit + integration tests** | **28 tests (13 unit + 15 integration)** | **+~0.7 pp** | **~45.6%** |
| **Completed** | **DB integration: viable steps, decision point, batch processing** | **~28 tests (DB integration)** | **+~1.85 pp** | **47.45%** |
| **Completed** | **gRPC layer unit tests (DLQ conversions, analytics, auth, templates)** | **45 tests** | **+0.89 pp** | **48.34%** |
| Remaining | Messaging infrastructure, orchestration core, gRPC service integration, final push | ~TBD | ~+6.66 pp | ~55.0% |

**Key constraints**:

- The orchestration event system (64.21%) and fallback poller require messaging infrastructure (PGMQ or RabbitMQ) for further integration tests. `OrchestrationEventSystem::statistics()` uses `block_in_place` requiring a multi-threaded runtime — incompatible with `sqlx::test`'s current-thread runtime.
- gRPC tests can be split: pure conversion/error-mapping tests (no infrastructure) and service tests (tonic test server). The conversion module is already at 100%.
- The bootstrap module (9.68%, 401 uncovered lines) is the single largest non-gRPC gap but is challenging to test in isolation because it wires together the entire system.
- Many modules already have `#[cfg(test)]` blocks with inline tests. Extending these is often more efficient than adding new test files.
- **Coverage capture gap**: The root `tests/` directory contains E2E and integration tests (`tests/e2e/`, `tests/integration/`, `tests/grpc/`) that exercise orchestration code paths but are not included in per-crate coverage measurement. Investigating `cargo llvm-cov` workspace-mode or test binary inclusion could capture this coverage, potentially adding several percentage points without writing new tests.

**Files with existing inline tests (83+ files)**: The presence of `#[cfg(test)]` in 83+ of 133 source files (up from 69 at baseline) reflects the unit test additions across all phases. Remaining gaps are concentrated in gRPC services, orchestration infrastructure (bootstrap, listener, coordinator), and messaging-dependent modules.

### Completed Work (Jan 30, 2026)

**Unit test phase** added 183 inline tests across 14 source files covering error types, type conversions, channel wrappers, command types, middleware helpers, and service error classification. This provided +3.91 pp coverage.

**Integration test phase** added 41 `#[sqlx::test]` tests across 4 new test files (`task_query_service_tests.rs`, `step_query_service_tests.rs`, `template_query_service_tests.rs`, `analytics_service_tests.rs`). These validate database query services, template discovery, analytics SQL functions, and response transformation against real PostgreSQL. This provided +0.19 pp coverage.

Key modules now covered that were previously at 0%:

- `TaskQueryService` — get_task_with_context, list_tasks_with_context, to_task_response
- `StepQueryService` — list_steps_for_task, get_step_with_readiness, audit history, ownership verification
- `TemplateQueryService` — list_templates, get_template, template_exists, get_namespace
- `AnalyticsQueryService` — performance metrics and bottleneck analysis from live database
- `grpc/conversions.rs` — 28 proto-to-domain conversion tests
- `orchestration/errors.rs` — 10 From trait conversion tests
- `orchestration/backoff_calculator.rs` — 18 configuration and calculation tests
- `orchestration/error_handling_service.rs` — 9 action/result tests
- `orchestration/state_manager.rs` — 20 transition request/outcome tests (later removed with file)

### Refactoring Phase (Jan 30, 2026)

Decomposed four large files (4,240 lines total) into smaller, testable units. All refactoring is behavior-preserving — 506 library tests passing at each checkpoint.

**Dead code removed:**

- `state_manager.rs` deleted (1,297 lines, 442 coverable). 11/14 methods were dead code.
- Unused `current_queue_sizes: HashMap` removed from stats struct.

**Files restructured:**

- `command_processor_actor.rs`: 1,001 → 366 lines. Business logic extracted to `commands/service.rs`.
- `orchestration_event_system.rs`: 1,359 → ~700 lines. Helpers extracted, duplication eliminated.
- `viable_step_discovery.rs`: 583 → ~450 lines. Pure functions extracted.

**New testable modules created:**

- `commands/service.rs` (~340 lines) — `CommandProcessingService` with three lifecycle flows. **Testable with `InMemoryMessagingService`** — no PGMQ/RabbitMQ required for Flow 2 (FromMessage) and Flow 3 error paths.
- `commands/pgmq_message_resolver.rs` (~115 lines) — PGMQ-only signal resolution.
- `event_systems/command_outcome.rs` (~50 lines) — Pure `from_*` classifiers.
- `health_check_evaluator.rs` (~50 lines) — Pure function for health evaluation.

**Concurrency fix:** `OrchestrationProcessingStats` converted from `Arc<RwLock<struct>>` to `AtomicProcessingStats` with lock-free `AtomicU64` counters.

**Testability unlocked:** The `CommandProcessingService` can be constructed with `MessagingProvider::new_in_memory()` for unit tests. This eliminates the messaging infrastructure barrier identified in the original analysis for ~340 lines of command processing logic. Combined with the pure function extractions (CommandOutcome, health evaluator, step request builder, dependency filter), approximately 500 lines of previously untestable logic are now independently verifiable.

### Quick-Win Unit Tests + Result Processing Pipeline (Jan 31, 2026)

**Pure unit tests** added 45 inline tests across 3 files:

- `orchestration_statistics.rs`: 12 tests — default, clone, counters, processing_rate, average_latency, deployment_mode_score (0% → ~65%)
- `error_classifier.rs`: 25 tests — classify_state_error, classify_validation_error, classify_execution_error (9 variants), exponential backoff, category delays, suggestions (49% → ~75%)
- `system_events.rs`: 8 tests — get_event_metadata, get_step_transitions, get_transitions_from_state, validate_event_payload (63% → ~80%)

**Result processing pipeline integration tests** added 26 tests across 4 files:

- `state_transition_handler.rs`: 10 tests — extract_error_message, process_state_transition, should_retry_step, determine_success_event (14% → ~50%)
- `task_coordinator.rs`: 4 tests — coordinator actions for different task states (21% → ~35%)
- `metadata_processor.rs`: 6 tests — empty metadata, server/rate-limit/custom backoff hints, error context (14% → ~50%)
- `message_handler.rs`: 6 tests — handle_step_result_message, handle_step_execution_result, get_correlation_id (8% → ~35%)

**Coverage impact**: 37.58% → 41.0% (+3.42 pp), 683 → 754 tests

**New modules at 100% coverage** (from extractions):

- `event_systems/command_outcome.rs` — Pure `from_*` classifiers
- `health_check_evaluator.rs` — Pure function for health evaluation

**Notable coverage improvements** from refactoring + command processing tests:

- `commands/service.rs` — 57% (new module, testable with InMemoryMessagingService)
- `commands/types.rs` — 82% (extracted type definitions)
- Post-refactoring measurement: 37.58% line, 34.84% function, 683 tests

### Staleness Detector + Backoff Calculator (Jan 31, 2026)

**Pure unit tests** added 38 inline tests across 2 files, targeting the last remaining pure-logic-heavy modules:

- `staleness_detector.rs`: 20 tests — `is_batch_worker()` JSON cursor detection (with/without/null/empty/array batch_cursor), `is_batch_worker_checkpoint_healthy()` timestamp validation (recent/stale/threshold boundary/missing/invalid/numeric/null/different thresholds/non-object cursor), StalenessAction variant behavior, Debug/Clone impls, time bucket classification (21.2% → 63.67%)
- `backoff_calculator.rs`: 18 new tests + 7 converted from `#[sqlx::test]` to `#[tokio::test]` — RFC 2822 date parsing (future/past), zero/negative/large/empty retry-after values, jitter bounds at scale (10,000 iterations), zero max jitter, saturating arithmetic with u32::MAX, BackoffType serialization roundtrip, context header/metadata overwrite, BackoffResult clone, calculator Debug/Clone (33.8% → 93.27%)

**Key technique**: Converted 7 existing `#[sqlx::test]` database-dependent tests to `#[tokio::test]` using `PgPool::connect_lazy()`, removing the PostgreSQL requirement. All tests constructing objects with PgPool require `#[tokio::test]` (not `#[test]`) because `connect_lazy` needs a Tokio runtime context.

**Coverage impact**: 41.0% → 41.64% (+0.64 pp), 754 → 792 tests

**Measured per-file coverage improvements** (actual vs pre-phase):

| File | Before | After | Delta |
|------|--------|-------|-------|
| `backoff_calculator.rs` | 33.8% | 93.27% | +59.5 pp |
| `error_classifier.rs` | 49.2% | 90.42% | +41.2 pp |
| `system_events.rs` | 63% | 86.78% | +23.8 pp |
| `orchestration_statistics.rs` | 0% | 81.37% | +81.4 pp |
| `metadata_processor.rs` | 13.8% | 81.11% | +67.3 pp |
| `task_coordinator.rs` | 21.3% | 75.21% | +53.9 pp |
| `staleness_detector.rs` | 21.2% | 63.67% | +42.5 pp |
| `message_handler.rs` | 8.1% | 59.37% | +51.3 pp |
| `state_transition_handler.rs` | 14.2% | 56.17% | +42.0 pp |
| `error_handling_service.rs` | 0% | 41.97% | +42.0 pp |

**Pure unit test opportunities exhausted**: After this phase, remaining coverage gains require infrastructure (PostgreSQL, messaging, gRPC). The next phase targets database integration tests with `#[sqlx::test]`.

### DB Integration: Hydration + Finalization Pipeline (Jan 31, 2026)

**Database integration tests** added 22 `#[sqlx::test]` tests across 3 files, targeting the step result hydration and task finalization pipeline with real PostgreSQL transactions:

- `step_result_hydrator.rs`: 10 tests — full hydration flow from PgmqMessage and QueuedMessage, missing step error, no results error, invalid results JSON deserialization failure, invalid message format, Debug impl (2.0% → 80.87%)
- `completion_handler.rs`: 8 tests — complete_task from StepsInProcess (two-step transition through EvaluatingResults), from Initializing (no-steps), from Pending (error rejection), already-complete idempotent, with TaskExecutionContext data, error_task from EvaluatingResults → BlockedByFailures, from BlockedByFailures → Error terminal, already-Error idempotent (44.4% → 84.20%)
- `state_handlers.rs`: 4 tests — handle_processing_state NoAction with/without context, handle_unclear_state delegation to error_task, handle_waiting_state blocked-by-failures delegation (25.9% → 83.33%)

**Key technique**: Test helper functions using `sqlx::query()` (runtime-checked, no cache update needed) create complete FK dependency chains (namespace → named_task → task → named_step → workflow_step) for each test. `SystemContext::with_pool(pool)` provides the full orchestration context from the test pool.

**Side effect**: finalization_hydrator.rs rose to 95.45% and task_request_hydrator.rs to 95.16% — their inline tests were already comprehensive but weren't counted in the previous measurement because coverage was run with fewer tests.

**Coverage impact**: 41.64% → 42.67% (+1.03 pp), 792 → 814 tests

### Broad Coverage Push: Unit + DB Tests Across Gap Files (Jan 31, 2026)

**Unit and database integration tests** added 128 tests across 12 files, targeting the remaining below-55% gap files with a breadth-first approach:

**Round 1 (commit 5d4a4c8)**: 47 tests across 6 files

- `step_enqueuer.rs`: 10 tests — serialization, clone, debug, multi-namespace stats, empty results, config access (54.2% → 70.9%)
- `core.rs`: 11 tests — OrchestrationCoreStatus Display/PartialEq/Debug/Clone for all 6 variants (24.4% → 43.2%)
- `unified_event_coordinator.rs`: 9 tests — config defaults/clone/debug, health report construction/unhealthy/statistics (35.3% → 52.7%)
- `state_handlers.rs`: 5 tests — TaskEvent variants, ExecutionStatus debug, ReadyTaskInfo construction (54.3% → 63.2%)
- `batch_processor.rs`: 4 tests — empty batch DB test, debug, performance metrics (35.3% → 61.7%)
- `task_request_processor.rs`: 8 tests — config customization/debug/clone, stats fields, minimal message (36.1% → 59.6%)

**Round 2 (commit e9c560d)**: 54 tests across 5 files

- `batch_processing/service.rs`: 15 tests — all 7 error Display variants, From<StateMachineError> conversion, service creation DB test, BatchWorkerInputs, CursorConfig, BatchProcessingOutcome serde (8.9% → 42.1%)
- `error_handling_service.rs`: 17 tests — result construction/clone/debug/serialization, action variants/clone/debug/serde, config default/custom/clone/debug, service creation DB tests (42.0% → 54.9%)
- `db_status.rs`: 8 tests — evaluate_db_status with real DB (healthy + circuit breaker open), circuit breaker state detection/reset, DatabaseHealthStatus fields/error (26.7% → 78.6%)
- `task_readiness_event_system.rs`: 7 tests — processor_uuid, deployment_mode (no config/with config), debug, stop lifecycle, disabled start (30.2% → 47.7%)
- `listener.rs`: 7 tests — config custom/clone/debug, stats increment/debug, notification debug/clone (33.9% → 46.0%)

**Round 3 (commit e2f3dc6)**: 12 tests across 2 files

- `orchestration_queues/fallback_poller.rs`: 7 tests — OrchestrationPollerConfig defaults/custom/clone/debug, OrchestrationPollerStats defaults/increment/debug (0% → 32.3%)
- `task_readiness/fallback_poller.rs`: 5 tests — config debug/clone, poller debug/stop/config accessor DB tests (41.1% → 59.3%)

**Coverage impact**: 42.67% → 44.88% (+2.21 pp), 814 → 942 tests

**Notable per-file improvements (before → after)**:

| File | Before | After | Delta |
|------|--------|-------|-------|
| `db_status.rs` | 26.7% | 78.6% | +51.9 pp |
| `batch_processing/service.rs` | 8.9% | 42.1% | +33.2 pp |
| `task_request_processor.rs` | 36.1% | 59.6% | +23.5 pp |
| `orchestration_queues/fallback_poller.rs` | 0% | 32.3% | +32.3 pp |
| `batch_processor.rs` | 35.3% | 61.7% | +26.4 pp |
| `task_readiness/fallback_poller.rs` | 41.1% | 59.3% | +18.2 pp |
| `task_readiness_event_system.rs` | 30.2% | 47.7% | +17.5 pp |
| `unified_event_coordinator.rs` | 35.3% | 52.7% | +17.4 pp |
| `core.rs` | 24.4% | 43.2% | +18.8 pp |
| `step_enqueuer.rs` | 54.2% | 70.9% | +16.7 pp |
| `error_handling_service.rs` | 42.0% | 54.9% | +12.9 pp |
| `listener.rs` | 33.9% | 46.0% | +12.1 pp |

**Files now above 55% target**: db_status.rs (78.6%), step_enqueuer.rs (70.9%), state_handlers.rs (63.2%), batch_processor.rs (61.7%), task_request_processor.rs (59.6%), task_readiness/fallback_poller.rs (59.3%)

### Event System Tests: OrchestrationEventSystem (Jan 31, 2026)

**28 tests added** (13 unit + 15 integration) targeting `orchestration_event_system.rs` which was at **0% coverage** — the largest single uncovered file in the crate at 370 coverable lines.

**Unit tests** (13 `#[cfg(test)]` inline tests in `orchestration_event_system.rs`):

Two-pronged approach testing private static methods directly:

*`fire_and_forget_command` tests* (4 tests, `#[tokio::test]`, no DB needed):

- `test_fire_and_forget_success` — command reaches receiver, `operations_coordinated` incremented
- `test_fire_and_forget_closed_channel` — closed channel increments `events_failed`, no panic
- `test_fire_and_forget_task_request` — TaskRequest command routing
- `test_fire_and_forget_finalization` — Finalization command routing

*`process_orchestration_notification` tests* (9 tests, `#[sqlx::test]` with real DB):

- `test_notification_event_step_result` — Event(StepResult) → ProcessStepResultFromMessageEvent command
- `test_notification_event_task_request` — Event(TaskRequest) → InitializeTaskFromMessageEvent command
- `test_notification_event_task_finalization` — Event(TaskFinalization) → FinalizeTaskFromMessageEvent command
- `test_notification_event_unknown` — Event(Unknown) → events_failed incremented, no command
- `test_notification_connection_error` — ConnectionError → events_failed incremented
- `test_notification_reconnected` — Reconnected → no error stats change
- `test_notification_step_result_with_valid_payload` — StepResultWithPayload valid JSON → ProcessStepResultFromMessage command
- `test_notification_step_result_with_invalid_payload` — Invalid JSON → events_failed incremented
- `test_notification_updates_timestamp` — Any notification updates last_processing_time_epoch_nanos

**Integration tests** (15 `#[sqlx::test]` tests in `tests/services/event_system_tests.rs`):

Setup: `OrchestrationEventSystem` constructed with real DB pool, `OrchestrationCore`, `ChannelFactory` command channel, and `ChannelMonitor`. Mock command handler spawned on receiver to respond to ProcessStepResult/InitializeTask/FinalizeTask commands.

- `test_construction_and_getters` — system_id, deployment_mode, is_running, uptime
- `test_health_check_fails_when_not_running` — health_check error with "not running" message
- `test_config_returns_expected_values` — config deployment_mode matches
- `test_component_statistics_initially_empty` — no poller/listener/uptime stats
- `test_process_event_step_result` — StepResult event processed via mock handler
- `test_process_event_task_request` — TaskRequest event processed via mock handler
- `test_process_event_task_finalization` — TaskFinalization event processed via mock handler
- `test_process_event_unknown_succeeds` — Unknown event handled without error
- `test_process_event_multiple_succeeds` — 3 successive StepResult events
- `test_process_event_mixed_types` — one of each event type in sequence
- `test_stop_when_not_running` — stop() succeeds without panic
- `test_process_event_with_closed_channel` — StepResult fails with closed channel
- `test_process_event_task_request_closed_channel` — TaskRequest fails with closed channel
- `test_process_event_finalization_closed_channel` — TaskFinalization fails with closed channel
- `test_component_statistics_unchanged_after_process_event` — stats remain empty without start()

**Key discovery**: `OrchestrationEventSystem::statistics()` uses `tokio::task::block_in_place` for async aggregation, which requires a multi-threaded runtime. Since `sqlx::test` uses current-thread runtime, integration tests validate behavior through `process_event` return values (Ok/Err) and the async-native `component_statistics()` method instead of `statistics()`.

**Coverage impact**: 44.88% → ~45.6% (est. +~0.7 pp), 942 → 970 tests

**Estimated per-file improvement**:

| File | Before | After | Delta |
|------|--------|-------|-------|
| `orchestration_event_system.rs` | 0% | ~47% | +47 pp |

### gRPC Layer Unit Tests (Jan 31, 2026)

**45 tests added** (all inline `#[cfg(test)]`) targeting the gRPC service layer which was at **0% coverage** across 13 files (871 coverable lines). Focused on pure conversion helpers and error mapping functions that don't require infrastructure.

**DLQ service tests** (26 tests in `grpc/services/dlq.rs`):

- All 4 `DlqResolutionStatus` to-proto and 5 from-proto variants (including Unspecified → None)
- All 5 `DlqReason` to-proto variants
- All 3 `StalenessHealthStatus` to-proto variants
- `dlq_entry_to_proto` — full struct conversion with and without optional fields (resolution_timestamp, metadata)
- `dlq_stats_to_proto` — with and without optional timestamp/resolution fields
- `dlq_investigation_queue_entry_to_proto` — full struct including priority_score
- `staleness_monitoring_entry_to_proto` — full struct with health_status conversion

**Analytics service tests** (11 tests in `grpc/services/analytics.rs`):

- `tasker_error_to_status` — 9 error variant mappings: ValidationError → InvalidArgument, CircuitBreakerOpen → Unavailable, Timeout → DeadlineExceeded, DatabaseError/Internal/Messaging/StateTransition → Internal
- `convert_system_health_counts` — all-zeros default and populated with all 24 fields (13 task + 11 step counts)

**Auth interceptor tests** (6 tests in `grpc/interceptors/auth.rs`):

- `new(None)`, `is_enabled()` with no service, `clone()`, `debug()` formatting
- `authenticate()` with disabled auth returns permissive SecurityContext
- `SecurityContextExt` trait returns None when not set, `SECURITY_CONTEXT_KEY` constant

**Template service tests** (4 tests in `grpc/services/templates.rs`):

- `template_error_to_status` — NamespaceNotFound → NotFound, TemplateNotFound → NotFound, DatabaseError → Internal, Internal → Internal

**Remaining uncovered gRPC files** (require service infrastructure):

- `grpc/server.rs` (121 lines) — server bootstrap, requires full tonic server setup
- `grpc/services/tasks.rs` (82 lines) — task CRUD, requires TaskService + database
- `grpc/services/config.rs` (39 lines) — config endpoint, requires full orchestration context
- `grpc/services/steps.rs` (33 lines) — step operations, requires StepService + database
- `grpc/state.rs` (12 lines) — state container, requires SharedApiServices
- `grpc/services/health.rs` (11 lines) — health endpoints, requires HealthService

These remaining gRPC service methods are thin delegation layers to the same shared `services/` layer that REST handlers use (already at 65-96% coverage). The root `tests/grpc/` directory contains E2E tests that exercise these code paths but aren't captured in per-crate coverage measurement.

**Coverage impact**: 47.45% → 48.34% (+0.89 pp), 999 → 1,044 tests

**Per-file improvements**:

| File | Before | After | Delta |
|------|--------|-------|-------|
| `grpc/services/dlq.rs` | 0% | 83.73% | +83.7 pp |
| `grpc/services/analytics.rs` | 0% | 75.77% | +75.8 pp |
| `grpc/interceptors/auth.rs` | 0% | 54.95% | +55.0 pp |
| `grpc/services/templates.rs` | 0% | 44.44% | +44.4 pp |
