use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Behavior when validation fails.
///
/// Controls how the `validate` capability responds to schema violations:
///
/// - **`Error`** — Return `CapabilityError::InputValidation` with field-level details.
///   This is the default and the most common choice at trust boundaries.
/// - **`Warn`** — Pass data through with `_validation_warnings` metadata attached.
///   Useful for soft-validation scenarios where downstream logic handles the issues.
/// - **`Skip`** — Pass data through unchanged, silently.
///   For optional validation that should never block the composition.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// Return an error with field-level validation details.
    #[default]
    Error,
    /// Pass data through with `_validation_warnings` metadata.
    Warn,
    /// Pass data through unchanged, silently.
    Skip,
}

impl fmt::Display for OnFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warn => write!(f, "warn"),
            Self::Skip => write!(f, "skip"),
        }
    }
}

/// Error returned when an unknown `on_failure` value is encountered.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown on_failure mode: '{0}' (expected one of: error, warn, skip)")]
pub struct UnknownOnFailureError(pub String);

impl FromStr for OnFailure {
    type Err = UnknownOnFailureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "skip" => Ok(Self::Skip),
            _ => Err(UnknownOnFailureError(s.to_owned())),
        }
    }
}
