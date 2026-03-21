//! Composition explanation and data flow tracing.
//!
//! The [`ExplainAnalyzer`] produces an [`ExplanationTrace`] that visualizes how
//! data flows through a composition's invocation chain. Two modes:
//!
//! - **Static analysis**: traces structure, envelope field availability, expression
//!   references, output schemas, and checkpoint placement.
//! - **Simulated evaluation**: when [`SimulationInput`] is provided, evaluates jaq
//!   expressions against sample data and threads computed results through the chain.
//!
//! **Ticket**: TAS-344

mod analyzer;
mod types;

pub use analyzer::ExplainAnalyzer;
pub use types::{
    EnvelopeSnapshot, ExplanationTrace, ExpressionReference, InvocationTrace, OutcomeSummary,
    SimulationInput,
};

#[cfg(test)]
mod tests;
