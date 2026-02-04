# Skill: Project Principles and Tenets

## When to Use

Use this skill when making design decisions, reviewing code, evaluating architectural alternatives, or understanding why something was designed a particular way in tasker-core. These tenets should inform all code contributions.

## The 11 Tenets

### 1. Defense in Depth

Multiple overlapping protection layers provide robust idempotency without single-point dependency.

Four layers: Database atomicity -> State machine guards -> Transaction boundaries -> Application filtering.

**Rule**: Find the minimal set of protections that prevents corruption. Additional layers that prevent recovery are worse than none.

### 2. Event-Driven with Polling Fallback

Real-time responsiveness via PostgreSQL LISTEN/NOTIFY, with polling as a reliability backstop.

Three deployment modes: Hybrid (recommended), EventDrivenOnly, PollingOnly.

### 3. Composition Over Inheritance

Mixins and traits for handler capabilities, not class hierarchies.

```
Not: class Handler < API
But: class Handler < Base; include API, include Decision, include Batchable
```

### 4. Cross-Language Consistency

Unified developer-facing APIs across Rust, Ruby, Python, and TypeScript. Same patterns, idiomatic syntax per language.

Consistent: handler signatures `call(context)`, result factories `success()`/`failure()`, registry APIs.

### 5. Actor-Based Decomposition

Lightweight actors for lifecycle management and clear boundaries. Orchestration uses 4 actors, worker uses 5 actors.

### 6. State Machine Rigor

Dual state machines (Task 12 states + Step 9 states) for atomic transitions with full audit trails.

All transitions are atomic (CAS), audited (transitions table), and validated (state guards).

### 7. Audit Before Enforce

Track processor UUID for observability. Don't enforce it for ownership.

**Key insight (TAS-54)**: Ownership enforcement was removed because it blocked crash recovery while providing zero additional safety over layers 1-4.

### 8. Pre-Alpha Freedom

Break things early to get architecture right. Breaking changes are encouraged when architecture is fundamentally unsound. No backward compatibility required for greenfield work.

### 9. PostgreSQL as Foundation

Database-level guarantees with flexible messaging. PostgreSQL provides state storage, advisory locks, atomic functions, row-level locking. Messaging is pluggable (PGMQ default, RabbitMQ optional).

### 10. Bounded Resources

All channels bounded, backpressure everywhere.

**Rule**: Never use `unbounded_channel()`. Always configure bounds via TOML.

### 11. Fail Loudly

A system that lies is worse than one that fails. Errors are first-class citizens.

**Rule**: Never use `unwrap_or_default()` for required fields. Use `ok_or_else(|| ClientError::invalid_response(...))` instead.

| Wrong | Right |
|-------|-------|
| Return default value for missing field | Return `InvalidResponse` error |
| Use empty/zero defaults for absent config | Fail with clear message |
| Fabricate "unknown" status for missing data | Error: "data unavailable" |

## Twelve-Factor App Influence

The project's systems design is substantively informed by the [Twelve-Factor App](https://12factor.net/) methodology. Key alignments:

- **Config (III)**: Environment variables with TOML defaults — `config/tasker/base/`, `config/dotenv/`
- **Backing services (IV)**: Pluggable messaging (PGMQ/RabbitMQ), cache (Redis/Moka), observability (OTEL) — all via env vars
- **Processes (VI)**: Stateless services, all state in PostgreSQL — no in-memory workflow state
- **Disposability (IX)**: Graceful shutdown with signal handlers, PGMQ visibility timeouts for crash recovery
- **Dev/prod parity (X)**: Same config structure, same migrations, same Docker base across environments
- **Logs (XI)**: Stdout-only via tracing crate, structured fields, correlation IDs

When reviewing designs, ask: does this treat backing services as attached resources? Does this store state in processes or in the database? Is this configured via environment?

Full mapping with codebase references and gap assessment: `docs/principles/twelve-factor-alignment.md`

## Meta-Principles

1. **Simplicity Over Elegance**: Minimal protections that work > layered defense that blocks recovery
2. **Observation-Driven Design**: Let real behavior guide architecture
3. **Explicit Over Implicit**: Make boundaries and decisions visible
4. **Consistency Without Uniformity**: Align APIs while preserving language idioms
5. **Separation of Concerns**: Orchestration handles state; workers handle execution
6. **Errors Over Defaults**: When in doubt, fail with a clear error

## Applying Tenets in Code Reviews

When reviewing code, check:
1. **Bounded resources**: Are all channels bounded? All concurrency limited?
2. **State machine compliance**: Do transitions use atomic database operations?
3. **Language consistency**: Does the API align with other language workers?
4. **Composition pattern**: Are capabilities mixed in rather than inherited?
5. **Fail loudly**: Are missing/invalid data handled with errors, not silent defaults?

## Applying Tenets in Design Decisions

1. Check against tenets: Does this violate any of the 11 tenets?
2. Find precedent: Has a similar decision been made before? (See ticket-specs)
3. Document the trade-off: What are you gaining and giving up?
4. Consider recovery: If this fails, how does the system recover?

## Linting Standards (TAS-58)

- Microsoft Universal Guidelines + Rust API Guidelines enforced
- Use `#[expect(lint_name, reason = "...")]` instead of `#[allow]`
- All public types must implement `Debug`

## References

- Core tenets: `docs/principles/tasker-core-tenets.md`
- Defense in depth: `docs/principles/defense-in-depth.md`
- Fail loudly: `docs/principles/fail-loudly.md`
- Composition: `docs/principles/composition-over-inheritance.md`
- Cross-language: `docs/principles/cross-language-consistency.md`
- Chronology: `docs/CHRONOLOGY.md`
