//! Adapter registry and resource-specific adapter implementations.
//!
//! Each adapter wraps a `tasker_secure::ResourceHandle` and implements
//! the corresponding grammar operation trait (`PersistableResource`,
//! `AcquirableResource`, `EmittableResource`).

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "http")]
pub mod http;

pub mod messaging;
pub mod registry;

#[cfg(feature = "postgres")]
pub mod sql_gen;

pub use registry::AdapterRegistry;
