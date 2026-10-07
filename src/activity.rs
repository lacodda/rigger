//! A card's activity, read from git.
//!
//! A ticket is worked in branches and commits that carry its id: a branch
//! called `fix/ACME-7310-rtf`, a commit that says `fix(ACME-7310): ...`. Until
//! now the card knew where it was worked only when somebody ran `task
//! link`, and how recently only when somebody wrote it down - so the card
//! of a ticket in full swing read the same as one forgotten a month ago.
//!
//! So `sync` reads the branches and the recent commits of every repository
//! and looks for the names of the cards the record already holds. A name
//! found is a fact about the card: the repository is one it is worked in,
//! the branch is its branch there, the commit is its latest movement.
//!
//! Only cards the record holds are looked for. An id in a commit that names
//! no card is somebody else's ticket, or one never handed to this desk, and
//! making a card for it would fill the desk with work nobody took.

use std::collections::HashSet;

use anyhow::{Context, Result};
use furca_core::{Repository, Tips};

use crate::card::Card;
use crate::git::moment;

/// A branch whose name carries a card's name.
#[derive(Debug, Clone)]
pub struct BranchSeen {
    pub task_id: i64,
    /// The branch as it is checked out: `fix/ACME-7310-rtf`, without the
    /// remote's name in front.
    pub name: String,
    /// The remote the branch is on, when there is no local branch of that
    /// name - the one a worktree would be made from.
    pub remote: Option<String>,
    pub tip: String,
    /// The moment of the tip commit, in UTC.
    pub tip_at: String,
    pub subject: String,
}

/// A commit whose message carries a card's name.
#[derive(Debug, Clone)]
pub struct CommitSeen {
    pub task_id: i64,
    pub hash: String,
    pub at: String,
    pub subject: String,
}

#[derive(Debug, Default)]
pub struct Seen {
    pub branches: Vec<BranchSeen>,
    pub commits: Vec<CommitSeen>,
}

/// How many commits are read, newest first across every branch and tag.
/// Deeper than the release history `sync` reads: a ticket's branch is not
/// on the main line, and the walk shares its budget between all of them.
const READ_DEPTH: usize = 500;

/// The names a card is looked for by: its key, and the aliases that are
/// ids rather than words. An alias is anything a task went by, and one
/// like `rtf export` would find itself in every branch that touched rtf.
pub fn needles(card: &Card) -> Vec<String> {
    let mut out = vec![card.key.to_ascii_uppercase()];
    for alias in &card.aliases {
        let alias = alias.trim();
        let is_an_id = !alias.chars().any(char::is_whitespace)
            && alias.chars().any(|c| c.is_ascii_alphabetic())
            && alias.chars().any(|c| c.is_ascii_digit())
            && alias.contains(['-', '_']);
        let upper = alias.to_ascii_uppercase();
        if is_an_id && !out.contains(&upper) {
            out.push(upper);
        }
    }
    out
}

/// Whether `text` names `name` as a whole word: `ACME-73` is not in
/// `ACME-7310`, and is in `fix/ACME-73-rtf`. Case does not matter.
pub fn names(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // ASCII upper-casing keeps every byte where it was, so an offset in
    // one is an offset in the other.
    let hay = text.to_ascii_uppercase();
    let needle = name.to_ascii_uppercase();
    let bytes = hay.as_bytes();
    let mut from = 0;
    while let Some(at) = hay[from..].find(&needle) {
        let start = from + at;
        let end = start + needle.len();
        let before = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before && after {
            return true;
        }
        from = start + 1;
        while !hay.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}

/// The cards a text names, by id.
fn named(text: &str, cards: &[(i64, Vec<String>)]) -> Vec<i64> {
    cards
        .iter()
        .filter(|(_, names_of)| names_of.iter().any(|n| names(text, n)))
        .map(|(id, _)| *id)
        .collect()
}

/// Reads one repository for the cards given.
pub fn read(repo: &Repository, cards: &[Card]) -> Result<Seen> {
    let mut seen = Seen::default();
    let wanted: Vec<(i64, Vec<String>)> = cards.iter().map(|c| (c.id, needles(c))).collect();
    if wanted.is_empty() {
        return Ok(seen);
    }

    let refs = repo.refs().context("cannot read the repository's references")?;
    // Local branches first, so that a branch pushed and checked out is
    // recorded once, as the local one.
    let mut branches: Vec<(String, Option<String>, String)> = refs.branches.into_iter().map(|b| (b.name, None, b.target)).collect();
    for branch in refs.remote_branches {
        // `origin/fix/ACME-7310-rtf` is the branch `fix/ACME-7310-rtf` on
        // `origin`. The remote's own name may hold a slash, so it is taken
        // from the remotes the repository has before guessing at the first.
        let split = match &branch.remote {
            Some(remote) => branch
                .name
                .strip_prefix(remote.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .map(|name| (remote.clone(), name.to_string())),
            None => branch.name.split_once('/').map(|(remote, name)| (remote.to_string(), name.to_string())),
        };
        let Some((remote, name)) = split else { continue };
        if name == "HEAD" || branches.iter().any(|(n, _, _)| *n == name) {
            continue;
        }
        branches.push((name, Some(remote), branch.target));
    }

    let named_tips: HashSet<String> = branches
        .iter()
        .filter(|(name, _, _)| !named(name, &wanted).is_empty())
        .map(|(_, _, tip)| tip.clone())
        .collect();
    let tips = crate::git::commits(repo, &named_tips)?;
    for (name, remote, tip) in &branches {
        let ids = named(name, &wanted);
        if ids.is_empty() {
            continue;
        }
        let Some(commit) = tips.get(tip) else { continue };
        let Some(subject) = crate::git::subject(repo, tip) else { continue };
        let at = moment(commit.committer.seconds);
        for task_id in ids {
            seen.branches.push(BranchSeen {
                task_id,
                name: name.clone(),
                remote: remote.clone(),
                tip: tip.clone(),
                tip_at: at.clone(),
                subject: subject.clone(),
            });
        }
    }

    if branches.is_empty() {
        return Ok(seen);
    }
    let log = repo.log(Tips::All, READ_DEPTH).context("cannot walk the history")?;
    for commit in &log.commits {
        let Ok(message) = repo.message(&commit.id) else { continue };
        let ids = named(&message, &wanted);
        if ids.is_empty() {
            continue;
        }
        let at = moment(commit.committer.seconds);
        let subject = message.lines().next().unwrap_or_default().trim().to_string();
        for task_id in ids {
            seen.commits.push(CommitSeen {
                task_id,
                hash: commit.id.clone(),
                at: at.clone(),
                subject: subject.clone(),
            });
        }
    }
    Ok(seen)
}

/// Whole days from a moment to now; `None` when the moment does not read.
/// A moment in the future - a clock set wrong on the machine that
/// committed - is no days, not a negative number of them.
pub fn days_since(at: &str) -> Option<i64> {
    let then: jiff::Timestamp = at.parse().ok()?;
    let seconds = jiff::Timestamp::now().as_second() - then.as_second();
    Some((seconds / 86_400).max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, aliases: &[&str]) -> Card {
        Card {
            id: 1,
            key: key.to_string(),
            title: String::new(),
            status: "new".to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            summary: None,
            created_at: String::new(),
            updated_at: None,
            snoozed_until: None,
        }
    }

    #[test]
    fn a_name_is_found_as_a_whole_word_in_any_case() {
        assert!(names("fix/ACME-7310-rtf", "ACME-7310"));
        assert!(names("fix(acme-7310): read rtf", "ACME-7310"));
        assert!(names("ACME-7310", "ACME-7310"));
        assert!(names("see ACME-7310.", "ACME-7310"));
        // The name of another ticket that begins with this one's.
        assert!(!names("fix/ACME-73100-rtf", "ACME-7310"));
        assert!(!names("fix/XACME-7310", "ACME-7310"));
        assert!(!names("fix/ACME-73", "ACME-7310"));
        // A later occurrence is found after an earlier one that is not whole.
        assert!(names("ACME-73100 then ACME-7310", "ACME-7310"));
        // A local key, which is not the shape of a tracker's id.
        assert!(names("local-20260908-1-filter", "LOCAL-20260908-1"));
        assert!(!names("local-20260908-12", "LOCAL-20260908-1"));
        assert!(names("ёж ACME-7310 ёж", "ACME-7310"));
        assert!(!names("anything", ""));
    }

    #[test]
    fn an_alias_is_a_needle_only_when_it_is_an_id() {
        let c = card("ACME-7310", &["ops-512", "rtf export", "rtf", "LOCAL-20260908-1"]);
        assert_eq!(needles(&c), vec!["ACME-7310", "OPS-512", "LOCAL-20260908-1"]);
    }

    #[test]
    fn days_are_whole_and_never_negative() {
        assert_eq!(days_since(&jiff::Timestamp::now().to_string()), Some(0));
        let three = jiff::Timestamp::now().as_second() - 3 * 86_400 - 60;
        assert_eq!(days_since(&moment(three)), Some(3));
        let ahead = jiff::Timestamp::now().as_second() + 5 * 86_400;
        assert_eq!(days_since(&moment(ahead)), Some(0));
        assert_eq!(days_since("not a moment"), None);
    }
}
