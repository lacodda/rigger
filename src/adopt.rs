//! Recording a whole directory of repositories at once.
//!
//! Moving a line onto rigger was, before this, one `project add`, one
//! `import --hub` and one `sync` per project - fifty-one commands for
//! seventeen projects, typed by hand, and the hub path of every one of
//! them left unrecorded because the column arrived after the imports.
//!
//! `adopt` walks a directory instead. Every child that is a checkout
//! becomes a project; a hub with the same name under the hubs directory is
//! imported; git is read for what shipped. A project already recorded is
//! not recorded twice, and its hub and tags are read all the same - so the
//! command is the thing to run again when the line has grown.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::db::{Db, Kind, Project};
use crate::{hub, import, repo, sync};

/// What became of one repository.
#[derive(Debug, Serialize)]
pub struct Adopted {
    pub name: String,
    pub path: String,
    pub status: Status,
    /// Where the hub was found, when one was.
    pub hub: Option<String>,
    pub versions_added: u32,
    pub tasks_added: u32,
    pub diary_added: u32,
    /// Files whose prose the record took in on this run.
    pub prose_files: u32,
    /// Struck from the hub since it was last read.
    pub tasks_dropped: u32,
    pub versions_dropped: u32,
    /// Versions a tag proved shipped, on this run.
    pub shipped: u32,
    /// Changes read from commit messages, on this run.
    pub changes_read: u32,
    pub warnings: Vec<String>,
}

impl Adopted {
    /// What the hub gave, as a phrase: `2 versions, 6 tasks, 30 diary entries new`.
    ///
    /// Every kind is named, not only versions and tasks. The first run on
    /// a real line said "nothing new" for sixteen hubs whose prose and
    /// diary had just been read for the first time - the counts that were
    /// printed were the two that happened to be zero.
    pub fn hub_summary(&self) -> String {
        let mut parts = Vec::new();
        let mut part = |n: u32, one: &str, many: &str| {
            if n > 0 {
                parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        };
        part(self.versions_added, "version", "versions");
        part(self.tasks_added, "task", "tasks");
        part(self.diary_added, "diary entry", "diary entries");
        part(self.prose_files, "file of prose", "files of prose");
        let mut struck = Vec::new();
        let mut gone = |n: u32, one: &str, many: &str| {
            if n > 0 {
                struck.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        };
        gone(self.versions_dropped, "version", "versions");
        gone(self.tasks_dropped, "task", "tasks");
        let struck = match struck.is_empty() {
            true => String::new(),
            false => format!("; {} struck", struck.join(", ")),
        };
        match parts.is_empty() {
            true if struck.is_empty() => "hub: nothing new".to_string(),
            true => format!("hub: nothing new{struck}"),
            false => format!("hub: {} new{struck}", parts.join(", ")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Recorded on this run.
    Recorded,
    /// Was recorded already; read again.
    Known,
    /// Not recorded: a hubs directory was given and holds no hub of this
    /// name. A checkout beside the line's is not thereby one of the line.
    #[serde(rename = "no-hub")]
    NoHub,
    /// Left alone, for the reason given.
    Skipped(String),
}

/// The checkouts directly under `root`, by name.
///
/// One level only: a line keeps its repositories side by side, and a walk
/// that went deeper would find vendored trees and worktrees and call them
/// projects.
pub fn scan(root: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(root).with_context(|| format!("cannot read {}", root.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() && path.join(".git").exists() {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// The hub of a project, if the hubs directory has one under its name.
///
/// A directory is a hub when it holds any of the files a hub is read from.
/// An empty directory with the right name is not one, and reading it would
/// put five "missing file" warnings against a project that has no hub yet.
pub fn hub_for(hubs: Option<&Path>, name: &str) -> Option<PathBuf> {
    let dir = hubs?.join(name);
    if dir.is_dir() && hub::looks_like_a_hub(&dir) { Some(dir) } else { None }
}

/// Records, imports and syncs every checkout under `root`.
///
/// With `check` set nothing is written: the report says what would be
/// recorded and where a hub was found, and stops there.
pub fn adopt(db: &Db, root: &Path, hubs: Option<&Path>, check: bool) -> Result<Vec<Adopted>> {
    let root = dunce::canonicalize(root).with_context(|| format!("{} is not a directory rigger can read", root.display()))?;
    // Canonical too, so the hub path the record keeps is spelt the way the
    // platform spells it rather than the way the shell happened to.
    let hubs = match hubs {
        Some(dir) => Some(dunce::canonicalize(dir).with_context(|| format!("{} is not a directory rigger can read", dir.display()))?),
        None => None,
    };
    let hubs = hubs.as_deref();
    if let Some(hubs) = hubs
        && !hubs.is_dir()
    {
        bail!("{} is not a directory", hubs.display());
    }
    let checkouts = scan(&root)?;
    if checkouts.is_empty() {
        bail!("no repositories under {}; a repository is a directory with a .git in it", root.display());
    }

    let mut out = Vec::new();
    for checkout in checkouts {
        let name = repo::detect_name(&checkout);
        let path = checkout.to_string_lossy().into_owned();
        let mut adopted = Adopted {
            name: name.clone(),
            path: path.clone(),
            status: Status::Known,
            hub: hub_for(hubs, &name).map(|h| h.display().to_string()),
            versions_added: 0,
            tasks_added: 0,
            diary_added: 0,
            prose_files: 0,
            tasks_dropped: 0,
            versions_dropped: 0,
            shipped: 0,
            changes_read: 0,
            warnings: Vec::new(),
        };

        let project = match db.project_by_path(&path)? {
            Some(project) => {
                // Recorded under whatever name the owner gave it; that name
                // wins over the directory's, for the hub as well.
                if project.name != name {
                    adopted.name = project.name.clone();
                    adopted.hub = hub_for(hubs, &project.name).map(|h| h.display().to_string());
                }
                Some(project)
            }
            None if db.project_by_name(&name)?.is_some() => {
                let other = db.project_by_name(&name)?.map(|p| p.path).unwrap_or_default();
                adopted.status = Status::Skipped(format!("a project named '{name}' is recorded at {other}"));
                None
            }
            // With a hubs directory, the hub is what makes a checkout a
            // project of the line. A directory of repositories holds
            // strays - an experiment, a fork kept for reading, a plugin of
            // another product - and recording every one of them would put
            // eleven projects nobody works on into every list.
            None if hubs.is_some() && adopted.hub.is_none() => {
                adopted.status = Status::NoHub;
                None
            }
            None => {
                adopted.status = Status::Recorded;
                if check {
                    None
                } else {
                    let remote = repo::detect_remote(&checkout);
                    Some(db.add_project(&name, &path, remote.as_deref(), Kind::Repo)?)
                }
            }
        };

        if let Some(project) = project
            && !check
        {
            read_into(db, &project, &mut adopted)?;
        }
        out.push(adopted);
    }
    Ok(out)
}

/// The hub and the repository of one recorded project, read into the record.
fn read_into(db: &Db, project: &Project, adopted: &mut Adopted) -> Result<()> {
    if let Some(dir) = adopted.hub.as_deref().map(Path::new) {
        let hub = hub::read(dir)?;
        db.set_hub_path(project.id, dir)?;
        let report = import::import(db, project.id, &hub)?;
        adopted.versions_added = report.versions_added;
        adopted.tasks_added = report.tasks_added;
        adopted.diary_added = report.diary_added;
        adopted.prose_files = report.prose_files;
        adopted.tasks_dropped = report.tasks_dropped;
        adopted.versions_dropped = report.versions_dropped;
        adopted.warnings.extend(report.warnings);
    }
    let report = sync::sync(db, project)?;
    adopted.shipped = report.shipped.iter().filter(|s| s.newly).count() as u32;
    adopted.changes_read = report.changes_recorded;
    adopted.warnings.extend(report.warnings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_checkouts_one_level_down_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        std::fs::create_dir_all(a.join(".git")).unwrap();
        let b = dir.path().join("b");
        std::fs::write(b.with_extension("txt"), "").unwrap();
        std::fs::create_dir_all(b.join("nested").join(".git")).unwrap();
        // A worktree keeps a pointer file where a checkout keeps a directory.
        let c = dir.path().join("c");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join(".git"), "gitdir: elsewhere").unwrap();

        let found: Vec<String> = scan(dir.path()).unwrap().iter().map(|p| repo::detect_name(p)).collect();
        assert_eq!(found, vec!["a", "c"]);
    }

    #[test]
    fn a_hub_is_a_directory_with_hub_files_in_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("empty")).unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("План.md"), "# План\n").unwrap();

        assert_eq!(hub_for(Some(dir.path()), "empty"), None);
        assert_eq!(hub_for(Some(dir.path()), "missing"), None);
        assert_eq!(hub_for(Some(dir.path()), "real"), Some(real));
        assert_eq!(hub_for(None, "real"), None);
    }
}
