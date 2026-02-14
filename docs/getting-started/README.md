# Getting Started

Tasker is a distributed workflow orchestration system that coordinates complex, multi-step processes across services and languages. It provides:

- **Task Orchestration** — Define workflows as directed acyclic graphs (DAGs) with dependency management
- **Multi-Language Support** — Write handlers in Rust, Ruby, Python, or TypeScript
- **Built-in Resilience** — Automatic retries, error handling, and state persistence
- **Event-Driven Architecture** — Pub/sub events for real-time observability

## How Tasker Works

```
┌─────────────────────────────────────────────────────────────────────┐
│                        tasker-core (Rust)                           │
│  • REST API for task submission                                     │
│  • Workflow orchestration                                           │
│  • Step execution and dependency resolution                         │
│  • PostgreSQL state persistence                                     │
│  • Event publishing (NATS)                                          │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
              ┌──────────┐ ┌──────────┐ ┌──────────┐
              │  Ruby    │ │  Python  │ │TypeScript│
              │ Workers  │ │ Workers  │ │ Workers  │
              └──────────┘ └──────────┘ └──────────┘
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Task** | A unit of work submitted for execution |
| **Task Template** | YAML definition of a workflow's steps and dependencies |
| **Step** | A single operation within a workflow |
| **Step Handler** | Your code that executes a step's business logic |
| **Workflow Step** | A step that starts another task (sub-workflow) |

## What You'll Build

With Tasker, you'll typically:

1. **Define Task Templates** — YAML files describing workflow structure
2. **Write Step Handlers** — Functions that execute business logic
3. **Submit Tasks** — Use the client SDK to start workflows
4. **Monitor Execution** — Observe via events or the admin dashboard

## Learning Path

1. **[Core Concepts](concepts.md)** — Tasks, steps, handlers, templates, and dependencies
2. **[Installation](install.md)** — Installing packages and running infrastructure
3. **[Choosing Your Package](choosing-your-package.md)** — Which language package fits your needs?
4. **[Your First Handler](first-handler.md)** — Write a step handler in your language
5. **[Your First Workflow](first-workflow.md)** — Define a template, submit a task, watch it run
6. **[Next Steps](next-steps.md)** — Where to go from here

## Language Guides

Comprehensive guides for each supported language:

- **[Rust](rust.md)** — Native Rust workers with `tasker-worker`
- **[Ruby](ruby.md)** — Ruby workers with `tasker-rb`
- **[Python](python.md)** — Python workers with `tasker-py`
- **[TypeScript](typescript.md)** — TypeScript workers with `@tasker-systems/tasker`
