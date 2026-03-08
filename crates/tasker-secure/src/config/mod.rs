//! Configuration string type with transparent credential resolution.
//!
//! The [`ConfigString`] type wraps configuration values that may contain
//! secret references (e.g., `${SECRET:database_password}`) and resolves
//! them through a [`SecretsProvider`](crate::secrets) at access time.
