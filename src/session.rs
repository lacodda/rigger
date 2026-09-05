//! A session: the events of one sitting, gathered under one roof.
//!
//! Until now an event knew its project and its day and nothing else, so
//! "what did we do last time" could only be answered by a window of days -
//! which is wrong in both directions. Two sessions in an evening became one
//! blur; a session spanning midnight became two.
//!
//! A session fixes the boundary by recording it. Everything written while
//! one is open belongs to it, `end` says what that was, and the next
//! packet can open with the difference rather than with a fixed number of
//! recent events.
//!
//! The end-of-session ritual is the thing this replaces. It has always been
//! a list in a skill file that the assistant had to remember at exactly the
//! moment it was running out of context - which is when it is least likely
//! to remember anything. A ritual that depends on remembering is a ritual
//! that stops happening.

use serde::Serialize;

use crate::db::{RecentEvent, Session};

/// What one session turned out to hold.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub project: String,
    pub session: i64,
    pub started_at: String,
    pub ended_at: String,
    pub decisions: Vec<String>,
    pub findings: Vec<String>,
    pub pitfalls: Vec<String>,
    pub changes: Vec<String>,
    /// Questions raised for the owner during the session.
    pub questions: Vec<String>,
    /// The line the next session starts from, if one was set.
    pub next_step: Option<String>,
    /// Versions whose tags landed inside the session.
    pub shipped: Vec<String>,
    /// Tasks closed while it was open.
    pub tasks_closed: Vec<String>,
}

impl Summary {
    /// Whether anything at all was recorded.
    ///
    /// A session that recorded nothing is not a failure - some sittings are
    /// reading - but it is the one case where `end` has nothing to say and
    /// should say so in a line rather than print empty headings.
    pub fn empty(&self) -> bool {
        self.decisions.is_empty()
            && self.findings.is_empty()
            && self.pitfalls.is_empty()
            && self.changes.is_empty()
            && self.questions.is_empty()
            && self.shipped.is_empty()
            && self.tasks_closed.is_empty()
    }

    pub fn recorded(&self) -> usize {
        self.decisions.len() + self.findings.len() + self.pitfalls.len() + self.changes.len() + self.questions.len()
    }

    /// What the ritual asks the assistant to check before it stops.
    ///
    /// Not a scolding and not a gate: `end` closes the session either way.
    /// These are the two things the written ritual asks for by name and the
    /// two most often missing, because they are the last steps of a session
    /// and a session ends when attention has run out.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.next_step.is_none() {
            out.push("no next step - the next session will open without a first line");
        }
        // Work with nothing written down about why is work the record cannot
        // explain later. A session that shipped or changed something and
        // recorded no reasoning is the shape this is looking for.
        if !self.changes.is_empty() && self.decisions.is_empty() && self.findings.is_empty() && self.pitfalls.is_empty() {
            out.push("changes but no decision, finding or pitfall - nothing says why");
        }
        out
    }
}

/// Sorts a session's events into the summary's lists.
pub fn summarise(
    project: &str,
    session: &Session,
    events: &[RecentEvent],
    shipped: Vec<String>,
    tasks_closed: Vec<String>,
    next_step: Option<String>,
) -> Summary {
    let mut summary = Summary {
        project: project.to_string(),
        session: session.id,
        started_at: session.started_at.clone(),
        ended_at: session.ended_at.clone().unwrap_or_default(),
        decisions: Vec::new(),
        findings: Vec::new(),
        pitfalls: Vec::new(),
        changes: Vec::new(),
        questions: Vec::new(),
        next_step,
        shipped,
        tasks_closed,
    };
    for event in events {
        // The chronicle read out of commit messages is not what a session
        // did - it is what git already says, and `sync` writes it whenever
        // it runs. Counting it here would make every session look busy.
        if event.from_git {
            continue;
        }
        let into = match event.kind.as_str() {
            "decision" => &mut summary.decisions,
            "finding" => &mut summary.findings,
            "pitfall" => &mut summary.pitfalls,
            "change" => &mut summary.changes,
            "question" => &mut summary.questions,
            _ => continue,
        };
        into.push(event.body.clone());
    }
    summary
}

/// The diary entry a session leaves behind.
///
/// Composed from what the session recorded, not written for it. rigger has
/// no opinion about the day and inventing one would put words in the
/// owner's diary that nobody said - so the entry is the session's own
/// sentences, arranged, with the headings the hub already uses.
pub fn diary_entry(summary: &Summary, day: &str, heading: Option<&str>) -> String {
    let title = match heading {
        Some(text) if !text.trim().is_empty() => format!("## {day} · {}", text.trim()),
        _ => format!("## {day}"),
    };
    let mut out = format!("{title}\n");

    let section = |out: &mut String, label: &str, items: &[String]| {
        if items.is_empty() {
            return;
        }
        out.push_str(&format!("\n**{label}**\n"));
        for item in items {
            // One bullet per event, kept whole: the entry is a record, and
            // a summary of a summary loses the thing worth keeping.
            out.push_str(&format!("\n- {}\n", item.trim()));
        }
    };

    if !summary.shipped.is_empty() {
        out.push_str(&format!("\n**Выпущено.** {}\n", summary.shipped.join(", ")));
    }
    section(&mut out, "Сделано.", &summary.changes);
    section(&mut out, "Решения.", &summary.decisions);
    section(&mut out, "Находки.", &summary.findings);
    section(&mut out, "Грабли.", &summary.pitfalls);
    section(&mut out, "Ждёт владельца.", &summary.questions);

    if !summary.tasks_closed.is_empty() {
        out.push_str(&format!("\n**Закрыто задач:** {}\n", summary.tasks_closed.len()));
    }
    if let Some(next) = &summary.next_step {
        out.push_str(&format!("\n**Следующий шаг.** {}\n", next.trim()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            id: 7,
            project_id: 1,
            started_at: "2026-09-05T09:00:00Z".to_string(),
            ended_at: Some("2026-09-05T17:00:00Z".to_string()),
        }
    }

    fn event(kind: &str, body: &str, from_git: bool) -> RecentEvent {
        RecentEvent {
            kind: kind.to_string(),
            date: "2026-09-05".to_string(),
            body: body.to_string(),
            from_git,
        }
    }

    #[test]
    fn events_are_sorted_by_what_they_are() {
        let events = vec![
            event("decision", "the tier carries its rhythm", false),
            event("finding", "the grid wraps at 46 releases", false),
            event("pitfall", "a byte index panics on a dash", false),
            event("change", "added the calendar", false),
            event("question", "which tier for dowel?", false),
        ];
        let summary = summarise("alpha", &session(), &events, vec![], vec![], None);
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.findings.len(), 1);
        assert_eq!(summary.pitfalls.len(), 1);
        assert_eq!(summary.changes.len(), 1);
        assert_eq!(summary.questions.len(), 1);
        assert_eq!(summary.recorded(), 5);
    }

    /// The chronicle read out of commit messages is what git already says,
    /// not what the session did. Counting it would make every session that
    /// happened to run `sync` look productive.
    #[test]
    fn the_chronicle_from_git_is_not_what_a_session_did() {
        let events = vec![
            event("change", "feat: read tags into facts", true),
            event("change", "wrote the session summary", false),
        ];
        let summary = summarise("alpha", &session(), &events, vec![], vec![], None);
        assert_eq!(summary.changes, vec!["wrote the session summary"]);
        assert_eq!(summary.recorded(), 1);
    }

    #[test]
    fn a_session_that_recorded_nothing_says_so() {
        let summary = summarise("alpha", &session(), &[], vec![], vec![], None);
        assert!(summary.empty());
        // And a session that only shipped is not empty: the tag is the work.
        let shipped = summarise("alpha", &session(), &[], vec!["v0.1.0".to_string()], vec![], None);
        assert!(!shipped.empty());
    }

    /// The two things the written ritual asks for by name, and the two most
    /// often missing - because they are the last steps of a session, and a
    /// session ends when attention has run out.
    #[test]
    fn the_end_names_what_the_ritual_asks_for_and_did_not_get() {
        let bare = summarise("alpha", &session(), &[event("change", "did a thing", false)], vec![], vec![], None);
        let missing = bare.missing();
        assert!(missing.iter().any(|m| m.contains("next step")), "{missing:?}");
        assert!(missing.iter().any(|m| m.contains("nothing says why")), "{missing:?}");

        // With a reason recorded and a next step set, nothing is missing.
        let whole = summarise(
            "alpha",
            &session(),
            &[event("change", "did a thing", false), event("decision", "because", false)],
            vec![],
            vec![],
            Some("carry on".to_string()),
        );
        assert!(whole.missing().is_empty(), "{:?}", whole.missing());
    }

    /// A session with reasoning but no change has nothing to explain, so the
    /// "why" complaint must not fire - otherwise a day spent deciding gets
    /// told off for deciding.
    #[test]
    fn a_session_that_only_decided_is_not_asked_why() {
        let summary = summarise(
            "alpha",
            &session(),
            &[event("decision", "we will use ISO weeks", false)],
            vec![],
            vec![],
            Some("start on it".to_string()),
        );
        assert!(summary.missing().is_empty(), "{:?}", summary.missing());
    }

    #[test]
    fn the_diary_entry_is_the_sessions_own_sentences() {
        let summary = summarise(
            "alpha",
            &session(),
            &[
                event("change", "added the calendar", false),
                event("decision", "the tier carries its rhythm", false),
                event("pitfall", "the grid wraps at 46 releases", false),
            ],
            vec!["v0.10.0".to_string()],
            vec!["a task".to_string()],
            Some("ship the retro".to_string()),
        );
        let entry = diary_entry(&summary, "2026-09-05", Some("v0.10.0 «Календарь»"));

        assert!(entry.starts_with("## 2026-09-05 · v0.10.0 «Календарь»"), "{entry}");
        assert!(entry.contains("**Выпущено.** v0.10.0"), "{entry}");
        assert!(entry.contains("added the calendar"), "{entry}");
        assert!(entry.contains("the tier carries its rhythm"), "{entry}");
        assert!(entry.contains("the grid wraps at 46 releases"), "{entry}");
        assert!(entry.contains("**Закрыто задач:** 1"), "{entry}");
        assert!(entry.contains("**Следующий шаг.** ship the retro"), "{entry}");
        // An empty section is not printed as an empty heading.
        assert!(!entry.contains("**Находки.**"), "{entry}");
    }

    #[test]
    fn a_diary_entry_without_a_heading_still_names_its_day() {
        let summary = summarise("alpha", &session(), &[event("change", "a thing", false)], vec![], vec![], None);
        let entry = diary_entry(&summary, "2026-09-05", None);
        assert!(entry.starts_with("## 2026-09-05\n"), "{entry}");
        // A heading of nothing but spaces is the same as none.
        let blank = diary_entry(&summary, "2026-09-05", Some("   "));
        assert!(blank.starts_with("## 2026-09-05\n"), "{blank}");
    }
}
