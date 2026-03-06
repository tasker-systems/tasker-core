use std::fmt;

/// Errors that can occur during capability execution.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    /// The capability's configuration is invalid.
    #[error("config validation error: {0}")]
    ConfigValidation(String),

    /// The input data does not match the expected schema.
    #[error("input validation error: {0}")]
    InputValidation(String),

    /// The output data does not match the declared schema.
    #[error("output validation error: {0}")]
    OutputValidation(String),

    /// A jaq expression evaluation failed within the capability.
    #[error("expression evaluation error: {0}")]
    ExpressionEvaluation(String),

    /// The capability execution timed out.
    #[error("capability execution timed out")]
    Timeout,

    /// A referenced external resource was not found.
    #[error("resource not found: {0}")]
    ResourceNotFound(String),

    /// Checkpoint persistence or restoration failed.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// General execution error not covered by other variants.
    #[error("execution error: {0}")]
    Execution(String),
}

/// Errors that can occur during composition execution.
#[derive(Debug, thiserror::Error)]
pub enum CompositionError {
    /// A step within the composition failed.
    #[error("step {step_index} ({capability}) failed: {cause}")]
    StepExecution {
        step_index: usize,
        capability: String,
        #[source]
        cause: CapabilityError,
    },

    /// The composition spec failed validation.
    #[error("composition validation failed: {0}")]
    Validation(String),

    /// Checkpoint restoration failed during resume.
    #[error("checkpoint restore error: {0}")]
    CheckpointRestore(String),
}

/// Errors that can occur during capability or category registration.
#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    /// A capability or category with this name is already registered.
    #[error("name conflict: '{0}' is already registered")]
    NameConflict(String),

    /// The provided schema is invalid.
    #[error("invalid schema: {0}")]
    InvalidSchema(String),

    /// The capability is not compatible with its declared grammar category.
    #[error("incompatible with category: {0}")]
    IncompatibleWithCategory(String),
}

impl fmt::Display for super::ValidationFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.severity, self.code, self.message)
    }
}
