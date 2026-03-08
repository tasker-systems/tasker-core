//! `SecretValue` — opaque wrapper for sensitive strings.
//!
//! Wraps `secrecy::SecretString` (which is `SecretBox<str>`) to ensure
//! secret values are zeroized on drop and never accidentally exposed
//! through `Display` or `Debug` formatting.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};

/// An opaque wrapper around a secret string value.
///
/// - `Display` and `Debug` both emit `[REDACTED]`
/// - The only way to access the underlying value is `expose_secret()`
/// - Memory is zeroized on drop (via `secrecy` + `zeroize`)
///
/// The method name `expose_secret()` creates intentional friction at code
/// review — any diff adding it in a logging context is immediately visible
/// as a potential credential leak.
pub struct SecretValue(SecretString);

impl SecretValue {
    /// Create a new `SecretValue` from a string slice.
    pub fn new(secret: &str) -> Self {
        Self(SecretString::from(Box::<str>::from(secret)))
    }

    /// Access the underlying secret value.
    ///
    /// This method name is intentionally conspicuous to make code review
    /// of credential access points obvious.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl From<String> for SecretValue {
    fn from(s: String) -> Self {
        Self(SecretString::from(Box::<str>::from(s)))
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretValue").field(&"[REDACTED]").finish()
    }
}
