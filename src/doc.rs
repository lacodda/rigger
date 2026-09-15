//! A project's handwritten texts, kept in the record rather than in files.
//!
//! The hub kept five of them by hand: the vision, the prose preamble of the
//! decisions journal, the rituals only that project states about itself,
//! and the research notes. Everything else in a hub is already generated
//! from the record, so those five were the reason a hub had to exist at
//! all - and a text that lives only in a file is one the record cannot
//! show, search, or carry to another machine.
//!
//! Editing goes through `$EDITOR` rather than through flags: a vision is
//! prose, and prose is not written one `--set` at a time.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::db::{DOC_KINDS, SINGULAR_DOC_KINDS};

/// The variable that names the editor, checked before `EDITOR` so that a
/// person can point rigger somewhere other than their shell's default.
pub const EDITOR_ENV: &str = "RIGGER_EDITOR";

/// The editor of the line, when it is installed.
///
/// scheda is the line's own notepad and takes `--wait FILE`, which holds
/// the process open until the tab is closed and gives back its status -
/// the contract an `$EDITOR` has to meet (scheda's own v0.6). Preferring it
/// when it is there means the owner edits a vision in the editor they wrote
/// for exactly this, without configuring anything.
pub const LINE_EDITOR: &str = "scheda";
pub const LINE_EDITOR_ARGS: [&str; 1] = ["--wait"];

/// What the editor is, and where the choice came from.
pub struct Editor {
    pub program: String,
    pub args: Vec<String>,
    /// For saying which editor was opened, when one has to be reported.
    pub source: &'static str,
}

/// Picks the editor: what the owner named, else scheda when it is
/// installed, else what the system says, else a plain default.
///
/// The system's own variables come after scheda but before the fallback:
/// someone who has set `EDITOR` has said what they want, but someone who
/// has merely inherited one from their shell has not.
pub fn editor() -> Editor {
    for env in [EDITOR_ENV, "VISUAL", "EDITOR"] {
        if let Ok(raw) = std::env::var(env) {
            let raw = raw.trim();
            if !raw.is_empty() {
                let mut parts = raw.split_whitespace().map(str::to_string);
                let program = parts.next().unwrap_or_default();
                return Editor {
                    program,
                    args: parts.collect(),
                    source: if env == EDITOR_ENV { "RIGGER_EDITOR" } else { "the environment" },
                };
            }
        }
    }
    if is_installed(LINE_EDITOR) {
        return Editor {
            program: LINE_EDITOR.to_string(),
            args: LINE_EDITOR_ARGS.iter().map(|a| a.to_string()).collect(),
            source: "scheda, which is installed",
        };
    }
    Editor {
        program: fallback_editor().to_string(),
        args: Vec::new(),
        source: "the system default",
    }
}

#[cfg(windows)]
fn fallback_editor() -> &'static str {
    "notepad"
}

#[cfg(not(windows))]
fn fallback_editor() -> &'static str {
    "vi"
}

/// Whether a program can be found on the PATH.
fn is_installed(program: &str) -> bool {
    let resolved = crate::open::resolve(program);
    if Path::new(&resolved).is_file() {
        return true;
    }
    // On platforms where resolving is a no-op, the PATH still has to be
    // walked: `scheda` is only preferred when it is actually there.
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
}

/// Opens `path` in the editor and waits for it to close.
///
/// The child inherits the terminal: a console editor has to be able to draw
/// on it, and a windowed one is unaffected.
pub fn edit_file(path: &Path) -> Result<()> {
    let chosen = editor();
    let program = crate::open::resolve(&chosen.program);
    let mut command = crate::open::launcher(&program);
    for arg in &chosen.args {
        command.arg(arg);
    }
    command.arg(path);
    let status = command
        .status()
        .with_context(|| format!("cannot start {} ({})", chosen.program, chosen.source))?;
    if !status.success() {
        bail!("{} closed with {status}; the document is unchanged", chosen.program);
    }
    Ok(())
}

/// The file a document is edited through, in the system's temporary
/// directory: a name that says what is being edited, so that an editor
/// showing several tabs says which is which.
pub fn scratch_path(project: &str, slug: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rigger-{project}-{slug}.md"))
}

/// Checks a kind against the vocabulary, with the list in the error.
pub fn check_kind(kind: &str) -> Result<()> {
    if !DOC_KINDS.contains(&kind) {
        bail!("'{kind}' is not a kind of document; rigger knows {}", DOC_KINDS.join(", "));
    }
    Ok(())
}

/// Whether a project has at most one document of this kind.
pub fn is_singular(kind: &str) -> bool {
    SINGULAR_DOC_KINDS.contains(&kind)
}

/// The skeleton a new document starts from, by kind.
///
/// A blank file is the surest way to get a document nobody writes: the
/// headings are the line's own, so a vision written in rigger asks the same
/// four questions as a vision written anywhere else in the line.
pub fn template(kind: &str, title: &str) -> String {
    let sections: &[&str] = match kind {
        "vision" => &["## Why", "## The idea", "## Boundaries", "## What success looks like"],
        "research" => &["## The question", "## What was found", "## What was decided"],
        "rituals" => &["## What this is", "## Rules of the project", "## Where things are"],
        "decisions" => &["## How this journal is kept"],
        _ => &[],
    };
    let mut out = format!("# {title}\n");
    for section in sections {
        out.push_str(&format!("\n{section}\n\n\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owners_choice_wins_over_everything() {
        // Set and read back through the process environment would race
        // other tests, so the precedence is checked on the pure parts.
        assert!(is_singular("vision"));
        assert!(is_singular("rituals"));
        assert!(!is_singular("research"), "a project has many research notes");
    }

    #[test]
    fn a_new_document_starts_from_the_lines_own_questions() {
        let vision = template("vision", "Vision of rigger");
        assert!(vision.starts_with("# Vision of rigger\n"));
        assert!(vision.contains("## Why"));
        assert!(vision.contains("## What success looks like"));
        // A kind with no skeleton is a title and room to write, not an error.
        assert_eq!(template("other", "Notes"), "# Notes\n");
    }

    #[test]
    fn a_kind_outside_the_vocabulary_is_refused_with_the_list() {
        let e = check_kind("plan").unwrap_err().to_string();
        assert!(e.contains("vision"), "the error must name what rigger knows: {e}");
    }
}
