# Dependency Diagrams — Lane Detail

*Level 3 diagrams for the [Composition Architecture Roadmap](roadmap.md)*

These diagrams show individual work items within each lane with task-level dependencies. Use these when you're deep in a lane and need to see what's next.

---

## Phase 1: Grammar Foundations — Lane Detail

### Lane 1B: Operation Traits & Contracts

```mermaid
flowchart TD
    B1_1["Define ResourceOperationError\nenum in tasker-grammar::operations"]
    B1_2["Define PersistConstraints,\nAcquireConstraints, EmitMetadata\nconstraint types"]
    B1_3["Define PersistResult,\nAcquireResult, EmitResult\nresult types"]
    B1_4["Define PersistableResource trait\nasync fn persist(entity, data, constraints)"]
    B1_5["Define AcquirableResource trait\nasync fn acquire(entity, params, constraints)"]
    B1_6["Define EmittableResource trait\nasync fn emit(topic, payload, metadata)"]
    B1_7["Define OperationProvider trait\nget_persistable, get_acquirable,\nget_emittable by resource_ref"]
    B1_8["Implement InMemoryOperations\nfixture data + capture lists"]
    B1_9["Implement InMemoryOperationProvider\nwraps InMemoryOperations"]
    B1_10["Tests: operation traits with\nInMemoryOperations"]

    B1_1 --> B1_4
    B1_1 --> B1_5
    B1_1 --> B1_6
    B1_2 --> B1_4
    B1_2 --> B1_5
    B1_2 --> B1_6
    B1_3 --> B1_4
    B1_3 --> B1_5
    B1_3 --> B1_6
    B1_4 --> B1_7
    B1_5 --> B1_7
    B1_6 --> B1_7
    B1_4 --> B1_8
    B1_5 --> B1_8
    B1_6 --> B1_8
    B1_7 --> B1_9
    B1_8 --> B1_9
    B1_9 --> B1_10
```

### Lane 1C: Side-Effecting Executors

```mermaid
flowchart TD
    C1_1["PersistExecutor\nconfig parse → jaq eval →\nget_persistable → persist →\nvalidate_success → result_shape"]
    C1_2["AcquireExecutor\nconfig parse → jaq eval →\nget_acquirable → acquire →\nvalidate_success → result_shape"]
    C1_3["EmitExecutor\nconfig parse → jaq eval →\nget_emittable → emit →\nvalidate_success → result_shape"]
    C1_4["Tests: each executor with\nInMemoryOperations\nfull pipeline coverage"]

    C1_1 --> C1_4
    C1_2 --> C1_4
    C1_3 --> C1_4
```

### Lane 1D: Composition Engine

```mermaid
flowchart TD
    D1_1["CompositionValidator\nJSON Schema contract chaining\ncapability compatibility checks"]
    D1_2["CompositionExecutor\nstep sequencing, data threading\nvia composition envelope"]
    D1_3["Checkpoint integration\nmutating capability checkpoints\nresume from checkpoint state"]
    D1_4["Tests: multi-step compositions\nwith all capability types"]

    D1_1 --> D1_4
    D1_2 --> D1_3
    D1_3 --> D1_4
```

### Lane 1E: Workflow Modeling & Acceptance

```mermaid
flowchart TD
    E1_1["Model workflow 1:\ne-commerce order processing\n(acquire → transform → persist → emit)"]
    E1_2["Model workflow 2:\ndata pipeline / ETL\n(acquire → validate → transform → persist)"]
    E1_3["Model workflow 3:\nevent-driven integration\n(acquire → evaluate → assert → emit)"]
    E1_4["End-to-end acceptance tests\nall 3 workflows through\nCompositionExecutor"]

    E1_1 --> E1_4
    E1_2 --> E1_4
    E1_3 --> E1_4
```

---

## Phase 2: Runtime Infrastructure — Lane Detail

### Lane 2A: Runtime Scaffolding & Adapters

```mermaid
flowchart TD
    A2_1["Scaffold tasker-runtime crate\nCargo.toml, workspace member\nfeature flags (postgres, http)"]
    A2_2["PostgresPersistAdapter\nstructured data → INSERT/UPSERT SQL\nconflict strategy handling"]
    A2_3["PostgresAcquireAdapter\nparams → SELECT SQL\nlimit/offset/timeout"]
    A2_4["HttpPersistAdapter\nstructured data → POST/PUT\nentity → URL path"]
    A2_5["HttpAcquireAdapter\nparams → GET with query string\nconstraint → timeout/pagination"]
    A2_6["HttpEmitAdapter\npayload → POST webhook\nmetadata → headers"]
    A2_7["PgmqEmitAdapter\npayload → pgmq send\ntopic → queue name"]
    A2_8["AdapterRegistry\nResourceType → adapter factory\nstandard() with built-in mappings"]
    A2_9["Tests: each adapter unit tested\nagainst mock/test resources"]

    A2_1 --> A2_2
    A2_1 --> A2_3
    A2_1 --> A2_4
    A2_1 --> A2_5
    A2_1 --> A2_6
    A2_1 --> A2_7
    A2_2 --> A2_8
    A2_3 --> A2_8
    A2_4 --> A2_8
    A2_5 --> A2_8
    A2_6 --> A2_8
    A2_7 --> A2_8
    A2_8 --> A2_9
```

### Lane 2B: ResourcePoolManager

```mermaid
flowchart TD
    B2_1["PoolManagerConfig\nmax_pools, max_total_connections\nidle_timeout, eviction_strategy"]
    B2_2["ResourceAccessMetrics\norigin (static/dynamic)\nlast_accessed, access_count"]
    B2_3["ResourcePoolManager struct\nwraps ResourceRegistry\ndefinitions + metrics maps"]
    B2_4["initialize()\nstatic definitions from config\nfail loudly on init errors"]
    B2_5["get_or_initialize()\nfast path: touch metrics, return\nslow path: resolve → admit → init"]
    B2_6["admission_check()\npool count ceiling\nconnection budget check"]
    B2_7["eviction_sweep()\nidle timeout for dynamic pools\nLRU under capacity pressure"]
    B2_8["Tests: lifecycle, eviction,\nbackpressure, budget enforcement"]

    B2_1 --> B2_3
    B2_2 --> B2_3
    B2_3 --> B2_4
    B2_4 --> B2_5
    B2_5 --> B2_6
    B2_5 --> B2_7
    B2_6 --> B2_8
    B2_7 --> B2_8
```

### Lane 2C: ResourceDefinitionSource

```mermaid
flowchart TD
    C2_1["ResourceDefinitionSource trait\nresolve(name) → Option<Definition>\nwatch() → Receiver<Event>"]
    C2_2["ResourceDefinitionEvent enum\nAdded, Updated, Removed"]
    C2_3["StaticConfigSource\nreads worker.toml [[resources]]\nno watch support"]
    C2_4["SopsFileWatcher\nwatches mounted volume\nSOPS-encrypted YAML definitions"]
    C2_5["Tests: static source resolution\nSopsFileWatcher with test fixtures"]

    C2_1 --> C2_3
    C2_1 --> C2_4
    C2_2 --> C2_4
    C2_3 --> C2_5
    C2_4 --> C2_5
```

### Lane 2D: RuntimeOperationProvider

```mermaid
flowchart TD
    D2_1["RuntimeOperationProvider struct\npool_manager + adapter_registry"]
    D2_2["impl OperationProvider\nget_persistable: pool_manager.get_or_initialize\n→ adapter_registry.as_persistable"]
    D2_3["Per-composition adapter caching\nlazy resolve, cache for duration\nof composition execution"]
    D2_4["Tests: full resolution flow\nresource_ref → pool → adapter →\nArc<dyn PersistableResource>"]

    D2_1 --> D2_2
    D2_2 --> D2_3
    D2_3 --> D2_4
```

---

## Phase 3: Integration & Tooling — Lane Detail

### Lane 3A: StepContext Rename

```mermaid
flowchart TD
    A3_1["Rename TaskSequenceStep → StepContext\nin tasker-worker"]
    A3_2["Update all references\nin tasker-worker, tasker-orchestration"]
    A3_3["Verify FFI alignment\ntasker-py, tasker-rb, tasker-ts\nalready use StepContext"]
    A3_4["Tests: all existing tests pass\nwith renamed type"]

    A3_1 --> A3_2
    A3_2 --> A3_3
    A3_3 --> A3_4
```

### Lane 3B: CompositionExecutionContext

```mermaid
flowchart TD
    B3_1["CompositionExecutionContext struct\nstep: Arc<StepContext>\noperations: Arc<dyn OperationProvider>"]
    B3_2["CompositionEnvelope\n.context, .deps, .prev, .step\ndata threading for jaq expressions"]
    B3_3["Checkpoint integration\nArc<CheckpointService>\nresume from checkpoint state"]
    B3_4["Optional DataClassifier\nOption<Arc<DataClassifier>>\nintegrates when S4 ready"]
    B3_5["Tests: context construction\noperation resolution through context"]

    B3_1 --> B3_2
    B3_2 --> B3_3
    B3_3 --> B3_4
    B3_4 --> B3_5
```

### Lane 3C: Validation Tooling

```mermaid
flowchart TD
    C3_1["tasker-sdk: composition-aware\ntemplate validator"]
    C3_2["Expression syntax validator\nvariable resolution checker"]
    C3_3["Capability compatibility checker\nsemantic validation"]
    C3_4["tasker-mcp: grammar and\ncapability discovery tools"]
    C3_5["tasker-ctl: grammar and\ncomposition CLI commands"]
    C3_6["composition_explain: trace output\nfor data flow visualization"]
    C3_7["Validate 3 modeled workflows\nthrough tooling pipeline"]

    C3_1 --> C3_7
    C3_2 --> C3_7
    C3_3 --> C3_7
    C3_4 --> C3_7
    C3_5 --> C3_7
    C3_6 --> C3_7
```

### Lane 3D: Secure Foundations Integration

```mermaid
flowchart TD
    D3_1["TAS-369: ConfigString\ninto tasker-shared config loading"]
    D3_2["TAS-359 (S3): EncryptionProvider\nfield-level encryption at rest"]
    D3_3["TAS-360 (S4): DataClassifier\ntrace and log safety"]

    D3_1
    D3_2
    D3_3
```

*All items in lane 3D are independent of each other and of other lanes.*

---

## Phase 4: Worker Dispatch & Queues — Lane Detail

### Lane 4A: GrammarActionResolver

```mermaid
flowchart TD
    A4_1["GrammarActionResolver struct\nregistered in ResolverChain\nat priority 15"]
    A4_2["Resolve 'grammar:*' callables\nparse composition spec\nfrom step definition"]
    A4_3["GrammarResolvedHandler\nimplements StepHandler\nwraps CompositionExecutor"]
    A4_4["Context bridge\nStepContext → CompositionExecutionContext\nvia RuntimeOperationProvider"]
    A4_5["Tests: resolver finds grammar callables\nhandler executes through full pipeline"]

    A4_1 --> A4_2
    A4_2 --> A4_3
    A4_3 --> A4_4
    A4_4 --> A4_5
```

### Lane 4B: Composition Queue Routing

```mermaid
flowchart TD
    B4_1["TaskTemplate: add\ncomposition_queue field"]
    B4_2["StepEnqueuerActor:\nroute grammar steps to\ncomposition queues"]
    B4_3["Worker: composition queue\nsubscription mechanism"]
    B4_4["Tests: routing correctness\nsteps land in correct queues"]

    B4_1 --> B4_2
    B4_2 --> B4_3
    B4_3 --> B4_4
```

### Lane 4C: tasker-rs Binary

```mermaid
flowchart TD
    C4_1["Scaffold tasker-rs crate\nbinary, Cargo.toml\ndepends on tasker-worker + tasker-runtime"]
    C4_2["Startup: init ResourcePoolManager\nfrom worker.toml static definitions"]
    C4_3["Startup: register standard adapters\nin AdapterRegistry"]
    C4_4["Startup: register GrammarActionResolver\nin ResolverChain at priority 15"]
    C4_5["Startup: subscribe to namespace\nqueues + composition queues"]
    C4_6["Tests: binary starts, processes\na grammar-composed step"]

    C4_1 --> C4_2
    C4_2 --> C4_3
    C4_3 --> C4_4
    C4_4 --> C4_5
    C4_5 --> C4_6
```

### Lane 4D: End-to-End Acceptance

```mermaid
flowchart TD
    D4_1["Test: template with grammar-composed steps\ncreated via API"]
    D4_2["Test: orchestration routes steps\nto composition queue"]
    D4_3["Test: composition worker picks up step\nexecutes grammar pipeline\nagainst live test resources"]
    D4_4["Test: results flow back through\norchestration to task completion"]
    D4_5["Test: mixed workflow\ngrammar + domain handler steps\nin same task template"]

    D4_1 --> D4_2
    D4_2 --> D4_3
    D4_3 --> D4_4
    D4_4 --> D4_5
```

---

## Full Cross-Phase Dependency Graph

This diagram shows all lanes across all phases with their cross-phase dependencies.

```mermaid
flowchart TD
    subgraph done["✅ Completed"]
        S1["S1: SecretsProvider"]
        S2["S2: ResourceRegistry"]
        L1A["1A: Expression Engine\n& Core Types"]
    end

    subgraph p1["Phase 1: Grammar Foundations"]
        L1B["1B: Operation Traits"]
        L1C["1C: Side-Effecting Executors"]
        L1D["1D: Composition Engine"]
        L1E["1E: Workflow Acceptance"]
    end

    subgraph p2["Phase 2: Runtime Infrastructure"]
        L2A["2A: Adapters"]
        L2B["2B: Pool Manager"]
        L2C["2C: Definition Sources"]
        L2D["2D: RuntimeOperationProvider"]
    end

    subgraph p3["Phase 3: Integration & Tooling"]
        L3A["3A: StepContext Rename"]
        L3B["3B: CompositionExecutionContext"]
        L3C["3C: Validation Tooling"]
        L3D["3D: Secure Integration"]
    end

    subgraph p4["Phase 4: Worker Dispatch & Queues"]
        L4A["4A: GrammarActionResolver"]
        L4B["4B: Queue Routing"]
        L4C["4C: tasker-rs Binary"]
        L4D["4D: E2E Acceptance"]
    end

    %% From completed work
    L1A --> L1B
    L1A --> L1D
    S1 --> L2A
    S2 --> L2A
    S2 --> L2B
    S2 --> L3D

    %% Phase 1 internal
    L1B --> L1C
    L1C --> L1D
    L1D --> L1E

    %% Phase 1 → Phase 2
    L1B --> L2A

    %% Phase 2 internal
    L2A --> L2D
    L2B --> L2D

    %% Phase 2 → Phase 3
    L2D --> L3B
    L3A --> L3B
    L1D --> L3C

    %% Phase 3 → Phase 4
    L3B --> L4A
    L3B --> L4C
    L3C --> L4C
    L4A --> L4C
    L4B --> L4C
    L4C --> L4D

    style done fill:#d4edda,stroke:#28a745
    style p1 fill:#fff3cd,stroke:#ffc107
    style p2 fill:#cce5ff,stroke:#007bff
    style p3 fill:#e2d9f3,stroke:#6f42c1
    style p4 fill:#f8d7da,stroke:#dc3545
```
