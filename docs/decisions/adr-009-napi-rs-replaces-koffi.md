# napi-rs Research Spike: Findings

**Branch**: `research/napi-rs-ffi-spike`
**Date**: 2026-02-16
**Status**: Complete — **GO recommendation**

## Executive Summary

napi-rs is a viable replacement for the koffi + C FFI approach in the TypeScript worker. It eliminates the entire class of TAS-283 "trailing input" bugs by removing JSON serialization and C string marshalling from the FFI boundary. The spike successfully:

1. Built a `.node` module with 14 exported functions
2. Loaded and ran in Bun without issues
3. Passed `clientCreateTask` with a native JS object (no trailing input)
4. Auto-generated correct TypeScript definitions with proper camelCase conversion
5. Introduced zero workspace dependency conflicts

**Recommendation**: Create a formal ticket to replace koffi with napi-rs.

## Detailed Findings

### 1. Bun Compatibility: CONFIRMED

The napi-rs `.node` module loads directly in Bun via `require()`:

```typescript
const lib = require("./tasker-ts-napi.darwin-arm64.node");
lib.getVersion();  // "0.1.3"
lib.healthCheck();  // true
```

Bun has native support for N-API modules. No polyfills or compatibility layers needed.

### 2. TAS-283 Bug Elimination: CONFIRMED

The critical test — `clientCreateTask()` with a complex nested object — works without any serialization:

```typescript
lib.clientCreateTask({
  name: "ecommerce_order_processing",
  namespace: "ecommerce_ts",
  version: "0.1.0",
  context: {
    order_id: "test-napi-123",
    customer_email: "test@napi-spike.com",
    items: [{ sku: "WIDGET-1", qty: 2, price: 29.99 }],
    // ... complex nested object
  },
  initiator: "napi-rs-spike-test",
  sourceSystem: "test-spike",
  reason: "Validating napi-rs eliminates trailing input bug",
});
```

With orchestration running, the request completed the full round-trip:

- `clientCreateTask({...})` → 404 "Task template not found" (expected — template doesn't exist)
- `clientListTasks({ limit: 5 })` → Returns 489 tasks with full pagination and typed objects
- `clientHealthCheck()` → `{ success: true, data: { healthy: true } }`

No "trailing input" error anywhere. The JS object crosses into Rust as a native `#[napi(object)]` struct — no JSON, no C strings, no trailing bytes.

### 3. Type Generation: EXCELLENT

napi-rs auto-generates `index.d.ts` with:

- **snake_case → camelCase**: Automatic field name conversion (`worker_id` → `workerId`)
- **Option\<T\> → T | undefined**: Proper nullable types
- **HashMap\<String, T\> → Record\<string, T\>**: Correct map types
- **Vec\<T\> → Array\<T\>**: Correct array types
- **serde_json::Value → any**: JS-native any type
- **Rust doc comments → JSDoc comments**: Documentation preserved

Sample generated types:

```typescript
export interface NapiStepEvent {
  eventId: string
  taskUuid: string
  stepUuid: string
  task: NapiTaskInfo
  workflowStep: NapiWorkflowStep
  stepDefinition: NapiStepDefinition
  dependencyResults: Record<string, NapiDependencyResult>
}

export declare function pollStepEvents(): NapiStepEvent | null
export declare function completeStepEvent(eventId: string, result: NapiStepResult): boolean
```

### 4. Dependency Analysis

#### Added Dependencies

| Crate | Version | Purpose | Conflicts |
|-------|---------|---------|-----------|
| `napi` | 2.16.17 | Core N-API bindings | None |
| `napi-derive` | 2.16.13 | Proc macros for `#[napi]` | None |
| `napi-build` | 2.3.1 | Build script helper | None |
| `napi-sys` | 2.4.0 | Raw N-API FFI bindings | None |
| `convert_case` | 0.6.0 | snake→camelCase conversion | None |
| `ctor` | 0.2.9 | Module init registration | None |

**Total new transitive dependencies**: ~6 crates. No conflicts with existing workspace dependencies.

#### Removed Dependencies (vs koffi approach)

The napi-rs approach eliminates the need for:

- `koffi` npm package (JavaScript side)
- Manual `free_rust_string()` calls
- JSON `{success, error}` envelope pattern
- `serde_json::Deserializer::from_str` workaround for trailing bytes

### 5. Build Complexity

| Aspect | koffi (current) | napi-rs (spike) |
|--------|-----------------|-----------------|
| Crate type | `cdylib` | `cdylib` |
| Build command | `cargo build --release` | `npx napi build --release --platform` |
| Output | `.dylib/.so` | `.node` (per-platform) |
| Platform naming | Manual | Automatic (`darwin-arm64`, `linux-x64`, etc.) |
| TypeScript types | Manual ts-rs + export_bindings test | Auto-generated `index.d.ts` |
| npm packaging | Manual binary distribution | napi-rs handles platform packages |

napi-rs's platform-aware build system is actually simpler for npm distribution.

### 6. Performance Characteristics

| Aspect | koffi (current) | napi-rs (spike) |
|--------|-----------------|-----------------|
| Call overhead | C FFI + JSON ser/de | N-API native object conversion |
| Memory | Manual `free_rust_string()` | Automatic (V8/Bun GC) |
| String handling | C strings (null-terminated) | N-API strings (length-prefixed) |
| Object passing | JSON serialize → C string → JSON parse | Direct field-by-field conversion |

N-API object conversion is faster than JSON serialization for structured data, though both are fast enough that the difference is unlikely to be measurable in practice. The real win is correctness, not performance.

### 7. Code Comparison

#### Before (koffi): Creating a task

```typescript
// TypeScript side
const requestJson = JSON.stringify(taskRequest);
const resultPtr = lib.client_create_task(requestJson);
const resultStr = resultPtr.readString();
lib.free_rust_string(resultPtr);
const result = JSON.parse(resultStr);
if (!result.success) throw new Error(result.error);
return result.data;
```

```rust
// Rust side
pub extern "C" fn client_create_task(request_json: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(request_json) };
    let json_str = c_str.to_str().unwrap();
    // ↑ BUG: koffi may include trailing bytes in the C string buffer
    let mut deserializer = serde_json::Deserializer::from_str(json_str);
    // ↑ WORKAROUND: still fails (TAS-283)
    // ... serialize result to JSON, convert to C string, return pointer
}
```

#### After (napi-rs): Creating a task

```typescript
// TypeScript side
const result = lib.clientCreateTask({
  name: "order_processing",
  namespace: "ecommerce",
  version: "0.1.0",
  context: { order_id: "123" },
  initiator: "user",
  sourceSystem: "web",
  reason: "New order",
});
// result is a typed NapiClientResult — no JSON.parse, no free_rust_string
```

```rust
// Rust side
#[napi]
pub fn client_create_task(request: NapiTaskRequest) -> Result<NapiClientResult> {
    // request fields are already native Rust types — no JSON parsing
    let task_request = TaskRequest {
        name: request.name,
        context: request.context,  // serde_json::Value from JS object directly
        // ...
    };
    // Return typed object — no JSON serialization, no C string allocation
}
```

### 8. napi-rs as Single FFI Foundation

#### The Current Multi-Runtime Architecture

The existing TypeScript worker (`workers/typescript/`) has a multi-layer runtime abstraction:

```
TypeScript Public API (WorkerServer, StepHandler, TaskerClient)
    └── FfiLayer (src/ffi/ffi-layer.ts) — runtime detection + dispatch
        ├── NodeRuntime (src/ffi/node-runtime.ts) — koffi, used by Bun AND Node.js
        └── DenoRuntime (src/ffi/deno-runtime.ts) — Deno.dlopen
```

**Runtime detection** (`src/ffi/runtime.ts`) inspects globals at startup:

- `'Bun' in globalThis` → Bun
- `'Deno' in globalThis` → Deno
- `process.versions.node` → Node.js

Both Bun and Node.js use `NodeRuntime` (koffi via Node-API). Deno uses its own `DenoRuntime` with `Deno.dlopen`. The `FfiLayer` class abstracts this, discovering the correct runtime and loading the appropriate adapter.

#### Why napi-rs Should Replace the Entire Layer

napi-rs targets **N-API** — the stable, ABI-compatible native addon interface. N-API is supported by:

| Runtime | N-API Support | Status |
|---------|--------------|--------|
| **Bun** | Native (bun:ffi + N-API) | Primary runtime, tested in spike |
| **Node.js** | Native (since v8.0) | N-API was designed for Node.js |
| **Deno** | Via `--unstable-node-api` flag, or via `npm:` specifiers | Deno 2.x has improved N-API compat |

This means a single `.node` binary serves all three runtimes. The current architecture's runtime introspection, `NodeRuntime`/`DenoRuntime` split, and koffi dependency all become unnecessary.

#### Proposed Simplified Architecture

```
TypeScript Public API (WorkerServer, StepHandler, TaskerClient)
    └── Direct require() of .node module — no abstraction layer needed
```

**What gets deleted**:

- `src/ffi/runtime.ts` — Runtime detection (no longer needed)
- `src/ffi/ffi-layer.ts` — Runtime dispatch abstraction (no longer needed)
- `src/ffi/node-runtime.ts` — koffi wrapper (~250 lines of manual FFI function definitions)
- `src/ffi/deno-runtime.ts` — Deno.dlopen wrapper (~250 lines)
- `src/ffi/shims.d.ts` — Deno type shims
- `deno.json` — Deno-specific configuration
- `koffi` from `optionalDependencies`
- All `free_rust_string()` calls in the TypeScript codebase
- All JSON envelope parsing (`{success, error}` unwrapping)
- The `ts-rs` dev-dependency and `export_bindings` test (napi-rs generates types automatically)

**What stays unchanged**:

- `src/index.ts` — Public API exports
- `src/worker-server.ts` — WorkerServer class
- `src/handlers/` — StepHandler base class, handler registry
- `src/client/` — TaskerClient (rewired to call napi-rs directly)
- `src/events/` — Event system

**What gets simplified**:

- `src/ffi/index.ts` — Thin re-export of the `.node` module's auto-generated types
- Loading: `const native = require('./tasker-ts-napi.<platform>.node')` or napi-rs's built-in loader

#### Deno Compatibility Assessment

**Current state**: Deno support via `DenoRuntime` uses `Deno.dlopen` with the same `.dylib/.so` as koffi. This is a completely separate code path from Bun/Node.

**With napi-rs**: Deno's N-API support has matured significantly:

- Deno 2.x supports N-API natively via `--unstable-node-api` or when importing `npm:` packages
- The `@napi-rs/cli` toolchain generates `.node` files that Deno can load
- However, Deno's N-API is still marked unstable for direct `.node` loading

**Recommendation**: **Drop the dedicated `DenoRuntime` adapter.** Deno users can:

1. Use Deno's `npm:` specifier to import `@tasker-systems/tasker` (N-API works transparently)
2. Use `--unstable-node-api` flag for direct `.node` loading
3. The current `DenoRuntime` with `Deno.dlopen` has the same C FFI problems as koffi anyway

This is a net simplification — one code path instead of two, no runtime introspection, no conditional imports.

#### Type Generation Consolidation

Currently, TypeScript types are generated via a two-step process:

1. Rust DTOs in `src-rust/dto.rs` with `#[cfg_attr(test, derive(TS))]`
2. `cargo test export_bindings --package tasker-ts` generates `.ts` files to `src/ffi/generated/`
3. `src/ffi/types.ts` manually re-exports with API-friendly names

With napi-rs, this entire pipeline is replaced:

1. `#[napi(object)]` structs in Rust are the single source of truth
2. `npx napi build` auto-generates `index.d.ts` with all types
3. No manual re-export step, no separate `generated/` directory

The auto-generated types also get proper camelCase conversion for free, matching JavaScript conventions without any manual `#[serde(rename)]` annotations.

---

### 9. CI and Release Pipeline Impact

#### Current Artifact Flow

```
build-ffi-libraries.yml (matrix: linux-x64, darwin-arm64)
  ├── Docker build → libtasker_ts-x86_64-unknown-linux-gnu.so
  └── Native build → libtasker_ts-aarch64-apple-darwin.dylib
          ↓
release.yml: publish-typescript job
  ├── Download artifacts → workers/typescript/native/
  ├── bun install && bun run build
  └── npm publish @tasker-systems/tasker
```

Key files:

- `.github/workflows/build-ffi-libraries.yml` — Cross-platform matrix builds
- `.github/workflows/release.yml` (lines 419-497) — npm publish job
- `scripts/ffi-build/build-typescript.sh` — `cargo build -p tasker-ts --release`
- `scripts/release/publish-typescript.sh` — Version check + `npm publish`
- `docker/build/ffi-builder.Dockerfile` — Linux build container

#### What Changes with napi-rs

| Component | Current (koffi) | After (napi-rs) | Notes |
|-----------|----------------|-----------------|-------|
| **Rust crate** | `tasker-ts` (cdylib → `.so/.dylib`) | `tasker-ts-napi` (cdylib → `.node`) | Same crate type, different output |
| **Build command** | `cargo build -p tasker-ts --release` | `npx napi build --release --platform` | napi CLI handles platform naming |
| **Output naming** | Manual: `libtasker_ts-linux-x64.so` | Automatic: `tasker-ts-napi.linux-x64-gnu.node` | napi-rs convention |
| **Bundle location** | `native/libtasker_ts-*.{so,dylib}` | `tasker-ts-napi.*.node` at package root | napi-rs standard layout |
| **Platform detection** | `src/ffi/ffi-layer.ts` + `BUNDLED_LIBRARIES` map | napi-rs built-in `loadBinding()` | Eliminates manual path resolution |
| **npm dependency** | `koffi` (optionalDependency) | `@napi-rs/cli` (devDependency only) | koffi removed from production |
| **Type generation** | `ts-rs` + `cargo test export_bindings` | Automatic during `npx napi build` | One fewer build step |

#### Workflow Changes Required

**`build-ffi-libraries.yml`**:

```diff
# Build script change
- cargo build -p tasker-ts --release --locked
+ cd workers/typescript-napi && npx napi build --release --platform --target $TARGET
```

The matrix (linux-x64, darwin-arm64) stays the same. Output artifacts change from `.so/.dylib` to `.node`.

**`release.yml` publish-typescript job**:

```diff
# Bundle step — same pattern, different file names
- mkdir -p workers/typescript/native
- cp ffi-artifacts/typescript/libtasker_ts-x86_64-unknown-linux-gnu.so \
-    workers/typescript/native/libtasker_ts-linux-x64.so
- cp ffi-artifacts/typescript/libtasker_ts-aarch64-apple-darwin.dylib \
-    workers/typescript/native/libtasker_ts-darwin-arm64.dylib
+ # napi-rs .node files go at package root (loader expects them there)
+ cp ffi-artifacts/typescript/*.node workers/typescript/
```

No changes to OIDC, npm environment, or publish command — still `npm publish` of a single `@tasker-systems/tasker` package.

**`test-typescript-framework.yml`**:

- Remove Node.js and Deno FFI test steps (single runtime path)
- Simplify to: `bun test` (one command, one runtime)
- Client API tests unchanged

**`build-workers.yml` TypeScript job**:

```diff
- cargo make build-ffi  # cargo build -p tasker-ts
+ cd workers/typescript-napi && npx napi build --platform  # debug build for tests
```

**Docker production build** (`typescript-worker.prod.Dockerfile`):

```diff
- cargo build -p tasker-ts --release --locked
- ENV TASKER_FFI_LIBRARY_PATH=/app/lib/libtasker_ts.so
+ cd workers/typescript-napi && npx napi build --release --platform
+ # .node file discovered automatically by napi-rs loader, no env var needed
```

#### npm Distribution: Single Package with Bundled Binaries

napi-rs supports two distribution models:

1. **Platform packages** (separate `@org/pkg-linux-x64-gnu`, etc. as `optionalDependencies`)
2. **Single package** with `.node` files bundled alongside `index.js`

We use **approach 2** — the same strategy as our current `native/` directory approach, keeping everything in `@tasker-systems/tasker`. This avoids the significant overhead of platform packages, each of which would require its own unique OIDC trusted publishing setup (`(org, repo, workflow, environment)` tuple) in GitHub Actions and npm.

The napi-rs auto-generated `index.js` loader already supports this natively via a **dual resolution strategy**:

```javascript
// Generated by napi-rs — checks local file FIRST, falls back to platform package
case 'darwin':
  switch (arch) {
    case 'arm64':
      localFileExisted = existsSync(join(__dirname, 'tasker-ts-napi.darwin-arm64.node'))
      if (localFileExisted) {
        nativeBinding = require('./tasker-ts-napi.darwin-arm64.node')  // ← bundled
      } else {
        nativeBinding = require('@tasker-systems/tasker-darwin-arm64')  // ← never used
      }
```

Since the `.node` files are co-located in the package directory, the loader finds them locally and never attempts the optional dependency fallback. This is functionally identical to our current `native/` directory strategy:

```
# Current (koffi)                        # After (napi-rs)
@tasker-systems/tasker                    @tasker-systems/tasker
├── dist/                                 ├── dist/
├── native/                               ├── tasker-ts-napi.linux-x64-gnu.node
│   ├── libtasker_ts-linux-x64.so        ├── tasker-ts-napi.darwin-arm64.node
│   └── libtasker_ts-darwin-arm64.dylib  ├── index.js          (auto-generated loader)
└── package.json                          ├── index.d.ts        (auto-generated types)
                                          └── package.json
```

**What changes vs current approach**:

- The `native/` directory goes away — `.node` files live at package root (napi-rs convention)
- Platform resolution moves from our hand-written `FfiLayer.discoverLibraryPath()` to napi-rs's generated `index.js`
- The `TASKER_FFI_LIBRARY_PATH` environment variable override is no longer needed (napi-rs loader handles it)
- Same OIDC setup, same single `npm publish`, same `release.yml` — no new packages to configure

**Release artifact flow** stays parallel to what we have:

```
build-ffi-libraries.yml (matrix: linux-x64, darwin-arm64)
  ├── Docker: npx napi build --release --platform --target x86_64-unknown-linux-gnu
  │     → tasker-ts-napi.linux-x64-gnu.node
  └── Native: npx napi build --release --platform --target aarch64-apple-darwin
        → tasker-ts-napi.darwin-arm64.node
          ↓
release.yml: publish-typescript job
  ├── Download artifacts → cp *.node workers/typescript/
  ├── bun install && bun run build
  └── npm publish @tasker-systems/tasker   (single package, same OIDC)
```

#### Files Changed or Removed

| File | Change | Reason |
|------|--------|--------|
| `scripts/ffi-build/build-typescript.sh` | Update | `cargo build` → `npx napi build` |
| `cargo-make/scripts/ci-restore-typescript-artifacts.sh` | Simplify | `.node` files are self-contained (no lib prefix, no extension mapping) |
| `workers/typescript/deno.json` | Delete | No dedicated Deno adapter |
| `test-typescript-framework.yml` | Simplify | Remove multi-runtime test matrix (Node/Deno steps), keep Bun |
| `docker/build/typescript-worker.prod.Dockerfile` | Simplify | Remove `TASKER_FFI_LIBRARY_PATH` env var, napi-rs loader handles resolution |

---

### 10. Migration Path

The migration is a direct replacement, not incremental. The koffi FFI layer is broken (TAS-283) and the public TypeScript API (`WorkerServer`, `StepHandler`, `TaskerClient`) doesn't change — only the internal FFI plumbing.

**Phase 1: Replace FFI crate** (Rust side)

1. Rename/replace `workers/typescript/src-rust/` with napi-rs implementation
2. Update `Cargo.toml`: remove `cdylib` C FFI, add napi dependencies
3. Port all functions from C FFI signatures to `#[napi]` functions
4. Delete `conversions.rs` (JSON conversion helpers) — no longer needed
5. Delete `dto.rs` — replaced by `#[napi(object)]` structs that auto-generate TypeScript types

**Phase 2: Simplify TypeScript layer**

1. Delete `src/ffi/runtime.ts`, `ffi-layer.ts`, `node-runtime.ts`, `deno-runtime.ts`
2. Delete `src/ffi/generated/` directory and `ts-rs` binding generation
3. Add napi-rs module loader (one line: `const native = require('./index.node')` or use `@napi-rs/cli` generated loader)
4. Rewire `WorkerServer`, `TaskerClient`, event system to call napi-rs functions directly
5. Remove all `JSON.parse`/`JSON.stringify` at the FFI boundary
6. Remove all `free_rust_string()` calls
7. Remove `koffi` from `optionalDependencies`

**Phase 3: Update CI and release**

1. Update `build-ffi-libraries.yml` to use `npx napi build`
2. Update `release.yml` to use napi-rs platform package publishing
3. Simplify `test-typescript-framework.yml` to single-runtime tests
4. Update Docker builds

**Phase 4: Cleanup**

1. Remove `workers/typescript-napi/` spike directory
2. Update documentation

### 11. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| napi-rs version churn | Low | Pin napi v2, mature ecosystem (SWC, Rollup, Parcel use it) |
| Bun N-API compatibility gaps | Low | Tested in spike, Bun team actively maintains N-API |
| Build complexity for CI | Low | `npx napi build` handles platform detection automatically |
| Deno N-API gaps | Low | Deno 2.x N-API is stable for npm packages; dedicated adapter was more fragile |
| Platform package publishing | Low | Well-documented napi-rs workflow; used by major projects |

### 12. What This Spike Did NOT Test

- Multi-platform builds (only tested darwin-arm64)
- napi-rs platform package publishing (`npx napi prepublish`)
- Long-running event loop (poll/complete cycle under load)
- Concurrent access patterns (multiple JS threads)
- Memory leak detection under sustained use
- Deno loading the `.node` module via `npm:` specifier

These should be tested during formal implementation.

---

## Files Created

```
workers/typescript-napi/
├── Cargo.toml          # napi + workspace deps
├── build.rs            # napi-build setup
├── package.json        # @napi-rs/cli tooling
├── src/
│   ├── lib.rs          # Module entry (get_version, health_check)
│   ├── bridge.rs       # Worker lifecycle + poll/complete (14 napi object types)
│   ├── client_ffi.rs   # Client API (clientCreateTask — THE bug test)
│   └── error.rs        # Error types → JS exceptions
├── test-spike.ts       # Bun test script
├── index.d.ts          # Auto-generated TypeScript definitions
├── tasker-ts-napi.darwin-arm64.node  # Built binary
└── RESEARCH.md         # This document
```

## Conclusion

**GO**: napi-rs should replace koffi for the TypeScript FFI layer. It brings TypeScript to parity with Ruby (magnus) and Python (pyo3):

| Aspect | Before (koffi) | After (napi-rs) | Ruby (magnus) | Python (pyo3) |
|--------|---------------|-----------------|---------------|---------------|
| Type conversion | Manual JSON | Native objects | serde_magnus | pythonize |
| Memory mgmt | Manual free | Automatic (GC) | Automatic (GC) | Automatic (GC) |
| Error handling | JSON envelope | JS exceptions | Ruby exceptions | Python exceptions |
| String bugs | TAS-283 | Eliminated | None | None |
| Type generation | Manual ts-rs | Auto index.d.ts | N/A | N/A |
| Runtime adapters | 2 (koffi + Deno.dlopen) | 1 (N-API) | 1 (magnus) | 1 (pyo3) |
| Runtime detection | Required (3-way branch) | Not needed | Not needed | Not needed |
| Code to maintain | ~500 lines FFI wrappers | ~0 lines (auto-generated) | ~0 lines | ~0 lines |

The migration is a direct replacement — no dual-support phase needed. The public TypeScript API (`WorkerServer`, `StepHandler`, `TaskerClient`) is unchanged; only the FFI plumbing underneath is swapped. The koffi layer is broken (TAS-283), so there's no value in keeping it around.
