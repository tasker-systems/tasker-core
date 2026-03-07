# Virtual Handler Dispatch Architecture

*How grammar-composed virtual handlers execute within the existing worker infrastructure*

*March 2026 — Research Spike*

---

## The Core Insight

Virtual handlers are not "another kind of handler" that workers need to discover and register. They are **built-in infrastructure** — compiled into `tasker-worker` and `tasker-shared` — that every worker already has by virtue of being a Tasker worker. No FFI bridge, no language runtime, no handler registration required.

This means the composition executor is universally available. A Ruby worker, a Python worker, a TypeScript worker, and a native Rust worker can all execute virtual handler steps without any language-specific code. The composition spec *is* the handler — the executor interprets it at runtime.

---

## Current Dispatch Architecture

### How Steps Flow Today

```
TaskTemplate (namespace: "ecommerce", steps: [...])
        │
        ↓ (orchestrator enqueues steps to namespace queue)
        │
   worker_ecommerce_queue (PGMQ)
        │
        ↓ (worker polls namespace queue)
        │
   StepExecutorActor
        │ 1. Claims step atomically (SQL)
        │ 2. Hydrates TaskSequenceStep (task, step definition, dependency results)
        │ 3. Sends DispatchHandlerMessage (fire-and-forget)
        │
        ↓
   HandlerDispatchService
        │ 1. Acquires semaphore permit (bounds concurrency)
        │ 2. Looks up handler via registry.get(step)
        │ 3. Invokes handler.call(step) with timeout + panic catching
        │ 4. Releases permit
        │ 5. Sends StepExecutionResult to completion channel
        │ 6. Fires post-handler callback (domain events)
        │
        ↓
   StepExecutionResult → completion channel → orchestrator
```

### Handler Resolution Chain

Every FFI worker (Ruby, Python, TypeScript) and the Rust worker share an identical resolver pattern:

```
HandlerDefinition { callable, method, resolver }
        │
        ↓
   ResolverChain (priority-ordered)
        │
        ├─ [Priority 10]  ExplicitMappingResolver  ← registered key lookup
        ├─ [Priority 20-99] Custom resolvers        ← domain-specific patterns
        └─ [Priority 100] ClassConstant/Lookup      ← class path inference
        │
        ↓
   Resolved handler → MethodDispatchWrapper (if method != "call")
        │
        ↓
   handler.call(TaskSequenceStep) → StepExecutionResult
```

The `callable` field in `HandlerDefinition` is the lookup key. Its format varies by language:
- **Rust**: Explicit mapping key (`"validate_payment"`)
- **Ruby**: Class path (`"PaymentProcessing::ValidationHandler"`)
- **Python**: Module path (`"payment_processing.ValidationHandler"`)
- **TypeScript**: Class name or explicit key (`"validate_payment"`)
- **Custom**: Domain resolver pattern (`"payments:stripe:refund"`)

### What Workers Know

Workers register handlers at boot time. A Ruby worker discovers Ruby handler classes; a Python worker discovers Python handler modules. The `registered_callables()` method on each resolver returns what handlers that worker can process. This drives which steps the worker can claim — if the registry can resolve the step's `callable`, the worker handles it.

---

## Virtual Handler Dispatch: The New Path

### The Routing Decision

The composition executor becomes a new dispatch path alongside existing ones. The decision point is in `HandlerDispatchService` (or equivalently, in the `StepHandlerRegistry.get()` implementation):

```
HandlerDispatchService receives DispatchHandlerMessage
        │
        ↓
   Does this step have a composition spec?
        │
        ├─ YES → Route to CompositionExecutor (built-in, always available)
        │         CompositionExecutor processes the spec:
        │           1. Loads composition from StepDefinition
        │           2. Iterates capabilities in sequence
        │           3. Passes StepContext through the chain
        │           4. Produces StepExecutionResult
        │
        └─ NO  → Route to handler registry (existing path)
                  ResolverChain resolves callable → handler.call()
```

This is a **routing decision**, not a **capability registration**. The composition executor is always present — it doesn't need to be discovered, registered, or resolved. It's compiled into the worker binary.

### Where the Composition Spec Lives

Today, `StepDefinition` has:

```rust
pub struct StepDefinition {
    pub name: String,
    pub handler: HandlerDefinition,   // callable, method, resolver
    pub step_type: StepType,
    pub dependencies: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    // ...
}
```

A virtual handler step carries its composition spec as a first-class field on `StepDefinition`:

```rust
pub struct StepDefinition {
    pub name: String,
    pub handler: HandlerDefinition,
    pub composition: Option<CompositionSpec>,  // Grammar composition spec
    pub step_type: StepType,
    pub dependencies: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    // ...
}
```

When `composition` is `Some`, the dispatch service routes to the composition executor. The `handler` field can carry a sentinel value (e.g., `callable: "__composition__"`) or be ignored entirely — the composition spec is the handler.

This makes the intent visible at every level: in the TaskTemplate YAML (`composition:` vs `handler:`), in the hydrated `StepDefinition` (the field is present or absent), and in the dispatch routing (a simple `is_some()` check). The composition is a first-class concept, not metadata encoded in initialization data or resolver hints.

### What the Composition Executor Needs

The composition executor receives the same `TaskSequenceStep` that any handler receives:

```rust
pub struct TaskSequenceStep {
    pub task: TaskForOrchestration,              // task metadata
    pub workflow_step: WorkflowStepWithName,     // step metadata
    pub dependency_results: StepDependencyResultMap, // upstream results
    pub step_definition: StepDefinition,         // includes composition spec
}
```

This is everything the executor needs:
- **Task context**: `task.source_input` provides the original task request data
- **Dependency results**: `dependency_results` provides outputs from upstream steps
- **Composition spec**: `step_definition.composition` provides the capability chain
- **Checkpoint data**: Available through `CheckpointService` (see `checkpoint-generalization.md`)

The executor iterates through the composition's capabilities, threading the output of each capability as input to the next. The final capability's output becomes the `StepExecutionResult.result` — the `result_schema` contract.

### The Output Contract

The composition executor produces a standard `StepExecutionResult`:

```rust
StepExecutionResult {
    step_uuid: Uuid,
    success: bool,
    result: Value,       // ← conforms to step_definition.result_schema
    metadata: StepExecutionMetadata,
    status: String,      // "completed" | "failed" | "error"
    error: Option<StepExecutionError>,
}
```

The orchestrator doesn't know or care whether this result came from a Ruby class, a Python function, or a composition executor. It sees `StepExecutionResult` and processes it identically — evaluating dependencies, updating step state, advancing the workflow.

---

## Implications for Horizontal Scaling

### Universal Claimability

Today, step claimability is constrained by handler availability. A step with `callable: "PaymentProcessing::ValidationHandler"` can only be claimed by workers running the Ruby runtime with that class loaded. Workers on the same namespace queue that lack the Ruby handler will fail the registry lookup and produce "Handler not found" errors.

Virtual handler steps have no such constraint. The composition executor is compiled into every worker binary. **Any worker listening on the namespace queue can claim and execute a virtual handler step**, regardless of which FFI runtimes it has loaded.

This has concrete scaling implications:

```
Namespace: "ecommerce" — 5 workers listening
  Worker A: Rust-only     (can handle: virtual handler steps + Rust domain handlers)
  Worker B: Ruby FFI      (can handle: virtual handler steps + Ruby domain handlers)
  Worker C: Python FFI    (can handle: virtual handler steps + Python domain handlers)
  Worker D: Rust-only     (can handle: virtual handler steps + Rust domain handlers)
  Worker E: TypeScript FFI(can handle: virtual handler steps + TS domain handlers)
```

A TaskTemplate with 8 steps — 5 virtual handlers and 3 domain handlers (Ruby) — distributes work efficiently:
- The 5 virtual handler steps are claimable by **all 5 workers**
- The 3 Ruby domain handler steps are claimable by **Worker B only**

Without virtual handlers, all 8 steps would need to be Ruby-resolvable, limiting the pool to 1 worker. With virtual handlers, 5 of 8 steps benefit from the full 5-worker pool.

### Queue Hotspot Reduction

In the current model, a namespace queue can become a bottleneck if all steps require a specific language runtime and only one or two workers have it loaded. Virtual handlers break this pattern — grammar-composed steps are universally claimable, spreading load across all workers on the namespace.

### No New Queue Infrastructure

Virtual handler steps use the same namespace queue as domain handler steps. A TaskTemplate's DAG mixes both types freely, and they all flow through `worker_{namespace}_queue`. The lifecycle (step states, dependency resolution, checkpointing, retry) is identical. No new queues, no new routing logic at the queue level.

---

## Composition Queues: Cross-Namespace Virtual Handler Pools

### The Insight

In the namespace-local model, a virtual handler step enqueued to `worker_ecommerce_queue` is claimable by all workers listening on `ecommerce` — but *only* those workers. A Python worker listening on `analytics` cannot claim it, even though it has the same built-in composition executor and could process the step identically.

This is wasteful. Virtual handler steps have no language or namespace affinity. Their execution is deterministic — the same composition spec produces the same result regardless of which worker runs it. The full pool of online workers should be available for virtual handler work.

### Dedicated Composition Queues

The orchestrator can route virtual handler steps to a set of **composition queues** that all workers subscribe to automatically, regardless of their namespace assignments:

```
Orchestrator enqueues step:
        │
        ├─ step_definition.composition.is_some() AND template allows composition queues?
        │     → Enqueue to worker_composition_queue (all workers listen)
        │
        └─ Domain handler step (or composition queues disabled for this template)
              → Enqueue to worker_{namespace}_queue (namespace workers only)
```

The composition queue pool could be a single queue or sharded for throughput:

```
worker_composition_queue_0    ← all workers listen (round-robin claim)
worker_composition_queue_1    ← all workers listen
worker_composition_queue_2    ← all workers listen
...
worker_composition_queue_N    ← shard count configurable
```

### What This Enables

```
Workers online:
  Worker A: [ecommerce]           ← listens on ecommerce + composition queues
  Worker B: [ecommerce, billing]  ← listens on ecommerce, billing + composition queues
  Worker C: [analytics]           ← listens on analytics + composition queues
  Worker D: [analytics]           ← listens on analytics + composition queues
  Worker E: [billing]             ← listens on billing + composition queues

TaskTemplate: process_order (namespace: ecommerce)
  Step: prepare_payment (virtual handler)
    → Enqueued to composition queue → claimable by Workers A, B, C, D, E (all 5)
  Step: process_payment (domain handler, Ruby)
    → Enqueued to worker_ecommerce_queue → claimable by Workers A, B (2)
  Step: create_order (virtual handler)
    → Enqueued to composition queue → claimable by Workers A, B, C, D, E (all 5)
```

Without composition queues, `prepare_payment` and `create_order` are limited to the 2 workers on `ecommerce`. With composition queues, they benefit from the full 5-worker pool. The `analytics` workers contribute to `ecommerce` workflow throughput without being assigned to the `ecommerce` namespace.

### Lifecycle Coherence

A natural concern: if steps from the same TaskTemplate are spread across different queues, does the lifecycle fragment?

**No.** The orchestrator already manages step lifecycle independently of queue routing. Step state transitions (Pending → Enqueued → InProgress → Complete), dependency resolution, retry semantics, and task finalization are all driven by the orchestrator's database-backed state machine — not by queue co-location. The queue is a delivery mechanism, not a coordination mechanism.

Consider the flow:
1. Orchestrator enqueues `prepare_payment` to composition queue
2. Worker C (analytics) claims it, executes, returns `StepExecutionResult`
3. `ResultProcessorActor` processes the result, marks step complete
4. Orchestrator evaluates dependencies — `process_payment` is now unblocked
5. Orchestrator enqueues `process_payment` to `worker_ecommerce_queue`
6. Worker A (ecommerce, Ruby) claims it, executes with `prepare_payment`'s result in `dependency_results`
7. Flow continues normally

The `dependency_results` are populated from the database, not from queue proximity. A step doesn't need to know which queue its dependency was claimed from. This is already how the system works — steps from the same task can be claimed by different workers on the same namespace queue. Composition queues simply expand the pool of eligible workers.

### Configurable Per-TaskTemplate

Composition queues should be the **default behavior** but configurable per-TaskTemplate for cases where namespace-local routing is preferred:

```yaml
name: process_order
namespace: ecommerce
version: "1.0.0"
# composition_queue: true (default — virtual handler steps use composition queues)
steps:
  - name: validate_cart
    composition: { ... }  # → routed to composition queue
  - name: process_payment
    handler: { ... }      # → routed to worker_ecommerce_queue
```

```yaml
name: sensitive_financial_workflow
namespace: finance_us
version: "1.0.0"
composition_queue: false  # Override — all steps stay on namespace queue
steps:
  - name: validate_transaction
    composition: { ... }  # → routed to worker_finance_us_queue (not composition queue)
  - name: execute_transfer
    handler: { ... }      # → routed to worker_finance_us_queue
```

**When to disable composition queues**:

| Reason | Example |
|--------|---------|
| **Domain handler priority** | Virtual handler steps on the composition queue compete with other namespaces' virtual handler work. If domain handler steps in the same workflow are latency-sensitive and the virtual handler prep work needs to complete quickly to unblock them, keeping everything on the namespace queue reduces contention. |
| **Compute isolation** | A namespace with resource-intensive compositions (large reshapes, complex computations) shouldn't steal capacity from other namespaces' workers. Disabling composition queues keeps the blast radius contained. |
| **Tenancy boundaries** | Multi-tenant deployments where namespace maps to tenant. Tenant A's virtual handler steps must not execute on Tenant B's workers, even though the composition executor is identical. Data isolation requirements override compute efficiency. |
| **Regulatory compliance** | Financial, healthcare, or government workloads where data must not leave designated compute boundaries, even transiently during grammar evaluation. |

### Worker Configuration

Workers subscribe to composition queues at boot based on their own configuration, alongside their namespace queues:

```toml
# worker.toml
[worker]
namespaces = ["ecommerce", "billing"]   # Domain handler queues
composition_queues = true                # Subscribe to composition queues (default: true)
# composition_queue_shards = 4          # How many composition queue shards to listen on
```

Setting `composition_queues = false` at the worker level opts that worker out of the shared composition pool — it only processes virtual handler steps that arrive on its namespace queues (from templates with `composition_queue: false`). This is a **self-regarding operational decision**: the worker is managing its own compute allocation, not influencing how the orchestrator routes steps.

### Routing Authority: Template Only

**The template is the sole authority on routing.** The orchestrator decides where to enqueue a step based on two things: whether the step has a composition spec, and what the template declares. The orchestrator never consults worker configuration, worker registration tables, or any form of worker-reported capability.

This is a deliberate architectural boundary. The orchestration system has no concept of "workers." A task request arrives, gets transformed into a DAG of steps based on the template, and steps are placed onto namespace queues. Results come back. The orchestrator doesn't know or care what processes those steps — it would function identically if someone hand-crafted a `StepExecutionResult` via a database query. There is no stateful experience of a "worker" in the orchestration model.

This means a `worker_registrations` table cannot solve the routing problem it appears to address:

1. **No routing mechanism**: The orchestrator routes steps by namespace. A worker_registrations table would inject worker-awareness into a system that has no concept of workers and no mechanism by which worker-reported preferences could guide a routing determination for a step. The orchestrator doesn't route to workers — it routes to queues.
2. **State leakage risk**: If instead we tried to have worker boot inject its `composition_queues` preference into the task template's persisted JSON in `named_tasks.configuration`, boot-order would indeterminately mutate the template's serialized config. With N workers potentially disagreeing, whatever booted last "wins" — silently changing routing behavior for all subsequent tasks with no error, no log entry, just a race condition.
3. **Heartbeat complexity**: Workers are ephemeral — they scale up, scale down, crash, restart. A heartbeat protocol requires managing decay-rate, stale registration cleanup, and TTL-based staleness windows, all for information the orchestrator cannot act on.

None of these are necessary. The routing model is simpler and safer:

- **Template says `composition_queue: false`** → namespace queue. This is the tenancy/compliance/isolation boundary — an absolute domain constraint declared by the template author.
- **Template says `composition_queue: true`** (default) → composition queue. The step is routed to the shared composition queue pool where any subscribed worker can claim it.

The "what if nobody is listening?" scenario is identical to the existing model for namespace queues: if no workers are listening on `worker_ecommerce_queue`, steps sit until a worker comes online. That's a deployment problem, not a routing problem. The answer for composition queues is the same.

### Deployment Prerequisite: Composition-Only Worker

For any Tasker deployment using action grammar compositions, **at least one composition-only worker must be deployed**. This is the deployment-level guarantee that composition queue steps will be processed. The composition-only worker exists specifically for this purpose — it subscribes to composition queues exclusively, has no domain handlers, and processes virtual handler steps from any namespace.

This prerequisite is equivalent to existing deployment requirements: you must deploy at least one worker per namespace queue you intend to process. If you define templates in the `ecommerce` namespace, you need workers listening on `worker_ecommerce_queue`. If you define templates with composition steps, you need workers listening on composition queues.

Domain workers (Ruby, Python, TypeScript, Rust) **may also** subscribe to composition queues via their `composition_queues = true` config. This is additive capacity — it means the domain worker will claim composition steps in addition to its domain handler work. Opting out (`composition_queues = false`) is a resource management decision: "this worker is dedicated to domain handler throughput and shouldn't spend cycles on cross-namespace composition work." The routing is unaffected either way — the orchestrator doesn't know or care which workers are listening.

### Orchestrator Routing Logic

The `StepEnqueuerActor` gains a routing decision when enqueuing steps:

```rust
fn queue_for_step(
    step: &StepDefinition,
    template: &TaskTemplate,
) -> QueueTarget {
    if step.composition.is_none() {
        // Domain handler step — always namespace queue
        return QueueTarget::NamespaceQueue {
            namespace: template.namespace_name.clone(),
        };
    }

    // Virtual handler step — template is sole routing authority
    if template.composition_queue_enabled() {
        QueueTarget::CompositionQueue {
            shard: hash(step.name) % composition_queue_shards(),
        }
    } else {
        // Template explicitly opted out — keep on namespace queue
        // (tenancy isolation, regulatory compliance, compute containment)
        QueueTarget::NamespaceQueue {
            namespace: template.namespace_name.clone(),
        }
    }
}
```

Note the signature: `(step, template)` — no `SystemContext`, no worker registry cache, no feasibility check. The routing decision is deterministic from the step definition and template configuration alone.

The shard selection can use consistent hashing on step name, round-robin, or least-loaded — the strategy is configurable and independent of the routing decision itself.

---

## The Resolver Chain: No Changes Needed

The resolver chain pattern (identical across Rust, Ruby, Python, TypeScript) requires **no changes**. The composition check happens before the resolver chain is ever consulted — the dispatch service checks `step_definition.composition.is_some()` and routes to the built-in composition executor. The resolver chain only runs for domain handler steps.

The existing chain remains unchanged:

1. **Priority 10**: ExplicitMappingResolver (registered handlers by key)
2. **Priority 20-99**: Custom resolvers (domain-specific patterns)
3. **Priority 100**: ClassConstant/ClassLookup resolver (class path inference)

No changes to Ruby's `ResolverChain`, Python's `ResolverChain`, or TypeScript's `ResolverChain`.

---

## TaskTemplate YAML: What Changes

A TaskTemplate that mixes virtual and domain handlers. Note: this uses the 6-capability model from `transform-revised-grammar.md`, where `reshape`, `compute`, `evaluate`, and `evaluate_rules` are unified into a single `transform` capability powered by jaq (jq) filters. The composition context envelope provides `.context` (task input), `.deps` (dependency results), `.step` (step metadata), and `.prev` (previous capability output).

```yaml
name: process_order
namespace: ecommerce
version: "1.0.0"
steps:
  # Virtual handler — grammar composition
  - name: validate_cart
    composition:
      grammar: Validate
      compose:
        - capability: validate
          config:
            schema:
              items: { type: array, required: true, min_items: 1 }
              customer_id: { type: string, required: true }
            coercion: strict
            unknown_fields: drop
            on_failure: fail
        - capability: transform
          output:
            type: object
            required: [items, customer_id, subtotal, tax, total]
            properties:
              items: { type: array }
              customer_id: { type: string }
              subtotal: { type: number }
              tax: { type: number }
              total: { type: number }
          filter: |
            .prev
            | . + {subtotal: ([.items[].price * .items[].quantity] | add)}
            | . + {tax: ((.subtotal * .context.tax_rate) * 100 | round / 100)}
            | . + {total: (.subtotal + .tax)}
    result_schema:
      items: { type: array }
      customer_id: { type: string }
      subtotal: { type: number }
      tax: { type: number }
      total: { type: number }

  # Domain handler — traditional callable
  - name: process_payment
    handler:
      callable: "PaymentProcessing::ChargeHandler"
    dependencies: [validate_cart]

  # Virtual handler — grammar composition
  - name: create_order
    composition:
      grammar: Persist
      compose:
        - capability: transform
          output:
            type: object
            required: [total, items, payment_id]
            properties:
              total: { type: number }
              items: { type: array }
              payment_id: { type: string }
          filter: |
            {
              total: .deps.validate_cart.total,
              items: .deps.validate_cart.items,
              payment_id: .deps.process_payment.payment_id
            }
        - capability: persist
          config:
            resource:
              type: database
              entity: orders
            constraints:
              unique_key: payment_id
            validate_success:
              order_id: { type: string, required: true }
            result_shape: [order_id, created_at]
          data: |
            {
              total: .prev.total,
              items: .prev.items,
              payment_id: .prev.payment_id
            }
          checkpoint: true
    dependencies: [validate_cart, process_payment]
    result_schema:
      order_id: { type: string }
      created_at: { type: string }

  # Virtual handler — grammar composition
  - name: confirm_order
    composition:
      grammar: Emit
      compose:
        - capability: transform
          output:
            type: object
            required: [order_id, customer_email, total]
            properties:
              order_id: { type: string }
              customer_email: { type: string }
              total: { type: number }
          filter: |
            {
              order_id: .deps.create_order.order_id,
              customer_email: .deps.validate_cart.customer_email,
              total: .deps.validate_cart.total
            }
        - capability: emit
          config:
            event_name: "order.confirmed"
            event_version: "1.0"
            delivery_mode: durable
            condition: success
          payload: |
            {
              order_id: .prev.order_id,
              customer_email: .prev.customer_email,
              total: .prev.total
            }
    dependencies: [create_order]
```

The structural signal is clear: steps with `composition:` are virtual handlers; steps with `handler:` are domain handlers. The TaskTemplate parser knows which dispatch path each step takes.

---

## Load Shedding and Concurrency

The existing TAS-75 load shedding infrastructure works identically for virtual handler steps. The `CapacityChecker` tracks semaphore permits — it doesn't distinguish between permits held by domain handler executions and permits held by composition executor executions.

One consideration: virtual handler steps that include `acquire` or `persist` capabilities perform I/O. The composition executor should respect the same timeout and concurrency bounds as domain handlers. Since the executor runs within the `HandlerDispatchService`'s semaphore-bounded task, this is automatic — the executor inherits the timeout and concurrency limits of the dispatch service.

For checkpoint-capable compositions (see `checkpoint-generalization.md`), the executor uses `CheckpointService` to save intermediate state between capabilities. This follows the same pattern as batch worker checkpoint yields — the infrastructure is already there.

---

## What This Means for Worker Development

### For Worker Authors

Nothing changes. Workers register domain handlers exactly as they do today. Virtual handler steps are invisible to the handler registration process — the composition executor handles them automatically.

### For TaskTemplate Authors

A new option: some steps can be expressed as grammar compositions instead of handler code. The TaskTemplate YAML gains a `composition:` field on `StepDefinition`. The author chooses per-step whether to use a virtual handler or a domain handler based on the guidelines established in the case studies:

- **Virtual handler**: When the step's logic can be fully expressed as sequences of (action, resource, context) triples
- **Domain handler**: When the step contains opaque domain logic (fraud checks, payment gateway APIs, organization-specific classification rules)

### For Platform Engineers

The composition executor is a new component in `tasker-worker` (or `tasker-shared` if the executor types are shared across crates). It implements the `StepHandler` trait or is invoked directly by the dispatch service. Key responsibilities:

1. Parse the `CompositionSpec` from the step definition
2. Initialize capability executors for each capability in the chain
3. Thread `StepContext` through the capability chain
4. Handle checkpointing between capabilities (if composition is checkpoint-enabled)
5. Produce `StepExecutionResult` conforming to `result_schema`

The executor is compiled into the worker binary. No dynamic loading, no FFI bridge, no language runtime dependency.

---

## Composition-Only Worker: A First-Class Deployment Target

### The Deployment Insight

The universal claimability property has a natural deployment consequence: if every worker can execute virtual handler steps regardless of FFI runtime, then a worker that *only* executes virtual handler steps needs no FFI runtime at all. No Ruby, no Python, no TypeScript, no domain handler registry, no resolver chain. Just the composition executor compiled into a minimal Rust binary.

This isn't a hypothetical — it's the logical extreme of the architecture described above. The `crates/workers/rust` crate already demonstrates the pattern: a thin binary that bootstraps from `tasker-worker` and provides its own handler registry. A composition-only worker would be even thinner: **no handler registry**, because the `CompositionExecutor` is the handler.

### What Today's Rust Worker Does

The existing `crates/workers/rust` bootstrap (`crates/workers/rust/src/bootstrap.rs`) follows this sequence:

1. Create a `RustStepHandlerRegistry` with domain-specific step handlers
2. Create a `RustStepHandlerRegistryAdapter` implementing the `StepHandlerRegistry` trait
3. Bootstrap `WorkerCore` via `WorkerBootstrap::bootstrap_with_event_system()`
4. Set up `StepEventPublisherRegistry` with domain-specific event publishers (e.g., `PaymentEventPublisher`)
5. Take `DispatchHandles` from the worker handle
6. Spawn `HandlerDispatchService` with the registry adapter and domain event callback
7. Start legacy event handler (compatibility path)
8. Wait for shutdown signal

Steps 1, 2, 4, 6 (partially), and 7 are domain handler infrastructure. A composition-only worker skips all of them.

### What a Composition-Only Worker Does

```
workers/composition/src/main.rs

1. Initialize tracing
2. Bootstrap WorkerCore via WorkerBootstrap::bootstrap()
   → SystemContext, WorkerCore, DispatchHandles
   → Namespace queues + composition queues subscribed automatically
3. Take DispatchHandles from worker handle
4. Spawn HandlerDispatchService with CompositionOnlyRegistry
   → Registry.get() selects the appropriate wrapper type by step type
   → No domain handler lookup, no resolver chain consulted
5. Wait for shutdown signal
6. Graceful shutdown
```

The `CompositionOnlyRegistry` is a trivial implementation of `StepHandlerRegistry`. It selects the appropriate virtual handler wrapper type based on the step's `step_type` — `CompositionHandler` for standard steps, `DecisionCompositionHandler` for decision steps, and so on (see "Virtual Handler Wrapper Types" below for the full family):

```rust
pub struct CompositionOnlyRegistry {
    standard: Arc<CompositionHandler>,
    decision: Arc<DecisionCompositionHandler>,
    batch_analyzer: Arc<BatchAnalyzerCompositionHandler>,
    batch_worker: Arc<BatchWorkerCompositionHandler>,
}

impl StepHandlerRegistry for CompositionOnlyRegistry {
    fn get(&self, step: &StepDefinition) -> Option<Arc<dyn StepHandler>> {
        // Only accept steps with composition specs
        if step.composition.is_none() {
            return None;  // Cannot handle domain handler steps
        }

        // Select wrapper type based on step type
        match step.step_type {
            StepType::Decision => Some(self.decision.clone()),
            StepType::Batchable => Some(self.batch_analyzer.clone()),
            StepType::BatchWorker => Some(self.batch_worker.clone()),
            _ => Some(self.standard.clone()),
        }
    }

    fn registered_callables(&self) -> Vec<String> {
        vec!["__composition__".to_string()]
    }
}
```

This is the entire handler dispatch logic. No resolver chain, no priority ordering, no class path inference. The composition spec is the handler — if the step has one, the registry selects the appropriate wrapper type for its step kind; if not, this worker can't process it (and shouldn't have claimed it).

### Binary Size and Dependencies

The composition-only worker excludes:

| Excluded | Why |
|----------|-----|
| Ruby FFI (magnus) | No Ruby handlers |
| Python FFI (PyO3) | No Python handlers |
| TypeScript FFI (napi-rs) | No TypeScript handlers |
| Domain handler registries | No handler discovery |
| Resolver chains | No callable resolution |
| Step event publisher registries | No domain-specific event publishers |
| Legacy event handler path | Composition executor uses the standard dispatch path |

What remains is the `tasker-worker` foundation:

| Included | Why |
|----------|-----|
| `WorkerBootstrap` | System context, config, database pool, messaging |
| `WorkerCore` | Queue subscription, step claiming, lifecycle management |
| `HandlerDispatchService` | Semaphore-bounded dispatch, timeout, completion channel |
| `CompositionExecutor` | The grammar capability chain interpreter |
| `CheckpointService` | Intermediate state between capabilities |
| Health endpoints (REST/gRPC) | Container orchestration, load balancer probes |
| Metrics/observability | Prometheus, tracing, structured logging |
| Signal handling | Graceful shutdown (SIGTERM, SIGINT) |

The resulting binary is pure Rust with no FFI bridges — comparable in size and startup time to `tasker-server` (the orchestration binary). Both are statically compiled from the same workspace, use the same `wolfi-base` Docker image, and have identical operational characteristics.

### Crate Structure

The composition-only worker follows the same pattern as `crates/workers/rust` — a thin crate in the workspace:

```
workers/
├── composition/          # Composition-only worker
│   ├── Cargo.toml        # Depends on tasker-worker (no FFI features)
│   ├── src/
│   │   ├── main.rs       # Binary entry point (minimal — bootstrap + signal handling)
│   │   └── lib.rs        # CompositionOnlyRegistry + bootstrap helper
│   └── config/
│       └── tasks/        # Empty or not needed — no task templates with domain handlers
├── rust/                 # Native Rust worker (domain handlers)
├── ruby/                 # Ruby FFI worker
├── python/               # Python FFI worker
└── typescript/           # TypeScript FFI worker
```

The `Cargo.toml` is minimal:

```toml
[package]
name = "tasker-worker-composition"
version = "0.1.0"

[[bin]]
name = "composition-worker"
path = "src/main.rs"

[dependencies]
tasker-worker = { path = "../../tasker-worker", default-features = true }
tasker-shared = { path = "../../tasker-shared" }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
anyhow = { workspace = true }
```

No additional dependencies beyond the workspace foundation. The binary compiles against `tasker-worker` with its default features (web-api, gRPC, postgres) but brings no FFI crates.

### Docker Image

The composition-only worker uses the same hardened Docker image pattern as the orchestration server:

```dockerfile
# Same multi-stage cargo-chef pattern as orchestration.prod.Dockerfile
FROM rust:1.90-bookworm AS chef
# ... cargo-chef setup, system deps ...

FROM chef AS planner
# ... recipe.json generation ...

FROM chef AS builder
# ... dependency cooking, then:
RUN cargo build --release --locked --bin composition-worker -p tasker-worker-composition
RUN strip target/release/composition-worker

FROM cgr.dev/chainguard/wolfi-base:latest AS runtime
# Minimal runtime: bash, libpq, openssl, curl, ca-certificates
# NO Ruby, NO Python, NO Node.js, NO language runtimes
COPY --from=builder /app/target/release/composition-worker /app/composition-worker

EXPOSE 8086 9500
HEALTHCHECK CMD curl -f http://localhost:8086/health || exit 1
USER nonroot
ENTRYPOINT ["/app/docker/scripts/worker-entrypoint.sh"]
CMD ["/app/composition-worker"]
```

The image characteristics:

| Property | Orchestration Image | Composition Worker | Rust Worker | Ruby Worker |
|----------|--------------------|--------------------|-------------|-------------|
| Base | wolfi-base | wolfi-base | wolfi-base | wolfi-base + Ruby |
| Runtime deps | libpq, openssl | libpq, openssl | libpq, openssl | libpq, openssl, Ruby 3.x |
| Binary size | ~30-40MB | ~25-35MB | ~30-40MB | ~30-40MB + gems |
| Startup time | <1s | <1s | <1s | 2-5s (gem loading) |
| Image size | ~80-100MB | ~70-90MB | ~80-100MB | ~300-500MB |
| Attack surface | Minimal | Minimal | Minimal | Language runtime |

The composition-only image is the smallest possible worker image — potentially smaller than the orchestration image because it doesn't need `sqlx-cli` or migration scripts.

### Scaling Pattern

The composition-only worker is purpose-built for the composition queue architecture:

```
Deployment:
  orchestration:     1-3 replicas (manages lifecycle, routes steps)
  composition-worker: 2-N replicas (processes ALL virtual handler steps)
  rust-worker:       1-2 replicas (Rust domain handlers + virtual handlers on namespace)
  ruby-worker:       1-2 replicas (Ruby domain handlers + virtual handlers on namespace)
  python-worker:     0-1 replicas (Python domain handlers + virtual handlers on namespace)
```

The composition-only workers subscribe to composition queues exclusively. They don't need namespace queue assignments because they have no domain handlers to offer. Their configuration is minimal:

```toml
# worker.toml for composition-only worker
[worker]
worker_id = "composition-worker-001"
composition_queues = true       # Subscribe to composition queues (the entire purpose)
namespaces = []                 # No namespace queues — no domain handlers
# composition_queue_shards = 4  # How many composition queue shards to listen on
```

This creates a clean separation of concerns in the deployment:

- **Composition workers**: Horizontally scalable pool for grammar-composed steps. Add more when virtual handler throughput is the bottleneck. Stateless, identical, fast to provision.
- **Domain workers**: Language-specific workers for opaque business logic. Scaled per-language based on domain handler workload. Carry FFI overhead but provide organizational flexibility.

The composition-only worker pool absorbs virtual handler load across all namespaces simultaneously. A burst of virtual handler steps from the `ecommerce` namespace competes for the same composition worker pool as steps from `analytics` — this is desirable because virtual handler steps are deterministic, stateless (within a single execution), and have predictable resource profiles.

### When to Deploy Composition-Only Workers

| Scenario | Composition Workers | Why |
|----------|-------------------|-----|
| **High virtual handler ratio** | Deploy generously | Most steps are grammar-composed; domain workers are underutilized for virtual handler work |
| **Multi-namespace deployment** | Strong fit | Virtual handler steps from all namespaces benefit from the shared pool |
| **Cost optimization** | Replace oversized domain workers | Composition workers are cheaper (smaller image, faster startup, no runtime overhead) |
| **Burst scaling** | Auto-scale composition pool | Kubernetes HPA on composition queue depth; fast startup enables rapid scale-out |
| **Single namespace, few virtual handlers** | Skip — use domain workers | Domain workers already handle virtual handler steps on their namespace queue |
| **Tenancy isolation required** | Skip — use namespace workers with `composition_queue: false` | Data must not cross namespace boundaries |

### Relationship to Existing Workers

The composition-only worker doesn't replace any existing worker type. It's additive — a deployment optimization for virtual handler throughput:

```
Before composition workers:
  All workers handle both virtual + domain steps on their namespace queue.
  Virtual handler throughput is bounded by namespace worker count.

After composition workers:
  Composition workers handle virtual steps via composition queue (shared pool).
  Domain workers handle domain steps on namespace queue (unchanged).
  Domain workers can ALSO handle virtual steps on their namespace queue
    (for templates with composition_queue: false, or as overflow).
```

The domain workers' virtual handler capability isn't removed — it's supplemented. A template with `composition_queue: false` still routes virtual handler steps to the namespace queue, where both domain workers and (if configured) composition workers on that namespace can claim them.

---

## Virtual Handler Wrapper Types

The original concept of a single `CompositionExecutor` has evolved into a **family of virtual handler wrappers** in `tasker-worker`. Each wrapper implements `StepHandler` and handles the orchestration protocol mechanics for its step type, delegating pure data transformation to the `tasker-grammar` composition engine.

| Wrapper | Step Type | Behavior |
|---------|-----------|----------|
| **`CompositionHandler`** | Standard | Runs the grammar composition, returns result as `StepExecutionResult` |
| **`DecisionCompositionHandler`** | Decision | Runs grammar composition producing a decision shape (route + steps), translates JSON output into `DecisionPointOutcome` |
| **`BatchAnalyzerCompositionHandler`** | Batchable | Runs grammar composition producing cursor partitions, translates JSON output into `BatchProcessingOutcome` |
| **`BatchWorkerCompositionHandler`** | Batch Worker | Provides the cursor loop + checkpoint yield machinery; runs a per-chunk grammar composition for the transform/persist body |

The key architectural boundary: `tasker-grammar` remains pure — it produces `serde_json::Value` matching declared `output` schemas. The wrapper types in `tasker-worker` handle all translation to orchestrator protocol types (`DecisionPointOutcome`, `BatchProcessingOutcome`, `StepExecutionResult`). This keeps the grammar crate free of orchestration dependencies.

For decision and batch compositions, the grammar's `output` schema declares the expected shape (e.g., `{route, steps}` for decisions, `{batch_size, worker_count, cursors}` for batch analysis), and the wrapper validates the grammar output against that shape before translating it. For batch workers, the wrapper manages the iteration loop, checkpoint saves, and `checkpoint_yield()` calls — the grammar composition only sees one chunk at a time.

See `transform-revised-grammar.md` sections "Open Design: Decision and Batch Outcome Expression" and "Virtual Handler Wrapper Types" for the full design rationale and examples.

---

## Summary

| Property | Domain Handlers | Virtual Handlers |
|----------|----------------|------------------|
| **Defined by** | `handler.callable` in StepDefinition | `composition` spec in StepDefinition |
| **Resolved by** | ResolverChain (priority-ordered) | Built-in wrapper type family (selected by step type) |
| **Registration** | Required (per-worker, per-language) | None (always available) |
| **Language dependency** | Yes (Ruby/Python/TS/Rust runtime) | None (compiled Rust) |
| **Claimable by** | Workers with matching handler | All online workers (default) |
| **Queue** | `worker_{namespace}_queue` | `worker_composition_queue_N` (default) or namespace queue (configurable) |
| **Lifecycle** | Standard step states | Same lifecycle |
| **Output** | `StepExecutionResult` | Same type (wrapper translates for decision/batch steps) |
| **Timeout/concurrency** | Semaphore-bounded | Same bounds |
| **Checkpointing** | Handler-driven (batch yields) | Spec-driven (between capabilities); batch wrapper manages cursor loop |
| **Load shedding** | TAS-75 CapacityChecker | Same infrastructure |
| **Queue override** | N/A | `composition_queue: false` on TaskTemplate routes to namespace queue |
| **Routing authority** | N/A (namespace queue always) | Template only — orchestrator never consults worker config |
| **Dedicated worker** | Per-language worker binary | Composition-only worker (no FFI, wrapper family registry, minimal image) |
| **Deployment prerequisite** | Workers for each namespace used | At least one composition-only worker for grammar features |

The virtual handler dispatch architecture adds a new execution path, a new queue routing strategy, and a new deployment target (the composition-only worker) while preserving the worker lifecycle and orchestration protocol. The routing model is intentionally simple: the template is the sole authority on whether virtual handler steps use composition queues or namespace queues. The orchestrator never consults worker configuration — workers choose what to subscribe to for their own operational reasons, and the composition-only worker serves as the deployment-level guarantee that composition queue steps will be processed. This preserves the existing architectural boundary where the orchestrator manages step lifecycle and workers manage step execution, with no coupling between the two.

---

*This document should be read alongside `transform-revised-grammar.md` for the 6-capability model and wrapper type design, `grammar-trait-boundary.md` for the trait design, `composition-validation.md` for how compositions are validated before execution, `checkpoint-generalization.md` for the checkpoint model, the case studies in `case-studies/` for the grammar proposals that motivate this architecture, and `implementation-phases.md` for the phased roadmap from research to production.*
