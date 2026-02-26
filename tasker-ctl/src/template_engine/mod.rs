//! Runtime template engine — re-exported from `tasker-tooling`.
//!
//! All template engine logic lives in `tasker_tooling::template_engine`. This module
//! re-exports the public API for backwards compatibility within `tasker-ctl`.

pub(crate) use tasker_tooling::template_engine::TemplateEngine;
