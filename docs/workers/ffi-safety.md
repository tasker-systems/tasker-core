# FFI Safety Safeguards

**Last Updated**: 2026-02-02
**Status**: Production Implementation
**Applies To**: Ruby (Magnus), Python (PyO3), TypeScript (napi-rs) workers

---

## Overview

Tasker's FFI workers embed the Rust `tasker-worker` runtime inside language-specific host processes (Ruby, Python, TypeScript/JavaScript). This document describes the safeguards that prevent Rust-side failures from crashing or corrupting the host process, ensuring that infrastructure unavailability, misconfiguration, and unexpected panics are surfaced as language-native errors rather than process faults.

## FFI Architecture

```
Host Process (Ruby / Python / Node.js)
         │
         ▼
    FFI Boundary
    ┌─────────────────────────────────────┐
    │  Language Binding Layer              │
    │  (Magnus / PyO3 / napi-rs)          │
    │                                     │
    │  ┌─────────────────────────────┐    │
    │  │  Bridge Module              │    │
    │  │  (bootstrap, poll, complete)│    │
    │  └────────────┬────────────────┘    │
    │               │                     │
    │  ┌────────────▼────────────────┐    │
    │  │  FfiDispatchChannel         │    │
    │  │  (event dispatch, callbacks)│    │
    │  └────────────┬────────────────┘    │
    │               │                     │
    │  ┌────────────▼────────────────┐    │
    │  │  WorkerBootstrap            │    │
    │  │  (runtime, DB, messaging)   │    │
    │  └─────────────────────────────┘    │
    └─────────────────────────────────────┘
```

## Panic Safety by Framework

Each FFI framework provides different levels of automatic panic protection:

| Framework | Panic Handling | Mechanism |
|-----------|---------------|-----------|
| **Magnus** (Ruby) | Automatic | Catches panics at FFI boundary, converts to Ruby `RuntimeError` |
| **PyO3** (Python) | Automatic | Catches panics at `#[pyfunction]` boundary, converts to `PanicException` |
| **napi-rs** (TypeScript) | Automatic | Catches panics at `#[napi]` boundary, converts to JavaScript `Error` |

All three FFI frameworks now provide automatic panic safety. napi-rs (used since TAS-290) catches Rust panics at the `#[napi]` function boundary and converts them to JavaScript `Error` exceptions, matching the behavior of Magnus and PyO3. No manual `catch_unwind` wrappers are needed.

## Error Handling at FFI Boundaries

### Bootstrap Failures

When infrastructure is unavailable during worker startup, errors flow through the normal `Result` path rather than panicking:

| Failure Scenario | Handling | Host Process Impact |
|-----------------|----------|-------------------|
| Database unreachable | `TaskerError::DatabaseError` returned | Language exception, app can retry |
| Config TOML missing | `TaskerError::ConfigurationError` returned | Language exception with descriptive message |
| Worker config section absent | `TaskerError::ConfigurationError` returned | Language exception (was previously a panic) |
| Messaging backend unavailable | `TaskerError::ConfigurationError` returned | Language exception |
| Tokio runtime creation fails | Logged + language error returned | Language exception |
| Port already in use | `TaskerError::WorkerError` returned | Language exception |
| Redis/cache unavailable | Graceful degradation to noop cache | **No error** - worker starts without cache |

### Steady-State Operation Failures

Once bootstrapped, the worker handles infrastructure failures gracefully:

| Failure Scenario | Handling | Host Process Impact |
|-----------------|----------|-------------------|
| Database goes down during poll | Poll returns `None` (no events) | No impact - polling continues |
| Completion channel full | Retry loop with timeout, then logged | Step result may be lost after timeout |
| Completion channel closed | Returns `false` to caller | App code sees completion failure |
| Callback timeout (5s) | Logged, step completion unaffected | Domain events may be delayed |
| Messaging down during callback | Callback times out, logged | Domain events may not publish |
| Lock poisoned | Error returned to caller | Language exception |
| Worker not initialized | Error returned to caller | Language exception |

### Lock Acquisition

All three workers validate lock acquisition before proceeding:

```rust
// Pattern used in all workers
let handle_guard = WORKER_SYSTEM.lock().map_err(|e| {
    error!("Failed to acquire worker system lock: {}", e);
    // Convert to language-appropriate error
})?;
```

A poisoned mutex (from a previous panic) produces a language exception rather than propagating the original panic.

### EventRouter Availability

Post-bootstrap access to the `EventRouter` uses fallible error handling rather than `.expect()`:

```rust
// Use ok_or_else instead of expect to prevent panic at FFI boundary
let event_router = worker_core.event_router().ok_or_else(|| {
    error!("EventRouter not available from WorkerCore after bootstrap");
    // Return language-appropriate error
})?;
```

## Callback Safety

The `FfiDispatchChannel` uses a fire-and-forget pattern for post-completion callbacks, preventing the host process from being blocked or deadlocked by Rust-side async operations:

1. **Completion is sent first** - the step result is delivered to the completion channel before any callback fires
2. **Callback is spawned separately** - runs in the Tokio runtime, not the FFI caller's thread
3. **Timeout protection** - callbacks are bounded by a configurable timeout (default 5s)
4. **Callback failures are logged** - they never affect step completion or the host process

```
FFI Thread (Ruby/Python/JS)          Tokio Runtime
         │                                │
         ├──► complete(event_id, result)   │
         │    ├──► send result to channel  │
         │    └──► spawn callback ─────────┼──► callback.on_handler_complete()
         │                                 │    (with 5s timeout)
         ◄──── return true ────────────────│
         │  (immediate, non-blocking)      │
```

See `docs/development/ffi-callback-safety.md` for detailed callback safety guidelines.

## Backpressure Protection

### Completion Channel

The completion channel uses a try-send retry loop with timeout to prevent indefinite blocking:

- **Try-send** avoids blocking the FFI thread
- **Retry with sleep** (10ms intervals) handles transient backpressure
- **Timeout** (configurable, default 30s) prevents permanent stalls
- **Logged** when backpressure delays exceed 100ms

### Starvation Detection

The `FfiDispatchChannel` tracks event age and warns when polling falls behind:

- Events older than `starvation_warning_threshold_ms` (default 10s) trigger warnings
- `check_starvation_warnings()` can be called periodically from the host process
- `FfiDispatchMetrics` exposes pending count, oldest event age, and starvation status

## Infrastructure Dependency Matrix

| Component | Bootstrap | Poll | Complete | Callback |
|-----------|-----------|------|----------|----------|
| **Database** | Required (error on failure) | Not needed | Not needed | Errors logged |
| **Message Bus** | Required (error on failure) | Not needed | Not needed | Errors logged |
| **Config System** | Required (error on failure) | Not needed | Not needed | Not needed |
| **Cache (Redis)** | Optional (degrades to noop) | Not needed | Not needed | Not needed |
| **Tokio Runtime** | Required (error on failure) | Used | Used | Used |

## Worker Lifecycle Safety

### Start (`bootstrap_worker`)

- Validates configuration, creates runtime, initializes all subsystems
- All failures return language-appropriate errors
- Already-running detection prevents double initialization

### Status (`get_worker_status`)

- Safe when worker is not initialized (returns `running: false`)
- Safe when worker is running (queries internal state)
- Lock acquisition failure returns error

### Stop (`stop_worker`)

- Safe when worker is not running (returns success message)
- Sends shutdown signal and clears handle
- In-flight operations complete before shutdown

### Graceful Shutdown (`transition_to_graceful_shutdown`)

- Initiates graceful shutdown allowing in-flight work to drain
- Errors during transition are logged and returned
- Requires worker to be running (error otherwise)

## Adding a New FFI Worker

When implementing a new language worker:

1. **Check framework panic safety** - if the framework (like Magnus/PyO3) catches panics automatically, you get protection for free. If using raw C FFI, wrap all `extern "C"` functions with `catch_unwind`.

2. **Use the standard bridge pattern** - global `WORKER_SYSTEM` mutex, `BridgeHandle` struct containing `WorkerSystemHandle` + `FfiDispatchChannel` + runtime.

3. **Handle all lock acquisitions** - always use `.map_err()` on `.lock()` calls.

4. **Avoid `.expect()` and `.unwrap()` in FFI code** - use `ok_or_else()` or `map_err()` to convert to language-appropriate errors.

5. **Use fire-and-forget callbacks** - never block the FFI thread on async operations.

6. **Integrate starvation detection** - call `check_starvation_warnings()` periodically.

7. **Expose metrics** - expose `FfiDispatchMetrics` for health monitoring.

## Related Documentation

- [FFI Callback Safety](../development/ffi-callback-safety.md) - Detailed callback patterns and deadlock prevention
- [Worker Event Systems](../architecture/worker-event-systems.md) - Dispatch and completion channel architecture
- [MPSC Channel Guidelines](../development/mpsc-channel-guidelines.md) - Channel sizing and configuration
- [Worker Patterns & Practices](./patterns-and-practices.md) - General worker development patterns
