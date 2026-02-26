# ADR-010: Messaging Strategy Pattern Abstraction

**Status**: Accepted
**Date**: 2026-01
**Ticket**: TAS-133

## Context

Tasker Core originally used PGMQ as a hard-coded messaging backend. As the system matured, we needed to support alternative messaging providers (particularly RabbitMQ) without requiring code changes in orchestration or worker components. The messaging layer touches every component: task initialization, step enqueueing, result processing, and push notifications.

Key tensions:

- PGMQ provides signal-only notifications (via `pg_notify`), requiring a separate fetch step to retrieve the full message
- RabbitMQ provides full message delivery in push notifications, requiring no follow-up fetch
- Different providers have different capability sets (dead letter queues, delayed delivery, priority, message groups)
- We needed to avoid trait objects and dynamic dispatch in the hot path

## Decision

Adopt a **strategy pattern** using an enum-based provider abstraction with capability marker traits:

- **Core trait**: `MessagingService` defines queue lifecycle, send/receive/ack operations
- **Capability traits**: `SupportsPushNotifications`, `SupportsDeadLetter`, `SupportsPriority`, `SupportsDelayedDelivery` as marker traits for optional capabilities
- **`MessagingProvider` enum**: Zero-cost abstraction using exhaustive match (no trait objects, no vtable overhead)
- **`MessageClient` facade**: Domain-level operations (`send_step_message`, `send_step_result`) with queue classifier logic
- **`MessageNotification` enum**: Two variants handle the signal-vs-payload divide:
  - `Available { queue_name, msg_id }` — PGMQ style (signal-only, requires fetch)
  - `Message(QueuedMessage<Vec<u8>>)` — RabbitMQ style (full payload delivery)
- **Dual command variants**: `*FromMessage` (full payload) and `*FromMessageEvent` (signal-only) enable provider-agnostic command routing

Provider implementations live in `tasker-shared/src/messaging/service/providers/`:

- **PGMQ** (default): Signal-only push, requires fallback polling, supports fetch-by-ID
- **RabbitMQ** (first-class alternative): Full message push via `lapin` (AMQP 0.9.1), work queue model
- **InMemory**: For testing

## Consequences

### Positive

- Zero-code migration between PGMQ and RabbitMQ via configuration change
- Enum dispatch avoids trait object overhead in the messaging hot path
- Exhaustive match ensures all providers are handled at compile time
- Community providers (SQS, Redis Streams, Kafka) can be added via `tasker-contrib` without modifying core
- Each deployment can choose the messaging backend that fits its operational constraints

### Negative

- Dual command variants (`*FromMessage`/`*FromMessageEvent`) add complexity to command routing
- New providers require adding an enum variant (requires core change, not purely extensible)
- Signal-vs-payload abstraction leaks into the worker event system design

### Neutral

- PGMQ remains the default and only hard dependency (PostgreSQL is already required)
- RabbitMQ is opt-in via feature flag and configuration

## Alternatives Considered

### Alternative 1: Trait Objects (`Box<dyn MessagingService>`)

Dynamic dispatch via trait objects. Rejected because messaging is on the hot path and the number of providers is small and known at compile time. Enum dispatch is zero-cost and provides exhaustive matching.

### Alternative 2: Compile-Time Feature Flags Only

Select provider at compile time via feature flags, no runtime abstraction. Rejected because it prevents runtime provider selection and makes testing across providers harder.

### Alternative 3: Event Streaming (Kafka-Style)

Use an event streaming model instead of work queues. Rejected because Tasker's step execution model is work-queue-oriented (each step processed once by one worker), not event-streaming-oriented. RabbitMQ's `lapin` crate with work queue pattern fits better than Kafka's consumer group model.

## References

- [TAS-133 Spec](../ticket-specs/) (archived)
- [Messaging Abstraction Architecture](../architecture/messaging-abstraction.md)
- [Worker Event Systems](../architecture/worker-event-systems.md)
- [Events and Commands](../architecture/events-and-commands.md)
