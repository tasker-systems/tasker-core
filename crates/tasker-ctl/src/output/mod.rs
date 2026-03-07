//! Styled terminal output for `tasker-ctl`.
//!
//! Uses `anstyle` for ANSI style definitions and `anstream` for auto-detecting
//! terminal capabilities. Output gracefully degrades to plain text when piped
//! or when the terminal doesn't support colors.
//!
//! # Usage
//!
//! ```ignore
//! use crate::output;
//!
//! output::header("Task Details");
//! output::success("Task created successfully!");
//! output::error("Failed to connect to orchestration service");
//! output::warning("No audit records found for this step");
//! output::label("  Status", &task.status);
//! output::dim("  (use --verbose for more detail)");
//! ```

mod styles;

use std::io::Write;

pub(crate) use styles::clap_styles;

use styles::{DIM, ERROR, HEADER, HINT, LABEL, SUCCESS, WARNING};

use tasker_sdk::operational::confirmation;

/// Print a success message (green checkmark prefix).
pub(crate) fn success(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{SUCCESS}✓{SUCCESS:#} {SUCCESS}{msg}{SUCCESS:#}").ok();
}

/// Print an error message to stderr (red X prefix).
pub(crate) fn error(msg: impl std::fmt::Display) {
    let mut out = anstream::stderr().lock();
    writeln!(out, "{ERROR}✗ {msg}{ERROR:#}").ok();
}

/// Print a warning message (yellow exclamation prefix).
pub(crate) fn warning(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{WARNING}! {msg}{WARNING:#}").ok();
}

/// Print a section header (bold).
pub(crate) fn header(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{HEADER}{msg}{HEADER:#}").ok();
}

/// Print a labeled value ("  Label: value" with the label bolded).
pub(crate) fn label(name: impl std::fmt::Display, value: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "  {LABEL}{name}:{LABEL:#} {value}").ok();
}

/// Print dimmed/muted text (for secondary information).
pub(crate) fn dim(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{DIM}{msg}{DIM:#}").ok();
}

/// Print a hint/suggestion (dimmed, for guidance text).
pub(crate) fn hint(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{HINT}{msg}{HINT:#}").ok();
}

/// Print a status line with a colored icon based on health/status.
/// "healthy" or "ok" get green checkmark, anything else gets red X.
pub(crate) fn status_icon(healthy: bool, msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    if healthy {
        writeln!(out, "  {SUCCESS}✓{SUCCESS:#} {msg}").ok();
    } else {
        writeln!(out, "  {ERROR}✗{ERROR:#} {msg}").ok();
    }
}

/// Print a list item with a bullet prefix.
pub(crate) fn item(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "  • {msg}").ok();
}

/// Print a blank line.
pub(crate) fn blank() {
    let mut out = anstream::stdout().lock();
    writeln!(out).ok();
}

/// Print plain text to stdout (for output that doesn't need styling).
pub(crate) fn plain(msg: impl std::fmt::Display) {
    let mut out = anstream::stdout().lock();
    writeln!(out, "{msg}").ok();
}

/// Format and print an API error with permission-aware enrichment.
///
/// Uses the same 403 detection logic as `tasker-sdk::operational::confirmation`
/// so that permission denied errors show consistent, structured output matching
/// the behavior in `tasker-mcp`.
pub(crate) fn api_error(operation: &str, err: &dyn std::fmt::Display, tool_name: &str) {
    let err_str = err.to_string();
    if confirmation::is_permission_error(&err_str) {
        let perm = confirmation::required_permission(tool_name);
        let perm_str = perm.map(|p| p.as_str()).unwrap_or("unknown");
        let mut out = anstream::stderr().lock();
        writeln!(out, "{ERROR}✗ Permission denied{ERROR:#}").ok();
        writeln!(out, "  {LABEL}Operation:{LABEL:#} {operation}").ok();
        writeln!(out, "  {LABEL}Required permission:{LABEL:#} {perm_str}").ok();
        writeln!(
            out,
            "  {HINT}Check the JWT claims or API key scope configured for your profile.{HINT:#}"
        )
        .ok();
    } else {
        error(format!("Failed to {operation}: {err_str}"));
    }
}
