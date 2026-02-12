//! Plugin discovery and registry for `tasker-ctl`.
//!
//! Plugins are directories containing a `tasker-plugin.toml` manifest that declares
//! metadata and references template directories. The registry scans configured paths
//! to discover available plugins.

mod discovery;
mod manifest;
mod registry;

pub(crate) use manifest::PluginManifest;
pub(crate) use registry::PluginRegistry;
