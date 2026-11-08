//! The one door rigger reads git through: furca-core, the line's git library.
//!
//! rigger reads git and never writes it, and never spawns `git` to do it.
//! Every read goes through furca-core, so that what the line has learnt about
//! gitoxide - its feature flags, peeling an annotated tag, packed refs, the
//! edge of a shallow clone - lives in one crate rather than in each product
//! that happens to look at a repository.

use std::path::Path;

use anyhow::Result;
use furca_core::{Commit, Repository};
use serde::Serialize;

/// Opens the repository whose root is `path`, and only that one.
///
/// furca-core finds a repository the way git does, walking up through the
/// parent directories. A project recorded at a directory that is not a
/// checkout of its own must not borrow the tags of whatever repository
/// happens to contain it: a release read from the wrong history is worse
/// than no release read at all.
///
/// The error is a sentence for the owner, not a failure: a project can be
/// recorded before its repository exists.
pub fn open(path: &str) -> Result<Repository, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let root = repo.workdir().unwrap_or_else(|| repo.git_dir());
    if !same_dir(root, Path::new(path)) {
        return Err(format!("it lies inside the repository at {}", root.display()));
    }
    Ok(repo)
}

/// Whether two paths name one directory. Compared as canonical paths, and so
/// component by component: `C:\a\b` and `C:/a/b/` are one directory, and the
/// same letters in another case are too on a file system that ignores case.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// What a repository holds that has gone nowhere yet.
///
/// A stash is left out, where furca-core's `settled` counts it: nobody means
/// to finish a stash by the next tag, and one forgotten in a corner would
/// hold the whole line up for good.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Held {
    pub changed: usize,
    pub untracked: usize,
    pub unpushed: usize,
    pub operation: Option<String>,
}

/// Reads what a repository holds. A repository with no remote has nowhere to
/// push to, so nothing in it counts as unpushed.
pub fn held(repo: &Repository) -> Result<Held, String> {
    let standing = repo.standing().map_err(|e| e.to_string())?;
    let remotes = repo.remotes().map_err(|e| e.to_string())?;
    Ok(Held {
        changed: standing.changed,
        untracked: standing.untracked,
        unpushed: if remotes.is_empty() { 0 } else { standing.unpushed },
        operation: standing
            .operation
            .and_then(|op| serde_json::to_value(op).ok())
            .and_then(|op| op.as_str().map(str::to_string)),
    })
}

/// Whether a version's tag is on the remote; `None` when that cannot be said:
/// no such tag here, no remote, no answer. Not being able to ask is not a
/// reason to stop the line.
///
/// Asked over the network, so only for a version nothing else speaks for. A
/// scratch record reaches no remote that is not a path on this machine: its
/// projects' remotes are made up or borrowed.
pub fn tag_on_remote(repo: &Repository, version: &str) -> Option<bool> {
    let wanted = crate::db::version_order(version);
    let tag = repo.refs().ok()?.tags.into_iter().find(|t| crate::db::version_order(&t.name) == wanted)?;
    let upstream = repo.upstream().ok().flatten().map(|u| u.remote);
    let remote = match upstream {
        Some(name) => name,
        None => repo.remotes().ok()?.into_iter().next()?.name,
    };
    let url = repo.remote_url(&remote).ok().flatten()?;
    if crate::paths::scratch() && reaches_the_network(&url) {
        return None;
    }
    let found = repo.git().remote_ref(&remote, &format!("refs/tags/{}", tag.name)).ok()?;
    Some(found.is_some())
}

/// Whether a remote's address leaves this machine: a URL with a scheme other
/// than `file`, or the `host:path` form ssh takes. A drive letter is one
/// character before its colon, so `C:/repos/x` is a path.
fn reaches_the_network(url: &str) -> bool {
    if let Some((scheme, _)) = url.split_once("://") {
        return scheme != "file";
    }
    url.split_once(':').is_some_and(|(host, _)| host.len() > 1 && !host.contains(['/', '\\']))
}

/// The commit a ref's target names, read by its id: the commit a release
/// tag points at, the tip of a card's branch.
///
/// A lookup, not a walk: how deep the commit lies in the history does not
/// change what reading it costs. `None` when the id names no commit - a tag
/// on a tree, or a commit a shallow clone does not hold - since that is a
/// release or a branch of nothing. Any other failure to read is an error:
/// a damaged commit must not pass for a missing one.
pub fn commit(repo: &Repository, id: &str) -> Result<Option<Commit>> {
    match repo.commit(id) {
        Ok(commit) => Ok(Some(commit)),
        Err(furca_core::Error::Revision { .. }) => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("cannot read the commit {id}"))),
    }
}

/// A commit's message, its first line only.
pub fn subject(repo: &Repository, id: &str) -> Option<String> {
    let message = repo.message(id).ok()?;
    Some(message.lines().next().unwrap_or_default().trim().to_string())
}

/// A UNIX timestamp whole, in UTC - the moment, not the day.
pub fn moment(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds).map(|t| t.to_string()).unwrap_or_default()
}

/// A UNIX timestamp as the day it fell on, in UTC.
pub fn day(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds)
        .map(|t| t.to_string().split('T').next().unwrap_or_default().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timestamp_becomes_the_day_it_fell_on_in_utc() {
        // UTC, not the machine's zone: the same tag must date the same way
        // on every machine that reads it, and a release read here and abroad
        // cannot land on two different days of the calendar.
        assert_eq!(day(1788508019), "2026-09-04"); // 07:46 UTC
        assert_eq!(day(0), "1970-01-01");
        // Either side of midnight UTC, which is where a local zone would
        // silently shift the answer by a day.
        assert_eq!(day(1788479999), "2026-09-03"); // 23:59:59 UTC
        assert_eq!(day(1788480000), "2026-09-04"); // 00:00:00 UTC
    }

    #[test]
    fn only_an_address_that_leaves_the_machine_reaches_the_network() {
        for far in ["https://github.com/a/b.git", "ssh://git@host/a/b", "git@github.com:a/b.git"] {
            assert!(reaches_the_network(far), "{far}");
        }
        for near in ["C:/repos/b.git", "C:\\repos\\b.git", "/srv/b.git", "../b.git", "file:///srv/b.git"] {
            assert!(!reaches_the_network(near), "{near}");
        }
    }

    #[test]
    fn one_directory_spelt_two_ways_is_one_directory() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("a");
        std::fs::create_dir(&inner).unwrap();
        assert!(same_dir(&inner, &dir.path().join("a").join(".").join("..").join("a")));
        assert!(!same_dir(&inner, dir.path()));
        assert!(!same_dir(&inner, &dir.path().join("missing")));
    }
}
