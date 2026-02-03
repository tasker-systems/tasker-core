# TAS-189: Road to Tasker Alpha

## Overview

A phased plan to take Tasker from a well-engineered codebase that's easy to *develop on* to a project that's easy to *use*. The central thesis is that open source adoption requires more than code quality -- it requires a clear, low-friction path from "I want to use this" to "I am using this in my application."

## The Gap

Today, Tasker has:

- Comprehensive internal developer tooling (cargo-make, CI, 1185+ tests, benchmarks)
- Thorough documentation for people working *inside* the monorepo
- Quick start guide that demonstrates running services and curling APIs
- Four complete worker implementations (Rust, Ruby, Python, TypeScript)
- A Rust client library (`tasker-client`) with REST + gRPC transport
- CLI tooling (`tasker-cli`) for system operations
- Docker Compose files for development, testing, CI, and production profiles
- Release management scripts (TAS-170 Phase 1) for version coordination
- Blog example handlers across all four languages

Today, Tasker lacks:

- **Published packages** on any registry (crates.io, RubyGems, PyPI, npm)
- **Published container images** (ghcr.io or Docker Hub)
- **Cross-language client SDKs** -- only Rust applications can programmatically interact with Tasker
- **Consumer-oriented documentation** -- "how to use Tasker in *your* app" vs "how to work on Tasker"
- **Standalone example applications** -- all examples live inside the monorepo
- **A `curl | sh` or template-based bootstrap** for new projects
- **Community infrastructure** -- CONTRIBUTING.md, issue templates, PR templates, discussion guidance

The result: a developer who reads `why-tasker.md`, decides Tasker solves their problem, and wants to adopt it hits a wall. They can clone the repo and run docker-compose, but they cannot `pip install tasker-worker-py` and write a handler, nor can they use a client SDK to submit tasks from their Rails or FastAPI application.

---

## Principles

1. **Ship incrementally.** Each phase delivers usable value. Don't gate alpha on having everything.
2. **FFI-first for cross-language.** Wrap the Rust implementation rather than rebuilding in each language. One implementation to maintain, one set of bugs to fix.
3. **Consumer documentation over contributor documentation.** We have good contributor docs. The alpha needs user-facing docs.
4. **Docker is the default deployment.** Most users will run Tasker's orchestration server as infrastructure, not compile it from source.
5. **Prove it works outside the monorepo.** If it doesn't work as a dependency in a standalone project, it doesn't work.

---

## Phase 0: Foundation & Cleanup (Pre-Alpha Prerequisites)

**Goal:** Close remaining quality-of-life gaps and prepare the monorepo for publishing.

### 0.1 Inter-Crate Version Fields

Before any crate can be published, all workspace path dependencies need version fields:

```toml
# Current (won't publish)
tasker-pgmq = { path = "tasker-pgmq" }

# Required
tasker-pgmq = { path = "tasker-pgmq", version = "=0.1.0" }
```

This is documented in TAS-170 Phase 2 but is a hard prerequisite for everything else.

### 0.2 Python Package Rename

Rename `tasker-core-py` to `tasker-worker-py` for namespace consistency with Ruby (`tasker-worker-rb`) and TypeScript (`@tasker-systems/worker`). Nothing is published yet, so this is free.

### 0.3 Community Infrastructure

| Deliverable | Purpose |
|-------------|---------|
| `CONTRIBUTING.md` | How to contribute: workflow, testing expectations, PR guidelines |
| `.github/ISSUE_TEMPLATE/` | Bug report, feature request, question templates |
| `.github/PULL_REQUEST_TEMPLATE.md` | PR checklist and context requirements |
| `CHANGELOG.md` | Start tracking changes from alpha onward |
| `SECURITY.md` | Vulnerability reporting process |

### 0.4 Open Ticket Triage

Audit existing open tickets for anything that would be embarrassing or blocking in an alpha. Items that represent known broken behavior or missing fundamental capability should be addressed. Nice-to-haves can be deferred.

### Acceptance Criteria

- [ ] `cargo publish --dry-run` succeeds for all six publishable crates in dependency order
- [ ] Python package renamed to `tasker-worker-py` throughout
- [ ] CONTRIBUTING.md, CHANGELOG.md, SECURITY.md exist
- [ ] GitHub issue and PR templates in place
- [ ] No known P0 bugs in open tickets

---

## Phase 1: Publish What Exists

**Goal:** Get Tasker packages onto registries and container images into a registry so that people can actually install things.

### 1.1 Rust Crate Publishing (TAS-170 Phase 2)

Publish the six core crates to crates.io in dependency order:

```
Phase 1: tasker-pgmq
Phase 2: tasker-shared
Phase 3: tasker-client, tasker-orchestration  (parallel)
Phase 4: tasker-worker, tasker-cli            (parallel)
```

Scripts exist from TAS-170 Phase 1. This phase implements the actual `publish-crates.sh` and tests it with `--dry-run` before the real publish.

### 1.2 FFI Worker Package Publishing (TAS-170 Phase 3)

Publish language-specific worker packages:

| Package | Registry | Status |
|---------|----------|--------|
| `tasker-worker-rb` | RubyGems | Scripts designed, needs implementation |
| `tasker-worker-py` | PyPI | Scripts designed, needs implementation |
| `@tasker-systems/worker` | npm | Scripts designed, needs implementation |

### 1.3 Container Image Publishing

Publish pre-built Docker images to GitHub Container Registry (ghcr.io):

| Image | Purpose |
|-------|---------|
| `ghcr.io/tasker-systems/tasker-orchestration` | Orchestration server |
| `ghcr.io/tasker-systems/tasker-worker-rust` | Rust worker |
| `ghcr.io/tasker-systems/tasker-worker-ruby` | Ruby worker |
| `ghcr.io/tasker-systems/tasker-worker-python` | Python worker |
| `ghcr.io/tasker-systems/tasker-worker-typescript` | TypeScript worker |

These images already have production Dockerfiles (`docker/build/*.prod.Dockerfile`). The work is adding a GitHub Actions workflow to build and push on release tags.

### 1.4 CI Release Workflow (TAS-170 Phase 4)

Wire the release scripts into GitHub Actions so that tagging a release triggers the full publish pipeline: crates, language packages, container images.

### Acceptance Criteria

- [ ] All six Rust crates published to crates.io
- [ ] Worker packages published to RubyGems, PyPI, npm
- [ ] Container images published to ghcr.io
- [ ] `cargo add tasker-client` works in a fresh Rust project
- [ ] `gem install tasker-worker-rb` works
- [ ] `pip install tasker-worker-py` works
- [ ] `npm install @tasker-systems/worker` works
- [ ] `docker pull ghcr.io/tasker-systems/tasker-orchestration:0.1.0` works
- [ ] Release workflow triggers correctly on tags

---

## Phase 2: Cross-Language Client SDKs

**Goal:** Enable Ruby, Python, and TypeScript applications to programmatically interact with Tasker (create tasks, check status, manage templates) without writing raw HTTP/gRPC calls.

### 2.1 Architecture

The `tasker-client` Rust crate already provides a full-featured, transport-agnostic client with REST and gRPC support. Rather than rewriting this in each language, we wrap it via FFI -- the same pattern used for workers.

```
clients/
├── ruby/          → tasker-client-rb    (RubyGems)
├── python/        → tasker-client-py    (PyPI)
└── typescript/    → tasker-client-ts    (npm: @tasker-systems/client)
```

Each client crate:

1. Links against `tasker-client` (Rust)
2. Exposes a C ABI or language-specific FFI (Magnus for Ruby, PyO3 for Python, Napi/C ABI for TypeScript)
3. Provides idiomatic language wrappers (classes, async patterns, error types matching language conventions)

### 2.2 API Surface (Minimum Viable)

Each cross-language client should expose at minimum:

```
TaskerClient
├── .new(config)                    # Constructor with connection config
├── .create_task(params)            # Create a task from a template
├── .get_task(task_uuid)            # Get task status
├── .get_task_details(task_uuid)    # Get task with steps and transitions
├── .list_tasks(filters)            # List/filter tasks
├── .cancel_task(task_uuid)         # Cancel a running task
├── .list_templates()               # List available task templates
└── .health()                       # Check orchestration health
```

The Rust `tasker-client` already implements all of this via `OrchestrationClient`. The FFI layer translates types and handles async-to-sync bridging where needed.

### 2.3 Build Approach

Mirror the worker FFI approach exactly:

| Language | FFI Tool | Build Tool | Existing Pattern |
|----------|----------|------------|-----------------|
| Ruby | Magnus | rake compile (rb_sys) | `workers/ruby/` |
| Python | PyO3 | maturin | `workers/python/` |
| TypeScript | C ABI | cargo build + tsup | `workers/typescript/` |

### 2.4 Why Not Just HTTP Clients?

One could argue that writing a simple REST client in each language is easier than FFI. The reasons to prefer FFI:

- **Single transport logic.** REST, gRPC, auth, retry, circuit breaking -- all maintained in Rust once.
- **Consistency guarantee.** All languages behave identically because they run the same code.
- **gRPC for free.** Language clients get gRPC transport without each language needing protobuf tooling.
- **Proven pattern.** The worker FFI approach already works across all four languages.

The trade-off is FFI complexity (native extensions, platform-specific builds). For the worker packages this trade-off has already been accepted and the infrastructure exists.

### Acceptance Criteria

- [ ] `tasker-client-rb` gem installable, can create and query tasks
- [ ] `tasker-client-py` pip-installable, can create and query tasks
- [ ] `@tasker-systems/client` npm-installable, can create and query tasks
- [ ] All three clients work against both REST and gRPC transports
- [ ] Integration tests verify client SDK behavior against running services
- [ ] Published to respective registries

---

## Phase 3: Unified Language Packages (tasker-contrib)

**Goal:** Provide a single dependency per language that bundles both worker and client functionality.

### 3.1 Package Structure

These live in a new `tasker-contrib` repository:

| Package | Registry | Depends On |
|---------|----------|-----------|
| `tasker-rb` | RubyGems | `tasker-worker-rb` + `tasker-client-rb` |
| `tasker-py` | PyPI | `tasker-worker-py` + `tasker-client-py` |
| `tasker-ts` | npm: `@tasker-systems/tasker` | `@tasker-systems/worker` + `@tasker-systems/client` |

Each unified package:

- Re-exports worker and client APIs under a single namespace
- Provides convenience utilities (e.g., combined configuration, shared logging setup)
- Serves as the "recommended" dependency for new projects
- Adds language-idiomatic sugar where appropriate

### 3.2 What These Are Not (Yet)

These are *not* framework integrations. `tasker-rb` does not know about Rails, ActiveJob, or Sidekiq. `tasker-py` does not know about FastAPI, Celery, or Django. Framework integrations (`tasker-rails`, `tasker-fastapi`, etc.) are a future layer that depends on these unified packages.

The unified packages are the "framework-agnostic" layer:

```
Application Code
    ↓
Framework Integration (tasker-rails, tasker-fastapi)     ← future
    ↓
Unified Package (tasker-rb, tasker-py, tasker-ts)         ← this phase
    ↓
Core Packages (tasker-worker-rb + tasker-client-rb)       ← Phase 1 + 2
    ↓
Rust Core (tasker-worker, tasker-client, tasker-shared)   ← exists
```

### 3.3 Scope Control

For alpha, these packages should be thin. The value is:

1. **Single dependency** -- `gem 'tasker-rb'` instead of two separate gems
2. **Combined configuration** -- one config block sets up both worker and client
3. **Namespace unification** -- `TaskerRb::Client` and `TaskerRb::Handler` under one module

Resist the urge to add framework-specific features at this layer.

### Acceptance Criteria

- [ ] `tasker-contrib` repository created with CI
- [ ] `tasker-rb`, `tasker-py`, `tasker-ts` packages published
- [ ] Each unified package's README shows a complete "handler + task creation" example
- [ ] Integration tests verify worker + client work together through the unified package

---

## Phase 4: Consumer Documentation & Examples

**Goal:** A developer who has never seen Tasker can go from reading the README to running a working application in their language of choice.

### 4.1 "Use Tasker" Documentation

This is distinct from the existing developer documentation (which targets monorepo contributors). Create a new top-level documentation path for *consumers*:

```
docs/
├── getting-started/
│   ├── overview.md              # What Tasker is, 2-minute version
│   ├── concepts.md              # Tasks, steps, handlers, templates, namespaces
│   ├── install.md               # Installing packages + running infrastructure
│   ├── first-handler.md         # Write your first handler (all languages)
│   ├── first-workflow.md        # Define a template, submit a task, see it run
│   └── next-steps.md            # Links to deeper docs
│
├── language-guides/
│   ├── rust.md                  # Using Tasker from Rust
│   ├── ruby.md                  # Using Tasker from Ruby
│   ├── python.md                # Using Tasker from Python
│   └── typescript.md            # Using Tasker from TypeScript
```

Key differences from existing docs:

| Existing (Contributor) | New (Consumer) |
|----------------------|----------------|
| Assumes monorepo clone | Assumes `pip install` / `gem install` |
| References cargo-make tasks | References language-native tooling |
| Tests run against local checkout | Tests run against published packages |
| Docker compose for dev environment | Docker compose for Tasker infrastructure only |

### 4.2 Standalone Example Applications

In `tasker-contrib`, create minimal but complete example apps:

```
tasker-contrib/
├── examples/
│   ├── ruby-sinatra/          # Sinatra app using tasker-rb
│   ├── ruby-rails/            # Rails app using tasker-rb
│   ├── python-fastapi/        # FastAPI app using tasker-py
│   ├── python-flask/          # Flask app using tasker-py
│   ├── typescript-express/    # Express app using tasker-ts
│   ├── typescript-bun/        # Bun app using tasker-ts
│   └── rust-axum/             # Axum app using tasker-client directly
```

Each example app:

1. Has its own README with setup instructions
2. Defines 2-3 task templates demonstrating real patterns
3. Includes a docker-compose.yml that starts Tasker infrastructure
4. Shows both worker (handler) and client (task submission) usage
5. Runs in CI to ensure examples don't rot

### 4.3 Starter Docker Compose

A standalone `docker-compose.yml` that users copy into their project to run Tasker infrastructure:

```yaml
# docker-compose.tasker.yml
# Tasker infrastructure for your application
# Usage: docker compose -f docker-compose.tasker.yml up -d
services:
  postgres:
    image: ghcr.io/tasker-systems/tasker-postgres:0.1.0
    # Pre-configured with PGMQ extension and migrations
    ports: ["5432:5432"]
    volumes: ["tasker_data:/var/lib/postgresql/data"]

  orchestration:
    image: ghcr.io/tasker-systems/tasker-orchestration:0.1.0
    depends_on: [postgres]
    ports: ["8080:8080", "9190:9190"]
    environment:
      DATABASE_URL: postgresql://tasker:tasker@postgres/tasker

volumes:
  tasker_data:
```

This is deliberately simple. Users add their own workers as services or run them in their existing application process.

### Acceptance Criteria

- [ ] Getting-started documentation covers install-to-running-handler for each language
- [ ] At least one standalone example app per language, runnable with published packages
- [ ] Example apps tested in CI against published package versions
- [ ] Starter docker-compose.yml documented and tested
- [ ] Existing quick-start guide updated to reference published packages as the primary path

---

## Phase 5: Bootstrap Tooling (TAS-127 Plugin Architecture)

**Goal:** Reduce the friction of starting a new Tasker project to a single command, powered by an extensible plugin system that enables tasker-contrib and community-provided templates.

### Relationship to TAS-127

TAS-127 defines a CLI plugin architecture for runtime template loading and code generation. It is the *implementation mechanism* for the scaffolding described in this phase. Without TAS-127, every template would need to be compiled into the `tasker-cli` binary, creating coupling between core releases and template updates.

**Architecture evolution since TAS-127 was written:** `tasker-client` and `tasker-cli` are now separate crates (they were less clearly separated when TAS-127 was designed). The plugin system lives entirely in `tasker-cli`. Additionally, `tasker-cli` already has `--config`, `--profile` flags and a `config show` command, which means TAS-127 Phase 1 (configuration loading) is partially complete.

**Template engine decision deferred:** `tasker-cli` currently uses Askama (compile-time verified, strict typing) for documentation generation templates (`docs-gen` feature). TAS-127 originally proposed Tera (runtime-loaded) for code generation templates. The choice of template engine for plugin-provided code generation is deferred to implementation time (see Open Question 7). The plugin architecture design (discovery, manifests, parameters) is engine-agnostic.

### 5.1 Plugin Architecture (TAS-127 Phases 1-3)

**5.1a Configuration Extension (TAS-127 Phase 1 -- partially done)**

The existing `--config`, `--profile`, and `config show` infrastructure needs extension to support plugin discovery paths:

```toml
# .tasker-cli.toml (extends existing config)
[plugins]
paths = [
    "./.tasker-cli/plugins",
    "~/projects/tasker-systems/tasker-contrib",
]

[profiles.development]
plugin-paths = ["./tasker-cli-plugins"]

[profiles.ci]
use-published-plugins = true
```

**5.1b Plugin Discovery (TAS-127 Phase 2)**

Plugins are directories containing a `tasker-plugin.toml` manifest:

```toml
# tasker-plugin.toml
[plugin]
name = "tasker-contrib-rails"
version = "0.1.0"
description = "Rails templates for Tasker CLI"
languages = ["ruby"]
frameworks = ["rails"]

[templates]
step-handler = { path = "templates/step_handler", languages = ["ruby"] }
task-template = { path = "templates/task_template", languages = ["ruby"] }
rails-initializer = { path = "templates/rails_initializer", frameworks = ["rails"] }
```

Discovery order:
1. `--plugin-path` CLI argument (highest priority)
2. Project-level: `./.tasker-cli/plugins/`
3. Config file `plugin-paths` (per-profile)
4. User-level: `~/.config/tasker-cli/plugins/`
5. System-level: `/usr/local/share/tasker-cli/plugins/`

New commands:
```bash
tasker-cli plugin list              # Show discovered plugins
tasker-cli plugin validate ./path   # Validate plugin structure
```

**5.1c Template System (TAS-127 Phase 3)**

Template engine for code generation (engine choice deferred -- see Open Question 7):

```
templates/step_handler/
├── template.toml           # Template metadata + parameters
├── handler.rb.tmpl         # Template content
└── handler_spec.rb.tmpl    # Optional additional files
```

Required template filters regardless of engine: `snake_case`, `pascal_case`, `camel_case`, `kebab_case`.

New commands:
```bash
tasker-cli template list                        # List templates from all plugins
tasker-cli template info step-handler           # Show template details
tasker-cli template generate step-handler \
  --name ProcessPayment \
  --language ruby \
  --framework rails \
  --handler-type api
```

### 5.2 CLI Scaffolding (Built on Plugin System)

With the plugin system in place, `tasker init` and `tasker generate` become plugin-driven rather than hardcoded:

```bash
# Generate a new Tasker project (uses "project" template from plugins)
tasker-cli init my-project --language ruby
tasker-cli init my-project --language python
tasker-cli init my-project --language typescript
tasker-cli init my-project --language rust

# Generate a handler (uses "step-handler" template from plugins)
tasker-cli generate handler payment_processor --language ruby

# Generate a task template definition
tasker-cli generate template order_fulfillment --steps "validate,charge,ship,notify"
```

Each `tasker-cli init` creates:

- Project directory with language-appropriate structure
- `docker-compose.tasker.yml` for Tasker infrastructure
- Example handler and task template
- README with "getting started" instructions
- Test setup verifying the handler works

The built-in CLI ships with minimal templates (TOML configuration files only). All handler and project templates come from `tasker-contrib` plugins, which is the intended design -- the CLI is the engine, contrib provides the content.

### 5.3 Install Script

A `curl | sh` style installer for getting Tasker CLI + infrastructure running:

```bash
curl -fsSL https://tasker.systems/install.sh | sh
```

This script:

1. Detects platform (macOS/Linux)
2. Downloads the `tasker-cli` binary from GitHub releases
3. Optionally pulls Docker images
4. Installs default plugins from tasker-contrib
5. Runs `tasker-cli init` interactively if requested

### 5.4 Template Repositories

GitHub template repositories (`tasker-systems/tasker-template-{ruby,python,typescript}`) that users can click "Use this template" to get a pre-configured project. These are generated from the same plugin templates that `tasker-cli init` uses, ensuring consistency.

### 5.5 TAS-127 Phase Mapping

| TAS-127 Phase | TAS-189 Location | Status |
|---------------|-------------------|--------|
| Phase 1: Configuration Loading | 5.1a | Partially done (profile system exists) |
| Phase 2: Plugin Discovery | 5.1b | Not started |
| Phase 3: Template System | 5.1c | Not started (Askama exists for docs, Tera needed for codegen) |
| Phase 4: Integration | 5.2 | Not started |

### Acceptance Criteria

- [ ] `tasker-cli plugin list` discovers and displays plugins
- [ ] `tasker-cli plugin validate` validates plugin structure
- [ ] `tasker-cli template list` shows templates from discovered plugins
- [ ] `tasker-cli template generate` creates files from Tera templates
- [ ] `tasker-cli init` generates working projects for all four languages
- [ ] Generated projects pass their own test suites
- [ ] Plugin system loads templates from tasker-contrib without requiring CLI rebuild
- [ ] Install script works on macOS and Linux
- [ ] Template repositories exist and are kept in sync with plugin templates

---

## Phase 6: Alpha Release & Announcement

**Goal:** Declare alpha, communicate it, and establish the feedback loop.

### 6.1 Release Checklist

Before declaring alpha:

- [ ] All Phase 0-4 deliverables complete (Phase 5 is nice-to-have)
- [ ] All packages published with 0.1.0 version
- [ ] Container images published and tagged
- [ ] No known P0 or P1 bugs
- [ ] Quick-start path verified end-to-end by someone who didn't write it
- [ ] Consumer documentation reviewed for accuracy
- [ ] CHANGELOG.md reflects current state
- [ ] LICENSE, SECURITY.md, CONTRIBUTING.md current

### 6.2 Release Artifacts

| Artifact | Location |
|----------|----------|
| GitHub Release | `tasker-core` and `tasker-contrib` repos |
| Blog Post | Tasker website / dev.to / similar |
| Crates (6) | crates.io |
| Ruby Gems (2-3) | RubyGems |
| Python Packages (2-3) | PyPI |
| npm Packages (2-3) | npm |
| Docker Images (5+) | ghcr.io |
| CLI Binary | GitHub Releases (tasker-cli) |

### 6.3 Announcement Content

The announcement should address:

1. **What Tasker is** (distilled from `why-tasker.md`)
2. **What alpha means** (breaking changes expected, seeking feedback, not production-ready)
3. **Getting started** (link to consumer docs, 5-minute path)
4. **What feedback we want** (use cases, rough edges, missing features)
5. **How to contribute** (link to CONTRIBUTING.md)
6. **Roadmap** (what's coming post-alpha: framework integrations, web UI, TUI)

### 6.4 Feedback Infrastructure

- GitHub Discussions enabled for questions and use-case sharing
- Issue templates for bug reports and feature requests
- Clear labels: `alpha-feedback`, `good-first-issue`, `help-wanted`
- Response time commitment (e.g., "we aim to respond to issues within 48 hours during alpha")

### Acceptance Criteria

- [ ] Alpha release published across all registries
- [ ] Announcement published on at least two channels
- [ ] GitHub Discussions enabled with welcome post
- [ ] At least one external developer has completed the getting-started path and provided feedback

---

## Dependency Graph

```
Phase 0: Foundation
    │
    ▼
Phase 1: Publish What Exists
    │
    ├──────────────────────┬──────────────────────────────┐
    ▼                      ▼                              ▼
Phase 2: Client SDKs   Phase 4: Docs & Examples   TAS-127 Plugin Infra
    │                      │                        (Phases 1-3, optional)
    ▼                      │                              │
Phase 3: Unified Pkgs     │                              │
    │                      │                              │
    ├──────────────────────┘                              │
    ▼                                                     │
Phase 5: Bootstrap Tooling (TAS-127 Phase 4 + init) ◄────┘
    │                       (nice-to-have for alpha)
    ▼
Phase 6: Alpha Release
```

**Critical path:** 0 → 1 → 2 → 3 → 6

**Optional parallel track:** TAS-127 Phases 1-3 (plugin discovery, template engine) can be developed alongside Phases 2-3 since they only depend on `tasker-cli` existing, not on published packages. The user-facing payoff (Phase 5: `tasker-cli init`, `tasker-cli template generate`) requires both the plugin infrastructure *and* tasker-contrib templates to be populated.

**Parallel work:** Phase 4 (docs, examples) can begin as soon as Phase 1 is underway. Most of the consumer documentation can be drafted against the API shapes before packages are published. Example apps need published packages to be fully testable but can be structured ahead of time.

**Nice-to-have for alpha:** Phase 5 (bootstrap tooling / TAS-127). The plugin architecture and CLI scaffolding significantly improve first-touch experience but are not strictly required. Alpha can launch with just published packages + good docs + example apps. However, the plugin infrastructure (TAS-127 Phases 1-3) is independent of package publishing and could be developed in parallel with earlier phases if capacity allows. The plugin system only needs tasker-contrib templates to be *useful*, but can be built and tested before those templates exist.

---

## Existing Work That Feeds In

| Existing Asset | Feeds Into |
|----------------|-----------|
| TAS-170 release scripts (Phase 1) | Phase 0, Phase 1 |
| `workers/{ruby,python,typescript}` FFI patterns | Phase 2 (client SDKs mirror these) |
| `tasker-client` Rust crate | Phase 2 (wrapped by client SDKs) |
| `docker/build/*.prod.Dockerfile` | Phase 1 (container publishing) |
| Blog example handlers | Phase 4 (basis for standalone examples) |
| `docs/guides/quick-start.md` | Phase 4 (adapt for consumer path) |
| `why-tasker.md` | Phase 6 (announcement content) |
| `tasker-cli` existing infrastructure | Phase 5 (`--config`, `--profile`, `config show` already exist) |
| TAS-127 plugin architecture spec | Phase 5 (design for plugin discovery, Tera templates, CLI commands) |
| `tasker-cli` Askama doc templates | Phase 5 (pattern reference; codegen uses Tera instead) |

---

## What This Plan Deliberately Excludes

These are important but can come after alpha:

1. **Framework integrations** (`tasker-rails`, `tasker-fastapi`, etc.) -- These depend on unified packages (Phase 3) and benefit from real user feedback about what integrations matter most. Note that the plugin architecture (Phase 5 / TAS-127) is designed to support framework-specific CLI plugins (e.g., `tasker-contrib-rails` providing Rails generators), but the actual framework integration packages are post-alpha.

2. **Web UI** -- A monitoring and management dashboard (likely SvelteKit) is valuable but not required for alpha adoption. CLI + API is sufficient.

3. **TUI** -- A ratatui-based terminal UI for monitoring. Same reasoning as web UI.

4. **Managed hosting / cloud offering** -- Way beyond alpha scope.

5. **LTS versioning** -- Alpha is 0.1.x. LTS comes later.

6. **Durable execution / deterministic replay** -- Architectural feature that would be a post-alpha exploration if there's demand.

7. **Additional messaging backends** -- PGMQ and RabbitMQ cover alpha needs. Kafka, Redis Streams, etc. are post-alpha.

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|-----------|
| FFI client SDKs are harder than expected | Delays Phase 2 | Worker FFI already works; client is structurally similar. Start with one language as proof-of-concept. |
| Cross-platform native extension builds fail in CI | Blocks publishing | Use existing CI matrix from worker packages. Test on macOS + Linux early. |
| No one adopts despite good docs | Low alpha engagement | Focus announcement on specific communities (Ruby/Python workflow users). Target subreddits, forums, conferences where workflow pain is discussed. |
| Breaking changes needed during alpha | User frustration | Set clear alpha expectations. Version as 0.1.x. Document all breaking changes in CHANGELOG. |
| Maintaining four language SDKs is unsustainable | Long-term burden | FFI approach means most maintenance is in Rust. Language layers are thin. Consider community maintainers per language. |
| Example apps rot after initial creation | Broken first impressions | CI tests example apps against published packages. Renovate/Dependabot for dependency updates. |

---

## Success Metrics

**For alpha launch:**

- All packages published to registries
- Getting-started path verified by at least 2 people outside the core team
- At least one standalone example app per supported language
- Zero known P0 bugs

**For alpha period (first 3 months):**

- 50+ GitHub stars (signal of interest)
- 10+ issues filed by external contributors (signal of engagement)
- 5+ external PRs or discussions (signal of community formation)
- Weekly release cadence maintained
- At least one real use case reported by an external user

**Signals to watch:**

- Where do people get stuck in the getting-started flow? (Improve docs there)
- Which language has the most adoption? (Invest more there)
- What use cases are people trying? (Guide roadmap accordingly)
- What's the drop-off point? (Address the most common friction)

---

## Rough Sequencing

This is intentionally not time-estimated. The phases have natural ordering and dependencies, and the scope of each is defined above. Execution speed depends on availability and what parallel work is possible.

| Phase | Dependencies | Parallelizable With |
|-------|-------------|-------------------|
| Phase 0: Foundation | None | -- |
| Phase 1: Publish | Phase 0 | -- |
| Phase 2: Client SDKs | Phase 1 (published worker packages as reference) | Phase 4 (docs) |
| Phase 3: Unified Packages | Phase 2 | Phase 4 (docs) |
| Phase 4: Docs & Examples | Phase 1 (partially), Phase 3 (fully) | Phase 2, Phase 3 |
| Phase 5: Bootstrap (TAS-127) | Phase 3, Phase 4 | TAS-127 Phases 1-3 (plugin infra) can start during Phase 2 |
| Phase 6: Alpha Release | Phase 0-4, optionally Phase 5 | -- |

---

## Open Questions

1. **Should client SDKs live in `tasker-core` or `tasker-contrib`?** Arguments for core: same CI, same release cadence, simpler dependency graph. Arguments for contrib: keeps core focused on orchestration + workers, allows independent release cadence for clients. **Recommendation:** Start in `tasker-core` under `clients/` (mirrors `workers/`). Extract later if release cadence divergence becomes a real issue.

2. **Should unified packages (`tasker-rb`, etc.) bundle via FFI or be pure language packages that depend on published worker + client packages?** Arguments for FFI bundle: single native extension, simpler for users. Arguments for pure dependency: simpler to build, users can opt into just worker or just client. **Recommendation:** Pure dependency packages. The worker and client are already separately useful, and combining native extensions adds build complexity for marginal user benefit.

3. **PostgreSQL Docker image strategy.** Should we publish a `tasker-postgres` image with PGMQ pre-installed and migrations pre-run, or document how to add PGMQ to an existing Postgres instance? **Recommendation:** Both. Publish a convenience image for getting started; document the manual approach for production where users have existing Postgres infrastructure.

4. **Alpha naming.** Do we call 0.1.0 "alpha" explicitly in package metadata (e.g., `0.1.0-alpha.1`) or just use 0.1.x with documentation noting alpha status? **Recommendation:** Use plain `0.1.x` without pre-release suffixes. Simpler for dependency resolution, and "alpha" is a project status, not a version qualifier. Document alpha status in READMEs and announcements.

5. **Minimum viable client SDK surface.** Do we need the full `tasker-client` API surface in Phase 2, or is a subset sufficient? **Recommendation:** Start with task CRUD + template listing + health check. Add DLQ, analytics, and advanced features in later releases based on demand.

6. **TAS-127 scope for alpha.** Phase 5 is marked as nice-to-have for alpha. If we do include it, how much of TAS-127 is needed? The full four-phase plugin architecture is substantial. **Recommendation:** For alpha, the plugin system (TAS-127 Phases 1-3) and a basic set of tasker-contrib templates are the high-value items. The `tasker-cli init` command that uses those templates (TAS-127 Phase 4) is the user-facing payoff. If Phase 5 is deferred entirely, the example apps and consumer docs from Phase 4 serve as the onboarding path instead. If partially included, prioritize `tasker-cli template generate` over `tasker-cli init` -- generating a single handler is more immediately useful than scaffolding an entire project.

7. **Template engine for code generation.** `tasker-cli` already uses Askama (compile-time verified, strict typing) for documentation generation. TAS-127 originally proposed Tera (runtime-loaded, hash-key template values) for plugin-provided code generation templates. The core tension: Askama gives compile-time correctness but requires templates to be known at build time; Tera allows runtime discovery but trades compile-time safety for flexibility. There may also be Askama-based approaches (e.g., compiling plugin templates into the binary at install/update time, or using Askama's runtime features if sufficient). **Recommendation:** Defer this decision to implementation time. The plugin architecture design doesn't depend on which template engine renders the output -- it depends on discovery, manifests, and parameter schemas. Choose the engine when building TAS-127 Phase 3, informed by the actual requirements at that point.
