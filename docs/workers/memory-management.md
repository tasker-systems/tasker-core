# Native Module Memory Management in TypeScript Workers

**Status**: Active
**Applies To**: TypeScript/Bun/Node.js napi-rs native modules
**Related**: Ruby (Magnus), Python (PyO3)

---

## Overview

This document explains the memory management pattern used when calling Rust functions from TypeScript via napi-rs native modules (Node-API). napi-rs provides automatic memory management through the Node-API, eliminating most manual memory management concerns.

**Key Principle**: napi-rs handles memory lifecycle automatically via Node-API reference counting and the JavaScript garbage collector. Unlike raw FFI, you typically don't need manual memory management.

---

## The napi-rs Memory Pattern

### Automatic Memory Management

```typescript
// napi-rs handles memory automatically via Node-API
import { FfiLayer } from '@/ffi';

const ffi = new FfiLayer();

// Returns JavaScript objects directly (no manual memory management)
const status = ffi.getWorkerStatus();
// status is a JavaScript object managed by V8/JavaScriptCore GC

// No manual free() calls needed - napi-rs handles cleanup
```

### How napi-rs Works

napi-rs converts Rust types to JavaScript objects automatically using Node-API:

```rust
// Rust side with napi-rs:
#[napi]
pub fn get_worker_status() -> Result<WorkerStatus> {
    let status = WorkerStatus { /* ... */ };
    Ok(status)  // napi-rs converts to JS object automatically
}

#[napi(object)]
pub struct WorkerStatus {
    pub running: bool,
    pub pending_count: u32,
    // ... fields automatically mapped to JS object properties
}
```

napi-rs handles:

- Converting Rust structs to JavaScript objects
- Managing memory lifecycle via Node-API reference counting
- Automatic cleanup when JavaScript no longer references the object

No manual memory management needed in most cases.

---

## Automatic Cleanup

With napi-rs, cleanup happens automatically via Node-API:

```rust
// napi-rs automatically implements cleanup
#[napi]
impl FfiLayer {
    #[napi]
    pub fn poll_step_events(&self) -> Result<Vec<FfiStepEvent>> {
        // Returns Vec which napi-rs converts to JS Array
        // Memory cleaned up when JavaScript releases references
        Ok(self.inner.poll_events()?)
    }
}
```

When JavaScript garbage collector runs and the object is no longer referenced, napi-rs handles cleanup via the Node-API finalizer mechanism.

---

## Safety Guarantees

This pattern is safe because of three key properties:

### 1. Single-Threaded JavaScript Runtime

JavaScript (and TypeScript) runs on a single thread (ignoring Web Workers), which means:

- **No race conditions**: The read → free sequence is atomic from Rust's perspective
- **No concurrent access**: Only one piece of code can access the pointer at a time
- **Predictable execution order**: Steps always happen in sequence

### 2. One-Way Handoff

Rust follows a strict contract:

```
Rust allocates → Returns pointer → NEVER TOUCHES IT AGAIN
```

- Rust doesn't keep any references to the memory
- Rust never reads or writes to that memory after returning the pointer
- The memory is "orphaned" from Rust's perspective until `free_rust_string` is called

### 3. JavaScript Copies Before Freeing

JavaScript creates a new copy of the data before freeing:

```typescript
const ptr = this.lib.symbols.get_worker_status() as Pointer;

// Step 1: Read bytes from Rust memory into a JavaScript string
const json = new CString(ptr);  // COPY operation

// Step 2: Parse string into JavaScript objects
const status = JSON.parse(json);  // Creates new JS objects

// Step 3: Free the Rust memory
this.lib.symbols.free_rust_string(ptr);

// At this point:
// - 'status' is pure JavaScript (managed by V8/JavaScriptCore)
// - Rust memory has been freed (no leak)
// - 'ptr' is invalid (but we never use it again)
```

The `status` object is fully owned by JavaScript's garbage collector. It has no connection to the freed Rust memory.

---

## Comparison to Ruby and Python FFI

### Ruby (Magnus)

```ruby
# Ruby FFI with Magnus
result = TaskerCore::FFI.get_worker_status()
# No explicit free needed - Magnus manages memory via Rust Drop traits
```

**How it works**: Magnus creates a bridge between Ruby's GC and Rust's ownership system. When Ruby no longer references the object, Rust's `Drop` trait eventually runs.

### Python (PyO3)

```python
# Python FFI with PyO3
result = tasker_core.get_worker_status()
# No explicit free needed - PyO3 uses Python's reference counting
```

**How it works**: PyO3 wraps Rust data in `PyObject` wrappers. When Python's reference count reaches zero, the Rust data is dropped.

### TypeScript (napi-rs)

```typescript
// TypeScript with napi-rs - automatic memory management
import { FfiLayer } from '@/ffi';

const ffi = new FfiLayer();
const status = ffi.getWorkerStatus();  // Returns JS object directly
// No manual free needed - GC handles it
```

**Why different**: napi-rs is a high-level binding framework that uses Node-API to provide automatic memory management, similar to Magnus and PyO3.

**Tradeoff**: Cleaner API with automatic cleanup, matching the ergonomics of Ruby and Python bindings.

---

## Common Pitfalls and How We Avoid Them

### 1. Memory Management Simplified

**With napi-rs**: No manual memory management needed

```typescript
// napi-rs - automatic memory management
pollStepEvents(): FfiStepEvent[] {
  return this.ffi.pollStepEvents();  // Returns JS array directly
  // GC handles cleanup automatically
}
```

**How napi-rs avoids leaks**: All Rust objects are wrapped in Node-API handles that are automatically released when JavaScript GC runs. No manual free() calls needed.

### 2. No Double-Free Issues

**With napi-rs**: Node-API reference counting prevents double-free

```typescript
// napi-rs - safe to call multiple times
const status1 = this.ffi.getWorkerStatus();
const status2 = this.ffi.getWorkerStatus();
// Both are independent JS objects, GC handles cleanup
```

**How napi-rs prevents this**: Each call returns a new JavaScript object. The Rust side doesn't track individual object lifetimes - Node-API handles reference counting automatically.

### 3. No Use-After-Free

**With napi-rs**: JavaScript objects are independent of Rust memory

```typescript
// napi-rs - safe object access
const status = this.ffi.getWorkerStatus();
// status is a pure JavaScript object
// Rust side has already transferred ownership to JavaScript
// Safe to use indefinitely until JS GC collects it
```

**How napi-rs prevents this**: Data is fully copied from Rust to JavaScript during the napi-rs conversion. JavaScript objects don't reference Rust memory.

---

## Pattern in Practice

### Example: Worker Status

```typescript
getWorkerStatus(): WorkerStatus {
  // napi-rs handles everything automatically
  return this.ffi.getWorkerStatus();
  // Returns JS object directly, GC handles cleanup
}
```

### Example: Polling Step Events

```typescript
pollStepEvents(): FfiStepEvent[] {
  // napi-rs returns JS array directly
  return this.ffi.pollStepEvents();
  // Empty array returned if no events, GC handles cleanup
}
```

### Example: Bootstrap Worker

```typescript
bootstrapWorker(config: BootstrapConfig): BootstrapResult {
  // napi-rs accepts JS object directly and returns JS object
  return this.ffi.bootstrapWorker(config);
  // No JSON serialization, no manual memory management
}
```

---

## Memory Lifetime Diagrams

### Successful Pattern

```
Time →

JavaScript:    [allocate ptr] → [read data] → [free ptr] → [use data]
Rust Memory:   [allocated]    → [allocated] → [freed]    → [freed]
JS Objects:    [none]         → [created]   → [exists]   → [exists]
                                  ↑
                            Data copied here
```

### Memory Leak (Anti-Pattern)

```
Time →

JavaScript:    [allocate ptr] → [read data] → [use data] → ...
Rust Memory:   [allocated]    → [allocated] → [LEAK]     → [LEAK]
JS Objects:    [none]         → [created]   → [exists]   → [exists]
                                                ↑
                                    Forgot to free! Memory leaked
```

### Use-After-Free (Anti-Pattern)

```
Time →

JavaScript:    [allocate ptr] → [free ptr] → [read ptr] → CRASH!
Rust Memory:   [allocated]    → [freed]    → [freed]
JS Objects:    [none]         → [none]     → [CORRUPT]
                                              ↑
                                    Reading freed memory!
```

---

## Best Practices

### 1. Trust Automatic Memory Management

```typescript
// With napi-rs - no lifetime concerns
const result = this.getWorkerStatus();

// Safe to store references
this.cachedStatus = this.getWorkerStatus();  // GC handles it
```

### 2. Return Native JavaScript Objects

```typescript
// napi-rs - return JS objects freely
pollStepEvents(): FfiStepEvent[] {
  return this.ffi.pollStepEvents();
  // Returns JS array, safe to pass around
}

// Safe to return objects from any scope
getStatus(): WorkerStatus {
  return this.ffi.getWorkerStatus();
}
```

### 3. Handle Empty Results Naturally

```typescript
// napi-rs returns appropriate JavaScript types
const events = this.ffi.pollStepEvents();
if (events.length === 0) {
  return [];  // Empty array handled naturally
}
return events;
```

### 4. Document Return Types

```typescript
/**
 * Poll for step events from native module.
 *
 * @returns Array of FfiStepEvent objects (JavaScript managed)
 */
pollStepEvents(): FfiStepEvent[] {
  return this.ffi.pollStepEvents();
}
```

---

## Testing Memory Safety

### Rust Tests

Rust's test suite can verify FFI functions don't leak:

```rust
#[test]
fn test_status_no_leak() {
    let ptr = get_worker_status();
    assert!(!ptr.is_null());
    
    // Manually free to ensure it works
    free_rust_string(ptr);
    
    // If we had a leak, tools like valgrind or AddressSanitizer
    // would catch it
}
```

### TypeScript Tests

TypeScript tests verify correct behavior:

```typescript
test('status retrieval works correctly', () => {
  const ffi = new FfiLayer();

  // napi-rs handles memory automatically
  const status = ffi.getWorkerStatus();

  expect(status.running).toBeDefined();

  // Call multiple times - no leaks with napi-rs
  for (let i = 0; i < 100; i++) {
    ffi.getWorkerStatus();
  }
  // GC will clean up when objects are unreferenced
});
```

### Leak Detection Tools

- **Valgrind** (Linux): Detects memory leaks in Rust code
- **AddressSanitizer**: Detects use-after-free and double-free
- **Process memory monitoring**: Track RSS growth over time

---

## When in Doubt

**Golden Rule with napi-rs**: Trust the automatic memory management. napi-rs handles memory lifecycle via Node-API reference counting.

If you see a pattern like:

```typescript
const result = this.ffi.someFunction();
```

Remember:

1. `result` is a pure JavaScript object (not a pointer)
2. Memory is managed by JavaScript's GC (no manual free needed)
3. Safe to pass around, store, or return from any scope
4. napi-rs has already handled the Rust ↔ JavaScript conversion

If you're working with napi-rs, you're using the correct pattern automatically.

---

## References

- **napi-rs Documentation**: https://napi.rs/
- **Node-API Documentation**: https://nodejs.org/api/n-api.html
- **Rust napi-rs Guide**: https://napi.rs/docs/introduction/getting-started
- **docs/worker-crates/patterns-and-practices.md**: General worker patterns
