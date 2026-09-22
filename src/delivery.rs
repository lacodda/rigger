//! What the outside world says about a version the tag called shipped.
//!
//! A tag says the work was finished; it does not say anyone can have it.
//! v0.4.0 of this product was tagged, counted as shipped by every screen,
//! and missing from crates.io - the publish had gone red and nothing that
//! read the record could tell. The release, its archives and the publish
//! run are a second fact about the same version, and this module is where
//! that fact is named and read.
//!
//! Two sources speak to it. The release engine, when there is one, says
//! what it did - `note --kind shipped` or MCP's `record_shipped` - and is
//! believed: it is the thing that did it. Otherwise `sync` looks at GitHub
//! through `gh` for the newest shipped version, and says what it saw. The
//! engine's word is not overwritten by the look: the actor outranks the
//! observer.

use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

/// The environment variable naming the `gh` to run.
///
/// For tests and for a machine where `gh` is not on the PATH. It is also the
/// only way a scratch record reaches GitHub at all - see `gh`.
pub const GH_ENV: &str = "RIGGER_GH";

/// How far a shipped version got past its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
    /// A tag and nothing else: no release was made for it.
    TagOnly,
    /// A release exists but carries no archives to install from.
    NoAssets,
    /// A release with its archives; no publish run was found for the tag.
    Released,
    /// A publish run for the tag has not finished yet.
    PublishRunning,
    /// The publish run for the tag went red.
    PublishFailed,
    /// Released, and the publish run for the tag went green - or the
    /// release engine said which registries the version reached.
    Published,
}

impl Delivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Delivery::TagOnly => "tag-only",
            Delivery::NoAssets => "no-assets",
            Delivery::Released => "released",
            Delivery::PublishRunning => "publish-running",
            Delivery::PublishFailed => "publish-failed",
            Delivery::Published => "published",
        }
    }

    pub fn parse(text: &str) -> Option<Delivery> {
        Some(match text {
            "tag-only" => Delivery::TagOnly,
            "no-assets" => Delivery::NoAssets,
            "released" => Delivery::Released,
            "publish-running" => Delivery::PublishRunning,
            "publish-failed" => Delivery::PublishFailed,
            "published" => Delivery::Published,
            _ => return None,
        })
    }

    /// The state in the words a screen uses.
    pub fn describe(self) -> &'static str {
        match self {
            Delivery::TagOnly => "tag exists, no release",
            Delivery::NoAssets => "released without archives",
            Delivery::Released => "released",
            Delivery::PublishRunning => "released, publish still running",
            Delivery::PublishFailed => "released, publish failed",
            Delivery::Published => "released and published",
        }
    }

    /// Whether this is something the owner should hear about: a version
    /// that the calendar counts as shipped and nobody can install.
    pub fn is_short(self) -> bool {
        matches!(self, Delivery::TagOnly | Delivery::NoAssets | Delivery::PublishFailed)
    }

    /// What the release engine's word amounts to.
    ///
    /// The engine reports what it did rather than a state: a release made,
    /// registries reached. A registry is a publish that worked, whether or
    /// not a GitHub release went with it - a crate published from a
    /// workstation is as installable as one published from CI.
    pub fn from_engine(release: bool, registries: &[String]) -> Delivery {
        match (release, registries.is_empty()) {
            (_, false) => Delivery::Published,
            (true, true) => Delivery::Released,
            (false, true) => Delivery::TagOnly,
        }
    }
}

/// `owner/name` of a GitHub remote, or `None` for any other host.
///
/// Both spellings a clone leaves behind: `https://github.com/o/n.git` and
/// `git@github.com:o/n.git`.
pub fn github_repo(remote: &str) -> Option<String> {
    let rest = remote
        .strip_prefix("https://github.com/")
        .or_else(|| remote.strip_prefix("http://github.com/"))
        .or_else(|| remote.strip_prefix("git@github.com:"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (parts.next().is_none() && !owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}"))
}

/// The `gh` to run, or `None` when this record must not reach GitHub.
///
/// A scratch record's remotes are made up or borrowed, so it reaches GitHub
/// only through a `gh` it names itself (`paths::scratch`).
fn gh() -> Option<std::ffi::OsString> {
    match std::env::var_os(GH_ENV) {
        Some(named) => Some(named),
        None if crate::paths::scratch() => None,
        None => Some("gh".into()),
    }
}

/// Why GitHub could not be asked, when it could not.
#[derive(Debug)]
pub enum Unread {
    /// No `gh` for this record: scratch, or not installed.
    NoGh,
    /// `gh` ran and could not answer - offline, signed out, rate limited.
    Failed(String),
}

/// Runs `gh` and gives back what it printed, or why it could not.
fn run(args: &[&str]) -> Result<Result<String, String>, Unread> {
    let Some(program) = gh() else {
        return Err(Unread::NoGh);
    };
    let out = match Command::new(&program).args(args).output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Unread::NoGh),
        Err(e) => return Err(Unread::Failed(e.to_string())),
    };
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Ok(if out.status.success() { Ok(stdout) } else { Err(stderr) })
}

/// What GitHub says about one tag of one repository.
///
/// `Ok(None)` when the repository makes no releases at all: tagging without
/// releasing is a way of working, and calling every tag of such a project
/// "no release" would be a warning that fires for ever.
pub fn read(repo: &str, tag: &str) -> Result<Option<Delivery>, Unread> {
    let release = run(&["release", "view", tag, "--repo", repo, "--json", "assets,isDraft"])?;
    let assets = match release {
        Ok(json) => {
            let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| Unread::Failed(format!("gh said something that is not JSON: {e}")))?;
            // A draft is a release nobody can see, which is no release.
            if value["isDraft"].as_bool().unwrap_or(false) {
                None
            } else {
                Some(value["assets"].as_array().map(Vec::len).unwrap_or(0))
            }
        }
        Err(said) if said.contains("release not found") => None,
        Err(said) => return Err(Unread::Failed(said)),
    };
    let Some(assets) = assets else {
        // No release for this tag. Whether that is news depends on whether
        // the repository makes releases at all.
        return match run(&["release", "list", "--repo", repo, "--limit", "1", "--json", "tagName"])? {
            Ok(json) if json.trim().trim_start_matches('[').trim_end_matches(']').trim().is_empty() => Ok(None),
            Ok(_) => Ok(Some(Delivery::TagOnly)),
            Err(said) => Err(Unread::Failed(said)),
        };
    };
    let runs = match run(&[
        "run",
        "list",
        "--repo",
        repo,
        "--branch",
        tag,
        "--json",
        "workflowName,status,conclusion",
        "--limit",
        "20",
    ])? {
        Ok(json) => json,
        Err(said) => return Err(Unread::Failed(said)),
    };
    let runs: Vec<serde_json::Value> = serde_json::from_str(&runs).map_err(|e| Unread::Failed(format!("gh said something that is not JSON: {e}")))?;
    Ok(Some(publish_state(&runs, assets)))
}

/// The state the publish runs of a tag add up to.
///
/// A publish is a workflow whose name says so. The line's repositories keep
/// two for a release - one that builds the archives and one that publishes
/// to the registries - and only the second decides whether anyone can
/// install the version. With several runs of it (a re-run after a red one),
/// the newest is the answer, and `gh` lists newest first.
///
/// The publish is asked before the archives. A library published to npm
/// attaches nothing to its release - its package is the thing installed -
/// and reading "no archives" first called every such release short, for
/// ever. Found on the first run over the real line: the design system's
/// every release. Archives decide only where no publish ran at all.
fn publish_state(runs: &[serde_json::Value], assets: usize) -> Delivery {
    let publish = runs
        .iter()
        .find(|run| run["workflowName"].as_str().is_some_and(|name| name.to_ascii_lowercase().contains("publish")));
    let Some(run) = publish else {
        return if assets == 0 { Delivery::NoAssets } else { Delivery::Released };
    };
    match (run["status"].as_str(), run["conclusion"].as_str()) {
        (Some("completed"), Some("success")) => Delivery::Published,
        (Some("completed"), _) => Delivery::PublishFailed,
        _ => Delivery::PublishRunning,
    }
}

/// What recording a release came to.
#[derive(Debug, Serialize)]
pub struct Recorded {
    pub version: String,
    pub delivery: Delivery,
    pub registries: Vec<String>,
    /// Whether this is what closed the version; `false` when a tag had
    /// already done so.
    pub closed: bool,
}

/// Records a release as the engine reports it: the version closes, its
/// delivery is set, and a `shipped` event says so in the project's history.
///
/// One function for both doors - `note --kind shipped` and MCP's
/// `record_shipped` - so the two cannot come to mean different things.
pub fn record(db: &crate::db::Db, project: &crate::db::Project, tag: &str, release: bool, registries: &[String], text: Option<&str>) -> Result<Recorded> {
    let version = tag_version(tag)?;
    let registries = registries.iter().map(|r| registry(r)).collect::<Result<Vec<_>>>()?;
    let moment = crate::db::now();
    let day = moment.split('T').next().unwrap_or_default().to_string();
    let (name, change) = db.mark_released(project.id, &version, &day, &moment)?;
    let delivery = Delivery::from_engine(release, &registries);
    if let Some(id) = db.version_id(project.id, Some(&name))? {
        db.set_delivery(id, delivery, &registries, "engine")?;
    }
    // The event is the history; the columns are the current state. Written
    // in the project's words when the engine gave some, and composed from
    // the facts when it did not, so `why` and the diary have a line to show.
    let composed = {
        let mut reached = Vec::new();
        if release {
            reached.push("the release with its archives".to_string());
        }
        reached.extend(registries.iter().cloned());
        match reached.is_empty() {
            true => format!("{name} went out as a tag"),
            false => format!("{name} went out: {}", reached.join(", ")),
        }
    };
    let body = match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(text) => format!("{composed}\n\n{text}"),
        None => composed,
    };
    db.record_event(project.id, "shipped", &body, &moment, "engine")?;
    Ok(Recorded {
        version: name,
        delivery,
        registries,
        closed: change != crate::db::Change::Unchanged,
    })
}

/// Checks a registry name as the release engine gives it, so that a typo is
/// a refusal rather than a registry nobody publishes to.
pub fn registry(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) || name.contains(',') {
        bail!("{name:?} is not a registry name; write it as crates.io, npm, pypi");
    }
    Ok(name.to_string())
}

/// A tag as the version it names. Refused when it names none, because a
/// release recorded against a word would close nothing.
pub fn tag_version(tag: &str) -> Result<String> {
    let tag = tag.trim();
    let digits = tag.strip_prefix('v').unwrap_or(tag);
    let first = digits.split('.').next().unwrap_or_default();
    first
        .parse::<u32>()
        .ok()
        .and_then(|_| digits.contains('.').then_some(()))
        .with_context(|| format!("{tag:?} is not a version tag; write it as v0.23.0"))?;
    Ok(tag.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_remote_names_its_repository_in_either_spelling() {
        assert_eq!(github_repo("https://github.com/acme/widget.git").as_deref(), Some("acme/widget"));
        assert_eq!(github_repo("https://github.com/acme/widget").as_deref(), Some("acme/widget"));
        assert_eq!(github_repo("git@github.com:acme/widget.git").as_deref(), Some("acme/widget"));
        assert_eq!(github_repo("https://gitlab.com/acme/widget.git"), None);
        assert_eq!(github_repo("https://github.com/acme"), None);
    }

    /// The archive build and the publish are two workflows, and only the
    /// second says whether the version can be installed.
    #[test]
    fn only_the_publish_run_decides_publication() {
        let built_only = [json!({ "workflowName": "Release", "status": "completed", "conclusion": "success" })];
        assert_eq!(publish_state(&built_only, 3), Delivery::Released);
        // Built, with nothing attached and nothing published: short.
        assert_eq!(publish_state(&built_only, 0), Delivery::NoAssets);

        let red = [
            json!({ "workflowName": "Publish", "status": "completed", "conclusion": "failure" }),
            json!({ "workflowName": "Release", "status": "completed", "conclusion": "success" }),
        ];
        assert_eq!(publish_state(&red, 3), Delivery::PublishFailed);

        let running = [json!({ "workflowName": "Publish", "status": "in_progress", "conclusion": "" })];
        assert_eq!(publish_state(&running, 3), Delivery::PublishRunning);

        // Newest first: a green re-run after a red one is the answer.
        let rerun = [
            json!({ "workflowName": "Publish", "status": "completed", "conclusion": "success" }),
            json!({ "workflowName": "Publish", "status": "completed", "conclusion": "failure" }),
        ];
        assert_eq!(publish_state(&rerun, 3), Delivery::Published);

        // A library's release carries no archives - its package is what is
        // installed - and a green publish is the whole of its delivery.
        let library = [json!({ "workflowName": "Publish", "status": "completed", "conclusion": "success" })];
        assert_eq!(publish_state(&library, 0), Delivery::Published);
    }

    #[test]
    fn the_engine_is_read_by_what_it_did() {
        assert_eq!(Delivery::from_engine(true, &["crates.io".into()]), Delivery::Published);
        assert_eq!(Delivery::from_engine(false, &["crates.io".into()]), Delivery::Published);
        assert_eq!(Delivery::from_engine(true, &[]), Delivery::Released);
        assert_eq!(Delivery::from_engine(false, &[]), Delivery::TagOnly);
    }

    #[test]
    fn every_state_reads_back_as_itself() {
        for state in [
            Delivery::TagOnly,
            Delivery::NoAssets,
            Delivery::Released,
            Delivery::PublishRunning,
            Delivery::PublishFailed,
            Delivery::Published,
        ] {
            assert_eq!(Delivery::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn a_tag_must_name_a_version() {
        assert_eq!(tag_version("v0.23.0").unwrap(), "v0.23.0");
        assert_eq!(tag_version("1.2").unwrap(), "1.2");
        assert!(tag_version("latest").is_err());
        assert!(tag_version("v3").is_err());
    }
}
