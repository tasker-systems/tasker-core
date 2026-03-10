use std::fmt;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::envelope::CompositionEnvelope;
use super::error::CapabilityError;

/// Object-safe trait for executing a capability against concrete inputs.
///
/// This is the *dynamic dispatch* layer — used by the capability registry and
/// the composition executor to invoke capabilities polymorphically via
/// `Box<dyn CapabilityExecutor>`.
///
/// **Implementors should prefer [`TypedCapabilityExecutor`]** which provides a
/// typed `Config` associated type. A blanket impl bridges
/// `TypedCapabilityExecutor` → `CapabilityExecutor`, handling config
/// deserialization at the trait boundary so executor logic never touches
/// `serde_json::Value` for config.
///
/// Direct implementation of `CapabilityExecutor` is available for cases where
/// the config shape is truly dynamic or for test mocks.
pub trait CapabilityExecutor: Send + Sync + fmt::Debug {
    /// Execute this capability with the given envelope and config.
    ///
    /// - `envelope`: The typed composition context envelope.
    /// - `config`: Capability-specific configuration (`serde_json::Value`).
    /// - `context`: Execution metadata (step identity, checkpoint state).
    ///
    /// Returns the output conforming to the capability's `output_schema`.
    fn execute(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &Value,
        context: &ExecutionContext,
    ) -> Result<Value, CapabilityError>;

    /// Validate config without executing, for pre-flight checks.
    ///
    /// The blanket impl from [`TypedCapabilityExecutor`] deserializes the
    /// config into the associated `Config` type and discards the result.
    /// Direct implementors should override this with meaningful validation.
    fn validate_config(&self, config: &Value) -> Result<(), CapabilityError> {
        // Default: no validation. TypedCapabilityExecutor blanket impl overrides this.
        let _ = config;
        Ok(())
    }

    /// The capability name this executor handles.
    fn capability_name(&self) -> &str;
}

/// Strongly-typed capability executor with an associated `Config` type.
///
/// This is the **preferred** way to implement a capability executor. The
/// associated `Config` type is deserialized from `serde_json::Value` once at
/// the trait boundary by the blanket `CapabilityExecutor` impl — executor
/// logic receives the typed config directly, eliminating runtime field picking
/// and enabling config validation before execution.
///
/// # Example
///
/// ```
/// use std::fmt;
/// use serde::Deserialize;
/// use serde_json::{json, Value};
/// use tasker_grammar::types::{
///     TypedCapabilityExecutor, CapabilityExecutor, CompositionEnvelope,
///     ExecutionContext, CapabilityError,
/// };
///
/// #[derive(Debug, Deserialize)]
/// struct MyConfig {
///     greeting: String,
/// }
///
/// #[derive(Debug)]
/// struct MyExecutor;
///
/// impl TypedCapabilityExecutor for MyExecutor {
///     type Config = MyConfig;
///
///     fn execute_typed(
///         &self,
///         _envelope: &CompositionEnvelope<'_>,
///         config: &MyConfig,
///         _context: &ExecutionContext,
///     ) -> Result<Value, CapabilityError> {
///         Ok(json!({ "message": config.greeting }))
///     }
///
///     fn capability_name(&self) -> &str {
///         "my_capability"
///     }
/// }
///
/// // Pre-validate config before execution:
/// let executor: Box<dyn CapabilityExecutor> = Box::new(MyExecutor);
/// let good_config = json!({"greeting": "hello"});
/// assert!(executor.validate_config(&good_config).is_ok());
///
/// let bad_config = json!({"wrong_field": 42});
/// assert!(executor.validate_config(&bad_config).is_err());
/// ```
pub trait TypedCapabilityExecutor: Send + Sync + fmt::Debug {
    /// The strongly-typed configuration for this capability.
    ///
    /// Must implement [`DeserializeOwned`] so the blanket impl can deserialize
    /// from `serde_json::Value`, and [`fmt::Debug`] for diagnostics.
    type Config: DeserializeOwned + fmt::Debug;

    /// Execute this capability with a typed config.
    ///
    /// This is the method implementors define — config has already been
    /// deserialized and validated at this point.
    fn execute_typed(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &Self::Config,
        context: &ExecutionContext,
    ) -> Result<Value, CapabilityError>;

    /// The capability name this executor handles.
    fn capability_name(&self) -> &str;
}

/// Blanket impl: any `TypedCapabilityExecutor` is automatically a
/// `CapabilityExecutor`. Config deserialization happens exactly once, at
/// the trait boundary.
impl<T: TypedCapabilityExecutor> CapabilityExecutor for T {
    fn execute(
        &self,
        envelope: &CompositionEnvelope<'_>,
        config: &Value,
        context: &ExecutionContext,
    ) -> Result<Value, CapabilityError> {
        let typed_config: T::Config = serde_json::from_value(config.clone()).map_err(|e| {
            CapabilityError::ConfigValidation(format!(
                "invalid {} config: {}",
                TypedCapabilityExecutor::capability_name(self),
                sanitize_serde_error(&e)
            ))
        })?;
        self.execute_typed(envelope, &typed_config, context)
    }

    fn validate_config(&self, config: &Value) -> Result<(), CapabilityError> {
        let _typed: T::Config = serde_json::from_value(config.clone()).map_err(|e| {
            CapabilityError::ConfigValidation(format!(
                "invalid {} config: {}",
                TypedCapabilityExecutor::capability_name(self),
                sanitize_serde_error(&e)
            ))
        })?;
        Ok(())
    }

    fn capability_name(&self) -> &str {
        TypedCapabilityExecutor::capability_name(self)
    }
}

/// Extract structural information from a serde error without leaking input values.
///
/// The `Display` impl of `serde_json::Error` can include fragments of input data
/// (e.g., `invalid type: string "secret_value", expected u64`). This function
/// returns only the error category and position.
fn sanitize_serde_error(e: &serde_json::Error) -> String {
    use serde_json::error::Category;

    let category = match e.classify() {
        Category::Io => "I/O error",
        Category::Syntax => "syntax error",
        Category::Data => "data type mismatch",
        Category::Eof => "unexpected end of input",
    };

    let line = e.line();
    let column = e.column();
    if line == 0 && column == 0 {
        category.to_owned()
    } else {
        format!("{category} at line {line} column {column}")
    }
}

/// Context available during capability execution.
///
/// Provides step identity and checkpoint state for the executor.
/// Deliberately lightweight — no database handles, no messaging connections.
/// The composition executor (TAS-334) wraps this with runtime services.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Step name for correlation.
    pub step_name: String,

    /// Attempt number (1-indexed).
    pub attempt: u32,

    /// Existing checkpoint state if resuming after failure.
    pub checkpoint_state: Option<super::CompositionCheckpoint>,
}
