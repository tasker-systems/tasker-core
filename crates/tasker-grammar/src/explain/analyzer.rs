use crate::explain::types::{ExplanationTrace, SimulationInput};
use crate::types::CompositionSpec;
use crate::validation::CapabilityRegistry;
use crate::ExpressionEngine;

/// Analyzes a CompositionSpec to produce a data flow trace.
pub struct ExplainAnalyzer<'a> {
    #[expect(dead_code, reason = "used in Task 3 static analysis implementation")]
    registry: &'a dyn CapabilityRegistry,
    expression_engine: &'a ExpressionEngine,
}

impl std::fmt::Debug for ExplainAnalyzer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExplainAnalyzer")
            .field("expression_engine", &self.expression_engine)
            .finish_non_exhaustive()
    }
}

impl<'a> ExplainAnalyzer<'a> {
    /// Create a new analyzer with the given capability registry and expression engine.
    pub fn new(
        registry: &'a dyn CapabilityRegistry,
        expression_engine: &'a ExpressionEngine,
    ) -> Self {
        Self {
            registry,
            expression_engine,
        }
    }

    /// Produce a static analysis trace (no expression evaluation).
    pub fn analyze(&self, _spec: &CompositionSpec) -> ExplanationTrace {
        todo!("Task 3 implements this")
    }

    /// Produce a trace with simulated expression evaluation.
    pub fn analyze_with_simulation(
        &self,
        _spec: &CompositionSpec,
        _input: &SimulationInput,
    ) -> ExplanationTrace {
        todo!("Task 4 implements this")
    }
}
