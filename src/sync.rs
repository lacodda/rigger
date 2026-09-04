//! What git says, against what the record claims.
//!
//! Until now "what is done" depended on somebody writing it down. A stage
//! could be marked closed in a plan whose tag was never pushed, and a tag
//! could exist for a version the plan still listed as open - and nothing
//! noticed, because the plan was the only place anyone looked.
//!
//! So git is read directly: a tag matching `v*` means that version shipped,
//! on the date of the commit it points at, and commits since the newest tag
//! are the project's activity. Where the plan and git disagree, the record
//! follows git for the fact it can prove - a tag - and reports the rest
//! rather than correcting it silently (ADR 0005).
//!
//! Reading happens in-process through gix; `git` is never spawned. A shelled
//! command would cost a process per project, depend on git being installed
//! and on its output format, and inherit whatever the caller's environment
//! did to it.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::db::{Change, Db, Project, version_order};

/// What one project's repository says, and what changed in the record.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub project: String,
    /// Versions the record now knows shipped, because a tag proves it.
    pub shipped: Vec<Shipped>,
    /// Tags whose version the plan never mentioned.
    pub unplanned: Vec<String>,
    /// Versions the plan closed without a tag to show for it.
    pub untagged: Vec<String>,
    pub commits_since_tag: u32,
    pub last_commit_at: Option<String>,
    /// Changes read from commit messages that the record did not have.
    pub changes_recorded: u32,
    /// Said rather than failed: a project can be recorded before its
    /// repository exists, and a hub can be imported from a directory that
    /// was never a checkout.
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Shipped {
    pub version: String,
    pub date: String,
    /// Whether this run is what closed it.
    pub newly: bool,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.shipped.iter().any(|s| s.newly) || !self.unplanned.is_empty() || self.changes_recorded > 0
    }
}

/// A tag that names a version, and the day its commit was made.
struct Tag {
    version: String,
    date: String,
}

pub fn sync(db: &Db, project: &Project) -> Result<Report> {
    let mut report = Report {
        project: project.name.clone(),
        ..Report::default()
    };

    let repo = match gix::open(&project.path) {
        Ok(repo) => repo,
        Err(err) => {
            report
                .warnings
                .push(format!("{} is not a git repository rigger can read ({err})", project.path));
            return Ok(report);
        }
    };

    // Taken before anything is written: once a tag has been recorded, its
    // version is in the record, and "did the plan know about this?" can no
    // longer be asked.
    let known_before = db.version_names(project.id)?;

    let tags = read_tags(&repo)?;
    for tag in &tags {
        // A tag is proof the version shipped; the plan does not get a vote.
        let change = db.mark_shipped(project.id, &tag.version, &tag.date)?;
        match change {
            Change::Unchanged => report.shipped.push(Shipped {
                version: tag.version.clone(),
                date: tag.date.clone(),
                newly: false,
            }),
            Change::Updated => report.shipped.push(Shipped {
                version: tag.version.clone(),
                date: tag.date.clone(),
                newly: true,
            }),
            // No such version in the plan: the tag is a fact all the same,
            // so it is recorded. Whether that is worth reporting depends on
            // where it falls - see `unplanned` below.
            Change::Added => report.shipped.push(Shipped {
                version: tag.version.clone(),
                date: tag.date.clone(),
                newly: true,
            }),
        }
    }

    // A release the plan never mentioned is only news if the plan was
    // supposed to know about it. Every hub starts somewhere - kasl's plan
    // begins at v1.4 and its repository carries twenty-one releases from
    // 2024 - and calling all of those unplanned buries the one that matters
    // under the history of the project. So the floor is the oldest version
    // the plan does know, and only tags above it are reported.
    let floor = known_before.iter().map(|n| version_order(n)).min();
    for tag in &tags {
        if !known_before.iter().any(|k| version_order(k) == version_order(&tag.version)) && floor.is_none_or(|floor| version_order(&tag.version) > floor) {
            report.unplanned.push(tag.version.clone());
        }
    }

    // The other direction: closed in the plan, with no tag to show for it.
    // Left exactly as the owner wrote it - the record cannot prove a
    // negative, a tag may simply not have been fetched - but said out loud.
    let tagged: Vec<&str> = tags.iter().map(|t| t.version.as_str()).collect();
    for claimed in db.shipped_versions(project.id)? {
        if !tagged.iter().any(|t| version_order(t) == version_order(&claimed)) {
            report.untagged.push(claimed);
        }
    }

    let newest = tags.iter().max_by_key(|t| version_order(&t.version));
    let history = read_history(&repo, newest.map(|t| t.version.as_str()))?;
    report.commits_since_tag = history.commits_since_tag;
    report.last_commit_at = history.last_commit_at.clone();
    db.record_activity(project.id, history.commits_since_tag, history.last_commit_at.as_deref())?;

    // Changes are recorded oldest first, so that the record reads forwards
    // even though history is walked backwards.
    for change in history.changes.iter().rev() {
        // The timestamp is the commit's day: an event about a change that
        // landed in August must not date from the sync that read it.
        let at = format!("{}T00:00:00Z", change.date);
        if db.record_commit_event(project.id, &change.hash, &change.body, &at)? == Change::Added {
            report.changes_recorded += 1;
        }
    }

    Ok(report)
}

/// Tags that name a version, each with the day of the commit it points at.
///
/// The commit's date rather than the tag's own: a lightweight tag - which is
/// what `git tag v0.5.0` makes, and what this line uses - has no date of its
/// own at all, so the commit is the only date both kinds share.
fn read_tags(repo: &gix::Repository) -> Result<Vec<Tag>> {
    let mut tags = Vec::new();
    let refs = repo.references().context("cannot read the repository's references")?;
    for tag in refs.tags().context("cannot list tags")? {
        let mut tag = match tag {
            Ok(tag) => tag,
            // One unreadable ref must not cost the whole sync.
            Err(_) => continue,
        };
        let name = tag.name().shorten().to_string();
        if !names_a_version(&name) {
            continue;
        }
        // Peeling follows an annotated tag through to its commit; a
        // lightweight tag already is one.
        let Ok(id) = tag.peel_to_id() else { continue };
        let Ok(object) = repo.find_object(id) else { continue };
        let Ok(commit) = object.try_into_commit() else { continue };
        let Ok(time) = commit.time() else { continue };
        tags.push(Tag {
            version: name,
            date: day(time.seconds),
        });
    }
    Ok(tags)
}

/// Whether a tag names a version: `v` and then a digit.
///
/// Deliberately narrow. A repository carries tags that are not releases -
/// `latest`, `nightly`, a name someone tagged a branch point with - and
/// treating one of those as a version would put a phantom release in the
/// record and on the calendar.
fn names_a_version(name: &str) -> bool {
    name.strip_prefix('v').is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// One pass over history: what has landed since the newest release, and the
/// changes worth recording along the way.
///
/// One walk rather than two. The counting stops at the newest tag - the
/// answer is "work since the release", not the length of history - but the
/// reading goes further back, so that a project synced for the first time
/// arrives with its recent changes rather than with only what happened since
/// Friday.
struct History {
    commits_since_tag: u32,
    last_commit_at: Option<String>,
    changes: Vec<CommitChange>,
}

/// A commit that changed the product, ready to be recorded.
struct CommitChange {
    hash: String,
    body: String,
    date: String,
}

/// How far back changes are read. Deep enough that a first sync brings a
/// project's recent history, shallow enough that syncing the whole line
/// stays a thing you run at the start of a session. Older changes are not
/// lost - they are in git, which is where a question about 2024 belongs.
const READ_DEPTH: u32 = 300;

fn read_history(repo: &gix::Repository, newest_tag: Option<&str>) -> Result<History> {
    let Ok(head) = repo.head_commit() else {
        // An empty repository: no commits, and nothing to say about them.
        return Ok(History {
            commits_since_tag: 0,
            last_commit_at: None,
            changes: Vec::new(),
        });
    };
    let last_commit_at = head.time().ok().map(|t| day(t.seconds));

    let boundary = newest_tag.and_then(|name| {
        let mut reference = repo.find_reference(&format!("refs/tags/{name}")).ok()?;
        reference.peel_to_id().ok().map(|id| id.detach())
    });

    let mut commits_since_tag = 0u32;
    let mut past_the_tag = false;
    let mut changes = Vec::new();
    let mut seen = 0u32;

    let walk = head.ancestors().all().context("cannot walk the history")?;
    for step in walk {
        let Ok(info) = step else { break };
        if Some(info.id) == boundary {
            past_the_tag = true;
        }
        if !past_the_tag {
            commits_since_tag += 1;
        }

        if let Some(change) = read_change(repo, info.id) {
            changes.push(change);
        }

        seen += 1;
        if seen >= READ_DEPTH {
            break;
        }
    }
    Ok(History {
        commits_since_tag,
        last_commit_at,
        changes,
    })
}

/// A commit, if its message says it changed the product.
fn read_change(repo: &gix::Repository, id: gix::ObjectId) -> Option<CommitChange> {
    let commit = repo.find_object(id).ok()?.try_into_commit().ok()?;
    let message = commit.message_raw().ok()?.to_string();
    let parsed = crate::commit::parse(&message)?;
    if !parsed.is_a_change() {
        return None;
    }
    Some(CommitChange {
        hash: id.to_hex().to_string(),
        body: parsed.body(),
        date: commit.time().ok().map(|t| day(t.seconds)).unwrap_or_default(),
    })
}

/// A UNIX timestamp as the day it fell on, in UTC.
fn day(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds)
        .map(|t| t.to_string().split('T').next().unwrap_or_default().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_tags_that_name_a_version_are_releases() {
        assert!(names_a_version("v0.5.0"));
        assert!(names_a_version("v1.9"));
        // A repository carries tags that are not releases; treating one as a
        // version would put a phantom release on the calendar.
        assert!(!names_a_version("latest"));
        assert!(!names_a_version("nightly"));
        assert!(!names_a_version("v-final"));
        assert!(!names_a_version("release-1.0"));
    }

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
}
