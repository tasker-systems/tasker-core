use serde_json::Value;

use crate::explain::types::{
    EnvelopeSnapshot, ExplanationTrace, ExpressionReference, InvocationTrace, OutcomeSummary,
    SimulationInput,
};
use crate::types::{CompositionSpec, GrammarCategoryKind, MutationProfile};
use crate::validation::{CapabilityRegistry, CompositionValidator};
use crate::ExpressionEngine;

/// Analyzes a CompositionSpec to produce a data flow trace.
pub struct ExplainAnalyzer<'a> {
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

/// Handles two shapes:
/// - Flat string: `"filter": ".context.name"` → `Some(".context.name")`
/// - ExpressionField object: `"data": {"expression": ".prev"}` → `Some(".prev")`
/// - Anything else: `None`
fn extract_expression(value: &Value) -> Option<&str> {
    if let Some(s) = value.as_str() {
        return Some(s);
    }
    value
        .as_object()
        .and_then(|obj| obj.get("expression"))
        .and_then(|v| v.as_str())
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
    pub fn analyze(&self, spec: &CompositionSpec) -> ExplanationTrace {
        // Run validator and capture findings
        let validator = CompositionValidator::new(self.registry, self.expression_engine);
        let validation_result = validator.validate(spec);
        let findings = validation_result.findings;

        let outcome = OutcomeSummary {
            description: spec.outcome.description.clone(),
            output_schema: spec.outcome.output_schema.clone(),
        };

        // Handle degenerate cases: empty or over-limit compositions
        // The validator already returns early for these, so findings will have the error.
        // We detect this by checking if any finding is a terminal structural error.
        let is_degenerate = findings
            .iter()
            .any(|f| f.code == "EMPTY_COMPOSITION" || f.code == "TOO_MANY_INVOCATIONS");

        if is_degenerate {
            return ExplanationTrace {
                name: spec.name.clone(),
                outcome,
                invocations: vec![],
                validation: findings,
                simulated: false,
            };
        }

        // Walk invocations in order, building trace entries
        let mut invocation_traces = Vec::with_capacity(spec.invocations.len());
        let mut prev_schema: Option<Value> = None;
        let mut prev_source: Option<String> = None;

        for (idx, invocation) in spec.invocations.iter().enumerate() {
            let decl = self.registry.get_capability(&invocation.capability);

            // Build envelope snapshot
            let envelope = EnvelopeSnapshot {
                context: true,
                deps: true,
                step: true,
                has_prev: prev_schema.is_some() || prev_source.is_some(),
                prev_source: prev_source.clone(),
                prev_schema: prev_schema.clone(),
            };

            // If capability not found, produce partial trace entry
            let Some(decl) = decl else {
                invocation_traces.push(InvocationTrace {
                    index: idx,
                    capability: invocation.capability.clone(),
                    category: GrammarCategoryKind::Transform, // default; unknown
                    checkpoint: invocation.checkpoint,
                    is_mutating: false,
                    envelope_available: envelope,
                    expressions: vec![],
                    output_schema: None,
                    simulated_output: None,
                    mock_output_used: false,
                });
                // Don't update prev_schema — unknown capability produces no output
                prev_schema = None;
                prev_source = None;
                continue;
            };

            let is_mutating = matches!(decl.mutation_profile, MutationProfile::Mutating { .. });

            // Extract expressions from category-specific config fields
            let expression_fields: &[&str] = match decl.grammar_category {
                GrammarCategoryKind::Transform => &["filter"],
                GrammarCategoryKind::Assert => &["filter"],
                GrammarCategoryKind::Persist => &["data", "validate_success", "result_shape"],
                GrammarCategoryKind::Acquire => &["params", "validate_success", "result_shape"],
                GrammarCategoryKind::Emit => {
                    &["payload", "condition", "validate_success", "result_shape"]
                }
                GrammarCategoryKind::Validate => &[],
            };

            let mut expressions = Vec::new();

            for field in expression_fields {
                if let Some(value) = invocation.config.get(*field) {
                    let (expr, field_path) = match extract_expression(value) {
                        Some(e) if value.is_string() => (e, format!("config.{field}")),
                        Some(e) => (e, format!("config.{field}.expression")),
                        None => continue,
                    };

                    let referenced_paths = self
                        .expression_engine
                        .extract_references(expr)
                        .unwrap_or_default();

                    expressions.push(ExpressionReference {
                        field_path,
                        expression: expr.to_owned(),
                        referenced_paths,
                        simulated_result: None,
                    });
                }
            }

            // Emit metadata expressions (nested one level deeper)
            if matches!(decl.grammar_category, GrammarCategoryKind::Emit) {
                if let Some(metadata) = invocation.config.get("metadata").and_then(Value::as_object)
                {
                    for meta_field in &["correlation_id", "idempotency_key"] {
                        if let Some(value) = metadata.get(*meta_field) {
                            if let Some(expr) = extract_expression(value) {
                                let field_path = format!("config.metadata.{meta_field}.expression");
                                let referenced_paths = self
                                    .expression_engine
                                    .extract_references(expr)
                                    .unwrap_or_default();

                                expressions.push(ExpressionReference {
                                    field_path,
                                    expression: expr.to_owned(),
                                    referenced_paths,
                                    simulated_result: None,
                                });
                            }
                        }
                    }
                }
            }

            // Extract declared output schema
            let output_schema = self.extract_output_schema(invocation, decl);

            // Track for next invocation's envelope
            if let Some(ref schema) = output_schema {
                prev_schema = Some(schema.clone());
                prev_source = Some(format!(
                    "output of invocation {} ({})",
                    idx, invocation.capability
                ));
            } else {
                // Assert passes .prev through unchanged; others don't declare output
                match decl.grammar_category {
                    GrammarCategoryKind::Assert => {
                        // prev_schema and prev_source remain unchanged
                    }
                    _ => {
                        prev_schema = None;
                        prev_source = None;
                    }
                }
            }

            invocation_traces.push(InvocationTrace {
                index: idx,
                capability: invocation.capability.clone(),
                category: decl.grammar_category,
                checkpoint: invocation.checkpoint,
                is_mutating,
                envelope_available: envelope,
                expressions,
                output_schema,
                simulated_output: None,
                mock_output_used: false,
            });
        }

        ExplanationTrace {
            name: spec.name.clone(),
            outcome,
            invocations: invocation_traces,
            validation: findings,
            simulated: false,
        }
    }

    /// Produce a trace with simulated expression evaluation.
    pub fn analyze_with_simulation(
        &self,
        _spec: &CompositionSpec,
        _input: &SimulationInput,
    ) -> ExplanationTrace {
        todo!("Task 4 implements this")
    }

    /// Extract the output schema declared by an invocation, if any.
    fn extract_output_schema(
        &self,
        invocation: &crate::types::CapabilityInvocation,
        decl: &crate::types::CapabilityDeclaration,
    ) -> Option<Value> {
        match decl.grammar_category {
            GrammarCategoryKind::Transform => invocation
                .config
                .get("output")
                .filter(|v| !v.is_null() && v.as_object().is_some_and(|o| !o.is_empty()))
                .cloned(),
            GrammarCategoryKind::Assert => None,
            _ => None,
        }
    }
}
