//! Profiles: one binary, several ways of working.
//!
//! At home a project is a repository with a plan, and the unit of work is a
//! version that ends in a tag. At work the unit is a ticket - one id, a
//! branch in each of several repositories, an inbox the tickets arrive in -
//! and none of it belongs in the same record as the line. A profile is what
//! tells the two apart: its own database, its own roots, its own way of
//! naming work. The profile is configuration, not a branch of the code, so
//! every command reads the current one and needs to be told nothing.
//!
//! The config is a TOML file a person edits, in the data directory. The
//! database of a profile lives under `profiles/<name>/`; the one database
//! rigger kept before profiles existed is moved into the default profile the
//! first time a profile-aware rigger opens it, and the old file is kept.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

/// Names the profile to use, over the one the config points at.
pub const PROFILE_ENV: &str = "RIGGER_PROFILE";
pub const CONFIG_FILE: &str = "config.toml";
pub const DEFAULT: &str = "line";
pub const DB_FILE: &str = "rigger.db";
/// What the database rigger kept before profiles is renamed to, once moved.
pub const LEGACY_KEPT_AS: &str = "rigger.db.before-profiles";

/// What the unit of work is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A version that ends in a tag; projects are the products of a line.
    #[default]
    Line,
    /// A ticket with an id, worked on branches across several repositories.
    Tickets,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Line => "line",
            Kind::Tickets => "tickets",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub kind: Kind,
    /// Directories whose children are repositories; what `adopt` walks
    /// when it is not told where.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<PathBuf>,
    /// The directory whose children are hubs, one per project name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hubs: Option<PathBuf>,
    /// How a ticket id is spelt, as a regular expression: `[A-Z]{2,8}-\d+`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_pattern: Option<String>,
    /// Where the trays of the cards are: the material that arrives for a
    /// task. `trays/` in the profile's directory when it is not said.
    ///
    /// Read under its first name, `inbox`, too, and written back under this
    /// one: in rigger the inbox is the queue of questions for the owner.
    #[serde(default, alias = "inbox", skip_serializing_if = "Option::is_none")]
    pub trays: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The profile every command uses unless `RIGGER_PROFILE` says otherwise.
    pub current: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

impl Default for Config {
    fn default() -> Config {
        let mut profiles = BTreeMap::new();
        profiles.insert(DEFAULT.to_string(), Profile::default());
        Config {
            current: DEFAULT.to_string(),
            profiles,
        }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        Ok(paths::data_dir()?.join(CONFIG_FILE))
    }

    /// The config on disk, or the default when there is none yet: a rigger
    /// that has never been told about profiles behaves as it always did.
    pub fn load() -> Result<Config> {
        let path = Config::path()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("cannot read {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Config::path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).context("cannot write the config")?;
        std::fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
        Ok(())
    }

    /// The name of the profile in use: the environment first, then the file.
    pub fn current_name(&self) -> String {
        std::env::var(PROFILE_ENV)
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| self.current.clone())
    }

    pub fn current(&self) -> Result<(String, &Profile)> {
        let name = self.current_name();
        let Some(profile) = self.profiles.get(&name) else {
            bail!("no profile named '{name}'; see `rigger profile list`");
        };
        Ok((name, profile))
    }
}

/// Where a profile keeps what is its own.
pub fn dir(name: &str) -> Result<PathBuf> {
    Ok(paths::data_dir()?.join("profiles").join(name))
}

pub fn db_path_for(name: &str) -> Result<PathBuf> {
    Ok(dir(name)?.join(DB_FILE))
}

/// The directory of the profile in use.
pub fn current_dir() -> Result<PathBuf> {
    let config = Config::load()?;
    let (name, _) = config.current()?;
    dir(&name)
}

/// The database of the profile in use.
///
/// The default profile inherits the database rigger kept before profiles
/// existed: it is moved into place the first time, and the old file is
/// kept beside the config under another name, so nothing is lost if the
/// move has to be undone.
pub fn current_db_path() -> Result<PathBuf> {
    let config = Config::load()?;
    let (name, _) = config.current()?;
    let path = db_path_for(&name)?;
    if name == DEFAULT {
        move_legacy_database(&path)?;
    }
    Ok(path)
}

fn move_legacy_database(target: &Path) -> Result<()> {
    let data = paths::data_dir()?;
    let legacy = data.join(DB_FILE);
    if !legacy.is_file() || target.exists() {
        return Ok(());
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::copy(&legacy, target).with_context(|| format!("cannot copy {} to {}", legacy.display(), target.display()))?;
    let kept = data.join(LEGACY_KEPT_AS);
    std::fs::rename(&legacy, &kept).with_context(|| format!("cannot rename {}", legacy.display()))?;
    eprintln!(
        "Moved the database into the '{DEFAULT}' profile: {}\nThe file it was is kept as {}",
        target.display(),
        kept.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_has_the_line_profile_and_round_trips_through_toml() {
        let config = Config::default();
        assert_eq!(config.current, "line");
        let text = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.profiles["line"], Profile::default());
        assert_eq!(back.profiles["line"].kind, Kind::Line);
    }

    #[test]
    fn a_ticket_profile_keeps_its_fields() {
        let text = "current = \"work\"\n\n[profiles.work]\nkind = \"tickets\"\nroots = [\"C:\\\\work\"]\nid_pattern = \"[A-Z]{2,8}-\\\\d+\"\ntrays = \"C:\\\\work\\\\trays\"\n";
        let config: Config = toml::from_str(text).unwrap();
        let work = &config.profiles["work"];
        assert_eq!(work.kind, Kind::Tickets);
        assert_eq!(work.roots, vec![PathBuf::from("C:\\work")]);
        assert_eq!(work.id_pattern.as_deref(), Some("[A-Z]{2,8}-\\d+"));
        assert_eq!(work.trays, Some(PathBuf::from("C:\\work\\trays")));
        assert!(work.hubs.is_none());
    }

    #[test]
    fn the_first_name_of_the_trays_is_read_and_written_back_as_trays() {
        let text = "current = \"work\"\n\n[profiles.work]\nkind = \"tickets\"\ninbox = \"C:\\\\work\\\\inbox\"\n";
        let config: Config = toml::from_str(text).unwrap();
        assert_eq!(config.profiles["work"].trays, Some(PathBuf::from("C:\\work\\inbox")));
        let written = toml::to_string_pretty(&config).unwrap();
        assert!(written.contains("trays = "), "{written}");
        assert!(!written.contains("inbox ="), "{written}");
    }
}
