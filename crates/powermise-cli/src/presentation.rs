//! CLI presentation layer: color handling that honors `NO_COLOR`, aligned
//! tables, semantic status symbols, and a schema-stable JSON envelope.
use clap::ValueEnum;
use std::io::IsTerminal;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Clone, Copy)]
pub enum Style {
    Green,
    Yellow,
    Red,
    Cyan,
    Dim,
}

impl Style {
    fn code(self) -> &'static str {
        match self {
            Style::Green => "32",
            Style::Yellow => "33",
            Style::Red => "31",
            Style::Cyan => "36",
            Style::Dim => "2",
        }
    }
}

/// Captures how commands render their results.
pub struct Presentation {
    color: bool,
    json: bool,
}

impl Presentation {
    pub fn new(color: ColorMode, output: OutputFormat) -> Self {
        let terminal = std::io::stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        let color = match color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => terminal && !no_color,
        };
        Self {
            color,
            json: output == OutputFormat::Json,
        }
    }

    pub fn json(&self) -> bool {
        self.json
    }

    /// Whether to stream human-readable progress lines to stderr: only when
    /// stdout is a terminal and the output format is the table (not JSON).
    pub fn show_progress(&self) -> bool {
        !self.json && std::io::stdout().is_terminal()
    }

    /// Applies an ANSI style when color is enabled, otherwise returns the text unchanged.
    pub fn paint(&self, style: Style, text: impl AsRef<str>) -> String {
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", style.code(), text.as_ref())
        } else {
            text.as_ref().to_owned()
        }
    }

    pub fn ok(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Green, text)
    }

    pub fn warning(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Yellow, text)
    }

    pub fn yellow(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Yellow, text)
    }

    pub fn error(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Red, text)
    }

    pub fn accent(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Cyan, text)
    }

    pub fn dim(&self, text: impl AsRef<str>) -> String {
        self.paint(Style::Dim, text)
    }
}

/// Renders an aligned table with a header row and right-trimmed trailing space.
pub fn render_table(header: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return header.join("  ");
    }
    let mut widths = header.iter().map(|column| column.len()).collect::<Vec<_>>();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
    }
    let pad = |cell: &str, width: usize| {
        let cell = cell.to_string();
        let padding = width.saturating_sub(cell.chars().count());
        format!("{cell}{}", " ".repeat(padding))
    };
    let mut output = String::new();
    for (index, column) in header.iter().enumerate() {
        output.push_str(&pad(column, widths[index]));
        output.push_str(if index + 1 < header.len() { "  " } else { "" });
    }
    output.push('\n');
    for (index, width) in widths.iter().enumerate() {
        output.push_str(&"-".repeat(*width));
        output.push_str(if index + 1 < header.len() { "  " } else { "" });
    }
    output.push('\n');
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            output.push_str(&pad(cell, widths[index]));
            output.push_str(if index + 1 < header.len() { "  " } else { "" });
        }
        output.push('\n');
    }
    output
}
