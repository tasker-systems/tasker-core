//! Style constants and clap help styling configuration.

use anstyle::{AnsiColor, Effects, Style};

/// Green — success messages, healthy status.
pub(crate) const SUCCESS: Style =
    Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)));

/// Red — errors, unhealthy status.
pub(crate) const ERROR: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));

/// Yellow — warnings, caution messages.
pub(crate) const WARNING: Style =
    Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));

/// Bold — section headers.
pub(crate) const HEADER: Style = Style::new().effects(Effects::BOLD);

/// Bold — label names in "Label: value" pairs.
pub(crate) const LABEL: Style = Style::new().effects(Effects::BOLD);

/// Dimmed — secondary/muted information.
pub(crate) const DIM: Style = Style::new().effects(Effects::DIMMED);

/// Dimmed — hints and guidance text.
pub(crate) const HINT: Style = Style::new().effects(Effects::DIMMED);

/// Custom clap styles for help output, matching our CLI palette.
pub(crate) fn clap_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(
            Style::new()
                .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)))
                .effects(Effects::BOLD),
        )
        .usage(
            Style::new()
                .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)))
                .effects(Effects::BOLD),
        )
        .literal(Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan))))
        .placeholder(Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan))))
        .error(
            Style::new()
                .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)))
                .effects(Effects::BOLD),
        )
        .valid(Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green))))
        .invalid(Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow))))
}
