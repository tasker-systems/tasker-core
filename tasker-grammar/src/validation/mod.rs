//! Composition validation.
//!
//! The [`CompositionValidator`] checks that a [`CompositionSpec`](crate::types) is
//! well-formed before execution: JSON Schema contract chaining between steps,
//! capability configuration validation, and structural integrity checks.
//!
//! **Ticket**: TAS-333
