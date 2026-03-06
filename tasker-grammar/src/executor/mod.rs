//! Standalone composition executor.
//!
//! The [`CompositionExecutor`] chains capability executions according to a
//! [`CompositionSpec`](crate::types), threading the composition context envelope
//! (`.context`, `.deps`, `.prev`, `.step`) through each step.
//!
//! This executor is a pure data transformation — it knows nothing about workers,
//! queues, handlers, or orchestration. The worker integration layer wraps this
//! executor as a `StepHandler`.
//!
//! **Ticket**: TAS-334
