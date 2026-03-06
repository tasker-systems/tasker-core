use serde_json::Value;

/// Typed view over the composition context envelope passed to capability executors.
///
/// The composition context envelope is a `serde_json::Value` with four well-known
/// fields. This type provides stable accessor methods so executors don't scatter
/// raw `.get("prev")` / `.get("context")` calls — the shape is defined once here.
///
/// ## Envelope structure
///
/// | Field | Source | Mutates between invocations? |
/// |-------|--------|------------------------------|
/// | `.context` | Task-level input data (immutable) | No |
/// | `.deps` | Dependency step results keyed by step name | No |
/// | `.step` | Step metadata: name, attempt count, inputs | No |
/// | `.prev` | Output of the most recent capability invocation | **Yes** |
///
/// ## Usage
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::types::CompositionEnvelope;
/// let raw = json!({
///     "context": {"order_id": "ORD-001"},
///     "deps": {"step_a": {"total": 42}},
///     "step": {"name": "create_order"},
///     "prev": {"validated": true}
/// });
///
/// let env = CompositionEnvelope::new(&raw);
/// assert_eq!(env.context()["order_id"], json!("ORD-001"));
/// assert_eq!(env.dep("step_a")["total"], json!(42));
/// assert!(env.has_prev());
/// assert_eq!(env.prev()["validated"], json!(true));
/// ```
///
/// The `resolve_target` method implements the common pattern: use `.prev` when
/// present and non-null, otherwise fall back to `.context`.
///
/// ```
/// # use serde_json::json;
/// # use tasker_grammar::types::CompositionEnvelope;
/// // First invocation — prev is null, resolves to context
/// let first = json!({"context": {"name": "Alice"}, "deps": {}, "step": {}, "prev": null});
/// let env = CompositionEnvelope::new(&first);
/// assert_eq!(env.resolve_target()["name"], json!("Alice"));
///
/// // Subsequent invocation — prev has data
/// let later = json!({"context": {}, "deps": {}, "step": {}, "prev": {"validated": true}});
/// let env = CompositionEnvelope::new(&later);
/// assert_eq!(env.resolve_target()["validated"], json!(true));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CompositionEnvelope<'a> {
    raw: &'a Value,
}

impl<'a> CompositionEnvelope<'a> {
    /// Wrap a raw `serde_json::Value` as a typed envelope view.
    pub fn new(raw: &'a Value) -> Self {
        Self { raw }
    }

    /// The raw envelope value, for passing directly to expression engines.
    pub fn raw(&self) -> &'a Value {
        self.raw
    }

    /// Task-level input data (immutable across invocations).
    pub fn context(&self) -> &'a Value {
        self.raw.get("context").unwrap_or(&Value::Null)
    }

    /// Dependency step results keyed by step name (immutable).
    pub fn deps(&self) -> &'a Value {
        self.raw.get("deps").unwrap_or(&Value::Null)
    }

    /// Result of a specific dependency step, or `Value::Null` if not present.
    pub fn dep(&self, step_name: &str) -> &'a Value {
        self.deps().get(step_name).unwrap_or(&Value::Null)
    }

    /// Step metadata: name, attempt count, inputs (immutable).
    pub fn step(&self) -> &'a Value {
        self.raw.get("step").unwrap_or(&Value::Null)
    }

    /// Output of the most recent capability invocation, or `Value::Null` for the
    /// first invocation.
    pub fn prev(&self) -> &'a Value {
        self.raw.get("prev").unwrap_or(&Value::Null)
    }

    /// Whether `.prev` is present and non-null.
    pub fn has_prev(&self) -> bool {
        matches!(self.raw.get("prev"), Some(v) if !v.is_null())
    }

    /// Resolve the validation/processing target: `.prev` if present and non-null,
    /// otherwise `.context`.
    ///
    /// This is the standard pattern for capabilities that process "the most recent
    /// data" — validate, assert, etc. Transform doesn't need this because jaq
    /// filters address fields explicitly.
    pub fn resolve_target(&self) -> &'a Value {
        if self.has_prev() {
            self.prev()
        } else {
            self.context()
        }
    }
}
