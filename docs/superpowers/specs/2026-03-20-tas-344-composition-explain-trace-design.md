# TAS-344: Composition Explain Trace Output for Data Flow Visualization

**Ticket**: TAS-344
**Phase**: 3C (Validation Tooling)
**Predecessors**: TAS-333 (CompositionValidator), TAS-334 (CompositionExecutor), TAS-342/343 (grammar discovery tools)
**Successor**: TAS-345 (validate 3 modeled workflows through composition tooling pipeline)

## Problem

When compositions grow complex, developers and LLM agents need to understand how data flows from invocation to invocation. The existing `composition_validate` tool checks correctness but doesn't visualize the data threading chain. An agent drafting a composition in an MCP context needs to verify that its jaq expressions reference the right envelope fields and that data threads coherently through the chain — before committing to execution.

## Solution

Add a `composition_explain` tool that produces a structured trace of data flow through a composition. Two modes:

1. **Static analysis** (always): Traces structure, envelope field availability, expression references, output schemas, and checkpoint placement.
2. **Simulated evaluation** (opt-in): When sample data is provided, evaluates jaq expressions against real values and threads computed results through the chain, showing concrete data at each step.

## Architecture

Three layers following the pattern established in TAS-342/343:

```
tasker-grammar (ExplainAnalyzer)
    ↓ used by
tasker-sdk (grammar_query::explain_composition)
    ↓ called by
tasker-mcp (composition_explain tool) + tasker-ctl (grammar composition explain command)
```

### Layer 1: `tasker-grammar` — ExplainAnalyzer

New module `explain` in `tasker-grammar` containing the core analysis engine.

#### Core Types

```rust
/// Complete trace of data flow through a composition.
pub struct ExplanationTrace {
    /// Composition name (if declared).
    pub name: Option<String>,
    /// Declared outcome description and output schema.
    pub outcome: OutcomeSummary,
    /// Per-invocation trace entries, in execution order.
    pub invocations: Vec<InvocationTrace>,
    /// Validation findings (errors/warnings) from the underlying validator.
    pub validation: Vec<ValidationFinding>,
    /// Whether simulation was performed (sample data provided).
    pub simulated: bool,
}

/// Summary of the declared outcome.
pub struct OutcomeSummary {
    pub description: String,
    pub output_schema: Value,
}

/// Trace for a single capability invocation.
pub struct InvocationTrace {
    /// Position in the invocation chain (0-based).
    pub index: usize,
    /// Capability name.
    pub capability: String,
    /// Grammar category (Transform, Persist, etc.).
    pub category: String,
    /// Whether this is a checkpoint boundary.
    pub checkpoint: bool,
    /// Whether this capability is mutating.
    pub is_mutating: bool,
    /// Envelope fields available at this invocation.
    pub envelope_available: EnvelopeSnapshot,
    /// Jaq expressions found in config and which envelope paths they reference.
    pub expressions: Vec<ExpressionReference>,
    /// Declared output schema (if any — transforms declare this).
    pub output_schema: Option<Value>,
    /// Simulated output value (when sample data provided).
    pub simulated_output: Option<Value>,
    /// For side-effecting capabilities: whether a mock output was provided.
    pub mock_output_used: bool,
}

/// What's available in the envelope at a given point in the chain.
pub struct EnvelopeSnapshot {
    /// Always true — task-level input.
    pub context: bool,
    /// Always true — dependency step results.
    pub deps: bool,
    /// Always true — step metadata.
    pub step: bool,
    /// Whether .prev is non-null at this point.
    pub has_prev: bool,
    /// Description of what .prev contains (e.g., "output of invocation 0 (transform)").
    pub prev_source: Option<String>,
    /// Schema of .prev if known (from prior invocation's output schema).
    pub prev_schema: Option<Value>,
}

/// A jaq expression found in an invocation's config.
pub struct ExpressionReference {
    /// Config field path (e.g., "filter", "data.expression").
    pub field_path: String,
    /// The raw expression string.
    pub expression: String,
    /// Envelope paths referenced (e.g., [".context.order_id", ".prev.total"]).
    pub referenced_paths: Vec<String>,
    /// Simulated result value (when sample data provided).
    pub simulated_result: Option<Value>,
}
```

#### SimulationInput

```rust
/// Sample data for simulated evaluation.
pub struct SimulationInput {
    /// Sample task-level input — populates .context
    pub context: Value,
    /// Sample dependency results — populates .deps
    pub deps: Value,
    /// Sample step metadata — populates .step
    pub step: Value,
    /// Mock outputs for side-effecting invocations, keyed by invocation index.
    /// Used as .prev for the next invocation when the capability can't be
    /// evaluated purely (persist, acquire, emit).
    pub mock_outputs: HashMap<usize, Value>,
}
```

#### ExplainAnalyzer

```rust
pub struct ExplainAnalyzer<'a> {
    registry: &'a dyn CapabilityRegistry,
    expression_engine: &'a ExpressionEngine,
}

impl<'a> ExplainAnalyzer<'a> {
    pub fn new(
        registry: &'a dyn CapabilityRegistry,
        expression_engine: &'a ExpressionEngine,
    ) -> Self;

    /// Produce a static analysis trace (no expression evaluation).
    pub fn analyze(&self, spec: &CompositionSpec) -> ExplanationTrace;

    /// Produce a trace with simulated expression evaluation.
    pub fn analyze_with_simulation(
        &self,
        spec: &CompositionSpec,
        input: &SimulationInput,
    ) -> ExplanationTrace;
}
```

#### Analysis Flow

1. Run `CompositionValidator::validate()` and include findings in the trace.
2. Walk invocations in order. For each:
   a. Resolve capability from registry (skip if missing, note in validation findings).
   b. Build `EnvelopeSnapshot` — track what `.prev` is based on prior invocation's output.
   c. Extract jaq expressions from config fields (category-specific: `filter` for transform/assert, `data`/`params`/`payload` for action capabilities, plus `validate_success`, `result_shape`, metadata expressions for emit).
   d. For each expression, call `ExpressionEngine::extract_references()` to find envelope path references.
   e. Determine output schema (transform: `config.output`; others: future extension).
   f. **If simulating**: build the envelope from sample data + accumulated outputs, evaluate expressions, record results. For pure capabilities, use evaluated output as next `.prev`. For side-effecting capabilities, use mock output from `SimulationInput` if provided; otherwise `.prev` becomes `null`.
3. Assemble `ExplanationTrace`.

#### Expression Reference Extraction

New method on `ExpressionEngine`:

```rust
impl ExpressionEngine {
    /// Extract envelope field references from a jaq expression.
    /// Returns paths like [".context.order_id", ".prev.total", ".deps.step_a.result"]
    pub fn extract_references(&self, expression: &str) -> Result<Vec<String>, ExpressionError>;
}
```

**Implementation**: Parse the expression using jaq's parser, walk the AST looking for path access chains rooted at `.context`, `.deps`, `.prev`, or `.step`. Extract to practical depth — `.context.order_id` yes, but dynamic expressions like `.context | keys` report `.context` as the reference.

**Fallback**: If jaq AST internals prove too unstable, fall back to regex-based extraction scanning for `\.context\b`, `\.deps\b`, `\.prev\b`, `\.step\b` with subsequent dotted path segments. The interface is the same regardless of implementation strategy.

### Layer 2: `tasker-sdk` — Grammar Query Function

New function in `tasker_sdk::grammar_query`:

```rust
pub fn explain_composition(
    yaml_str: &str,
    simulation: Option<SimulationInput>,
) -> CompositionExplanation;
```

`CompositionExplanation` is a `Serialize`-deriving mirror of `ExplanationTrace`, following the same pattern as `CompositionValidationReport`. The function parses YAML/JSON, constructs `ExplainAnalyzer` with the standard capability registry and expression engine, and calls the appropriate analyze method.

`SimulationInput` is re-exported from `tasker-grammar` since it's just `Value` fields and a `HashMap`.

### Layer 3: MCP Tool and CLI Command

#### MCP: `composition_explain` (Tier 1 offline tool)

```rust
pub struct CompositionExplainParams {
    /// Composition YAML or JSON string.
    pub composition: String,
    /// Optional sample context data (JSON string, parsed to Value).
    pub sample_context: Option<String>,
    /// Optional sample deps data (JSON string).
    pub sample_deps: Option<String>,
    /// Optional sample step metadata (JSON string).
    pub sample_step: Option<String>,
    /// Mock outputs for side-effecting invocations (JSON string).
    /// Object keyed by invocation index: {"2": {"id": 123}, "4": {"status": "sent"}}
    pub mock_outputs: Option<String>,
}
```

Handler parses JSON params, constructs `SimulationInput` if any sample data is provided, calls `explain_composition()`, returns JSON.

Registered alongside the existing 13 Tier 1 tools, bringing the count to 14.

#### CLI: `tasker-ctl grammar composition explain`

```
tasker-ctl grammar composition explain <path> [--step <name>] \
    [--sample-context <json-or-path>] \
    [--sample-deps <json-or-path>] \
    [--mock-outputs <json-or-path>]
```

- `<path>` — path to composition YAML file
- `--step <name>` — extract a specific step's composition from a full template (same as `composition validate --step`)
- `--sample-context` — inline JSON or `@path/to/file.json`
- `--sample-deps` — inline JSON or `@path/to/file.json`
- `--mock-outputs` — inline JSON or `@path/to/file.json`

**Table output**: Shows invocation chain with columns for index, capability, category, checkpoint, `.prev` source, expressions, and (when simulating) simulated values.

**JSON output**: Full `CompositionExplanation` struct.

## Testing Strategy

### `tasker-grammar` (bulk of tests)

- **Static analysis**: Simple transform-only composition — verify envelope snapshots, expression references, output schema tracking
- **Multi-invocation chain**: Verify `.prev` source tracking progresses correctly
- **Mixed pure + side-effecting**: Verify checkpoint marking, mutating flags
- **Simulated evaluation**: Sample data threads through jaq expressions, simulated outputs recorded
- **Mock outputs**: Side-effecting invocations use mock values as `.prev` for subsequent steps
- **Missing mock output**: Graceful degradation — `.prev` becomes null, noted in trace
- **Invalid composition**: Partial trace produced alongside validation findings
- **`extract_references`**: Unit tests for various expression patterns (`.context.x`, `.prev | keys`, nested deps access, expressions with no envelope references)

### `tasker-sdk` (integration)

- Valid YAML, no simulation — verify serializable output structure
- Valid YAML with simulation — verify sample data flows through
- Invalid YAML — parse error handling

### `tasker-mcp` (assertion updates)

- Update Tier 1 tool count from 13 to 14
- Update protocol test expectations

### `tasker-ctl`

No dedicated tests — CLI handler is a thin wrapper.

## Non-Goals

- Schema-derived synthetic data generation — user provides sample data
- Full jaq expression static analysis (variable tracking, type inference) — bounded to path extraction
- Runtime tracing of actual execution — this is design-time tooling
- Backwards compatibility with the outdated InputMapping model
