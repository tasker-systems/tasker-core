# Case Study: Grammar Proposals for Test Fixture Handlers

*Expressing test fixture handler logic as grammar compositions, and identifying orchestration boundaries*

*March 2026 — Research Spike (revised for 6-capability model)*

---

## Approach

The `tests/fixtures/` handlers test Tasker's core orchestration features — mathematical DAGs, decision routing, batch processing, checkpoint resumption. Many are deliberately simple (single-operation handlers for verifiable math). This case study evaluates each handler's internal logic for grammar expressibility, and draws sharper lines around what grammars can and cannot represent.

With the adoption of **jaq-core** as the unified expression language, the original 9-capability model has been refined to 6 capabilities. The pure data capabilities (`reshape`, `compute`, `evaluate`, `evaluate_rules`, `group_by`, `rank`) all collapse into a single `transform` capability — jaq natively expresses projection, arithmetic, boolean derivation, and conditional logic in one filter expression. The remaining capabilities are `validate` (JSON Schema trust boundary), `assert` (execution gate), `persist` (write), `acquire` (read), and `emit` (domain event). See `transform-revised-grammar.md` for the full design rationale.

**Input data convention**: All jaq filters operate on the composition context envelope:
- `.context` — the task's original input data
- `.deps.{step_name}` — upstream workflow step dependency results
- `.prev` — output of the previous capability invocation within this composition
- `.step` — minimal step metadata (name, attempts, inputs)

---

## Mathematical Handlers (Linear, Diamond, Tree, Complex DAG)

### Internal Logic

All mathematical handlers follow one pattern: extract input → apply arithmetic → return result. The operations are:
- **Square**: `n → n²`
- **Multiply and square**: `a × b → (a × b)²`
- **Multiply three and square**: `a × b × c → (a × b × c)²`
- **Verification**: recompute `original^power` and compare

### Grammar Proposals

**Simple step** (LinearStep1, DiamondStart, TreeRoot, DagInit, all branches/leaves):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [result]
      properties:
        result: { type: number }
    filter: |
      { result: (.context.value | . * .) }
```

**Two-input convergence** (DiamondEnd, DagValidate):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [result]
      properties:
        result: { type: number }
    filter: |
      .deps.branch_b.result as $a | .deps.branch_c.result as $b
      | ($a * $b) as $prod
      | { result: ($prod * $prod) }
```

**Four-input convergence** (TreeFinalConvergence):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [result]
      properties:
        result: { type: number }
    filter: |
      .deps.leaf_d.result as $d | .deps.leaf_e.result as $e
      | .deps.leaf_f.result as $f | .deps.leaf_g.result as $g
      | ($d * $e * $f * $g) as $prod
      | { result: ($prod * $prod) }
```

Note: The `precision: arbitrary` concern from the original proposal remains relevant — jaq operates on JSON numbers (IEEE 754 double-precision). For the very large numbers these DAG fixtures produce, a BigUint-aware computation would need a custom jaq function or a traditional handler. This is a known limitation of the expression language approach for arbitrary-precision math.

**Triple convergence with verification** (DagFinalize):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [computed, original]
      properties:
        computed: { type: number }
        original: { type: number }
    filter: |
      .deps.dag_validate.result as $v | .deps.dag_transform.result as $t
      | .deps.dag_analyze.result as $a
      | ($v * $t * $a) as $prod
      | { computed: ($prod * $prod), original: .context.even_number }

  - capability: assert
    filter: '.prev.computed == (.prev.original | pow(.; 64))'
    error: "Verification failed: computed result does not match original^64"
```

### Assessment

**Are these good composition candidates?** No. Each mathematical handler is a single operation — the composition adds ceremony without value. A single `transform` is one capability for what is one line of code in a handler function.

**What they *do* validate**: The grammar system must handle multi-input convergence (projecting 2, 3, or 4 dependency results via `.deps` into a single computation). The `transform` capability handles this naturally — jaq can reference multiple `.deps.{step_name}` paths and combine them in a single filter expression. This is more concise than the old `reshape` + `compute` two-step pattern, since jaq natively combines projection and arithmetic.

---

## Decision Point Handlers

### RoutingDecisionHandler

**Internal logic**:
1. Extract `amount` from task context
2. Apply threshold routing:
   - < $1,000 → create `auto_approve`
   - $1,000–$5,000 → create `manager_approval`
   - ≥ $5,000 → create `manager_approval` AND `finance_review`
3. Return `DecisionPointOutcome::create_steps()` with step names and routing metadata

### Grammar Proposal

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [route, steps]
      properties:
        route: { type: string, enum: [auto_approval, manager_only, dual_approval] }
        steps: { type: array, items: { type: string } }
    filter: |
      .context.amount as $amt
      | if $amt < 1000 then {route: "auto_approval", steps: ["auto_approve"]}
        elif $amt < 5000 then {route: "manager_only", steps: ["manager_approval"]}
        else {route: "dual_approval", steps: ["manager_approval", "finance_review"]} end
```

**But there's a problem**: The output must be a `DecisionPointOutcome` that the orchestrator understands — it's a protocol type, not a domain result. The composition executor would need special handling for decision-point steps: the final output must be translated into the `create_steps` protocol.

**Options**:
1. **Dedicated grammar**: A `Decide` grammar category whose outcome type is `DecisionPointOutcome`. The composition executor knows that `Decide` grammars produce step creation instructions.
2. **Adapter capability**: A final `emit_decision` capability that wraps the rule evaluation output in the decision protocol.
3. **Stay as domain handlers**: Decision handlers are tightly coupled to the orchestrator protocol. Keep them as domain handler code.

**Recommendation**: Option 3 for now. Decision routing logic is simple (threshold checks) and tightly bound to Tasker's step creation protocol. The value of grammar composition is low here. A future `Decide` grammar category could revisit this when the decision patterns become more complex.

---

## Approval Branch Handlers

### AutoApproveHandler, ManagerApprovalHandler, FinanceReviewHandler

**Internal logic** (all three are similar):
1. Extract amount and routing context from dependencies
2. Apply branch-specific approval logic (auto-approve always succeeds, manager checks limit, finance applies policy)
3. Return approval result

### Grammar Proposals

**AutoApproveHandler**:

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [approved, approval_type, approved_by]
      properties:
        approved: { type: boolean }
        approval_type: { type: string }
        approved_by: { type: string }
    filter: |
      {approved: true, approval_type: "automatic", approved_by: "system"}
```

**ManagerApprovalHandler**:

```yaml
grammar: Transform
compose:
  - capability: validate
    config:
      schema:
        amount: { type: number, max: 10000 }
      coercion: strict
      on_failure: fail

  - capability: transform
    output:
      type: object
      required: [approved, approval_type]
      properties:
        approved: { type: boolean }
        approval_type: { type: string }
    filter: |
      {approved: true, approval_type: "manager"}
```

**Assessment**: These are too simple for composition. One or two operations per handler. The grammar adds overhead without benefit. Keep as domain handlers.

---

## Batch Processing Handlers

### DatasetAnalyzerHandler (Batchable)

**Internal logic**:
1. Extract `dataset_size` from task context
2. Read config: `batch_size`, `max_workers`, `worker_template_name`
3. Calculate optimal worker count: `min(ceil(size / batch_size), max_workers)`
4. Generate cursor configs: `[{start: 0, end: batch_size}, {start: batch_size, end: 2*batch_size}, ...]`
5. Return `BatchProcessingOutcome::create_batches()` with cursor configs

### Grammar Proposal

**Not a composition candidate.** Like decision handlers, batch analyzers produce orchestrator protocol types (`BatchProcessingOutcome`). The cursor calculation logic is specific to Tasker's batch system. Batch cursor calculation stays outside grammar scope — the (action, resource, context) triple cannot be deterministically expressed because the output is an orchestrator protocol type, not a domain result.

A hypothetical composition would use `transform` for the arithmetic:

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [worker_count]
      properties:
        worker_count: { type: integer }
    filter: |
      .context as $c
      | (($c.dataset_size / $c.batch_size) | ceil) as $needed
      | {worker_count: ([$needed, $c.max_workers] | min)}
```

But the result must be a `BatchProcessingOutcome` with cursor configs — an orchestrator protocol type that grammar compositions cannot produce. If different batch strategies existed (fixed-size batches, count-based batches, key-range partitioning), a dedicated grammar category (like the hypothetical `Decide` grammar for decisions) could revisit this. For now, batch analyzers should stay as domain handlers.

### BatchWorkerHandler (Batch Worker)

**Internal logic**:
1. Extract batch context (cursor range)
2. Detect no-op placeholder
3. Process items from start_position to end_position in chunks
4. Track processed count and checkpoint count
5. Return batch worker success with metrics

### Grammar Proposal

Not a composition candidate. Batch workers have their own execution lifecycle (cursor iteration, checkpoint yield, resume from checkpoint) that is fundamentally different from composition's sequential capability chain. The handler loop (`while cursor < end`) is stateful iteration, not a sequence of discrete capabilities.

### ResultsAggregatorHandler (Deferred Convergence)

**Internal logic**:
1. Detect aggregation scenario (NoBatches vs. WithBatches)
2. For NoBatches: return zero totals
3. For WithBatches: iterate all batch worker dependency results
4. Sum processed_count and checkpoint_count across workers
5. Validate total against expected dataset size

### Grammar Proposal

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [total_processed, total_checkpoints]
      properties:
        total_processed: { type: integer }
        total_checkpoints: { type: integer }
    filter: |
      [.deps | to_entries[] | select(.key | startswith("process_batch_")) | .value] as $results
      | {
          total_processed: ([$results[].processed_count] | add // 0),
          total_checkpoints: ([$results[].checkpoint_count] | add // 0)
        }

  - capability: assert
    filter: '.prev.total_processed == .context.expected_dataset_size'
    error: "Not all items were processed: total does not match expected dataset size"
```

**Assessment**: Two capabilities — a `transform` that projects and aggregates batch worker results, and an `assert` to validate totals. This is more concise than the original three-capability (`reshape` + `compute` + `assert`) proposal because jaq naturally combines projection and aggregation in a single filter. However, the NoBatches vs. WithBatches scenario detection is specific to Tasker's batch lifecycle (deferred convergence detection), which stays outside grammar scope. This composition would only cover the WithBatches path; the handler would still need domain logic for scenario branching.

---

## Checkpoint Yield Handlers

### CheckpointYieldWorkerHandler

**Internal logic**:
1. Check for no-op placeholder
2. Extract batch cursor range
3. Load checkpoint (if resuming): restore cursor, accumulated results, items processed
4. Loop from cursor to end:
   - Process one item
   - Accumulate results (running_total, item_ids)
   - If chunk threshold reached: `checkpoint_yield(cursor, items_processed, accumulated)`
5. Return batch worker success

### Grammar Proposal

**Not a composition candidate.** The handler's core logic is a stateful loop with checkpoint yields *within* the loop. Grammar compositions checkpoint *between* capabilities (discrete steps), not within a capability's iteration. The checkpoint yield pattern is fundamentally about intra-handler state management, which is orthogonal to composition's inter-capability state management.

**What this confirms**: Composition checkpointing (from `checkpoint-generalization.md`) and batch worker checkpointing serve different needs:
- Composition checkpoint: "I completed capability 3 of 5, save my progress"
- Batch worker checkpoint: "I processed 50 of 200 items in this batch, save my cursor"

Both use the same `CheckpointService` infrastructure, but the execution models are distinct.

---

## Error/Retry Handlers

### FailNTimesHandler

**Internal logic**:
1. Read `fail_count` from handler initialization
2. Read current `attempts` from workflow_step
3. If attempts < fail_count: return error (retryable)
4. Else: return success

### Grammar Proposal

Not a meaningful composition candidate — this is a test utility, not business logic. But it does illustrate a useful pattern for grammar composition testing:

```yaml
# Test fixture: composition that fails at a specific capability
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      properties:
        passthrough: { type: boolean }
    filter: |
      {passthrough: true}

  - capability: assert
    filter: '.step.attempts >= 3'
    error: "Simulated failure: not enough attempts yet"

  - capability: transform
    output:
      type: object
      properties:
        passthrough: { type: boolean }
    filter: |
      {passthrough: true}
```

This would test composition retry behavior: the second capability (an `assert` gate on attempt count) fails on attempts 1 and 2, causing step-level retries. On the third attempt, it succeeds and the composition completes. If capability 1 were checkpointed, it would be skipped on retry. Note the use of `.step.attempts` from the composition context envelope — this is exactly the kind of retry-aware logic the envelope was designed to support.

---

## Diamond-Decision-Batch Combined Handler

### RoutingDecisionHandler (in diamond-decision-batch context)

**Internal logic**:
1. Get counts from both diamond branches (evens count, odds count)
2. Route based on comparison: `even_count >= odd_count → "even" else "odd"`
3. Create steps via `DecisionPointOutcome`

### Grammar Proposal

Same as the standalone decision handler — decision protocol output keeps this outside grammar scope. But the *input reshaping and routing logic* is exactly what `transform` expresses well:

```yaml
compose:
  - capability: transform
    output:
      type: object
      required: [route, steps]
      properties:
        route: { type: string, enum: [even, odd] }
        steps: { type: array, items: { type: string } }
    filter: |
      .deps.branch_evens.count as $evens | .deps.branch_odds.count as $odds
      | if $evens >= $odds then {route: "even", steps: ["even_batch_analyzer"]}
        else {route: "odd", steps: ["odd_batch_analyzer"]} end
```

The composition *expresses* the logic well — what was previously a `reshape` + `evaluate_rules` two-step chain becomes a single `transform` since jaq combines projection, comparison, and conditional logic in one filter. The issue is only the output protocol requirement.

---

## Synthesis: Handler Complexity vs. Grammar Value

### Complexity Distribution

From the handler survey, handlers fall into four complexity bands:

| Band | Operations | Examples | Composition Value |
|------|-----------|----------|-------------------|
| **Trivial** (1 op) | Single arithmetic or assignment | Math steps, auto-approve, assertions | **None** — composition adds overhead |
| **Simple** (2-3 ops) | Validate + transform, or project + compute | Branch handlers, simple approval | **Low** — handler is already clear |
| **Moderate** (4-6 ops) | Multi-step validation, aggregation, rule evaluation | Cart validation, insights, policy checks | **Medium** — composition enables configurability |
| **Complex** (7+ ops) | Stateful loops, multi-source aggregation, checkpoint management | Batch workers, convergence handlers | **Low** — execution model doesn't fit |

### The Sweet Spot

Grammar compositions add the most value for **moderate-complexity handlers** where:
1. Internal logic follows a recognizable pattern (validate → transform → persist)
2. The steps are configurable (different rules, different thresholds, different schemas)
3. The handler doesn't need special orchestrator protocol interactions
4. The operations are individually useful as vocabulary capabilities

### What Stays Outside Grammar Scope

| Pattern | Why Outside |
|---------|------------|
| Decision point step creation | Orchestrator protocol output — (action, resource, context) cannot be deterministically expressed |
| Batch cursor calculation | Orchestrator protocol output — (action, resource, context) cannot be deterministically expressed |
| Checkpoint yield within loops | Intra-handler stateful iteration — not a discrete capability chain |
| Deferred convergence detection | Orchestrator batch lifecycle — scenario branching is protocol-specific |
| Cross-namespace delegation | Orchestration coordination — not a domain data operation |
| Single-operation domain handlers | Composition adds ceremony without value |

### Capabilities That Emerged from Test Fixtures

| Capability | Fixture Context | Generalizability |
|-----------|----------------|-----------------|
| `transform` | Convergence handlers (multi-input projection + arithmetic), derived metrics, boolean routing, aggregation sums, conditional rule evaluation | High — unified data transformation primitive; replaces reshape, compute, evaluate, evaluate_rules |
| `validate` | Input schema enforcement, approval checks | High — trust boundary with coercion |
| `assert` | Math verification, total validation, execution gates, retry-aware gating | High — cross-validation and precondition checks |

The side-effecting capabilities (`persist`, `acquire`, `emit`) did not surface in the test fixture handlers — these fixtures focus on orchestration mechanics (DAG topology, decision routing, batch lifecycle) rather than domain data operations. The contrib case studies (`workflow-patterns.md`) exercise the full 6-capability vocabulary.

### Impact of the Transform Unification

The move from 9 capabilities to 6 has a notable effect on the test fixture proposals:

1. **Convergence patterns simplify**: What was `reshape` + `compute` (two capabilities, two input mappings) becomes a single `transform` — jaq naturally projects multiple `.deps` paths and computes in one expression.
2. **Decision logic simplifies**: What was `evaluate_rules` with a bespoke rule engine config becomes a `transform` with jaq `if-then-elif-else` — no custom rule syntax to learn.
3. **Aggregation simplifies**: What was `reshape` (flatten) + `compute` (sum) becomes a single `transform` using jaq's native `group_by`, `add`, and array operations.
4. **Input mapping disappears**: The old `input_mapping: { type: task_context }` / `{ type: previous }` / `{ type: merged }` configuration is replaced by jaq's direct access to `.context`, `.deps`, `.prev`, and `.step` — the data routing is in the expression itself, not a separate config field.

The net effect: compositions are shorter, the capability vocabulary is smaller, and the expression language is a well-documented standard (jq) rather than a bespoke DSL.

---

*This case study should be read alongside `workflow-patterns.md` for the contrib handler grammar proposals, `transform-revised-grammar.md` for the 6-capability model design, `grammar-trait-boundary.md` for the trait design, and `checkpoint-generalization.md` for the composition vs. batch checkpoint distinction.*
