# Tasker CLI Architecture

**Audience**: Developers, Contributors
**Status**: Active
**Related Docs**: [Crate Architecture](./crate-architecture.md) | [Configuration Management](../guides/configuration-management.md)

---

## Overview

`tasker-ctl` is the primary command-line interface for the Tasker orchestration system. It serves two roles:

1. **Operator tool** — manage tasks, monitor workers, investigate DLQ entries, validate configuration, and generate documentation against running Tasker services.
2. **Developer tool** — discover CLI plugins, inspect templates, and generate project scaffolding from community-contributed templates.

The CLI is built as a single Rust binary with no runtime dependencies beyond the Tasker services it connects to (for operator commands) or the filesystem (for plugin/template/config commands).

---

## Module Structure

```
tasker-ctl/src/
├── main.rs              # CLI definition (Clap derive), arg parsing, command dispatch
├── output/              # Styled terminal output (anstream/anstyle)
│   └── mod.rs
├── commands/            # Command handlers (one file per command group)
│   ├── mod.rs
│   ├── task.rs          # Task CRUD, step operations, audit trail
│   ├── worker.rs        # Worker listing, status, health checks
│   ├── system.rs        # Cross-service health aggregation
│   ├── config.rs        # Config generate, validate, explain, dump, analyze
│   ├── dlq.rs           # Dead letter queue investigation
│   ├── auth.rs          # JWT key generation, token creation/validation
│   ├── docs.rs          # Configuration documentation generation
│   ├── plugin.rs        # Plugin discovery and validation
│   ├── template.rs      # Template listing, info, and code generation
│   ├── remote.rs        # Remote repository management (add, remove, update, list)
│   └── init.rs          # Bootstrap .tasker-ctl.toml with sensible defaults
├── cli_config/          # CLI-specific config (.tasker-ctl.toml)
│   ├── mod.rs
│   └── loader.rs
├── remotes/             # Remote git repository fetching and caching
│   ├── mod.rs
│   └── cache.rs         # Git clone/fetch operations, cache directory management
├── plugins/             # Plugin discovery and registry
│   ├── mod.rs
│   ├── manifest.rs      # Parse tasker-plugin.toml manifests
│   ├── discovery.rs     # Filesystem scanning for plugins
│   └── registry.rs      # Plugin registry with filtering
├── template_engine/     # Runtime template rendering (Tera)
│   ├── mod.rs
│   ├── metadata.rs      # Parse template.toml definitions
│   ├── engine.rs        # Tera wrapper with custom filters
│   ├── loader.rs        # Load .tera template files
│   └── filters.rs       # Case conversion filters (snake, pascal, camel, kebab)
├── docs/                # Askama compile-time templates for docs generation
│   ├── mod.rs
│   └── templates.rs
└── templates/           # Askama template files (.md.jinja, .toml.jinja, .txt.jinja)
```

---

## Key Subsystems

### Command Dispatch

The CLI uses Clap's derive API to define a two-level command hierarchy (`tasker-ctl <group> <action>`). Each command group maps to a handler function in `commands/`:

```
Commands enum → match arm → handle_{group}_command() → API client calls or local operations
```

Commands that interact with Tasker services (task, worker, system, dlq) create API clients from `ClientConfig`. Commands that operate locally (config, docs, plugin, template) work directly with the filesystem and don't require running services.

### Client Configuration

Two separate configuration systems serve different purposes:

- **`ClientConfig`** (from `tasker-client`): Server URLs, transport (REST/gRPC), authentication. Loaded via profiles from `.config/tasker-client.toml` with CLI flag and environment variable overrides.
- **`CliConfig`** (from `cli_config/`): Plugin search paths, default language, default output directory. Loaded from `.tasker-ctl.toml` with project-local and user-global discovery.

### Output Styling

The `output` module provides structured terminal output using `anstream` and `anstyle`:

| Function | Purpose | Stream |
|----------|---------|--------|
| `success()` | Green check mark + message | stdout |
| `error()` | Red X + message | stderr |
| `warning()` | Yellow exclamation + message | stderr |
| `header()` | Bold text | stdout |
| `label()` | Bold name + value | stdout |
| `dim()` | Dimmed informational text | stdout |
| `hint()` | Dimmed hint with arrow prefix | stdout |
| `item()` | Bullet point item | stdout |
| `status_icon()` | Green check or red X based on boolean | stdout |
| `plain()` | Unstyled text | stdout |
| `blank()` | Empty line | stdout |

`anstream` auto-detects terminal capabilities and strips ANSI codes when output is piped. Commands designed for scripting (e.g., `config dump`, `auth generate-token`) write raw data to stdout so they remain safe for piping and redirection.

Clap's built-in help rendering also uses custom styles via `clap_styles()` for a consistent visual appearance.

### Plugin System

The plugin system enables external code to extend `tasker-ctl` with new templates without modifying the binary.

**Discovery** (`plugins/discovery.rs`): Scans configured paths with a three-level search strategy:
1. Check if the path root contains `tasker-plugin.toml`
2. Scan immediate subdirectories for `tasker-plugin.toml`
3. Scan `*/tasker-cli-plugin/` subdirectories (handles `tasker-contrib` layout)

**Manifest** (`plugins/manifest.rs`): Each plugin is defined by a `tasker-plugin.toml`:
```toml
[metadata]
name = "rails"
version = "0.1.0"
description = "Rails integration templates"
language = "ruby"
framework = "rails"

[[templates]]
name = "step_handler"
path = "templates/step_handler"
description = "Generate a Tasker step handler"
```

**Registry** (`plugins/registry.rs`): Aggregates discovered plugins and provides lookup by template name, language, or framework.

### Template Engine

The template engine renders plugin-provided templates using Tera (runtime evaluation).

**Metadata** (`template_engine/metadata.rs`): Each template directory contains a `template.toml` defining parameters and output files:
```toml
[metadata]
name = "step_handler"
description = "Generate a step handler class"
language = "ruby"

[[parameters]]
name = "name"
description = "Handler class name"
required = true

[[output_files]]
path = "{{ name | snake_case }}_handler.rb"
template = "handler.rb.tera"
```

**Engine** (`template_engine/engine.rs`): Wraps Tera with custom case-conversion filters registered at initialization. Renders both output file paths and template content from the same context.

**Filters** (`template_engine/filters.rs`): Custom Tera filters via the `heck` crate:
- `snake_case` — `ProcessPayment` → `process_payment`
- `pascal_case` — `process_payment` → `ProcessPayment`
- `camel_case` — `process_payment` → `processPayment`
- `kebab_case` — `process_payment` → `process-payment`

### Remote System (TAS-270)

The remote system enables `tasker-ctl` to fetch plugins and configuration from git repositories, removing the need for local checkouts of community template repositories like `tasker-contrib`.

**Cache** (`remotes/cache.rs`): Manages local clones of remote git repos under `~/.cache/tasker-ctl/remotes/<name>/`. Uses `git2` for clone and fetch operations. A `.tasker-last-fetch` timestamp file tracks cache freshness against the configurable `cache-max-age-hours` threshold.

**Configuration** (`cli_config/mod.rs`): Remotes are defined in `.tasker-ctl.toml`:
```toml
[[remotes]]
name = "tasker-contrib"
url = "https://github.com/tasker-systems/tasker-contrib.git"
git-ref = "main"
config-path = "config/tasker/"
```

**Integration**: Remote cached paths are transparently injected into the existing plugin discovery and config generation pipelines. The `--remote` and `--url` flags on `template` and `config` commands select a specific remote or ad-hoc URL. The `plugin list` command auto-discovers plugins from all configured remotes.

**Commands** (`commands/remote.rs`): `remote list`, `remote add`, `remote remove`, `remote update` manage the configured remotes and their caches.

### Init Command

The `init` command (`commands/init.rs`) bootstraps a new `.tasker-ctl.toml` in the current directory with sensible defaults:

```bash
tasker-ctl init              # Creates config with tasker-contrib remote pre-configured
tasker-ctl init --no-contrib # Creates config without any remotes
```

The command refuses to overwrite an existing `.tasker-ctl.toml` to prevent accidental data loss. After creation, it prints next-step hints guiding the user toward fetching remotes and generating templates.

### Documentation Generation

Documentation generation uses a separate template system from plugins. Askama provides compile-time template verification for the built-in documentation templates (configuration reference, annotated configs, parameter explanations). These templates live in `templates/` and are bound to Rust structs at compile time via the `docs-gen` feature flag.

The data source for documentation is `DocContextBuilder` from `tasker-shared`, which extracts `_docs` metadata annotations from the TOML configuration files.

---

## Design Decisions

### Two Template Engines

Askama (compile-time) and Tera (runtime) serve different needs:

- **Askama** renders built-in documentation templates. Compile-time verification catches template errors early, and the templates ship with the binary.
- **Tera** renders plugin-provided templates. Runtime evaluation is necessary because templates are discovered from the filesystem, not known at compile time.

Both are lightweight and the overlap is intentional — they solve different problems at different phases of the tool's lifecycle.

### Plugin Discovery vs. Package Manager

Plugins are discovered by scanning filesystem paths rather than through a package manager. This keeps the system simple and predictable: drop a directory with a `tasker-plugin.toml` into a configured path and it's immediately available. No installation step, no version resolution, no network requests.

### Cache-as-Local-Path for Remotes

Remote repos are cloned to a local cache directory, then the existing filesystem-based plugin discovery and config generation pipelines operate on the cached path. This avoids adding "remote-aware" logic throughout the codebase — the remote system's only job is to ensure a local directory exists and is reasonably fresh. Everything downstream sees a regular directory.

### Piping-Safe Output

Commands that produce data for scripting (`config dump`, `auth generate-token`, docs rendering to stdout) write raw unformatted output via `println!`. Styled output is reserved for interactive feedback (status messages, errors, progress). This ensures `tasker-ctl config dump | jq .` and `tasker-ctl auth generate-token | pbcopy` work correctly.

---

## Dependency Summary

| Dependency | Purpose |
|------------|---------|
| `tasker-client` | REST/gRPC API client for service communication |
| `tasker-shared` | Shared types, config models, doc context builder |
| `clap` | CLI argument parsing with derive macros |
| `anstream` + `anstyle` | TTY-aware styled output |
| `tera` | Runtime template rendering for plugins |
| `heck` | Case conversion for template filters |
| `askama` | Compile-time templates for documentation (optional, `docs-gen` feature) |
| `git2` | Git clone/fetch for remote repositories |
| `toml_edit` | Format-preserving TOML editing for `remote add`/`remove`/`init` |
| `rsa` + `rand` | RSA key pair generation for JWT auth |
| `tokio` | Async runtime |

---

## Related Documentation

- [Crate Architecture](./crate-architecture.md) — workspace-level crate overview
- [Configuration Management](../guides/configuration-management.md) — TOML config structure and environments
- [Auth Configuration](../auth/configuration.md) — JWT and API key setup
