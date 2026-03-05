# Case Study: Grammar Proposals for Test Fixture Handlers

*Expressing test fixture handler logic as grammar compositions, and identifying orchestration boundaries*

*March 2026 — Research Spike*

---

## Approach

The `tests/fixtures/` handlers test Tasker's core orchestration features — mathematical DAGs, decision routing, batch processing, checkpoint resumption. Many are deliberately simple (single-operation handlers for verifiable math). This case study evaluates each handler's internal logic for grammar expressibility, and draws sharper lines around what grammars can and cannot represent.

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
  - capability: compute
    config:
      operations:
        - select: "$"
          derive: { result: "value ^ 2" }
    input_mapping: { type: previous }
```

**Two-input convergence** (DiamondEnd, DagValidate):

```yaml
grammar: Transform
compose:
  - capability: reshape
    config:
      fields:
        a: "branch_b.result"
        b: "branch_c.result"
    input_mapping: { type: task_context }

  - capability: compute
    config:
      formula: "(a * b) ^ 2"
    input_mapping: { type: previous }
```

**Four-input convergence** (TreeFinalConvergence):

```yaml
grammar: Transform
compose:
  - capability: reshape
    config:
      fields:
        d: "leaf_d.result"
        e: "leaf_e.result"
        f: "leaf_f.result"
        g: "leaf_g.result"
    input_mapping: { type: task_context }

  - capability: compute
    config:
      formula: "(d * e * f * g) ^ 2"
      precision: arbitrary  # BigUint needed for large numbers
    input_mapping: { type: previous }
```

**Triple convergence with verification** (DagFinalize):

```yaml
grammar: Transform
compose:
  - capability: reshape
    config:
      fields:
        validate: "dag_validate.result"
        transform: "dag_transform.result"
        analyze: "dag_analyze.result"
    input_mapping: { type: task_context }

  - capability: compute
    config:
      formula: "(validate * transform * analyze) ^ 2"
      overflow_handling: saturate
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        result_matches_expected:
          expr: "computed == original ^ 64"
      quantifier: all
      on_failure: fail
      original_source: task_context.even_number
    input_mapping:
      type: merged
      sources:
        - { type: step_output, index: 1 }
        - { type: task_context }
```

### Assessment

**Are these good composition candidates?** No. Each mathematical handler is a single operation — the composition adds ceremony without value. The `reshape` + `compute` pattern is two capabilities for what is one line of code in a handler function.

**What they *do* validate**: The grammar system must handle multi-input convergence (reshaping 2, 3, or 4 dependency results into a single structure) and arbitrary-precision arithmetic. The `reshape` + `compute` pattern works structurally, even if these specific handlers don't benefit from it.

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
grammar: Transform  # or a hypothetical "Decide" grammar
compose:
  - capability: evaluate_rules
    config:
      rules:
        - condition: "amount < 1000"
          result: { route: auto_approval, steps: [auto_approve] }
        - condition: "amount < 5000"
          result: { route: manager_only, steps: [manager_approval] }
        - condition: "amount >= 5000"
          result: { route: dual_approval, steps: [manager_approval, finance_review] }
      first_match: true
    input_mapping: { type: task_context }
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
  - capability: compute
    config:
      static:
        approved: true
        approval_type: automatic
        approved_by: system
    input_mapping: { type: previous }
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
    input_mapping: { type: previous }

  - capability: compute
    config:
      fields:
        approved: true
        approval_type: manager
        approved_by: { type: generate, prefix: "mgr_" }
    input_mapping: { type: previous }
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

A hypothetical composition would use `compute` for the arithmetic:

```yaml
grammar: Transform
compose:
  - capability: compute
    config:
      operations:
        - derive:
            worker_count: "min(ceil(dataset_size / batch_size), max_workers)"
    input_mapping: { type: task_context }
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
  - capability: reshape
    config:
      fields:
        batch_results: "process_batch_*.result"
      flatten: true
    input_mapping: { type: task_context }

  - capability: compute
    config:
      operations:
        - derive:
            total_processed: "sum(batch_results, 'processed_count')"
            total_checkpoints: "sum(batch_results, 'checkpoint_count')"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        all_items_processed:
          expr: "total_processed == expected_dataset_size"
      quantifier: all
      on_failure: fail
    input_mapping:
      type: merged
      sources:
        - { type: previous }
        - { type: task_context }
```

**Assessment**: Three canonical capabilities — `reshape` to project batch worker results into a flat structure, `compute` for summation, and `assert` to validate totals. The pattern uses only canonical grammar primitives. However, the NoBatches vs. WithBatches scenario detection is specific to Tasker's batch lifecycle (deferred convergence detection), which stays outside grammar scope. This composition would only cover the WithBatches path; the handler would still need domain logic for scenario branching.

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
  - capability: identity
    config: {}
    input_mapping: { type: task_context }

  - capability: fail_n_times
    config:
      fail_count: 2
    input_mapping: { type: previous }

  - capability: identity
    config: {}
    input_mapping: { type: previous }
```

This would test composition retry behavior: the second capability fails twice, causing two step-level retries. On the third attempt, it succeeds and the composition completes. If capability 1 were checkpointed, it would be skipped on retry.

---

## Diamond-Decision-Batch Combined Handler

### RoutingDecisionHandler (in diamond-decision-batch context)

**Internal logic**:
1. Get counts from both diamond branches (evens count, odds count)
2. Route based on comparison: `even_count >= odd_count → "even" else "odd"`
3. Create steps via `DecisionPointOutcome`

### Grammar Proposal

Same as the standalone decision handler — decision protocol output keeps this outside grammar scope. But the *input reshaping* pattern (two branches → comparison → routing) is exactly what `reshape` + `evaluate_rules` express well:

```yaml
compose:
  - capability: reshape
    config:
      fields:
        evens: "branch_evens.count"
        odds: "branch_odds.count"
    input_mapping: { type: task_context }

  - capability: evaluate_rules
    config:
      rules:
        - condition: "evens >= odds"
          result: { route: even, steps: [even_batch_analyzer] }
        - condition: "true"
          result: { route: odd, steps: [odd_batch_analyzer] }
    input_mapping: { type: previous }
```

The composition *expresses* the logic well; the issue is only the output protocol requirement.

---

## Synthesis: Handler Complexity vs. Grammar Value

### Complexity Distribution

From the handler survey, handlers fall into four complexity bands:

| Band | Operations | Examples | Composition Value |
|------|-----------|----------|-------------------|
| **Trivial** (1 op) | Single arithmetic or assignment | Math steps, auto-approve, assertions | **None** — composition adds overhead |
| **Simple** (2-3 ops) | Validate + compute, or reshape + compute | Branch handlers, simple approval | **Low** — handler is already clear |
| **Moderate** (4-6 ops) | Multi-step validation, aggregation, rule evaluation | Cart validation, insights, policy checks | **Medium** — composition enables configurability |
| **Complex** (7+ ops) | Stateful loops, multi-source aggregation, checkpoint management | Batch workers, convergence handlers | **Low** — execution model doesn't fit |

### The Sweet Spot

Grammar compositions add the most value for **moderate-complexity handlers** where:
1. Internal logic follows a recognizable pattern (validate → compute → persist)
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
| `reshape` | Convergence handlers (2, 3, 4 inputs) | High — multi-source projection is common |
| `compute` | Math handlers, derived metrics, aggregation sums | High — unified formula evaluation |
| `evaluate` | Conditional routing, boolean determination | High — evaluability primitive |
| `assert` | Math verification, total validation, execution gates | High — cross-validation and precondition checks |
| `evaluate_rules` | Decision routing, approval paths | High — configurable first-match rule engine |
| `validate` | Input schema enforcement, approval checks | High — trust boundary with coercion |
| `fail_n_times` | Error testing | Test utility only |

---

*This case study should be read alongside `workflow-patterns.md` for the contrib handler grammar proposals, `grammar-trait-boundary.md` for the trait design, and `checkpoint-generalization.md` for the composition vs. batch checkpoint distinction.*
