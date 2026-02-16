# Plan: Example Apps Testing, Background Jobs, and Quickstart Docs

## Context

The tasker-contrib example apps (bun, fastapi, rails, axum) have integration tests that verify HTTP endpoints return correct responses, but never verify that Tasker tasks actually complete or that step handlers produce expected results. We need to:
1. Validate current tests pass with live infrastructure
2. Add completion verification tests using client wrapper polling
3. Add minimal background job integration demos (one async endpoint per app)
4. Write framework-specific quickstart guides

All work is in **tasker-contrib** (`/Users/petetaylor/projects/tasker-systems/tasker-contrib/`).

## PR Strategy

- **PR 1**: Phases 1 + 2 (test fixes + completion verification)
- **PR 2**: Phase 3 (background job demos)
- **PR 3**: Phase 4 (quickstart docs)

---

## Phase 1: Ensure Current Tests Pass (Sequential)

Work through each app one at a time: start it, run its tests, fix failures, then move on.

### Order of operations
1. Start shared infrastructure: `docker compose -f examples/docker-compose.yml up -d`
2. Wait for orchestration healthcheck
3. **Rails** (10 tests): `cd examples/rails-app && bundle exec rspec spec/integration/`
   - In-process tests, no external server needed
   - Worker bootstrapped in `before(:all)` via `TaskerCore::Worker::Bootstrap.start!`
4. **FastAPI** (16 tests): `cd examples/fastapi-app && pytest tests/ -v`
   - Uses ASGI transport (in-process), no external server needed
   - Worker bootstrapped in conftest.py fixture
5. **Bun** (16 tests): `cd examples/bun-app && bun run dev` then `bun test`
   - Tests hit external server at `localhost:3002`
   - Worker bootstrapped at app startup
6. **Axum** (5 tests): `cd examples/axum-app && cargo run` then `cargo test`
   - Tests hit external server at `localhost:3000`
   - Worker bootstrapped at app startup

### Potential issues to watch for
- Database schema: init-db.sql creates `example_{rails,fastapi,bun,axum}` DBs but app migrations may not have run
- FFI library loading: `TASKER_FFI_LIBRARY_PATH` may need to be set for TypeScript
- Version mismatches between installed packages and orchestration image
- The `ORCHESTRATION_URL` for task creation (each app's code may assume different defaults)

### Files potentially modified
- `examples/docker-compose.yml` — if infra adjustments needed
- Individual `.env` files — if connection strings need updating
- App-specific config or startup files — only as needed to fix failures

---

## Phase 2: Task Completion & Step Result Verification

### 2A. Polling Helpers (one per app)

Each helper polls `GET /v1/tasks/{uuid}` on the orchestration API until `status == "complete"` or timeout.

| App | Helper File | Pattern |
|-----|-------------|---------|
| Bun | `examples/bun-app/tests/helpers.ts` | `fetch()` with async/await polling loop |
| FastAPI | `examples/fastapi-app/tests/helpers.py` | `httpx.AsyncClient` polling loop |
| Rails | `examples/rails-app/spec/support/task_polling.rb` | `Net::HTTP` polling module included in specs |
| Axum | `examples/axum-app/tests/helpers.rs` or inline in `integration.rs` | `reqwest` async polling |

Each helper provides:
- `wait_for_task_completion(task_uuid, max_wait=30s, poll_interval=1s)` — returns TaskResponse or raises/panics
- `get_task_steps(task_uuid)` — returns step list
- `get_step(task_uuid, step_uuid)` — returns step with results

Environment variable: `ORCHESTRATION_URL` (default `http://localhost:8080`)

### 2B. New Test Cases

For each app, add to existing test files:

**E-commerce Completion Test** (all apps):
1. POST to create order
2. Extract `task_uuid`
3. `wait_for_task_completion(task_uuid)`
4. Assert task status == "complete", completion_percentage == 100
5. Get steps, assert 5 steps all "complete"
6. Get a specific step (e.g. validate_cart), verify results contain expected output fields

**Analytics Pipeline Completion Test** (all apps):
1. POST to create analytics job
2. Extract `task_uuid`
3. `wait_for_task_completion(task_uuid)`
4. Get steps, assert 8 steps all "complete"
5. Verify the 3 extract steps ran (parallel dependency resolution)

### Files to create/modify

| File | Action |
|------|--------|
| `examples/bun-app/tests/helpers.ts` | Create |
| `examples/bun-app/tests/integration.test.ts` | Add 2-4 completion tests |
| `examples/fastapi-app/tests/helpers.py` | Create |
| `examples/fastapi-app/tests/test_workflows.py` | Add 2-4 completion tests |
| `examples/rails-app/spec/support/task_polling.rb` | Create |
| `examples/rails-app/spec/integration/workflows_spec.rb` | Add 2-4 completion tests |
| `examples/axum-app/tests/integration.rs` | Add helper fns + 2-4 completion tests |

### Verification
- Run each app's tests with infrastructure up
- Completion tests should pass within 30s timeout
- If a test times out, it means handlers aren't dispatching or worker isn't connected

---

## Phase 3: Background Job Integration (Minimal Demo)

One async endpoint per app showing: HTTP request -> background job -> Tasker task creation.

### Rails — ActiveJob with `:async` adapter

| File | Action |
|------|--------|
| `examples/rails-app/app/jobs/create_tasker_task_job.rb` | Create — wraps `TaskerCore::Client.create_task` |
| `examples/rails-app/config/application.rb` | Add `config.active_job.queue_adapter = :async` |
| `examples/rails-app/app/controllers/orders_controller.rb` | Add `create_async` action |
| `examples/rails-app/config/routes.rb` | Add `post 'orders/async', to: 'orders#create_async'` |
| `examples/rails-app/spec/integration/workflows_spec.rb` | Add async path test |

### FastAPI — `asyncio.create_task()`

| File | Action |
|------|--------|
| `examples/fastapi-app/app/background.py` | Create — async task creation wrapper |
| `examples/fastapi-app/app/routes/orders.py` | Add `POST /orders/async` endpoint |
| `examples/fastapi-app/tests/test_workflows.py` | Add async path test |

### Bun — `queueMicrotask` / deferred execution

| File | Action |
|------|--------|
| `examples/bun-app/src/background.ts` | Create — deferred task creation helper |
| `examples/bun-app/src/routes/orders.ts` | Add `/orders/async` route |
| `examples/bun-app/tests/integration.test.ts` | Add async path test |

### Axum — `tokio::spawn`

| File | Action |
|------|--------|
| `examples/axum-app/src/background.rs` | Create — spawn task creation |
| `examples/axum-app/src/routes/orders.rs` | Add `/orders/async` handler |
| `examples/axum-app/src/main.rs` | Add `mod background;` and route |
| `examples/axum-app/tests/integration.rs` | Add async path test |

### Pattern across all apps
- Async endpoint returns `202 Accepted` with `{ id, status: "queued" }`
- Background job creates Tasker task and updates domain record
- Test: POST to async endpoint, wait briefly, GET record to verify task_uuid populated
- Optionally: poll for task completion to prove end-to-end works

---

## Phase 4: Quickstart Documentation

Location: `tasker-contrib/docs/quickstart/`

### Files to create

| File | Content |
|------|---------|
| `docs/quickstart/README.md` | Index linking to all 4 guides |
| `docs/quickstart/rails.md` | Rails quickstart (~250 lines) |
| `docs/quickstart/fastapi.md` | FastAPI quickstart (~250 lines) |
| `docs/quickstart/bun.md` | Bun/TypeScript quickstart (~250 lines) |
| `docs/quickstart/axum.md` | Axum/Rust quickstart (~250 lines) |

### Each guide covers
1. **Prerequisites** — language runtime, Docker, tasker-ctl
2. **Start Infrastructure** — `docker compose -f examples/docker-compose.yml up -d`
3. **Generate Configuration** — `tasker-ctl config generate --context worker --environment development --source-dir config/tasker --output <path>/worker.toml`
4. **Scaffold a Handler** — `tasker-ctl template generate tasker-contrib-<lang>:step_handler --name ValidateCart`
5. **Define a Task Template** — YAML structure with steps, dependencies, handlers
6. **Run the App** — framework-specific startup
7. **Create a Task** — curl to app endpoint or orchestration API
8. **Verify Completion** — curl to `GET /v1/tasks/{uuid}`, explain the response
9. **Background Job Pattern** — reference Phase 3's async endpoint
10. **Next Steps** — link to architecture docs, handler patterns, DAG workflows

### References to include
- tasker-ctl plugin templates: `tasker-contrib/{rails,python,typescript,rust}/tasker-cli-plugin/`
- Example app source: `tasker-contrib/examples/{rails,fastapi,bun,axum}-app/`
- Core getting-started docs: `tasker-core/docs/getting-started/`
- Shared worker config: `tasker-contrib/examples/shared/tasker-worker.toml`

---

## Scope Summary

| Phase | New Files | Modified Files | ~Lines |
|-------|-----------|---------------|--------|
| 1 | 0 | 0-3 (fixes) | ~50 |
| 2 | 4 helpers | 4 test files | ~600-800 |
| 3 | 5-6 jobs/routes | 4-5 existing | ~400-500 |
| 4 | 5 docs | 0 | ~1200 |
| **Total** | **~15** | **~12** | **~2500** |

## Execution Order

```
Phase 1: Start each app sequentially, fix issues
    ↓
Phase 2: Add polling helpers + completion tests (same PR as Phase 1)
    ↓
Phase 3: Add background job demos (separate PR)
    ↓
Phase 4: Write quickstart docs referencing Phases 2 & 3 (separate PR)
```
