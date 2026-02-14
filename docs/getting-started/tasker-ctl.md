# Getting Started with tasker-ctl

`tasker-ctl` is the command-line tool for managing Tasker workflows, generating project scaffolding, and working with configuration. This guide covers the developer-facing features for bootstrapping new projects.

## Initialize Your Project

Run `tasker-ctl init` to create a `.tasker-ctl.toml` configuration file in your project directory:

```bash
tasker-ctl init
```

This creates a `.tasker-ctl.toml` pre-configured with the [tasker-contrib](https://github.com/tasker-systems/tasker-contrib) remote, which provides community templates for all supported languages and default configuration files.

To skip the tasker-contrib remote (e.g., if you only use private templates):

```bash
tasker-ctl init --no-contrib
```

## Fetch Remote Templates

After initialization, fetch the remote templates:

```bash
tasker-ctl remote update
```

This clones the configured remotes to a local cache (`~/.cache/tasker-ctl/remotes/`). Subsequent fetches only pull changes. The cache is checked for freshness automatically and warnings are shown when it becomes stale (default: 24 hours).

## Browse Templates

List all available templates:

```bash
tasker-ctl template list
```

Filter by language:

```bash
tasker-ctl template list --language ruby
tasker-ctl template list --language python
```

Get detailed information about a template:

```bash
tasker-ctl template info step_handler --language ruby
```

## Generate Code

Generate a step handler from a template:

```bash
tasker-ctl template generate step_handler \
  --language ruby \
  --param name=ProcessPayment \
  --output ./app/handlers/
```

This creates handler files using the naming conventions and patterns for your chosen language. Template parameters (like `name`) are transformed automatically — `ProcessPayment` becomes `process_payment` for file names and `ProcessPaymentHandler` for class names.

## Generate Configuration

Generate a deployable configuration file from the base + environment configs:

```bash
# From local config directory
tasker-ctl config generate \
  --context orchestration \
  --environment production \
  --output config/orchestration.toml

# From a remote (tasker-contrib provides default configs)
tasker-ctl config generate \
  --remote tasker-contrib \
  --context orchestration \
  --environment development \
  --output config/orchestration.toml
```

The `config generate` command merges base configuration with environment-specific overrides and strips documentation metadata, producing a clean deployment-ready TOML file.

## Manage Remotes

```bash
tasker-ctl remote list                    # Show configured remotes and cache status
tasker-ctl remote add my-templates URL    # Add a new remote
tasker-ctl remote update                  # Fetch latest for all remotes
tasker-ctl remote update tasker-contrib   # Fetch a specific remote
tasker-ctl remote remove my-templates     # Remove a remote and its cache
```

## Typical Workflow

A new project typically follows this sequence:

```bash
# 1. Initialize CLI configuration
tasker-ctl init

# 2. Fetch community templates
tasker-ctl remote update

# 3. Generate a step handler
tasker-ctl template generate step_handler --language python --param name=ValidateOrder

# 4. Generate a task template
tasker-ctl template generate task_template --language python --param name=OrderProcessing

# 5. Generate environment-specific config
tasker-ctl config generate --remote tasker-contrib \
  --context worker --environment development --output config/worker.toml
```

## Next Steps

- [Your First Handler](first-handler.md) — Write a step handler from scratch
- [Your First Workflow](first-workflow.md) — Define a task template and run it
- [Configuration Management](../guides/configuration-management.md) — Understanding the TOML config structure
- [tasker-ctl Architecture](../architecture/tasker-ctl.md) — How the CLI is built
