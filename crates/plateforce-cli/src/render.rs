//! How a line is drawn, and what it may carry when it reaches a terminal.
//!
//! Every glyph is ASCII unless the operator asks otherwise: `cmd.exe` under a raster font
//! mangles box drawing, and a rule nobody can read is worse than a rule drawn with hyphens.
//! Hierarchy is built from layout, so colour is left to carry the one fact the registry
//! records about an entry rather than to separate a heading from a body.

use std::io::IsTerminal;

use clap::ValueEnum;

/// The operator's answer to whether this run may emit colour at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum Colour {
    Auto,
    Always,
    Never,
}

/// How much colour reaches the other end.
///
/// Four-bit is the ceiling on purpose. Inside tmux and screen `TERM` is usually
/// `screen-256color` and a 24-bit sequence is dropped or mangled there, so the ceiling
/// removes the case rather than branching on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Palette {
    Absent,
    Ansi16,
}

/// What a span of text is, which decides what it may be drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Heading,
    /// A status the registry marks as anything other than current.
    NotCurrent,
}

impl Role {
    fn select_graphic_rendition(self) -> &'static str {
        match self {
            Role::Heading => "\x1b[1m",
            Role::NotCurrent => "\x1b[33m",
        }
    }
}

const RESET: &str = "\x1b[0m";
const WIDTH_WHEN_NOT_A_TERMINAL: usize = 80;
const FIELD_NAME_COLUMNS: usize = 12;

pub struct Renderer {
    width_columns: usize,
    palette: Palette,
}

impl Renderer {
    /// Reads the terminal the run is attached to. A destination file is never a terminal,
    /// so a redirected document carries no escape byte whatever the shell reports.
    pub fn for_stdout(colour: Colour, writing_to_a_file: bool) -> Self {
        let attached = std::io::stdout().is_terminal() && !writing_to_a_file;
        Self {
            width_columns: if attached {
                terminal_size::terminal_size()
                    .map(|(width, _)| width.0 as usize)
                    .unwrap_or(WIDTH_WHEN_NOT_A_TERMINAL)
            } else {
                WIDTH_WHEN_NOT_A_TERMINAL
            },
            palette: palette_for(colour, attached, writing_to_a_file),
        }
    }

    /// A renderer that draws to a string with no terminal behind it.
    #[cfg(test)]
    pub fn plain() -> Self {
        Self {
            width_columns: WIDTH_WHEN_NOT_A_TERMINAL,
            palette: Palette::Absent,
        }
    }

    pub fn paint(&self, role: Role, text: &str) -> String {
        match self.palette {
            Palette::Absent => text.to_string(),
            Palette::Ansi16 => format!("{}{text}{RESET}", role.select_graphic_rendition()),
        }
    }

    /// One labelled line, the shape `registry show` has printed since the first commit.
    pub fn field(&self, name: &str, text: &str) -> String {
        format!("  {name:<FIELD_NAME_COLUMNS$}{text}")
    }

    /// A labelled line whose value is prose, continued under itself rather than cut short.
    /// A rationale ending in an ellipsis reads as the registry being brief, which it is not.
    pub fn field_wrapped(&self, name: &str, value: &str) -> Vec<String> {
        let indent = 2 + FIELD_NAME_COLUMNS;
        let mut lines = self.wrap(value, indent);
        match lines.first_mut() {
            Some(first) => {
                first.replace_range(..indent, &format!("  {name:<FIELD_NAME_COLUMNS$}"));
                lines
            }
            None => vec![self.field(name, "")],
        }
    }

    /// Breaks on spaces so a refusal read at eighty columns never splits a method id.
    /// A word longer than the width is left whole rather than cut, since a cut id resolves
    /// nowhere.
    pub fn wrap(&self, text: &str, indent: usize) -> Vec<String> {
        let room = self.width_columns.saturating_sub(indent).max(20);
        let padding = " ".repeat(indent);
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > room {
                lines.push(format!("{padding}{current}"));
                current = String::new();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(format!("{padding}{current}"));
        }
        lines
    }
}

fn palette_for(colour: Colour, attached_to_a_terminal: bool, writing_to_a_file: bool) -> Palette {
    // A document asked for by path is read by a parser rather than by a terminal, and an
    // escape byte inside one is rejected by `json.load`, `jq`, `pandas.read_json` and
    // `jsonlite` alike. This outranks the operator's answer because the two do not conflict:
    // the answer is about a terminal and the destination is not one.
    if writing_to_a_file {
        return Palette::Absent;
    }
    match colour {
        Colour::Never => Palette::Absent,
        Colour::Always => Palette::Ansi16,
        Colour::Auto => {
            if anstyle_query::no_color() || !anstyle_query::term_supports_color() {
                return Palette::Absent;
            }
            if anstyle_query::clicolor_force() {
                return Palette::Ansi16;
            }
            if attached_to_a_terminal {
                Palette::Ansi16
            } else {
                Palette::Absent
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_renderer_emits_no_escape_byte() {
        let renderer = Renderer::plain();
        for role in [Role::Heading, Role::NotCurrent] {
            assert!(!renderer
                .paint(role, "onset.threshold.noise_relative")
                .contains('\x1b'));
        }
    }

    /// A method id split across two lines resolves nowhere, so the wrap breaks on spaces
    /// and leaves an over-long word whole.
    #[test]
    fn wrapping_never_splits_a_word() {
        let renderer = Renderer::plain();
        let lines = renderer.wrap(
            "Net impulse reliability runs from 0.984 to 0.479 across published onset rules on identical data.",
            6,
        );
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.chars().count() <= 80, "{line}");
            assert!(line.starts_with("      "));
        }
        let rejoined: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
        assert!(rejoined.contains(&"reliability"));
    }

    #[test]
    fn colour_never_beats_every_other_signal() {
        assert_eq!(palette_for(Colour::Never, true, false), Palette::Absent);
        assert_eq!(palette_for(Colour::Always, false, false), Palette::Ansi16);
    }

    /// A named destination is read by a parser. `always` is an answer about a terminal, and
    /// a file is not one, so the two do not disagree.
    #[test]
    fn a_document_asked_for_by_path_carries_no_escape_byte() {
        for colour in [Colour::Auto, Colour::Always, Colour::Never] {
            assert_eq!(
                palette_for(colour, true, true),
                Palette::Absent,
                "{colour:?}"
            );
        }
    }
}
