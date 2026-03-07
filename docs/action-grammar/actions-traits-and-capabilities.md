# Actions, Traits, and Capabilities: Toward an Agent-Native Action Grammar

*Research spike for evolving Tasker's generative workflow architecture*

*Branch: `jcoletaylor/genai-workflows-research` — March 2026*

> **Revision note**: This document established the foundational grammar architecture (trait-based extensibility, three-layer model, singular-outcome constraint). The capability model has since been refined from 9 capabilities to 6, with **jaq-core** (Rust-native jq) as the unified expression language. See [`transform-revised-grammar.md`](transform-revised-grammar.md) for the current 6-capability model, composition context envelope, and virtual handler wrapper types.

---

## Motivation

Tasker's vision documents describe a phased path from templates-as-generative-contracts (Phase 0) through action grammars (Phase 1) to LLM planning interfaces and recursive workflows (Phases 2–3). Phase 0 is substantially complete — the MCP server, shared SDK tooling, multi-profile management, template validation, and handler scaffolding are operational. Before cutting tickets for Phase 1, we need to revisit the action grammar design in light of what we've learned — both from building Phase 0 and from the rapidly evolving landscape of agent-first development.

This document captures the problem framing, key insights, and architectural proposals from the research spike. It is a working document, not a specification. The goal is to arrive at a design we have confidence in before updating the vision documents and creating implementation tickets.

---

## The Changing Landscape

### What We're Observing

The agent-first experience is not a future state — it is the current reality in development tooling and increasingly in enterprise software. Several trends are converging:

**Agent-first interfaces are displacing curated UIs.** Enterprise customers are requesting MCP access directly to domain actions, preferring to work through Claude Desktop, ChatGPT, or similar agent interfaces rather than through carefully crafted web UIs. The affordances that web UIs provide — information display, guided data entry, notifications — are being subsumed by natural language interaction with intelligent partners. This is not a degradation; it is a shift to higher-fidelity interaction where typing-and-talking is more expressive of intent than any form-based experience.

**Agents solve problems dynamically.** The most effective agent workflows — whether in Claude Code, Cursor, Zed, or enterprise automation — are characterized by iterative, self-refining problem solving. Agents write scripts, use shell commands, research on the web, chain tools in novel combinations, and adapt their approach based on intermediate results. This creative, exploratory process is fundamentally different from executing a pre-planned workflow.

**The boundary between planning and execution is blurring.** In practice, agents don't cleanly separate "figure out what to do" from "do it." They investigate, try things, learn, adjust, and iterate. The most powerful agent capabilities emerge from this tight feedback loop, not from a two-phase plan-then-execute model.

### What This Means for Tasker

These observations create a productive tension with Tasker's core value proposition:

**Tasker's deterministic execution guarantees become *more* valuable, not less.** When agents bypass curated UIs and interact directly with domain actions, the guardrails that UIs provided — validation, sequencing, confirmation — disappear. Tasker provides the transactional, retryable, observable execution envelope that agents need when operating autonomously. The more autonomous the agent, the more critical it is that execution has strong guarantees.

**But the action grammar design must serve agent consumers.** The Phase 1 vision document describes compile-time Rust type checking, static composition validation, and a rigid single-mutation boundary. This is intellectually elegant but may be over-constraining for agent use cases. Agents need a vocabulary of capabilities they can discover and compose, not a type system they need to understand.

**Planning-time tools and execution-time actions are distinct concerns.** An agent's creative, exploratory problem-solving — writing scripts, researching, chaining tools — happens *outside* Tasker. Tasker's action grammar should not try to capture this. Instead, it should provide the richest possible vocabulary for expressing *what the agent has determined needs to happen* as a deterministic, reusable workflow.

---

## Key Insights

### 1. Grammars Define Categories; Vocabularies Define Capabilities

The original Phase 1 design conflates two concepts that should be separate layers:

**Action grammars** answer "what kind of action is this?" They define *categories* of action — Acquire, Transform, Validate, Persist, Emit — and declare what properties each category guarantees. A grammar category says: "things of this kind are non-mutating and idempotent" or "things of this kind require checkpointing and represent a commitment point."

**Capability vocabularies** answer "how will I perform this action?" They are the concrete, discoverable, composable surface that agents and planners interact with. An `acquire` capability fetches data from a named resource. A `persist` capability writes data to a named resource. Each capability declares its contracts via JSON Schema — input, output, mutation profile, checkpoint behavior — and is expressed as an (action, resource, context) triple.

The refined model defines **6 core capabilities**: `transform`, `validate`, `assert`, `persist`, `acquire`, `emit`. Pure data operations (projection, computation, boolean derivation, rule matching, grouping, ranking) are unified under a single `transform` capability powered by **jaq-core** (Rust-native jq). JSON Schema declares the output shape; a jaq filter declares how to produce it. This replaces bespoke configuration schemas with a single, well-understood expression language. See [`transform-revised-grammar.md`](transform-revised-grammar.md) for the full capability model.

This is a general-to-specific relationship. The grammar provides categorical guarantees; the vocabulary provides concrete implementations. They are two views of the same thing — the grammar is the implementation contract; the vocabulary is the consumer interface.

### 2. Singular Outcome, Not Singular Mutation

The Phase 1 document's single-mutation boundary — at most one external mutation per composition — is too rigid for real workflows, but the relaxation needs to be precise about *which* workflows benefit.

Many multi-mutation processes — payment rails, for example, where creating a hold, validating it, converting to a charge, and recording a receipt are each independently meaningful business actions — should absolutely remain discrete steps. Each has independent failure semantics, independent auditability, and independent retry behavior. Tasker's step model exists precisely because these are separately meaningful. The ~10ms orchestration overhead per step is negligible compared to the value of discrete lifecycle management.

The cases where interior checkpointing matters are different. Consider a step that runs a deep computational aggregation — a series of Apache AGE graph queries joined with third-party data sets, or a multi-phase statistical computation over a large corpus. The step might persist intermediate aggregation results as a checkpoint, not because those results are the business outcome, but because they represent expensive progress that shouldn't be lost on retry. The step then takes action on the final aggregated output. The interior mutations (caching partial results, persisting intermediate aggregations) are *instrumental* — they are side effects of doing the work well, not business actions in their own right. They serve the outcome but aren't the outcome themselves.

The better constraint: **a step's action composition must converge to a declared outcome, and any intermediate mutations that contribute to that outcome must be checkpointed so the step can resume rather than restart.** Interior mutations are expected to be instrumental — caching, intermediate persistence, progress tracking — rather than independently meaningful business actions. When mutations *are* independently meaningful, they belong in separate steps.

This aligns with existing Tasker capabilities. Batch step processing already has checkpoint semantics. Extending that to "any step with interior mutations must checkpoint" is a natural generalization. The step state machine already tracks progress — this makes tracking more granular for steps that declare multi-mutation compositions.

The safety properties are preserved:
- **Auditability**: Every checkpoint is recorded, every mutation is traceable
- **Retryability**: The step resumes from the last checkpoint, not from the beginning
- **Singular purpose**: The step still has one declared outcome; interior mutations are instrumental to that end
- **Observability**: Checkpoint progression is visible in step telemetry

### 3. JSON Schema Contracts as First-Class Constraints

The `result_schema` introduced in TAS-280 was advisory metadata for tooling. For virtual handlers composed from the capability vocabulary, JSON Schema contracts should be *mandatory and enforced*:

- Every capability declares its input and output contracts as JSON Schema
- Composition validation checks that contracts chain — output schema of step N is compatible with input schema of step N+1
- The MCP server and CLI expose contracts for agent discoverability
- Generated type scaffolding (Python Pydantic, Ruby Dry::Struct, TypeScript interfaces, Rust structs) derives from the same schemas

This is a natural evolution of what already exists. Task templates already use `input_schema` and `result_schema`. The step is making these schemas the load-bearing contract mechanism for composed capabilities, not just documentation.

### 4. No `Execute` Primitive — Extend the Vocabulary Instead

It is tempting to add an `Execute` action that runs arbitrary scripts or code within the grammar. This would be the equivalent of `eval` — a sharp enough tool that it undermines the grammar's guarantees.

The resolution is that Tasker already has the mechanism for arbitrary computation: **traditional handlers**. When an agent needs scriptable execution that doesn't map to the capability vocabulary, it uses tasker-py, tasker-rb, the TypeScript or Rust packages to create a handler, have it merged to the codebase, and register it against its callable. Virtual handlers composed from the vocabulary intermingle seamlessly with curated handlers in the same task template.

This creates healthy evolutionary pressure. Every time an agent can't express something through the vocabulary, that's a design signal: "this capability should exist as a first-class action." Over time the vocabulary grows to cover real needs through principled extension rather than escape hatches.

The vocabulary grows through two channels:
- **System evolution**: New grammar categories and native capabilities added to the core
- **Handler authoring**: Agents and developers create traditional handlers that fill vocabulary gaps

### 5. Trait-Based Extensibility via Build-from-Source

The grammar must not be a closed system. The core ships with fundamental grammar categories (Acquire, Transform, Validate, Persist, Emit) mapping to 6 capabilities (`acquire`, `transform`, `validate`, `assert`, `persist`, `emit`), but organizations need to extend both the grammar and the vocabulary for their domain.

Grammar categories should be defined by a `dyn GrammarCategory` trait, not a bounded enum. Organizations extend the system by depending on Tasker's public crates, implementing the traits, and building their own binary — the same model as custom handler registration today. A financial services company implements a domain-specific category, registers it at startup alongside standard handlers, and builds their worker binary from source.

This means both layers are extensible:
- **Grammar layer**: New action categories via trait implementations, registered at startup
- **Vocabulary layer**: New capabilities registered with JSON Schema contracts against existing or new grammar categories

No dynamic library loading (`.so`/`.dylib`) is needed. The traits are the contract; good documentation and public crate interfaces are the extension mechanism. This avoids the stable ABI problem, eliminates runtime loading failure modes, and uses standard Rust tooling. The trait boundary is object-safe and *could* support dynamic loading later if demand warrants, but build-from-source is the idiomatic Rust approach and matches Tasker's existing extensibility model.

### 6. Three Distinct Layers, Not Two Tiers

The original vision describes a "two-tier trust model" — developer-authored handlers vs. system-invoked action grammars. The evolved architecture has three layers that compose rather than compete:

**Layer 1: Action Grammars (trait-based, extensible)**
- Define categories of action and their guaranteed properties
- `dyn GrammarAction` trait with build-from-source extensibility (no dynamic library loading required)
- Core set: Acquire, Transform, Validate, Persist, Emit (5 grammar categories mapping to 6 capabilities — Validate encompasses both `validate` and `assert`)
- Each category declares: mutation profile, checkpoint requirements, idempotency characteristics
- Composition rules operate at this level — the validator knows what properties each category guarantees

**Layer 2: Capability Vocabulary (JSON Schema contracts, registered implementations)**
- Concrete capabilities categorized by their grammar
- Each capability declares input/output contracts via JSON Schema
- Backed by: native Rust implementations or registered polyglot handlers (build-from-source extensibility)
- Discoverable by agents through MCP tools — "what can I do, what does it accept, what does it produce?"
- Composition validation works here — JSON Schema contract chaining, checkpoint requirements, outcome singularity

**Layer 3: Handlers (the existing system, unchanged)**
- Traditional registered callables — Python, Ruby, TypeScript, Rust
- DSL patterns, class-based patterns, FFI dispatch
- Task templates freely mix virtual handlers (composed from vocabulary) with curated handlers (traditional callables)
- The orchestrator doesn't care about provenance — it's all steps with lifecycles

### 7. Planning and Execution Are Separate Concerns

The most important architectural boundary: **do not conflate the tools available to an agent during planning with the actions available during execution.**

**Planning layer** (outside Tasker): Agents use whatever tools they need — shell commands, Python scripts, web research, curl/jq, file system operations, MCP tools against external services. This is investigation and design. It's inherently non-deterministic, iterative, and creative. Tasker has no business constraining this.

**Execution layer** (inside Tasker): Once the agent has determined what needs to happen, it expresses that as a Tasker workflow composed from the capability vocabulary. This is deterministic execution. Retryable, auditable, traceable.

**The bridge**: The MCP server and capability vocabulary are how agents translate from "I've figured out what to do" to "here's a Tasker workflow that does it." The richer the vocabulary, the more an agent can express through deterministic execution rather than ad-hoc scripting.

---

## Architectural Proposal

### The Grammar Trait

The grammar trait is the load-bearing design decision. Everything else — specific categories, vocabulary format, composition rules — flows from what the trait requires.

A grammar action must declare enough about itself that:

1. The composition validator can check compositions involving it
2. The capability vocabulary can expose it to agents
3. The checkpoint system knows how to manage its interior mutations
4. The step state machine can track its progress
5. Plugin authors have a clear, stable contract to implement

```rust
/// The core trait that all grammar action categories implement.
/// This trait is object-safe for dynamic dispatch and build-from-source extensibility.
trait GrammarAction: Send + Sync + Debug {
    /// Human-readable name of this action category (e.g., "Acquire", "Transform")
    fn category_name(&self) -> &str;

    /// The mutation profile of this action category.
    /// Non-mutating actions can appear in any quantity.
    /// Mutating actions require checkpointing and contribute to outcome tracking.
    fn mutation_profile(&self) -> MutationProfile;

    /// Whether actions of this category are inherently idempotent.
    /// Non-idempotent actions require explicit idempotency strategies.
    fn idempotency(&self) -> IdempotencyProfile;

    /// Whether this action category requires checkpoint support
    /// for resumable execution.
    fn requires_checkpointing(&self) -> bool;

    /// The JSON Schema for this action category's configuration.
    /// Used to validate capability registrations against this grammar.
    fn config_schema(&self) -> &serde_json::Value;

    /// Validate that a capability's declared contracts are compatible
    /// with this grammar category's constraints.
    fn validate_capability(&self, capability: &CapabilityDeclaration) -> ValidationResult;
}

/// Mutation profile for grammar categories
enum MutationProfile {
    /// Never mutates external state. Idempotent by nature.
    /// Examples: Acquire, Transform, Validate
    NonMutating,

    /// May mutate external state. Requires checkpoint tracking.
    /// The step's singular outcome encompasses all mutations.
    /// Examples: Persist, Emit
    Mutating {
        /// Whether this mutation type supports idempotency keys
        supports_idempotency_key: bool,
    },

    /// Conditionally mutating based on configuration.
    /// Must declare at registration time whether a specific
    /// capability instance is mutating.
    /// Example: Emit with delivery_mode=fast may be fire-and-forget
    Conditional,
}

/// How a grammar category relates to idempotency
enum IdempotencyProfile {
    /// Inherently idempotent — safe to re-execute with same inputs
    Inherent,

    /// Idempotent with key — requires an idempotency key to prevent
    /// duplicate mutations
    WithKey,

    /// Requires explicit strategy — the capability must declare
    /// how it achieves idempotency
    RequiresStrategy,
}
```

### Capability Declaration

> **Updated**: The initial design anticipated a larger set of capabilities (reshape, compute, evaluate, evaluate\_rules, group\_by, rank, validate, assert, persist, acquire, emit). The refined model consolidates all pure data operations into a single `transform` capability using jaq-core. The 6 canonical capabilities are: `transform`, `validate`, `assert`, `persist`, `acquire`, `emit`. The struct below remains structurally valid — the `grammar_category` field references one of the 5 grammar categories (Acquire, Transform, Validate, Persist, Emit).

A capability is a concrete, registered implementation within a grammar category:

```rust
/// A registered capability in the vocabulary
struct CapabilityDeclaration {
    /// Unique identifier for this capability
    name: String,

    /// Which grammar category this capability belongs to
    grammar_category: String,

    /// Human-readable description for agent discoverability
    description: String,

    /// JSON Schema for what this capability accepts
    input_schema: serde_json::Value,

    /// JSON Schema for what this capability produces
    output_schema: serde_json::Value,

    /// Configuration schema for parameterizing this capability
    config_schema: serde_json::Value,

    /// Mutation profile (must be compatible with grammar category)
    mutation_profile: MutationProfile,

    /// Checkpoint behavior for multi-step execution
    checkpoint_behavior: CheckpointBehavior,

    /// Retry semantics
    retry_profile: RetryProfile,

    /// How this capability is implemented
    implementation: CapabilityImplementation,
}

/// How a capability is backed
enum CapabilityImplementation {
    /// Native Rust implementation (system-provided)
    Native { handler_fn: String },

    /// Registered polyglot handler (existing tasker handler system)
    Handler { callable: String },
}
```

### Composition Model

> **Updated**: The `input_mapping` field in `CompositionStep` below has been superseded by the **composition context envelope**. Instead of an `InputMapping` enum (Previous, StepOutput, TaskContext, Mapped, Merged), each capability invocation receives a unified context object with `.context` (task input), `.deps` (dependency results), `.prev` (previous capability output), and `.step` (step metadata). jaq filters operate directly on this envelope, eliminating the need for a separate input mapping abstraction. See [`transform-revised-grammar.md`](transform-revised-grammar.md) for the revised data flow model.

A composition chains capabilities toward a singular outcome:

```rust
/// A composed virtual handler
struct CompositionSpec {
    /// The declared outcome of this composition
    outcome: OutcomeDeclaration,

    /// Ordered sequence of capability invocations
    /// (may include branches via input_mapping)
    steps: Vec<CompositionStep>,

    /// Cross-cutting concerns applied to the composition
    mixins: Vec<String>,
}

/// A single step within a composition
struct CompositionStep {
    /// Which capability to invoke
    capability: String,

    /// Configuration for this invocation
    config: serde_json::Value,

    /// Where this step gets its input
    /// (previous step output, task context, or explicit mapping)
    input_mapping: InputMapping,

    /// Whether this step is a checkpoint boundary
    /// (required for mutating steps, optional for others)
    checkpoint: bool,
}

/// The declared singular outcome of a composition
struct OutcomeDeclaration {
    /// What this composition produces (JSON Schema)
    output_schema: serde_json::Value,

    /// Human-readable description of the outcome
    description: String,
}
```

### Composition Validation

The validator checks compositions at assembly time:

1. **Capability existence**: Every referenced capability exists in the vocabulary
2. **Configuration validity**: Each step's config matches the capability's config schema
3. **Contract chaining**: Output schema of step N is compatible with input schema of step N+1 (via JSON Schema compatibility checking)
4. **Checkpoint coverage**: Every mutating step is marked as a checkpoint boundary
5. **Outcome convergence**: The final step's output is compatible with the declared outcome schema
6. **Mixin compatibility**: Declared mixins are applicable to the composition's capability chain
7. **Grammar rule compliance**: Category-specific rules from the grammar trait are satisfied

Note what is *not* checked: a rigid single-mutation boundary. Multiple mutations are permitted, but each must be checkpointed. The constraint is singular *outcome*, not singular *mutation*.

---

## How This Differs from the Phase 1 Vision

| Aspect | Phase 1 Vision (Current) | Revised Proposal |
|--------|--------------------------|------------------|
| **Type system** | Compile-time Rust generics (`ActionPrimitive<Input, Output>`) | Runtime JSON Schema contracts with `dyn GrammarAction` trait |
| **Composition checking** | Compile-time (`B: ActionPrimitive<Input = A::Output>`) | Assembly-time JSON Schema compatibility |
| **Mutation rule** | Single-mutation boundary (at most one) | Singular-outcome with checkpointed interior mutations |
| **Extensibility** | Rust-native only; new primitives require recompilation | Build-from-source; public traits, custom binaries, same model as handler registration |
| **Consumer model** | Rust generics exposed through FFI wrappers | JSON Schema vocabulary exposed through MCP tools |
| **Agent UX** | Agent must understand Rust type composition | Agent discovers capabilities and composes via JSON specifications |
| **Escape hatch** | None (grammar is the only path) | Traditional handlers coexist seamlessly; no `Execute` primitive needed |
| **Planning boundary** | Implicit (planning produces compositions) | Explicit separation: planning is outside Tasker, execution is inside |

### What's Preserved

- **Deterministic execution**: Steps have lifecycles, retry semantics, transactional guarantees
- **Composability**: Capabilities compose into virtual handlers with validated contracts
- **Auditability**: Every capability invocation, checkpoint, and mutation is observable
- **The grammar concept**: Action categories with declared properties still exist and matter
- **Two-channel vocabulary growth**: System evolution + handler authoring
- **Agent-as-client pattern**: Agents use MCP for design-time, API for runtime

### What's Changed

- **From compiler to validator**: Composition correctness is checked at assembly time via JSON Schema, not at compile time via Rust generics. This trades some static safety for significant flexibility.
- **From type system to trait system**: The grammar is extensible via `dyn Trait` and plugins, not via generic type parameters.
- **From mutation counting to outcome tracking**: The safety invariant is "one purpose with checkpointed progress," not "one mutation."
- **From two tiers to three layers**: Grammar → Vocabulary → Handlers, where all three coexist in the same workflow.

---

## Research Questions for Follow-Up

### Trait Boundary Design
- What methods does `GrammarAction` need beyond what's sketched above?
- What crate should own the public traits? `tasker-shared` or a new `tasker-grammar` crate? **Resolved**: `tasker-grammar` — new workspace member, no dependency on crates/tasker-worker/orchestration/DB. See `implementation-phases.md`.
- Should there be sub-traits for specific grammar properties (e.g., `Checkpointable`, `Idempotent`)?
- How do grammar categories compose with each other? Can a composition step belong to multiple categories?

### Composition Validation
- What does JSON Schema "compatibility" mean precisely? Subset? Structural subtyping? Exact match? **Partially resolved**: The `transform` capability uses explicit `output` JSON Schema declarations, enabling contract chaining via schema comparison. See `composition-validation.md`.
- How should optional fields be handled in contract chaining? (Step A produces `{a, b?, c}`; Step B requires `{a, b}`)
- What error messages does the validator produce? How actionable are they for agents?
- What's the performance profile of JSON Schema validation at composition scale?

### Checkpoint Generalization
- How does the existing batch checkpoint model extend to arbitrary multi-mutation steps?
- What does the step state machine need to look like for "resume from checkpoint N"? **Partially resolved**: The composition context envelope includes `.prev` which is restored from checkpoint on resumption. See `transform-revised-grammar.md` "Checkpoint resumption" section.
- How are checkpoint states stored? Inline in step results? Separate checkpoint table?
- What happens when a checkpointed step is retried — does it resume from last checkpoint or restart?

### Vocabulary Evolution
- How do new capabilities get registered at runtime vs. at deployment time?
- What's the process for an organization to extend the vocabulary with domain-specific capabilities?
- How do capability versions work? Can a composition pin to a specific version?
- Should there be a capability registry service, or is this configuration-driven?

### Agent Composition UX
- What does an agent's MCP interaction look like when composing a workflow? Walk through concrete scenarios.
- How does the agent iterate on a composition when validation fails?
- Should the MCP server offer "suggest capabilities for this task" functionality?
- How does the agent know when to compose from the vocabulary vs. when to write a traditional handler?

### Resolved in Subsequent Documents
- **Expression language**: jaq-core (Rust-native jq) selected as the unified expression language for all data transformation. See `transform-revised-grammar.md`.
- **Capability consolidation**: 9 capabilities refined to 6. Pure data operations unified under `transform`. See `transform-revised-grammar.md`.
- **Input mapping**: `InputMapping` enum replaced by composition context envelope (`.context`, `.deps`, `.prev`, `.step`). See `transform-revised-grammar.md`.
- **Decision and batch step grammar scope**: Virtual handler wrapper types (`DecisionCompositionHandler`, `BatchAnalyzerCompositionHandler`, `BatchWorkerCompositionHandler`) bridge grammar JSON output to orchestration protocol types. See `transform-revised-grammar.md` "Virtual Handler Wrapper Types" section.

---

## Relationship to Vision Documents

This research spike proposes revisions to several vision documents:

- **Phase 1 (Action Grammars)**: Substantial revision — from compile-time to trait-based, from single-mutation to singular-outcome, from closed to plugin-extensible
- **Technical Approach**: Section updates to reflect the three-layer model and revised composition validation approach
- **Agent Orchestration**: Minor updates — the planning/execution boundary is now more explicitly articulated
- **Vision overview**: The "two trust tiers" section should evolve to reflect the three-layer model

These revisions should be made after the research spike produces concrete proposals for the trait boundary, composition validation, and checkpoint generalization — the three load-bearing design decisions.

---

## Next Steps

1. **Phase 0 completion assessment** — Verify where we stand against Phase 0's validation criteria
2. **Trait boundary proposal** — Detailed design for `GrammarAction` and related traits
3. **Composition validation proposal** — JSON Schema contract chaining mechanics with worked examples
4. **Checkpoint generalization proposal** — Extension of batch checkpoint model to multi-mutation compositions
5. **Vocabulary registration proposal** — How capabilities are declared, discovered, and versioned
6. **Vision document updates** — Revise Phase 1 and related documents based on agreed proposals
7. **Ticket creation** — Cut implementation tickets from the agreed design

---

*This is a living research document. It will be updated as proposals are developed and refined through the spike.*
