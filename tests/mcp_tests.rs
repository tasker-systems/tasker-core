//! MCP connected integration tests.
//!
//! These tests exercise Tier 2 read-only MCP tools against running services.
//! They validate tool outputs with real data, not just offline error paths.
//!
//! Requires: `--features test-services` and running orchestration + worker services.

#![cfg(feature = "test-services")]

mod common;
mod mcp;
