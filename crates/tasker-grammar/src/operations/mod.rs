//! Operation-level resource traits for grammar capability executors.
//!
//! These traits define the grammar's interface for resource operations.
//! They are what persist, acquire, and emit capability executors call through.
//! They know nothing about PostgreSQL, HTTP, or any specific backend —
//! they speak the grammar's language of entities, structured data, and constraints.
//!
//! # Architecture
//!
//! - **Trait definitions** live here in tasker-grammar
//! - **Production implementations** (adapters) live in tasker-runtime
//! - **Test implementations** (`InMemoryOperations`) live here alongside the traits
//! - **`OperationProvider`** is the seam between grammar executors and the runtime
//!
//! See: `docs/composition-architecture/roadmap.md` (Lane 1B)
//! See: `docs/research/resource-handle-traits-and-seams.md`

mod error;
mod testing;
mod traits;
mod types;

pub use error::*;
pub use testing::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod tests;
