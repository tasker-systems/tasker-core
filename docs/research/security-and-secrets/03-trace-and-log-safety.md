# Concern 3: Trace and Log Safety

*How PII and sensitive data in legitimate step contexts is protected at the observability boundary*

*Research spike — March 2026*

---

## The Different Nature of This Problem

Concerns 1 and 2 address credentials — values that should never enter the data path. The solution is architectural: keep credentials in the resource layer, never in jaq contexts or step results.

This concern is different. It addresses sensitive data that legitimately flows through the data path because it is the data being processed. A payment workflow's `.context` contains a credit card number. A KYC workflow's step result contains a passport number. A healthcare workflow's `.prev` contains a diagnosis code. These values cannot be excluded — excluding them would be excluding the point of the workflow.

The question is not "how do we keep this data out of the system" but "how do we prevent it from leaking out of the system into observability infrastructure."

---

## The Observability Leak Surface

The specific surfaces where sensitive data can escape from its legitimate processing context into observability systems:

**Structured log records**: When a step fails, error logs may include the step context, the input data, or the partially-processed output. Without classification-aware filtering, a log record might serialize the entire composition context envelope including sensitive fields.

**Distributed trace spans**: Span attributes added for debugging purposes (e.g., "step inputs," "capability execution results") may contain PII. Traces are exported to external systems (Jaeger, Honeycomb, Datadog) where they are stored indefinitely and searchable.

**Error messages surfaced via API**: A `StepExecutionError` that includes the field value that failed validation should not include sensitive data in the message that is returned to the API caller.

**Checkpoint data in logs**: The `CompositionCheckpoint` stored in `workflow_steps.checkpoint` is fine in the database (encryption at rest is Concern 4). But if checkpoint data is logged during resume/retry operations, sensitive fields in `.prev` could appear in logs.

**What is NOT in scope**: The database itself (that is Concern 4, encryption at rest). The data flowing through jaq filters during execution (that's the point; we want it there). The step results stored in the database (also Concern 4).

---

## The DataClassifier Design

The `DataClassifier` is the mechanism that enforces field-level classification at emission points. It is configured by the template author, operates at the observability layer, and is transparent to the composition execution engine.

```rust
/// Applies data classification rules to structured data before it leaves the process
/// boundary into observability systems (traces, logs, error messages).
///
/// The DataClassifier is configured from the TaskTemplate's `data_classification`
/// section. A single classifier instance is created per task execution and passed
/// through the execution context.
pub struct DataClassifier {
    rules: Vec<ClassificationRule>,
}

impl DataClassifier {
    /// Redact sensitive fields in a JSON value according to the configured rules.
    /// Returns a new Value with classified fields replaced by their redaction token.
    /// The original value is not modified.
    pub fn redact(&self, value: &Value, scope: DataScope) -> Value;

    /// Check whether a specific JSON path is classified.
    pub fn is_classified(&self, path: &str, scope: DataScope) -> bool;

    /// Apply classification to a tracing span's attributes.
    /// Replaces field values in span attribute maps.
    pub fn redact_span_attributes(
        &self,
        attrs: HashMap<String, String>,
        scope: DataScope,
    ) -> HashMap<String, String>;
}

#[derive(Debug, Clone)]
pub struct ClassificationRule {
    /// JsonPath expression identifying the field(s) to classify.
    /// Supports wildcards: "$.items[*].card_number" matches all item card numbers.
    pub path: String,

    /// What kind of sensitive data this is.
    pub classification: DataClassification,

    /// How to handle this field in trace spans.
    pub trace_behavior: TraceBehavior,

    /// How to handle this field in structured log records.
    pub log_behavior: LogBehavior,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataClassification {
    /// Personally Identifiable Information — names, addresses, SSNs, passport numbers
    Pii,
    /// Payment card data — card numbers, CVVs, expiry dates
    PaymentCard,
    /// Healthcare data — diagnoses, medications, insurance IDs
    HealthcarePhi,
    /// Authentication credentials — tokens, keys (should not be in context, but belt + suspenders)
    Credential,
    /// Organization-defined sensitive data category
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum TraceBehavior {
    /// Replace field value with "[REDACTED:<classification>]"
    Redact,
    /// Remove the field from the span attribute entirely
    Omit,
    /// Include the field with its actual value (explicit opt-in to exposure)
    Include,
}

#[derive(Debug, Clone)]
pub enum LogBehavior {
    /// Replace field value with "[REDACTED:<classification>]"
    Redact,
    /// Remove the field from the log record entirely
    Omit,
    /// Include the field with its actual value
    Include,
}

/// Which data context is being classified.
/// Different scopes can have different rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataScope {
    /// Task input context (.context in composition context envelope)
    TaskContext,
    /// Dependency step result (.deps.step_name)
    DependencyResult,
    /// Capability invocation output (.prev)
    CapabilityOutput,
    /// Step execution result (StepExecutionResult.result)
    StepResult,
    /// Checkpoint data
    CheckpointData,
}
```

---

## TaskTemplate Classification Spec

The template author declares field classifications in the `data_classification` section of the TaskTemplate YAML. This is the authoring surface — where the person who understands the data's sensitivity makes explicit declarations.

```yaml
name: process_payment
namespace: billing
version: "1.0.0"

# Data classification declarations.
# These apply to all composition steps in this template.
# Domain handler steps are not affected — their data handling is their own concern.
data_classification:
  context_fields:
    - path: "$.credit_card_number"
      classification: payment_card
      trace_behavior: redact
      log_behavior: omit
    - path: "$.card_cvv"
      classification: payment_card
      trace_behavior: omit
      log_behavior: omit
    - path: "$.billing_address.*"   # wildcard covers all address subfields
      classification: pii
      trace_behavior: redact
      log_behavior: redact

  result_fields:
    # The step result contains a payment token — not a raw card number, but still sensitive
    - path: "$.steps.tokenize_card.card_token"
      classification: payment_card
      trace_behavior: redact
      log_behavior: redact
```

```yaml
name: kyc_verification
namespace: compliance
version: "1.0.0"

data_classification:
  context_fields:
    - path: "$.ssn"
      classification: pii
      trace_behavior: omit
      log_behavior: omit
    - path: "$.passport_number"
      classification: pii
      trace_behavior: redact
      log_behavior: omit
    - path: "$.date_of_birth"
      classification: pii
      trace_behavior: redact
      log_behavior: redact
  result_fields:
    - path: "$.steps.*.verification_notes"
      classification: custom("compliance_sensitive")
      trace_behavior: omit
      log_behavior: omit
```

### Parsing and Validation

The `DataClassificationSpec` is parsed as part of the TaskTemplate by `tasker-sdk`. Validation checks:

- JsonPath expressions are syntactically valid
- Classification values are recognized (or `custom(...)` with a non-empty name)
- `trace_behavior` and `log_behavior` values are recognized
- No duplicate paths within the same scope

The validator emits warnings (not errors) for paths that don't resolve in the template's input schema or result schemas — these may be valid for dynamically-shaped data, but the warning prompts the author to verify.

---

## Integration in the Composition Executor

The `DataClassifier` is constructed from the TaskTemplate's classification spec when the composition executor is initialized for a step execution. It is passed through the `ExecutionContext`:

```rust
pub struct ExecutionContext {
    pub step_uuid: Uuid,
    pub correlation_id: String,
    pub checkpoint: Arc<CheckpointService>,
    pub checkpoint_state: Option<CheckpointRecord>,
    pub step_config: serde_json::Value,
    pub resources: Arc<ResourceRegistry>,
    /// Data classifier for this task's classification rules.
    /// None if the template has no data_classification section.
    pub classifier: Option<Arc<DataClassifier>>,
}
```

The composition executor uses the classifier at emission points:

```rust
// When emitting a trace span for capability execution:
if let Some(classifier) = &context.classifier {
    let redacted_input = classifier.redact(&capability_input, DataScope::CapabilityOutput);
    tracer.span("capability_execute")
        .with_attribute("input", redacted_input.to_string())
        .emit();
} else {
    // No classification spec — emit as-is
    tracer.span("capability_execute")
        .with_attribute("input", capability_input.to_string())
        .emit();
}

// When logging a step error:
if let Some(classifier) = &context.classifier {
    let redacted_context = classifier.redact(&step_context, DataScope::TaskContext);
    tracing::error!(
        step_uuid = %context.step_uuid,
        error = %err,
        context = %redacted_context,
        "Capability execution failed"
    );
} else {
    tracing::error!(
        step_uuid = %context.step_uuid,
        error = %err,
        "Capability execution failed"
    );
}
```

The important property: **the actual data flowing through jaq filters is unaffected**. The `DataClassifier::redact()` call produces a new `Value` for observability purposes — the original value continues to flow through the composition normally. Classification does not interfere with execution.

---

## What This Does NOT Do

**It does not scan for PII automatically.** The `DataClassifier` applies declared rules. If the template author does not declare that `$.ssn` is PII, it will not be redacted. This is intentional: automatic PII detection in arbitrary JSON is unreliable and would produce false positives and false negatives. The template author is the person who knows what the data is.

**It does not encrypt in transit.** That is handled by TLS on the connections themselves — the HTTP client handles used by `acquire` capability executors use HTTPS. The PGMQ connections are over TLS if the PostgreSQL connection is. This is not a gap in the DataClassifier; it is a different concern.

**It does not apply to domain handlers.** A Ruby handler's execution is opaque to Tasker. If a Ruby handler logs sensitive data, the DataClassifier cannot prevent it. Domain handler authors are responsible for their own log hygiene.

**It does not provide GDPR-style right-to-erasure.** Redaction in traces and logs reduces the blast radius of a trace/log exfiltration. It does not remove data from the database (that is Concern 4, at-rest encryption, plus a separate data lifecycle concern that is out of scope for this spike).

---

## Open Questions for the Research Spike

These are questions to be resolved in Spike S4:

1. **Wildcard path resolution performance.** JsonPath wildcards like `$.items[*].card_number` applied to a large array in every trace span emission could be expensive. Does the classifier need to compile rules to a more efficient representation? Should there be a simple mode (exact paths only) and a wildcard mode?

2. **Redaction token format.** `"[REDACTED:pii]"` is human-readable. But if trace analysis tooling parses attribute values, the token format may need to be consistent and parseable. Worth consulting with any existing trace infrastructure requirements.

3. **Where to store the compiled classifier.** Constructing the classifier per step execution (from the TaskTemplate's classification spec on every step claim) is wasteful. Should it be compiled once when the TaskTemplate is loaded and cached at the worker level? The `TaskTemplateManager` is the natural home.

4. **Domain handler integration (opt-in).** Should there be a mechanism for domain handler authors to opt into the DataClassifier for their own log/trace emission? This would be a convenience, not a requirement — domain handler authors can implement their own redaction. But a shared `DataClassifier` they can access through the handler context might reduce duplicated effort.

---

*Read next: `04-encryption-at-rest.md` for how sensitive data stored in the PostgreSQL database is protected.*
