# Changelog

All notable changes to Tasker Core will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

During alpha (0.1.x), all crates version together. Post-alpha, crates may adopt independent versioning.

## [Unreleased]

### Added

- Inter-crate version fields for crates.io publishing (TAS-190)
- Per-crate README, LICENSE, and CODE_OF_CONDUCT files
- Community infrastructure: CONTRIBUTING.md, issue/PR templates, SECURITY.md

## [0.1.0] - 2025-12-01

Initial alpha release of Tasker Core.

### Crates

- `tasker-pgmq` — PGMQ wrapper with PostgreSQL NOTIFY support
- `tasker-shared` — Shared types, models, state machines, configuration
- `tasker-client` — REST and gRPC API client library
- `tasker-ctl` — Command-line interface
- `tasker-orchestration` — Orchestration server with actor-based coordination
- `tasker-worker` — Worker foundation with FFI support

### Highlights

- DAG-based workflow orchestration with 12 task states and 8 step states
- PostgreSQL-native messaging via PGMQ with optional RabbitMQ backend
- Event-driven worker processing with push notifications and polling fallback
- Multi-language step handlers: Rust, Ruby (Magnus FFI), Python (PyO3), TypeScript (C ABI)
- REST and gRPC APIs with OpenAPI documentation
- Circuit breakers, Dead Letter Queue, batch processing
- TOML-based configuration with base/environment layering
- 1185+ tests across Rust, Ruby, Python, and TypeScript
