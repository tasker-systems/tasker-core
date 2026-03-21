use std::collections::HashMap;

use serde_json::Value;

use crate::explain::types::{
    EnvelopeSnapshot, ExplanationTrace, ExpressionReference, InvocationTrace, OutcomeSummary,
    SimulationInput,
};
use crate::types::{
    CompositionSpec, GrammarCategoryKind, MutationProfile, Severity, ValidationFinding,
};
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

/// Build the envelope for expression evaluation, matching the executor's shape.
fn build_envelope(
    context: &Value,
    deps: &Value,
    step: &Value,
    prev: &Value,
    accumulated: &HashMap<usize, Value>,
) -> Value {
    let invocation_outputs: serde_json::Map<String, Value> = accumulated
        .iter()
        .map(|(idx, output)| (idx.to_string(), output.clone()))
        .collect();

    let mut deps_with_invocations = match deps {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    if !invocation_outputs.is_empty() {
        deps_with_invocations.insert("invocations".to_owned(), Value::Object(invocation_outputs));
    }

    serde_json::json!({
        "context": context,
        "deps": deps_with_invocations,
        "step": step,
        "prev": prev,
    })
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
        self.analyze_internal(spec, None)
    }

    /// Produce a trace with simulated expression evaluation.
    pub fn analyze_with_simulation(
        &self,
        spec: &CompositionSpec,
        input: &SimulationInput,
    ) -> ExplanationTrace {
        self.analyze_internal(spec, Some(input))
    }

    /// Shared implementation for both static analysis and simulated evaluation.
    fn analyze_internal(
        &self,
        spec: &CompositionSpec,
        simulation: Option<&SimulationInput>,
    ) -> ExplanationTrace {
        let is_simulated = simulation.is_some();

        // Run validator and capture findings
        let validator = CompositionValidator::new(self.registry, self.expression_engine);
        let validation_result = validator.validate(spec);
        let mut findings = validation_result.findings;

        let outcome = OutcomeSummary {
            description: spec.outcome.description.clone(),
            output_schema: spec.outcome.output_schema.clone(),
        };

        // Handle degenerate cases
        let is_degenerate = findings
            .iter()
            .any(|f| f.code == "EMPTY_COMPOSITION" || f.code == "TOO_MANY_INVOCATIONS");

        if is_degenerate {
            return ExplanationTrace {
                name: spec.name.clone(),
                outcome,
                invocations: vec![],
                validation: findings,
                simulated: is_simulated,
            };
        }

        // Walk invocations in order, building trace entries
        let mut invocation_traces = Vec::with_capacity(spec.invocations.len());
        let mut prev_schema: Option<Value> = None;
        let mut prev_source: Option<String> = None;

        // Simulation state: tracked .prev value and accumulated outputs
        let mut sim_prev: Value = Value::Null;
        let mut accumulated: HashMap<usize, Value> = HashMap::new();

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
                prev_schema = None;
                prev_source = None;
                sim_prev = Value::Null;
                continue;
            };

            let is_mutating = matches!(decl.mutation_profile, MutationProfile::Mutating { .. });

            // Build the simulation envelope if in simulation mode
            let sim_envelope = simulation.map(|input| {
                build_envelope(
                    &input.context,
                    &input.deps,
                    &input.step,
                    &sim_prev,
                    &accumulated,
                )
            });

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

                    // Evaluate expression against simulation envelope if available
                    let simulated_result = sim_envelope.as_ref().and_then(|env| {
                        match self.expression_engine.evaluate(expr, env) {
                            Ok(result) => Some(result),
                            Err(_) => {
                                findings.push(ValidationFinding {
                                    severity: Severity::Warning,
                                    code: "SIMULATION_EVAL_FAILURE".to_owned(),
                                    invocation_index: Some(idx),
                                    message: format!(
                                        "Expression evaluation failed for '{}' at invocation {}",
                                        expr, idx
                                    ),
                                    field_path: Some(field_path.clone()),
                                });
                                None
                            }
                        }
                    });

                    expressions.push(ExpressionReference {
                        field_path,
                        expression: expr.to_owned(),
                        referenced_paths,
                        simulated_result,
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

                                let simulated_result = sim_envelope.as_ref().and_then(|env| {
                                    self.expression_engine.evaluate(expr, env).ok()
                                });

                                expressions.push(ExpressionReference {
                                    field_path,
                                    expression: expr.to_owned(),
                                    referenced_paths,
                                    simulated_result,
                                });
                            }
                        }
                    }
                }
            }

            // Extract declared output schema
            let output_schema = self.extract_output_schema(invocation, decl);

            // Compute simulated output and update sim_prev for next invocation
            let (simulated_output, mock_output_used) = if let Some(sim_input) = simulation {
                self.compute_simulation_output(
                    idx,
                    decl.grammar_category,
                    &expressions,
                    &sim_prev,
                    sim_input,
                    &mut findings,
                )
            } else {
                (None, false)
            };

            // Update sim_prev for next invocation
            if let Some(ref output) = simulated_output {
                sim_prev = output.clone();
                accumulated.insert(idx, output.clone());
            }

            // Track for next invocation's envelope (static schema tracking)
            if let Some(ref schema) = output_schema {
                prev_schema = Some(schema.clone());
                prev_source = Some(format!(
                    "output of invocation {} ({})",
                    idx, invocation.capability
                ));
            } else {
                match decl.grammar_category {
                    GrammarCategoryKind::Assert | GrammarCategoryKind::Validate => {
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
                simulated_output,
                mock_output_used,
            });
        }

        ExplanationTrace {
            name: spec.name.clone(),
            outcome,
            invocations: invocation_traces,
            validation: findings,
            simulated: is_simulated,
        }
    }

    /// Compute simulated output for an invocation based on its category.
    ///
    /// Returns `(simulated_output, mock_output_used)`.
    fn compute_simulation_output(
        &self,
        idx: usize,
        category: GrammarCategoryKind,
        expressions: &[ExpressionReference],
        prev: &Value,
        input: &SimulationInput,
        findings: &mut Vec<ValidationFinding>,
    ) -> (Option<Value>, bool) {
        match category {
            // Transform: the filter expression result IS the simulated output
            GrammarCategoryKind::Transform => {
                let output = expressions
                    .first()
                    .and_then(|expr_ref| expr_ref.simulated_result.clone())
                    .unwrap_or(Value::Null);
                (Some(output), false)
            }
            // Assert and Validate: pass .prev through unchanged
            GrammarCategoryKind::Assert | GrammarCategoryKind::Validate => {
                (Some(prev.clone()), false)
            }
            // Side-effecting: use mock output or null
            GrammarCategoryKind::Persist
            | GrammarCategoryKind::Acquire
            | GrammarCategoryKind::Emit => {
                if let Some(mock) = input.mock_outputs.get(&idx) {
                    (Some(mock.clone()), true)
                } else {
                    findings.push(ValidationFinding {
                        severity: Severity::Info,
                        code: "MISSING_MOCK_OUTPUT".to_owned(),
                        invocation_index: Some(idx),
                        message: format!(
                            "No mock output provided for side-effecting invocation {} ({}); \
                             .prev will be null for subsequent invocations",
                            idx,
                            match category {
                                GrammarCategoryKind::Persist => "persist",
                                GrammarCategoryKind::Acquire => "acquire",
                                GrammarCategoryKind::Emit => "emit",
                                _ => unreachable!(),
                            }
                        ),
                        field_path: None,
                    });
                    (Some(Value::Null), false)
                }
            }
        }
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
