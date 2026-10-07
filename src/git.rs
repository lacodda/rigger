//! The one door rigger reads git through: furca-core, the line's git library.
//!
//! rigger reads git and never writes it, and never spawns `git` to do it.
//! Every read goes through furca-core, so that what the line has learnt about
//! gitoxide - its feature flags, peeling an annotated tag, packed refs, the
//! edge of a shallow clone - lives in one crate rather than in each product
//! that happens to look at a repository.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use furca_core::{Commit, Repository, Tips};

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

/// How many commits the first look at the history reads. A release tag or
/// the tip of a branch in work is almost always among the newest commits,
/// so the first look usually finds everything.
const FIRST_LOOK: usize = 512;

/// The commits `ids` name, found by walking every branch and tag.
///
/// furca-core 0.3 reads a commit only on the way through a walk, so a commit
/// wanted by its id is looked for: the newest few hundred first, and the
/// look widens eightfold until every id is found or the history runs out.
/// An id that names no commit - a tag on a tree - is simply not in the
/// answer. Every id given here is the tip of a ref, so the walk from all of
/// them reaches each one that is a commit.
pub fn commits(repo: &Repository, ids: &HashSet<String>) -> Result<HashMap<String, Commit>> {
    let mut found = HashMap::new();
    if ids.is_empty() {
        return Ok(found);
    }
    let mut limit = FIRST_LOOK;
    loop {
        let log = repo.log(Tips::All, limit).context("cannot walk the history")?;
        for commit in log.commits {
            if ids.contains(&commit.id) {
                found.insert(commit.id.clone(), commit);
            }
        }
        if found.len() == ids.len() || !log.truncated {
            return Ok(found);
        }
        limit = limit.saturating_mul(8);
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
    fn one_directory_spelt_two_ways_is_one_directory() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("a");
        std::fs::create_dir(&inner).unwrap();
        assert!(same_dir(&inner, &dir.path().join("a").join(".").join("..").join("a")));
        assert!(!same_dir(&inner, dir.path()));
        assert!(!same_dir(&inner, &dir.path().join("missing")));
    }
}
