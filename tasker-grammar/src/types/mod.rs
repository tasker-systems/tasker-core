//! Core type definitions for the action grammar system.
//!
//! This module defines the foundational types that all grammar capabilities depend on:
//!
//! - [`GrammarCategory`] — trait describing a category of capabilities
//! - [`GrammarCategoryKind`] — enum for exhaustive matching over categories
//! - [`MutationProfile`], [`IdempotencyProfile`] — category behavioral properties
//! - [`CapabilityDeclaration`] — serializable declaration of a capability's contracts
//! - [`CapabilityExecutor`] — trait for executing a capability
//! - [`CompositionSpec`] — ordered list of capability invocations with checkpoints
//! - [`CompositionStep`] — a single step within a composition
//! - [`CompositionCheckpoint`] — resumable execution state
//! - [`OutcomeDeclaration`] — declared output contract for a composition
//! - Grammar-specific error types ([`CapabilityError`], [`CompositionError`])
//!
//! All types are pure data structures with no runtime dependencies on workers,
//! orchestration, or infrastructure. Types that cross crate boundaries use
//! grammar-specific error types rather than `tasker-shared` types.
//!
//! **Ticket**: TAS-323

mod categories;
mod checkpoint;
mod composition;
mod declaration;
mod error;
mod executor;
mod validation;

pub use categories::{
    AcquireCategory, AssertCategory, EmitCategory, GrammarCategory, GrammarCategoryKind,
    IdempotencyProfile, MutationProfile, PersistCategory, TransformCategory, ValidateCategory,
};
pub use checkpoint::CompositionCheckpoint;
pub use composition::{CompositionSpec, CompositionStep, OutcomeDeclaration};
pub use declaration::CapabilityDeclaration;
pub use error::{CapabilityError, CompositionError, RegistrationError};
pub use executor::{CapabilityExecutor, ExecutionContext};
pub use validation::{CompositionConstraint, Severity, ValidationFinding};

#[cfg(test)]
mod tests;
