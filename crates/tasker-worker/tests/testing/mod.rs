//! Worker testing infrastructure for integration tests.
//!
//! Provides test data factories for creating worker test objects
//! with database-backed persistence.

pub mod factory;

pub use factory::{WorkerTestData, WorkerTestFactory};
