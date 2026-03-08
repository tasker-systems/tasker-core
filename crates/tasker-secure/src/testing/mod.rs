//! Test utilities for secrets resolution.
//!
//! Provides mock implementations of [`SecretsProvider`](crate::secrets)
//! for use in tests. Available when `cfg(test)` or the `test-utils`
//! feature is enabled.

mod mock_secrets;
