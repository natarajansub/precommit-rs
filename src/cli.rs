use clap::builder::{styling::AnsiColor, Styles};

/// Shared CLI styling for all precommit-rs binaries.
pub fn styles() -> Styles {
    Styles::styled()
        .usage(AnsiColor::Green.on_default().bold())
        .header(AnsiColor::Yellow.on_default().bold())
        .literal(AnsiColor::Cyan.on_default().bold())
        .placeholder(AnsiColor::Magenta.on_default().bold())
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default().bold())
}
