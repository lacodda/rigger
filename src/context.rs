//! The context packet: what an assistant needs to start a session.
//!
//! The packet exists because the alternative - reading a project's hub - costs
//! tens of thousands of tokens and, past a certain size, no longer fits at
//! all. So the packet has a budget, and the budget is a gate: when the record
//! does not fit, the packet drops the oldest events and says how many it left
//! out, rather than silently truncating.
//!
//! Sections are ordered by what a session needs first: where the project
//! stands, what is being built now, what waits for the owner, what happened
//! recently, and the one line the last session left behind.

use anyhow::Result;
use serde::Serialize;

use crate::db::{Db, Project, Task};

/// Tokens are approximated from characters. A tokeniser would be exact for
/// one model and wrong for the next, and this number decides only how much
/// history to include - four characters per token is the usual rule of thumb
/// for English, and Cyrillic runs denser, so the estimate errs on the safe
/// side by counting characters, not bytes.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

pub const DEFAULT_BUDGET: usize = 3000;

#[derive(Debug, Serialize)]
pub struct Packet {
    pub project: String,
    pub state: State,
    pub current: Option<Stage>,
    pub questions: Vec<Item>,
    pub wishes: Vec<Item>,
    pub events: Vec<Event>,
    pub next_step: Option<String>,
    /// How many recent events the budget left out.
    pub events_omitted: usize,
}

#[derive(Debug, Serialize)]
pub struct State {
    pub path: String,
    pub remote: Option<String>,
    pub last_shipped: Option<String>,
    pub last_shipped_on: Option<String>,
    pub versions_planned: u64,
    pub tasks_open: u64,
    /// Days since anything was recorded about this project. A project that
    /// has gone quiet is worth noticing at the top of a session.
    pub days_quiet: Option<i64>,
    /// Commits since the newest tag, as the last `sync` read them.
    pub commits_since_tag: Option<u32>,
    /// Days since the last commit - the owner's question: how long has this
    /// project actually been still? Quiet in the record and quiet in git are
    /// different things, and only the second one means nobody has worked.
    pub days_since_commit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Serialize)]
pub struct Item {
    pub id: i64,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub kind: String,
    pub date: String,
    pub body: String,
}

/// What each section of the packet costs, for `--explain`.
#[derive(Debug, Serialize)]
pub struct Cost {
    pub section: &'static str,
    pub tokens: usize,
}

pub fn build(db: &Db, project: &Project, budget: usize) -> Result<Packet> {
    let activity = db.activity(project.id)?;
    let state = State {
        path: project.path.clone(),
        remote: project.remote.clone(),
        last_shipped: db.last_shipped_version(project.id)?.map(|(name, _)| name),
        last_shipped_on: db.last_shipped_version(project.id)?.map(|(_, on)| on),
        versions_planned: db.count_versions(project.id, "planned")?,
        tasks_open: db.count_open_tasks(project.id)?,
        days_quiet: db.last_event_at(project.id)?.as_deref().and_then(days_since),
        commits_since_tag: activity.as_ref().map(|a| a.commits_since_tag),
        days_since_commit: activity.as_ref().and_then(|a| a.last_commit_at.as_deref()).and_then(days_since_day),
    };

    let current = db.current_stage(project.id)?.map(|s| Stage {
        version: s.version,
        title: s.title,
        tasks: s.tasks,
    });
    let questions = db
        .open_events(project.id, "question")?
        .into_iter()
        .map(|(id, text)| Item { id, text })
        .collect();
    let wishes = db.open_events(project.id, "wish")?.into_iter().map(|(id, text)| Item { id, text }).collect();
    let next_step = db.latest_event_body(project.id, "next")?;

    let mut packet = Packet {
        project: project.name.clone(),
        state,
        current,
        questions,
        wishes,
        events: Vec::new(),
        next_step,
        events_omitted: 0,
    };

    // Everything above is what a session cannot start without, so it is never
    // dropped. Recent events fill whatever budget is left, newest first, and
    // the count of what did not fit is part of the packet.
    //
    // The window is wider than any packet can hold, so that "left out" counts
    // events the budget refused rather than events this query never asked
    // for: a packet that drops history silently is the failure this whole
    // command exists to avoid. Older events beyond the window are the job of
    // `find` and `why`, not of a session's first screen.
    const WINDOW: u32 = 200;
    let recent = db.recent_events(project.id, WINDOW)?;
    let beyond_window = db.count_recent_events(project.id)?.saturating_sub(recent.len() as u64) as usize;

    // The line that says what was dropped costs tokens of its own, and so
    // does the heading above the events. Both are reserved before anything is
    // added, so that a packet never ends up over the budget it reports.
    let reserve = estimate_tokens("\n## Recent\n(999 older events left out by the budget)\n");
    let mut spent = estimate_tokens(&render(&packet)) + reserve;
    packet.events_omitted = beyond_window;
    for (kind, date, body) in recent {
        let body = summarise(&body);
        let event = Event { kind, date, body };
        let cost = estimate_tokens(&render_event(&event));
        if spent + cost > budget {
            packet.events_omitted += 1;
            continue;
        }
        spent += cost;
        packet.events.push(event);
    }
    Ok(packet)
}

/// The line of an event a session needs, and a pointer to the rest.
///
/// A decision in these hubs averages 1500 characters and reaches 5600 - it is
/// a document, not a note. Three of them at full length crowd out ten others,
/// and a session that needs the whole argument can read it with `why` later.
/// So the packet keeps the heading and the first sentence of the reasoning,
/// which is where these entries state what was decided.
fn summarise(text: &str) -> String {
    let mut out = String::new();
    for para in text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let para = para.trim().replace('\n', " ");
        if out.is_empty() {
            out = para;
            continue;
        }
        // The heading alone rarely says what was decided; the paragraph after
        // it does. Two is enough, and the second is cut to one sentence.
        out.push_str(" — ");
        out.push_str(&first_sentence(&para));
        break;
    }
    let full = text.chars().count();
    let kept = out.chars().count();
    if full > kept + 40 {
        out.push_str(&format!(" (+{} chars)", full - kept));
    }
    out
}

/// The first sentence, or the whole text when it has none within reach.
fn first_sentence(text: &str) -> String {
    let mut end = None;
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?')
            && text[i + c.len_utf8()..].starts_with(|n: char| n.is_whitespace())
            // An abbreviation or a version number is not a sentence end.
            && !text[..i].ends_with(|p: char| p.is_ascii_digit())
        {
            end = Some(i + c.len_utf8());
            break;
        }
    }
    match end {
        Some(end) if end <= 400 => text[..end].to_string(),
        _ => shorten(text, 400),
    }
}

/// "today", "yesterday" or "N days ago" - a session reads a word faster
/// than it reads a number it has to subtract from the date.
fn days_ago(days: i64) -> String {
    match days {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        n => format!("{n} days ago"),
    }
}

/// Whole days between a recorded day (`YYYY-MM-DD`) and today.
fn days_since_day(day: &str) -> Option<i64> {
    days_since(&format!("{day}T00:00:00Z"))
}

/// Whole days between a recorded timestamp and now.
fn days_since(timestamp: &str) -> Option<i64> {
    let then: jiff::Timestamp = timestamp.parse().ok()?;
    let seconds = jiff::Timestamp::now().as_second() - then.as_second();
    Some((seconds / 86_400).max(0))
}

/// Trims at a word boundary, saying how much was left out.
fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit).collect();
    let cut = match cut.rsplit_once(char::is_whitespace) {
        Some((head, _)) => head.to_string(),
        None => cut,
    };
    format!("{cut}…")
}

pub fn costs(packet: &Packet) -> Vec<Cost> {
    let mut costs = vec![
        Cost {
            section: "state",
            tokens: estimate_tokens(&render_state(packet)),
        },
        Cost {
            section: "current stage",
            tokens: packet.current.as_ref().map(|s| estimate_tokens(&render_stage(s))).unwrap_or(0),
        },
        Cost {
            section: "questions",
            tokens: estimate_tokens(&render_items("", &packet.questions)),
        },
        Cost {
            section: "wishes",
            tokens: estimate_tokens(&render_items("", &packet.wishes)),
        },
        Cost {
            section: "events",
            tokens: packet.events.iter().map(|e| estimate_tokens(&render_event(e))).sum(),
        },
        Cost {
            section: "next step",
            tokens: packet.next_step.as_deref().map(estimate_tokens).unwrap_or(0),
        },
    ];
    costs.retain(|c| c.tokens > 0);
    costs
}

fn render_state(p: &Packet) -> String {
    let mut out = format!("# {}\n\n{}\n", p.project, p.state.path);
    if let Some(remote) = &p.state.remote {
        out.push_str(remote);
        out.push('\n');
    }
    match (&p.state.last_shipped, &p.state.last_shipped_on) {
        (Some(v), Some(on)) => out.push_str(&format!("Last shipped: {v} on {on}\n")),
        _ => out.push_str("Nothing shipped yet\n"),
    }
    out.push_str(&format!("{} versions planned, {} tasks open\n", p.state.versions_planned, p.state.tasks_open));
    // What git says, which is a different question from what the record
    // says: a project can be busy in commits and silent in events, and the
    // owner's question is how long it has actually been still.
    match (p.state.commits_since_tag, p.state.days_since_commit) {
        (Some(0), Some(days)) if days > 0 => out.push_str(&format!("Nothing committed since the last release, {}\n", days_ago(days))),
        (Some(commits), Some(days)) if commits > 0 => {
            let plural = if commits == 1 { "commit" } else { "commits" };
            out.push_str(&format!("{commits} {plural} since the last release, the last one {}\n", days_ago(days)))
        }
        _ => {}
    }
    // Said only when it means something. A day or two of quiet is the normal
    // rhythm of a project; a fortnight is worth seeing before starting work.
    if let Some(days) = p.state.days_quiet
        && days >= 7
    {
        out.push_str(&format!("Quiet for {days} days\n"));
    }
    out
}

fn render_stage(stage: &Stage) -> String {
    let mut out = format!("\n## Current stage: {}", stage.version);
    if let Some(title) = &stage.title {
        out.push_str(&format!(" · {title}"));
    }
    out.push('\n');
    for task in &stage.tasks {
        out.push_str(&format!("- {}\n", task.title));
    }
    out
}

fn render_items(heading: &str, items: &[Item]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = if heading.is_empty() { String::new() } else { format!("\n## {heading}\n") };
    for item in items {
        out.push_str(&format!("- [{}] {}\n", item.id, item.text));
    }
    out
}

fn render_event(event: &Event) -> String {
    format!("- {} · {} · {}\n", event.date, event.kind, event.body)
}

/// The packet as the assistant reads it.
pub fn render(packet: &Packet) -> String {
    let mut out = render_state(packet);
    if let Some(stage) = &packet.current {
        out.push_str(&render_stage(stage));
    }
    out.push_str(&render_items("Waiting for the owner", &packet.questions));
    out.push_str(&render_items("Wishes, not yet sorted", &packet.wishes));
    if !packet.events.is_empty() || packet.events_omitted > 0 {
        out.push_str("\n## Recent\n");
        for event in &packet.events {
            out.push_str(&render_event(event));
        }
        // Said even when nothing fit at all: a section that is simply absent
        // reads as "nothing happened", which is the opposite of the truth.
        if packet.events_omitted > 0 {
            out.push_str(&format!("({} older events left out by the budget)\n", packet.events_omitted));
        }
    }
    if let Some(next) = &packet.next_step {
        out.push_str(&format!("\n## Next step\n{next}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_body_is_left_alone() {
        assert_eq!(shorten("short enough", 600), "short enough");
    }

    #[test]
    fn a_long_body_is_cut_at_a_word_boundary() {
        let text = "word ".repeat(200);
        let cut = shorten(&text, 50);
        assert!(cut.len() < text.len());
        assert!(cut.trim_end_matches('…').trim_end().ends_with("word"), "{cut}");
    }

    #[test]
    fn a_summary_keeps_the_heading_and_the_first_sentence_of_the_reason() {
        let text =
            "**The record is the database.**\n\nProse cannot be filtered. And a second sentence that the packet does not need.\n\nA third paragraph entirely.";
        let summary = summarise(text);
        assert!(summary.starts_with("**The record is the database.**"), "{summary}");
        assert!(summary.contains("Prose cannot be filtered."), "{summary}");
        assert!(!summary.contains("second sentence"), "{summary}");
        assert!(summary.ends_with("chars)"), "{summary}");
    }

    #[test]
    fn a_short_event_is_kept_whole_without_a_pointer() {
        let summary = summarise("Renamed the npm package.");
        assert_eq!(summary, "Renamed the npm package.");
    }

    #[test]
    fn a_version_number_does_not_end_a_sentence() {
        // "v0.3.0" would otherwise cut the sentence at its first dot.
        let sentence = first_sentence("The rule holds since v0.3.0 for every project. And then more.");
        assert_eq!(sentence, "The rule holds since v0.3.0 for every project.");
    }

    #[test]
    fn tokens_are_counted_in_characters_not_bytes() {
        // Cyrillic is two bytes per character; counting bytes would double
        // the estimate and shrink every packet of a Russian-language hub.
        assert_eq!(estimate_tokens("решение"), 2);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }
}
