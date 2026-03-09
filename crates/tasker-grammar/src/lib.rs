//! Action grammar crate for Tasker workflow orchestration.
//!
//! This crate implements the grammar system — a pure data transformation library
//! that enables "virtual handler" workflow steps whose behavior is defined
//! declaratively as a composition of typed capabilities rather than imperative
//! handler code.
//!
//! # Module structure
//!
//! - [`expression`] — jaq-core expression engine with sandboxed evaluation (TAS-321)
//! - [`types`] — core type definitions: `GrammarCategory`, `CapabilityDeclaration`,
//!   `CompositionSpec`, `CapabilityExecutor` (TAS-323)
//! - [`capabilities`] — capability executor implementations: transform, validate,
//!   assert, persist, acquire, emit (TAS-324–332)
//! - [`validation`] — `CompositionValidator` with JSON Schema contract chaining (TAS-333)
//! - [`executor`] — standalone `CompositionExecutor` with capability chaining (TAS-334)
//!
//! # Design principles
//!
//! - **No infrastructure dependencies**: no database, messaging, workers, or orchestration.
//! - **Pure data transformation**: all operations take `serde_json::Value` and produce `Value`.
//! - **Grammar-specific error types**: no dependency on `tasker-shared`; other crates
//!   transform at the boundary.
//! - **Independently testable**: `cargo test -p tasker-grammar` with no services running.

pub mod capabilities;
pub mod executor;
pub mod expression;
pub mod operations;
pub mod types;
pub mod validation;

pub use expression::{ExpressionEngine, ExpressionEngineConfig, ExpressionError};
pub use operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, CapturedEmit, CapturedPersist,
    ConflictStrategy, EmitMetadata, EmitResult, EmittableResource, InMemoryOperationProvider,
    InMemoryOperations, OperationProvider, PersistConstraints, PersistResult, PersistableResource,
    ResourceOperationError,
};
pub use types::{
    AcquireCategory, AssertCategory, CapabilityDeclaration, CapabilityError, CapabilityExecutor,
    CapabilityInvocation, CompositionCheckpoint, CompositionConstraint, CompositionEnvelope,
    CompositionError, CompositionSpec, EmitCategory, ExecutionContext, GrammarCategory,
    GrammarCategoryKind, IdempotencyProfile, MutationProfile, OnFailure, OutcomeDeclaration,
    PersistCategory, RegistrationError, Severity, TransformCategory, TypedCapabilityExecutor,
    UnknownCategoryError, UnknownOnFailureError, ValidateCategory, ValidationFinding,
};
