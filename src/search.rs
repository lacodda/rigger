//! Asking the record questions: where was this decided, and why is it so.
//!
//! The record has been accumulating for as long as the line has - by the
//! time this arrived, nine hundred events. That is past the point where
//! anyone remembers where a thing was settled, and re-deciding something the
//! line already argued through is the expensive kind of forgetting.
//!
//! Two questions, then. `find` answers "where did we say this?" across every
//! project at once. `why` answers "what led to this release?" - and it can,
//! without anyone having tagged events by version, because a version is
//! bounded by the release before it.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::db::{Db, Found, VersionFacts};

/// A version, the release it followed, and the work between them.
#[derive(Debug, Serialize)]
pub struct Why {
    pub project: String,
    pub version: VersionFacts,
    pub after: Option<VersionFacts>,
    pub events: Vec<Found>,
}

pub fn why(db: &Db, project: &crate::db::Project, version: &str) -> Result<Why> {
    let Some((version, after)) = db.version_and_predecessor(project.id, version)? else {
        bail!("no version '{version}' recorded for {}; `rigger context` lists what there is", project.name);
    };

    // The window: from the previous release to this one. The moment is used
    // where it is known, because this line ships several versions on one day
    // and a day-wide window would hand every one of them the whole day's
    // work. A version imported from a hub has only the day, which is still
    // better than nothing; an unshipped version has no upper bound at all,
    // so its window runs to now - exactly the work in progress.
    let from = after.as_ref().and_then(bound_after);
    let until = bound_until(&version);
    let events = db.events_between(project.id, from.as_deref(), until.as_deref())?;

    Ok(Why {
        project: project.name.clone(),
        version,
        after,
        events,
    })
}

/// The query as FTS5 should read it.
///
/// A bare word becomes a prefix search. Russian inflects, and FTS5 has no
/// stemmer for it: searching this record for `миграция` found two events
/// where `миграц*` finds thirty-four, because the word appears as
/// `миграции`, `миграцию`, `миграцией`. English loses nothing by the same
/// rule - `budget` and `budget*` match the same events here.
///
/// Anything with FTS5 syntax in it - quotes, an operator, a column filter,
/// an explicit `*` - is passed through untouched, so the full language stays
/// available to whoever wants it.
pub fn as_fts_query(query: &str) -> String {
    let query = query.trim();
    let has_syntax = query.contains(['"', '*', '(', ')', ':', '^', '-']) || query.split_whitespace().any(|w| matches!(w, "AND" | "OR" | "NOT" | "NEAR"));
    if has_syntax || query.is_empty() {
        return query.to_string();
    }
    // Several bare words: each becomes a prefix, and FTS5 requires all of
    // them - which is what a person typing two words means.
    query.split_whitespace().map(|word| format!("{word}*")).collect::<Vec<_>>().join(" ")
}

/// One event, as both commands print it.
///
/// Fixed columns rather than prose: these are read by scanning down the
/// project and kind, not by reading sentences.
pub fn render_event(event: &Found, show_project: bool) -> String {
    let mut out = String::new();
    if show_project {
        out.push_str(&format!("{:<12} ", truncate(&event.project, 12)));
    }
    out.push_str(&format!("{}  {:<9} ", event.date, event.kind));
    out.push_str(&one_line(&event.body));
    out.push('\n');
    out
}

/// The first line of an event, cut to fit a terminal.
///
/// A decision in this record runs to fifteen hundred characters. A list of
/// them at full length is not a list, so the search shows the line that
/// identifies each one and leaves reading it to `why` or to the packet.
fn one_line(body: &str) -> String {
    let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    truncate(first, 96)
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

/// What `find` prints when nothing matched, which is a result, not an error.
pub fn nothing_found(query: &str, project: Option<&str>, kind: Option<&str>) -> String {
    let mut out = format!("Nothing matches {query:?}");
    match (project, kind) {
        (Some(p), Some(k)) => out.push_str(&format!(" in {p}, among {k} events")),
        (Some(p), None) => out.push_str(&format!(" in {p}")),
        (None, Some(k)) => out.push_str(&format!(" among {k} events")),
        (None, None) => {}
    }
    // A bare word is already searched as a prefix, so "try a prefix" would
    // be advice the command has taken already; what is left is a shorter
    // stem, or the syntax.
    out.push_str(".\nA bare word is searched as a prefix: try a shorter one, or FTS5 syntax - \"exact phrase\", `one OR two`.");
    out
}

/// The lower bound of a window: everything after the previous release.
///
/// A day is padded to its end, so that a release dated only `2026-09-03`
/// does not claim the events of that whole day as its successor's work.
fn bound_after(v: &VersionFacts) -> Option<String> {
    match (&v.shipped_ts, &v.shipped_at) {
        (Some(ts), _) => Some(ts.clone()),
        (None, Some(day)) => Some(format!("{day}T23:59:59Z")),
        (None, None) => None,
    }
}

/// The upper bound: everything up to and including this release.
fn bound_until(v: &VersionFacts) -> Option<String> {
    match (&v.shipped_ts, &v.shipped_at) {
        (Some(ts), _) => Some(ts.clone()),
        (None, Some(day)) => Some(format!("{day}T23:59:59Z")),
        // Still being built: no upper bound, so the window runs to now.
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, body: &str) -> Found {
        Found {
            project: "sample".into(),
            kind: kind.into(),
            date: "2026-09-04".into(),
            body: body.into(),
            from_git: false,
            commit_hash: None,
        }
    }

    #[test]
    fn an_event_is_one_line_however_long_the_body() {
        // A decision here averages 1500 characters; a list of them at full
        // length is not a list.
        let long = event("decision", &"слово ".repeat(400));
        let line = render_event(&long, true);
        assert_eq!(line.lines().count(), 1, "{line}");
        assert!(line.chars().count() < 140, "{} chars: {line}", line.chars().count());
        assert!(line.contains('…'), "a cut line must say it was cut: {line}");
    }

    #[test]
    fn the_first_line_is_what_identifies_an_event() {
        let e = event("decision", "**The record is the database.**\n\nBecause prose cannot be filtered.");
        let line = render_event(&e, false);
        assert!(line.contains("The record is the database."), "{line}");
        assert!(!line.contains("Because prose"), "{line}");
    }

    #[test]
    fn a_leading_blank_line_does_not_produce_an_empty_result() {
        let e = event("finding", "\n\nThe thing that was learnt.");
        assert!(render_event(&e, false).contains("The thing that was learnt."));
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // Cyrillic is two bytes per character; counting bytes would cut
        // these lines to half the width and could split a character.
        let text = "я".repeat(200);
        let cut = truncate(&text, 96);
        assert_eq!(cut.chars().count(), 96);
    }

    #[test]
    fn a_bare_word_becomes_a_prefix_search() {
        // Russian inflects and FTS5 has no stemmer for it: `миграция` found
        // two events in the real record where `миграц*` finds thirty-four.
        assert_eq!(as_fts_query("миграция"), "миграция*");
        assert_eq!(as_fts_query("budget"), "budget*");
        assert_eq!(as_fts_query("two words"), "two* words*");
        assert_eq!(as_fts_query("  spaced  "), "spaced*");
    }

    #[test]
    fn a_query_that_uses_the_syntax_is_left_alone() {
        // The full FTS5 language stays available; guessing at it would turn
        // a deliberate search into a different one.
        for query in ["\"exact phrase\"", "budget AND packet", "миграц*", "packet NOT commit", "body:decision"] {
            assert_eq!(as_fts_query(query), query, "{query} must pass through untouched");
        }
    }

    #[test]
    fn nothing_found_says_where_it_looked() {
        let msg = nothing_found("budget", Some("kasl"), Some("decision"));
        assert!(msg.contains("kasl") && msg.contains("decision"), "{msg}");
        // And how to search further, since a bare word already searches
        // as a prefix - repeating that advice would be useless.
        assert!(msg.contains("prefix"), "{msg}");
        assert!(msg.contains("syntax"), "{msg}");
    }
}
