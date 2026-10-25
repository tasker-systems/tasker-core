# Rust

Applies to every tasker-systems Rust workspace. Canonical config is in [`../rust/`](../rust/). Each
rule's enforcement status per repo is in [`rules.md`](rules.md). Rules adopted or adapted from
Google's Rust style guide are traced, with what was rejected and why, in
[`../sources/google/rust.md`](../sources/google/rust.md).

## Scope: library and application crates

**RUST-API-1 — API-design rules bind library crates; for application crates they are defaults.** A
*library crate* is one that is published (`publish` is not `false`) or consumed from another
repository — the clients and SDKs. Everything else — servers, workers, CLIs, internal service
crates — is application code. The rules in [API shape](#api-shape) are binding for the first and a
strong default for the second, where directness beats generality: no speculative `impl Into<T>`,
no iterator-returning signature when every caller collects. *Why:* a library's signature is a
promise to callers this repository cannot see or fix; an application's is not, and generalising it
early costs readability for flexibility no one uses. (Google Rust style guide, "Apply API design
rules in context".)

## Lints

**RUST-LINT-1 — one workspace lint table.** The workspace `Cargo.toml` carries the table in
[`rust/workspace-lints.toml`](../rust/workspace-lints.toml), and every member crate sets
`[lints] workspace = true`. *Why:* lints configured per crate, or only on a command line, differ
between crates and between the developer's machine and CI. One table is one stance.

**RUST-LINT-2 — warnings fail the build.** Clippy and rustdoc run with `-D warnings` on every pull
request, not only before a release. *Why:* a warning that never fails is noise everyone learns to
scroll past; the lint table means nothing if its findings can merge.

**RUST-LINT-3 — `#[expect]` with a reason, never `#[allow]`.** A lint is silenced with
`#[expect(lint, reason = "...")]`. `clippy::allow_attributes` and
`clippy::allow_attributes_without_reason` enforce it. *Why:* an `#[expect]` reports when it stops
being needed, so dead suppressions surface; `#[allow]` hides a problem forever. The reason is for
the next reader, who otherwise cannot tell a considered exception from a silenced bug. (Microsoft
Pragmatic Rust Guidelines, M-LINT-OVERRIDE-EXPECT.)

*The one exception:* a helper module compiled into several test binaries (`tests/common/…`) can
carry an item that is dead in some binaries and live in others, so an `#[expect(dead_code)]` is
unfulfilled in some of them. That site takes `#[allow(dead_code, reason = "...")]`, the reason
naming the shared-module cause.

**RUST-LINT-4 — adopting the table is audit-before-enforce.** When a repo first adopts the table, or
the table gains a lint, existing findings are fixed or given a reasoned `#[expect]` before the lint
gates. A blanket crate-level `#![allow]` to get green is not adoption.

**RUST-LINT-5 — a held-off group is opened lint by lint.** The `style`, `complexity`, `perf` and
`pedantic` groups stay off (see [Not specified](#not-specified-in-v0)), but a named lint from one of
them can be turned on alone when it enforces a rule written here. The canonical table lists such
lints with the rule each one serves; [`rust/workspace-lints.toml`](../rust/workspace-lints.toml)
carries the candidates, with their measured finding counts, until each is ruled in. *Why:* a whole
group arrives as hundreds of findings about taste; a single lint arrives as the mechanical half of a
rule someone already agreed to.

## Formatting and imports

**RUST-FMT-1 — rustfmt, default configuration, checked in CI.** No `rustfmt.toml`: the defaults
(100 columns among them) are the style. `#[rustfmt::skip]` is for deliberate alignment — a table,
a matrix — or generated code, never for a personal preference. *Why:* formatting is the one style
question a tool answers completely, so nothing about it should reach a review. The rustfmt options
Google's guide also sets (`imports_granularity`, `hex_literal_case`, `format_macro_matchers`) are
nightly-only, and [RUST-TC-1](#toolchain) floats on stable.

**RUST-ORG-1 — no glob imports.** Name what is imported. Allowed: `use super::*` in a test module;
an enum's variants inside a function that is about that enum; a third-party crate's documented
`prelude`. A repository does not define a prelude of its own. `clippy::wildcard_imports` enforces it
(its default configuration already exempts the first and third). *Why:* a glob hides where a name
comes from — for a reader without an IDE, and for an agent reading one file at a time, that is
every reader — and a new export upstream can silently shadow a local name.

**RUST-ORG-2 — a file is named for what it holds; non-leaf modules only declare and re-export.**
A leaf module whose main item is `Foo` is `foo.rs`. A `mod.rs` or `lib.rs` with submodules holds
`mod` declarations, `pub use` re-exports and, at most, a little glue. *Why:* the file tree is the
index; an item defined in the module that also organises its siblings is the one nobody finds.

## Unsafe

**RUST-UNSAFE-1 — unsafe is forbidden by default (hard rule).** Every crate root declares
`#![forbid(unsafe_code)]`. *Why:* almost all of both workspaces needs no unsafe, and `forbid`
cannot be undone by an inner attribute, so a crate that has none keeps none.

**RUST-UNSAFE-2 — an unsafe site is a registered exception.** A crate that genuinely needs unsafe
omits the `forbid`, and is listed in the repo's unsafe-exception register. The workspace table sets
`unsafe_code = "deny"`, so every site must carry `#[expect(unsafe_code, reason = "...")]`, and
`clippy::undocumented_unsafe_blocks` requires a `// SAFETY:` comment on each block. A tripwire pins
both sets — crates without `forbid`, and `expect(unsafe_code)` sites — so a new one fails CI until
it is reviewed into the register. *Why:* this is how "forbid by default" stays a hard rule while the
few legitimate sites (CPU feature probes, binding glue) stay reviewable.

A register entry answers two questions: why the same thing cannot be written in safe code, and — if
the answer is performance — where the benchmark is. The unsafe sits behind a safe API, isolated from
business logic, ideally in a small module or crate of its own. *Why:* "it was faster" without a
measurement is a guess, and unsafe mixed into domain code puts every edit to that code inside the
safety argument.

**RUST-UNSAFE-3 — FFI goes through binding frameworks.** Ruby, Python and JavaScript bindings use
magnus, PyO3 and napi-rs rather than hand-written `extern` glue. *Why:* the frameworks carry the
unsafe and its review; tasker-core's three FFI crates hold no hand-written unsafe as a result.

## Errors

**RUST-ERR-1 — typed errors in libraries, `anyhow` at the edges.** Library crates define errors
with `thiserror` and a `Result<T>` alias; `anyhow` is for binaries, scripts, tests and the outermost
glue. *Why:* a caller can only handle what it can name.

**RUST-ERR-2 — no silent defaults for required values.** `unwrap_or_default()` and friends are for
values that are genuinely optional. A required field that is missing is an error
(`ok_or_else(...)`). *Why:* a default that stands in for a missing value turns a loud failure into
quietly wrong data (tasker-core's Fail Loudly tenet).

**RUST-ERR-3 — panics are for bugs.** Libraries do not panic on bad input. An `expect("...")` is
acceptable where its message states the invariant that makes it unreachable. The choice of channel
follows the contract: `Option` when the cause is obvious and the caller needs nothing more,
`Result` when the caller must distinguish failures, a panic only for a violated precondition — a
bug to fix, not a condition to handle. A public function of a library crate that can panic says
when, in a `# Panics` section.

**RUST-ERR-4 — no unchecked `unwrap` outside tests.** Production code does not call `unwrap()`. A
value whose presence the code has just established is destructured (`if let`, `let … else`,
`match`), not tested and then unwrapped. Where an invariant cannot be expressed in the types,
`expect("why this holds")` states it. Tests may unwrap. `clippy::unwrap_used` (with
`allow-unwrap-in-tests`) and `clippy::unnecessary_unwrap` enforce it. *Why:* an `unwrap` is an
`expect` whose reason was never written, and the panic it produces names a line, not the broken
assumption. Pattern matching makes the compiler check the assumption instead of the reader.

## Panics

**RUST-PANIC-1 — `catch_unwind` is defense in depth, and only works if the build unwinds.** Code
never relies on catching a panic for correctness: a panic that is caught is still a bug, and is
reported as one. Catching one at a supervision boundary, so that one bad handler or command does not
kill the process, is a legitimate second line ([CONC-FAIL-1](concurrency.md#failure-is-heard)) — but
only in a binary built with `panic = "unwind"`. Under `panic = "abort"`, `catch_unwind` compiles and
does nothing: the process aborts. So a workspace with a production `catch_unwind` keeps every
shipped profile on `unwind`, or drops the catch and lets the process supervisor restart it. A
tripwire can pin this: no shipped profile sets `abort` while a non-test `catch_unwind` exists.
*Why:* tests run in the `dev` profile, which unwinds, so a containment test passes while the
release binary it describes aborts. tasker-core shipped exactly this: three containment sites
(TAS-228's command processor and queue listener, the worker's handler dispatch) and
`[profile.release] panic = "abort"`. Google's guide forbids `catch_unwind` for the same reason —
its targets build with `abort` — and allows it only as an unrelied-upon backstop.

A catch that **observes and resumes** — catches the unwind, does its cleanup, then
`resume_unwind`s — changes nothing a caller sees and is fine in either profile: under `abort` it
simply never runs. temper's request-span flush (`temper-telemetry/src/request_span.rs`) is the
pattern.

## Types

**RUST-TYPE-1 — public types implement `Debug`.** Enforced by `missing_debug_implementations`.
Types that carry a credential implement it by hand, redacting the secret (see
[`security.md`](security.md)).

**RUST-TYPE-2 — a value that could be mistaken for another is its own type.** Identifiers,
quantities with units, and anything else easy to swap with a sibling of the same representation
are newtypes, not aliases; a type alias is only a shorter spelling. A `bool` parameter whose meaning
is not obvious at the call site is a two-variant `enum`. Structs with more than one field name
their fields rather than being tuples. *Why:* `transfer(from, to)` with two `Uuid`s compiles either
way round; with `AccountId` and `LedgerId` it does not. This is [tenet 11](tenets.md)'s
"credentials are a type" applied to everything else that matters.

## API shape

Binding for library crates, a default for application crates
([RUST-API-1](#scope-library-and-application-crates)).
Naming and conversions follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/);
the rules below are the ones a lint or a review can hold.

**RUST-API-2 — published types that may grow are `#[non_exhaustive]`; workspace-internal types are
not.** A public enum or public-field struct in a library crate is `#[non_exhaustive]` unless it
describes a closed set (`Visibility { Visible, Hidden }`). Inside one workspace, types stay
exhaustive. *Why:* across a release boundary, adding a variant or field breaks callers this
repository cannot fix. Inside a workspace the opposite is true: an exhaustive `match` is the tool
that finds every site a new variant must reach, and `#[non_exhaustive]` would turn those into
silent wildcard arms.

**RUST-API-3 — standard traits over bespoke methods; derive over hand-written impls.** A
`from_str` method is an `impl FromStr`; a `new()` with no arguments comes with `Default`; `len()`
comes with `is_empty()`; an impl that a `#[derive]` would produce is a derive. Lints:
`should_implement_trait`, `new_without_default`, `len_without_is_empty`, `derivable_impls`.
*Why:* a caller reaches for the standard trait first, and generic code can only use what is a trait.

**RUST-API-4 — accept the most general borrow.** `&str` not `&String`, `&[T]` not `&Vec<T>`, `&T`
not `&Box<T>`, `Option<&T>` not `&Option<T>`. Lints: `ptr_arg`, `borrowed_box`. *Why:* callers
pass what they have without first converting it.

**RUST-API-5 — conversions and constructors are named by the convention.** `as_` is free and
borrows, `to_` is costly, `into_` consumes (`wrong_self_convention`). Constructors are `new`,
`new_with_…`, or `try_new` when they return `Result`. Getters carry no `get_` prefix; `foo_mut`,
`set_foo`, `take_foo`, `replace_foo` and `with_foo` mean what they mean in the standard library. An
extension trait is named `…Ext`. *Why:* the name tells the reader the cost and the ownership
without opening the function.

**RUST-API-6 — `#[must_use]` where ignoring the value is a bug.** On builders, on types that are
lazy until consumed, and on functions whose return carries the only evidence of failure (when the
return type is not already `#[must_use]`, as `Result` is). *Why:* a dropped builder or an ignored
status is a silent no-op, and the compiler can catch it for free.

## Toolchain

**RUST-TC-1 — float on stable, declare a minimum.** Workspaces build on the current stable toolchain
(no `rust-toolchain.toml` pin) and declare `rust-version` in `[workspace.package]`, which every
crate inherits. *Why:* floating keeps every consumer on the same compiler without a bump ritual; the
declared minimum tells a consumer what the published crates need, and gives
`clippy::incompatible_msrv` something to check against.

*Known cost, observed:* a new stable release can add or widen lints, and with RUST-LINT-2 that
turns CI red on a pull request that touched nothing related. On 2026-10-21, clippy 1.99 did this to
tasker-core: `double_must_use` fired on code generated by async-trait before 0.1.90, and
`result_large_err` was extended to async fns. Treat these fixes as routine maintenance, not as part
of the change that happened to hit them. A `#[expect]` written for a new lint is unfulfilled on
older toolchains, so contributors must stay on current stable (`rustup update`). If this recurs often,
the alternative is a pinned CI toolchain bumped deliberately, which would be a new ruling.

**RUST-TC-2 — edition is a workspace setting.** `edition` lives in `[workspace.package]`. Moving
to a new edition is one deliberate change per repo, not a per-crate drift.

**RUST-TC-3 — no unstable features.** No `#![feature(...)]`. Floating on stable (RUST-TC-1) makes
this self-enforcing: the stable compiler refuses the attribute. A `--cfg` flag a dependency reads,
such as tokio's `tokio_unstable`, is not a compiler feature; it is declared in `unexpected_cfgs`
and documented where it is set. *Why:* an unstable feature can change or vanish in any nightly, and
one crate's need would pin the whole workspace to a nightly toolchain.

## Documentation and readability

**RUST-DOC-1 — library crates document their public API.** Every public item of a library crate
has a doc comment, and the crate sets `#![warn(missing_docs)]`. A doc comment says what the code
does not: when the function panics (`# Panics`), how it fails (`# Errors`), what an unsafe caller
must uphold (`# Safety`). A comment that restates the name — `/// Removes an element.` on
`remove_element` — is omitted rather than written. `//!` documents the module, `///` the item, `//`
everything else. *Why:* rustdoc is a library's contract, and an agent consuming the crate reads the
docs before the source. A comment with nothing to add is noise that drifts.

**RUST-READ-1 — readability over cleverness.** Judged in review:
- Write the type where inference hides it from a reader: after `.into()`, `.parse()`, `.collect()`
  and long iterator chains. Prefer a typed binding to a turbofish.
- A combinator chain that branches or mutates captured state is a `for` loop. Bind a chain to a
  named variable rather than building it in a `for` header. Prefer `for` to `for_each`.
- An `assert!` that can fail in front of a person carries a message saying why it should hold, or
  whether a failure means misuse or a bug — not a restatement of the condition.
- Operators are overloaded only for their mathematical meaning.

*Why:* code is read far more often than it is written, by people and agents that do not have the
author's context.

**RUST-MACRO-1 — a macro is the last resort.** Prefer, in order: functions, types and traits;
`macro_rules!`; build-time code generation; procedural macros. *Why:* a macro is a dialect. Tools,
reviewers and agents can assume less about code that uses one, and it is harder to change across a
codebase.

**RUST-INIT-1 — no first-party code runs before `main`.** No `ctor`-style static constructors in
code we write. Binding frameworks that register a module at load time (napi-rs pulls in `ctor`) are
the one exception, under [RUST-UNSAFE-3](#unsafe). *Why:* life-before-main runs in an unspecified
order, before logging or configuration exist, and its failures have nowhere to go.

## SQL (sqlx)

**RUST-SQL-1 — queries are compile-checked.** Use the `query!` / `query_as!` macros and commit the
`.sqlx` cache. A runtime-checked `sqlx::query(...)` is an exception with a reason (dynamic `ORDER BY`,
a dynamic table, a vector cast), and the set of exceptions is pinned by a tripwire. *Why:* a
runtime query is the one place a schema change can break production without breaking the build.

## Not specified in v0

- The `too-many-arguments` threshold: temper treats more than five domain parameters as a smell to
  fix with a parameter struct; tasker-core sets the threshold at ten. Clippy's default (seven) is
  left in place until that is ruled.
- Clippy's `pedantic`, `style`, `complexity` and `perf` groups: off, as in tasker-core today. Turning
  any on is a separate ruling, adopted per [RUST-LINT-4](#lints). Single lints from them come in
  under [RUST-LINT-5](#lints).
- Ownership style — avoiding `Rc`/`Arc`, preferring a borrow to a `.clone()`, returning iterators
  rather than `Vec`, `impl Into<T>` parameters. Google's guide states these as preferences; in
  async services `Arc` is the normal way to share state across tasks, and the rest are judgment
  calls a rule would only get wrong. Review decides.
- Import style beyond RUST-ORG-1 (one module per `use`, qualifying free functions by module,
  function-local trait imports). Google's own guide says these exist to end reviewer disagreements;
  rustfmt's defaults end them here.
- Crate size and file layout beyond RUST-ORG-2. Google's small-crate, flat-directory rules serve
  Chromium's build system, not Cargo.
