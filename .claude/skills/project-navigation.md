# Skill: Project Navigation

## When to Use

Use this skill when searching the codebase, investigating issues, or deciding which documentation to consult for a given question. It captures Tasker-specific terminology, investigation workflows, and documentation structure knowledge.

## Tasker-Specific Terminology

When searching the codebase, use Tasker vocabulary for better results:

| Search For | Not |
|-----------|-----|
| "step handler" | "handler class" |
| "decision point" | "conditional" |
| "batch cursor" | "pagination" |
| "checkpoint yield" | "save progress" |
| "domain event" | "message" |
| "orchestration actor" | "processor" |
| "workflow step" | "job" or "task item" |
| "task template" | "workflow definition" |

## Don't Search -- Use General Knowledge

Use training knowledge directly for these topics (don't search Tasker docs):

- General Rust/Ruby/Python/TypeScript language questions
- PostgreSQL syntax and standard operations
- Basic git, cargo, bundler, npm, bun commands
- HTTP status codes, REST conventions
- Standard library usage
- Tokio async patterns (general)
- SQLx usage patterns (general)

Only search Tasker-specific documentation for Tasker-specific concepts.

## Documentation Hierarchy

The docs follow a "why / what / how" hierarchy:

| Layer | Directory | Purpose | When to Read |
|-------|-----------|---------|--------------|
| **Why** | `docs/principles/` | Core values, design philosophy | Design decisions, trade-offs |
| **What** | `docs/architecture/` | System structure, patterns | Understanding components |
| **How** | `docs/guides/` | Practical implementation | Implementing features |
| **Language** | `docs/workers/` | Handler development per language | Writing handlers |
| **Reference** | `docs/reference/` | Technical specifications | Precise details, edge cases |
| **Standards** | `docs/development/` | Coding standards, tooling | Build system, code quality |
| **Ops** | `docs/operations/` | Production guidance | Deployment, monitoring |

## Common Investigation Patterns

### "Why doesn't X work?"

1. Check `docs/architecture/states-and-lifecycles.md` for valid state transitions
2. Check `docs/guides/dlq-system.md` if task appears stuck
3. Check `docs/guides/retry-semantics.md` for error handling behavior

### "How do I implement Y?"

1. Check `docs/workers/patterns-and-practices.md` for patterns
2. Check `docs/workers/{language}.md` for language specifics
3. Check `docs/workers/example-handlers.md` for code examples

### "Why was Z designed this way?"

1. Check `docs/principles/tasker-core-tenets.md` for relevant tenet
2. Check `docs/CHRONOLOGY.md` for historical context
3. Check `docs/ticket-specs/TAS-*/` if specific ticket referenced

### "How do I add a new capability?"

1. Check `docs/principles/composition-over-inheritance.md` for pattern
2. Check `docs/workers/api-convergence-matrix.md` for API alignment
3. Check `docs/principles/cross-language-consistency.md` for multi-language considerations

### "How do I checkpoint long-running batch work?"

1. Check `docs/guides/batch-processing.md#checkpoint-yielding-tas-125` for full guide
2. Check `docs/workers/patterns-and-practices.md#checkpoint-yielding` for code patterns

## Ticket-Spec Patterns

- Active tickets reference `docs/ticket-specs/TAS-XXX/` for detailed specs
- Key insights are extracted to `docs/principles/` and `docs/CHRONOLOGY.md`
- Don't read full ticket-specs directories unless specifically asked
- Ticket-specs are historical records; principles docs are the living reference

## Crate-Level Context

Each major crate has its own `AGENTS.md` with module-level detail:

| Crate | AGENTS.md | Focus |
|-------|-----------|-------|
| tasker-orchestration | `tasker-orchestration/AGENTS.md` | Actor pattern, services, state machines |
| tasker-worker | `tasker-worker/AGENTS.md` | Handler dispatch, FFI, completions |

## Deep-Dive Reference Routing

For topics that need more detail than skills provide, consult these docs:

| Topic | Primary Document | Secondary |
|-------|-----------------|-----------|
| Retry, backoff, error handling | `docs/guides/retry-semantics.md` | `docs/architecture/circuit-breakers.md` |
| Batch processing, cursors | `docs/guides/batch-processing.md` | |
| Stuck/failing tasks | `docs/guides/dlq-system.md` | |
| Event systems, PGMQ | `docs/architecture/events-and-commands.md` | `docs/architecture/domain-events.md` |
| Concurrency, backpressure | `docs/architecture/backpressure-architecture.md` | `docs/development/mpsc-channel-guidelines.md` |
| Cross-language API alignment | `docs/workers/api-convergence-matrix.md` | `docs/principles/cross-language-consistency.md` |
| Auth and security | `docs/auth/README.md` | `docs/guides/api-security.md` |

## References

- Documentation hub: `docs/README.md`
- Project context: `CLAUDE.md`
- Core tenets: `docs/principles/tasker-core-tenets.md`
- Chronology: `docs/CHRONOLOGY.md`
