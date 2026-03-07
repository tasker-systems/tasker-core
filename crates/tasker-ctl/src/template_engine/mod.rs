//! Runtime template engine — re-exported from `tasker-sdk`.
//!
//! All template engine logic lives in `tasker_sdk::template_engine`. This module
//! re-exports the public API for backwards compatibility within `tasker-ctl`.

pub(crate) use tasker_sdk::template_engine::TemplateEngine;
