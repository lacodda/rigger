//! What a repository says about how it is released: its `release.toml`.
//!
//! The file belongs to the release engine, which reads it in CI with no record
//! around, so it is public and has to stand on its own. That makes it the truth
//! for what it states, and a line in the record saying the same thing would be a
//! second copy that the first edit to either makes disagree. The record reads
//! two things from it: the gate, and what a release owes after the publish.
//!
//! Only those are read. The rest of the file is the engine's, and an unknown
//! section here is not a mistake.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The file's name, at the root of a repository.
pub const FILE: &str = "release.toml";

/// The parts of a `release.toml` the record reads.
#[derive(Debug, Default, Deserialize)]
pub struct ReleaseFile {
    #[serde(default)]
    gate: Gate,
    #[serde(default)]
    post: Post,
}

#[derive(Debug, Default, Deserialize)]
struct Gate {
    #[serde(default)]
    commands: Vec<String>,
}

/// What happens after the publish. Each section's settings are the engine's:
/// that it is there is what the record needs.
#[derive(Debug, Default, Deserialize)]
struct Post {
    install: Option<toml::Table>,
    deploy: Option<toml::Table>,
}

impl ReleaseFile {
    /// The file in a directory: `None` when there is none, an error when it
    /// cannot be read. An unreadable file is not an absent one: acting as if it
    /// were absent would run a gate the repository no longer states.
    pub fn read(dir: &Path) -> Result<Option<ReleaseFile>> {
        let path = dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        };
        let file = toml::from_str(&text).with_context(|| format!("cannot read {}", path.display()))?;
        Ok(Some(file))
    }

    /// The gate as one shell line, as the engine runs it; `None` when the file
    /// states none.
    pub fn gate(&self) -> Option<String> {
        let commands: Vec<&str> = self.gate.commands.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
        (!commands.is_empty()).then(|| commands.join(" && "))
    }

    /// A release ends with an install for the owner.
    pub fn installs(&self) -> bool {
        self.post.install.is_some()
    }

    /// A release ends with a deploy.
    pub fn deploys(&self) -> bool {
        self.post.deploy.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> ReleaseFile {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn commands_read_as_one_shell_line() {
        let file = file("[gate]\ncommands = [\"cargo fmt --check\", \"  \", \" cargo test \"]\n");
        assert_eq!(file.gate().as_deref(), Some("cargo fmt --check && cargo test"));
    }

    #[test]
    fn a_file_with_no_commands_states_no_gate() {
        assert_eq!(file("[project]\nname = \"sample\"\n").gate(), None);
        assert_eq!(file("[gate]\ncommands = []\n").gate(), None);
        assert_eq!(file("[gate]\ncommands = [\" \"]\n").gate(), None);
    }

    /// `[post.install]` with nothing under it is still an install: every setting
    /// in it has a default.
    #[test]
    fn a_section_says_what_a_release_ends_with() {
        let both = file("[post.install]\n[post.deploy]\nstand = \"a\"\n");
        assert!(both.installs() && both.deploys());
        let install = file("[post.install]\nbin = \"sample\"\n");
        assert!(install.installs() && !install.deploys());
        let neither = file("[gate]\ncommands = [\"cargo test\"]\n");
        assert!(!neither.installs() && !neither.deploys());
    }

    #[test]
    fn no_file_is_not_an_error_and_a_broken_one_is() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ReleaseFile::read(dir.path()).unwrap().is_none());

        std::fs::write(dir.path().join(FILE), "[gate\n").unwrap();
        assert!(format!("{:#}", ReleaseFile::read(dir.path()).unwrap_err()).contains(FILE));

        // A list written as a string is a mistake, not a file with no gate.
        std::fs::write(dir.path().join(FILE), "[gate]\ncommands = \"cargo test\"\n").unwrap();
        assert!(ReleaseFile::read(dir.path()).is_err());
    }
}
