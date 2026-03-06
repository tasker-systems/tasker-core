//! Capability executor implementations.
//!
//! Each capability is a pure function: `(input: Value, config: Value) → Result<Value>`.
//! No database, no messaging, no worker context.
//!
//! ## Data capabilities (pure, no side effects)
//!
//! - `transform` — jaq filter execution with JSON Schema output validation (TAS-325/326/327)
//! - `validate` — JSON Schema validation with coercion modes (TAS-324)
//! - `assert` — jaq boolean filter evaluation; gates execution (TAS-328)
//!
//! ## Action capabilities (side-effecting, tested with stubs in Phase 1)
//!
//! - `persist` — resource abstraction layer with jaq data filter (TAS-330)
//! - `acquire` — resource abstraction layer with jaq result filter (TAS-331)
//! - `emit` — domain event construction with jaq payload filter (TAS-332)
//!
//! **Tickets**: TAS-324 through TAS-332
