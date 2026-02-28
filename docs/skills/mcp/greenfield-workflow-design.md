# Skill: Greenfield Workflow Design with Tasker MCP

Use this skill when a developer wants to **design a new workflow from scratch** using
Tasker's MCP tools. The developer has a process they want to model as a DAG — they may
describe it in plain language, a diagram, or a list of steps — and wants to end up with
a validated task template and typed handler scaffolding ready to implement.

## When to Apply

- Developer says they want to "create a new workflow", "set up a pipeline", or "add a
  task template"
- A new feature requires orchestrating multiple steps with dependencies
- Developer is starting a new Tasker-based project or adding a new namespace to an
  existing one

## Agent Posture

You are **co-designing** the workflow with the developer. Your job is to ask the right
questions to elicit a clean DAG specification, then use the MCP tools to make it real.
Resist the urge to generate immediately — a few minutes of design conversation prevents
hours of rework.

## Workflow Phases

### Phase 1: Elicit the Workflow Shape

Before touching any tools, understand the workflow through conversation.

**Questions to ask:**

1. **What triggers this workflow?** (HTTP request, scheduled job, event, manual)
2. **What are the major steps?** Get the developer to name them — step names become
   handler function names, so push for clear, verb-first names (`validate_order`,
   `charge_payment`, not `step1`, `processing`)
3. **What depends on what?** For each step, ask: "What must complete before this can
   start?" This defines the DAG edges
4. **What data flows between steps?** For each step, ask: "What does this step produce
   that downstream steps need?" This defines `result_schema` fields
5. **Are any steps independent?** Steps that share a dependency but don't depend on each
   other can run in parallel — this is where DAG value comes from
6. **What can fail?** Which steps are retriable vs. permanent failures? This informs
   handler implementation but not the template itself

**Common DAG patterns to suggest:**

| Pattern | Shape | When to Use |
|---------|-------|-------------|
| Linear | A → B → C → D | Strict sequential processing |
| Fan-out | A → [B, C, D] → E | Parallel independent work followed by aggregation |
| Diamond | A → [B, C] → D | Two parallel paths converging at a gate |
| Double-diamond | A → [B,C] → D → [E,F] → G | Validation + review + delivery (common) |

### Phase 2: Define Data Contracts

For each step, define the `result_schema` — the typed output that downstream steps can
depend on.

**Field type reference:**

| Type | JSON Schema | Example |
|------|-------------|---------|
| `string` | `type: string` | URLs, IDs, status messages |
| `integer` | `type: integer` | Counts, durations, sizes |
| `number` | `type: number` | Scores, percentages, prices |
| `boolean` | `type: boolean` | Flags, gates, success indicators |
| `array:string` | `type: array, items: {type: string}` | Lists of tags, IDs, messages |
| `array:object` | `type: array, items: {type: object}` | Complex nested collections |

**Guidelines:**

- Every step should have a `result_schema` — even if minimal, it documents the contract
- Mark fields as **required** if downstream steps depend on them
- Mark fields as **optional** (`required: false`) for supplementary data like notes,
  metadata, or debug info
- Use descriptive field names — `plagiarism_score` not `score`, `published_url` not `url`
- Keep schemas flat where possible; avoid deep nesting

### Phase 3: Generate and Validate

Execute the MCP tool sequence:

```
template_generate  →  template_validate  →  template_inspect
```

**Step 3a: Generate the template**

Use `template_generate` with the full specification gathered in Phases 1-2. Provide:
- `name`: snake_case task name (e.g., `process_order`, `publish_article`)
- `namespace`: snake_case namespace grouping related workflows (e.g., `ecommerce`,
  `content_publishing`)
- `steps`: complete step definitions with dependencies and output fields
- `version`: start at `1.0.0`

**Step 3b: Validate**

Pass the generated YAML to `template_validate`. Expected: `valid: true`, no errors.
If validation fails, fix the issue and re-validate.

**Step 3c: Inspect the DAG**

Use `template_inspect` to verify:
- Root steps (no dependencies) match your entry points
- Leaf steps (no dependents) match your terminal steps
- Execution order respects all dependency constraints
- All steps have `has_result_schema: true`
- Parallel-eligible steps appear at the same depth level

**Show the developer the execution order** — this is the most tangible output for
confirming the design is correct.

### Phase 4: Verify Data Contracts

Use schema tools to validate data flow:

```
schema_inspect  →  schema_compare (for connected steps)
```

- `schema_inspect` with `step_filter` to review individual step schemas
- `schema_compare` between directly connected producer/consumer steps to catch
  mismatches early

**Key insight**: `schema_compare` reports `MISSING_REQUIRED_FIELD` when a consumer step
defines required fields that the producer doesn't output. Between directly dependent
steps, these represent real data flow problems. Between unrelated steps, the comparison
shows complete divergence — technically correct but not actionable.

### Phase 5: Generate Handler Scaffolding

Use `handler_generate` with the developer's target language:

| Language | Framework | Key Patterns |
|----------|-----------|-------------|
| `python` | FastAPI | Pydantic models, `@step_handler` + `@depends_on` decorators |
| `typescript` | Bun/Hono | Zod schemas, `defineHandler` with typed `depends` config |
| `ruby` | Rails | `Dry::Struct` types, handler blocks with keyword args |
| `rust` | Axum | serde structs, plain functions + `StepHandlerRegistry` bridge |

Always use `scaffold: true` (default) — this generates handlers that import the
generated types, giving developers a type-safe starting point.

**Generated output structure:**

| Output | Purpose | Developer Action |
|--------|---------|-----------------|
| `types` (models) | Data contract types — **do not edit** | Regenerate when schema changes |
| `handlers` | Handler functions with TODO stubs | **Implement business logic here** |
| `tests` | Test stubs with mock dependencies | **Expand with real test cases** |
| `handler_registry` (Rust only) | StepHandler trait bridge | Minimal edits needed |

### Phase 6: Integrate into the Application

Guide the developer on where to place the generated code:

**Target architecture** (three-layer pattern):

```
services/       → Pure business logic (can throw, no Tasker awareness)
handlers/       → Thin wrappers that call services, return typed results
routes/         → HTTP endpoints that submit tasks via getTaskerClient/tasker_client
```

- Generated **models** go alongside handlers (or in a shared types module)
- Generated **handlers** go in the handlers directory, grouped by namespace
- Generated **tests** go in the test directory matching the handler structure
- **Services** are written by the developer — this is where real logic lives

## Iteration Pattern

When the developer wants to evolve the template:

1. Modify the specification and re-run `template_generate` with a bumped version
2. Use `schema_diff` with `before_yaml` (old) and `after_yaml` (new) to detect breaking
   changes before they reach production
3. Re-validate with `template_validate`
4. Regenerate handlers if schema changed — types file gets regenerated, handler TODOs
   remain for the developer to update

**Breaking change rules:**
- Adding a field: non-breaking
- Removing a required field: **breaking**
- Changing a field's type: **breaking**
- Making an optional field required: **breaking**
- Making a required field optional: non-breaking (relaxing)

## Anti-Patterns to Avoid

| Anti-Pattern | Why It's Bad | Instead |
|-------------|-------------|---------|
| One giant step | No parallelism, no partial retry | Break into independent substeps |
| Too many tiny steps | Overhead exceeds benefit | Combine tightly coupled logic |
| Circular dependencies | DAG validation will reject it | Re-examine the data flow |
| Putting business logic in handlers | Hard to test, hard to reuse | Keep handlers thin, logic in services |
| Skipping `schema_compare` | Data contract mismatches found at runtime | Validate contracts at design time |
| Not versioning templates | Can't detect breaking changes | Always bump version on schema changes |
