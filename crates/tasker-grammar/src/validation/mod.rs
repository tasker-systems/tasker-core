//! Composition validation.
//!
//! The [`CompositionValidator`] checks that a [`CompositionSpec`](crate::types) is
//! well-formed before execution: JSON Schema contract chaining between steps,
//! capability configuration validation, and structural integrity checks.
//!
//! ## Validation Checks
//!
//! 1. **Structural validity** — all referenced capabilities exist in the vocabulary
//! 2. **Config schema validation** — each invocation's config validates against
//!    the capability's `config_schema`
//! 3. **Contract chaining** — for `transform`, the declared `output` schema of
//!    invocation N is compatible with what invocation N+1 expects in `.prev`
//! 4. **Checkpoint coverage** — all mutating capabilities have checkpoint markers
//! 5. **Expression syntax** — all jaq expressions parse correctly
//! 6. **Output schema presence** — every `transform` invocation declares an `output` schema
//! 7. **Outcome convergence** — the final invocation's output is compatible with
//!    the declared outcome schema
//!
//! **Ticket**: TAS-333

mod schema_compat;
mod validator;

pub use validator::{CapabilityRegistry, CompositionValidator, ValidationResult};

#[cfg(test)]
mod tests;
