# Installation

This guide covers installing Tasker components for development.

## Prerequisites

- **Rust** 1.75+ (for tasker-core)
- **PostgreSQL** 14+ (state persistence)
- **NATS** (event messaging)
- **Ruby 3.2+**, **Python 3.10+**, or **Node.js 18+** (for workers)

## Quick Start with Docker

The fastest way to get started is with Docker Compose:

```bash
git clone https://github.com/tasker-systems/tasker-core.git
cd tasker-core
docker compose up -d
```

This starts PostgreSQL, NATS, and the Tasker orchestration service.

## Installing Worker Packages

Install the package for your language of choice:

### Rust

```bash
cargo add tasker-worker tasker-client
```

### Ruby

```bash
gem install tasker-rb
```

Or add to your Gemfile:

```ruby
gem 'tasker-rb', '~> 0.1'
```

### Python

```bash
pip install tasker-py
```

Or with uv:

```bash
uv add tasker-py
```

### TypeScript / JavaScript

```bash
npm install @tasker-systems/tasker
```

Or with bun:

```bash
bun add @tasker-systems/tasker
```

## Configuration

Tasker uses environment variables for configuration:

```bash
# Database connection
export DATABASE_URL="postgresql://localhost:5432/tasker"

# NATS connection for events
export NATS_URL="nats://localhost:4222"

# Orchestration API (for client SDK)
export TASKER_API_URL="http://localhost:3000"
```

See [Configuration Reference](../generated/configuration.md) for all options.

## Verifying Installation

Verify your installation by checking the package version:

```bash
# Rust
cargo run --example version

# Ruby
ruby -e "require 'tasker_core'; puts TaskerCore::VERSION"

# Python
python -c "import tasker_core; print(tasker_core.__version__)"

# TypeScript
npx tasker --version
```

## Next Steps

- [Choosing Your Package](choosing-your-package.md) — Language-specific guidance
- [Your First Handler](first-handler.md) — Write your first step handler
- [Your First Workflow](first-workflow.md) — Create a complete workflow
