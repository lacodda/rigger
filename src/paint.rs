//! Colour, where a person is reading.
//!
//! rigger prints plain text almost everywhere, and on purpose: its output is
//! read by assistants and scripts as often as by eyes, and an escape code in
//! a pipe is noise in someone else's parser. The one exception is a fact
//! that is late - a question past its day - where the whole point is that it
//! should not read like the lines around it.
//!
//! So colour is decided per stream, not per command: red when standard
//! output is a terminal, nothing otherwise, and nothing when `NO_COLOR` is
//! set (https://no-color.org). The words say the same thing either way -
//! colour repeats what the text says, it never carries it alone.

use std::io::IsTerminal;

/// Whether standard output is a terminal a person is looking at.
fn colour() -> bool {
    std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()) && std::io::stdout().is_terminal()
}

/// The text in red, when a person is reading.
pub fn red(text: &str) -> String {
    if colour() { format!("\x1b[31m{text}\x1b[0m") } else { text.to_string() }
}
