//! Core type definitions for the action grammar system.
//!
//! This module defines the foundational types that all grammar capabilities depend on:
//! - [`GrammarCategory`] — trait describing a category of capabilities (transform, validate, etc.)
//! - [`CapabilityDeclaration`] — serializable declaration of a capability's interface and schemas
//! - [`CompositionSpec`] — serializable specification of a capability composition
//! - [`CompositionStep`] — a single step within a composition
//! - [`CapabilityExecutor`] — trait for executing a capability against input data
//!
//! All types are pure data structures with no runtime dependencies on workers,
//! orchestration, or infrastructure.
//!
//! **Ticket**: TAS-323
