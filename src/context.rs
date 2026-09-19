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

use crate::db::{Db, DocLine, Project, Task};

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
    /// One line per handwritten text the project has: what exists to be
    /// read, not the reading of it.
    ///
    /// A vision runs to thousands of characters and would eat the packet
    /// whole. But a session that does not know a vision exists cannot ask
    /// for it, and the commonest way to contradict one is not to have
    /// heard of it. So the packet names them and says how to open one.
    pub documents: Vec<DocLine>,
    pub next_step: Option<String>,
    /// How many recent events the budget left out.
    pub events_omitted: usize,
    /// How many events are older than the window the packet looks at.
    ///
    /// Counted apart from what the budget refused, because they are a
    /// different fact about a different thing. The packet used to add the
    /// two and call the total "left out by the budget", which on this
    /// record made a project with four hundred events of history look as
    /// though a session had been denied four hundred of them - when the
    /// budget had refused none and the window simply does not reach that
    /// far back. A number that overstates what is missing is as useless
    /// as one that hides it.
    pub events_beyond_window: usize,
    /// What the budget refused, line by line, with the reason.
    ///
    /// The packet has always said how many events it dropped, which tells
    /// an assistant that it is missing something but not what - and
    /// "there is a decision you have not seen" is only useful if you can
    /// find out which one. Filled only for `--explain`: carrying it in
    /// every packet would spend the budget on an account of the budget.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<Dropped>,
}

/// A line the budget would not pay for.
#[derive(Debug, Serialize)]
pub struct Dropped {
    pub kind: String,
    pub date: String,
    /// The first line of what was left out, so it can be asked for by name.
    pub head: String,
    pub tokens: usize,
    pub why: &'static str,
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
    /// What has happened since the last session ended, if there was one.
    pub since_last_session: Option<SinceLastSession>,
    /// The command that says this project is fit to commit, as the project
    /// states it. One line, so that a session knows what green means here
    /// without being told in a skill file.
    pub gate: Option<String>,
}

/// The difference between now and where the last sitting stopped.
///
/// A packet opens with a fixed number of recent events, which answers "what
/// has been going on" and not "what changed while I was away" - and the
/// second is the question an assistant returning to a project actually has.
/// Without it every session re-reads the same history and cannot tell which
/// part of it is new.
#[derive(Debug, Serialize)]
pub struct SinceLastSession {
    /// When the previous session ended.
    pub ended_at: String,
    pub days_ago: Option<i64>,
    /// Events recorded since, not counting the chronicle read from commits.
    pub events: usize,
    /// Changes read out of commit messages since - work done outside a
    /// session, which is most of it.
    pub commits: usize,
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
    /// The neighbour who asked for it, when a wish came from one.
    ///
    /// On the line rather than in a list of its own: an order from a
    /// neighbour is a wish like any other - it is sorted into the plan by
    /// the same `resolve` and competes with the rest for the same stages -
    /// and a second list would invite it to be read as a second queue.
    /// What changes is only that somebody is waiting for the answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked_by: Option<String>,
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
        // A place the record keeps for itself has no location, and its path
        // column holds a marker rather than a directory. The packet opens
        // with this line, so a marker there would be the first thing an
        // assistant read and the first thing it got wrong.
        path: match project.kind {
            crate::db::Kind::Repo => project.path.clone(),
            crate::db::Kind::Service => "(no repository - a place the record keeps for itself)".to_string(),
        },
        remote: project.remote.clone(),
        last_shipped: db.last_shipped_version(project.id)?.map(|(name, _)| name),
        last_shipped_on: db.last_shipped_version(project.id)?.map(|(_, on)| on),
        versions_planned: db.count_versions(project.id, "planned")?,
        tasks_open: db.count_open_tasks(project.id)?,
        days_quiet: db.last_event_at(project.id)?.as_deref().and_then(days_since),
        commits_since_tag: activity.as_ref().map(|a| a.commits_since_tag),
        days_since_commit: activity.as_ref().and_then(|a| a.last_commit_at.as_deref()).and_then(days_since_day),
        since_last_session: since_last_session(db, project)?,
        gate: project.gate.clone(),
    };

    let current = db.current_stage(project.id)?.map(|s| Stage {
        version: s.version,
        title: s.title,
        tasks: s.tasks,
    });
    let questions = db
        .open_events(project.id, "question")?
        .into_iter()
        .map(|(id, text)| Item { id, text, asked_by: None })
        .collect();
    // Who asked is read once and matched by id: the wishes are already in
    // hand, and asking the record a second question per wish would be a
    // query per row on the one screen that is read at the start of every
    // session.
    let asked = db.asked_wishes(project.id)?;
    let wishes = db
        .open_events(project.id, "wish")?
        .into_iter()
        .map(|(id, text)| Item {
            id,
            text,
            asked_by: asked.iter().find(|a| a.id == id).map(|a| a.asked_by.clone()),
        })
        .collect();
    let next_step = db.latest_event_body(project.id, "next")?;

    let mut packet = Packet {
        project: project.name.clone(),
        state,
        current,
        questions,
        wishes,
        events: Vec::new(),
        documents: db.document_lines(project.id)?,
        next_step,
        events_omitted: 0,
        events_beyond_window: 0,
        dropped: Vec::new(),
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
    let reserve = estimate_tokens("\n## Recent\n(999 left out by the budget; 999 older than the window)\n");
    let mut spent = estimate_tokens(&render(&packet)) + reserve;
    packet.events_beyond_window = beyond_window;

    // Changes read from commits get a share of what is left; everything
    // written by a person competes for the rest.
    //
    // Without this the chronicle crowds out the reasoning: kasl's packet came
    // back as seventy commit lines against nineteen decisions, with ninety-one
    // events dropped. A commit can always be read again in git; a decision or
    // a pitfall exists nowhere else, and losing one costs a session the
    // argument behind the code it is about to change.
    let mut git_left = budget.saturating_sub(spent) / 4;

    for recent in recent {
        let from_git = recent.from_git;
        let event = Event {
            kind: recent.kind,
            date: recent.date,
            body: summarise(&recent.body),
        };
        let cost = estimate_tokens(&render_event(&event));
        let refused = if spent + cost > budget {
            Some("over the budget")
        } else if from_git && cost > git_left {
            Some("the chronicle's share of the budget is spent")
        } else {
            None
        };
        if let Some(why) = refused {
            packet.events_omitted += 1;
            packet.dropped.push(Dropped {
                kind: event.kind,
                date: event.date,
                head: head_of(&event.body),
                tokens: cost,
                why,
            });
            continue;
        }
        if from_git {
            git_left -= cost;
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

/// The first line of a body, short enough to name it by.
///
/// Enough to recognise which decision this was and go and read it with
/// `find` or `why`; not so much that the account of what was dropped
/// costs what keeping it would have.
fn head_of(body: &str) -> String {
    let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    shorten(first, 80)
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
            section: "documents",
            tokens: estimate_tokens(&render_documents(packet)),
        },
        Cost {
            section: "next step",
            tokens: packet.next_step.as_deref().map(estimate_tokens).unwrap_or(0),
        },
    ];
    costs.retain(|c| c.tokens > 0);
    costs
}

/// What has changed since the previous sitting ended.
fn since_last_session(db: &Db, project: &Project) -> Result<Option<SinceLastSession>> {
    let Some(last) = db.last_ended_session(project.id)? else {
        return Ok(None);
    };
    let Some(ended_at) = last.ended_at else {
        return Ok(None);
    };
    let events = db.events_since(project.id, &ended_at)?;
    let commits = events.iter().filter(|e| e.from_git).count();
    Ok(Some(SinceLastSession {
        days_ago: days_since(&ended_at),
        ended_at,
        events: events.len() - commits,
        commits,
    }))
}

fn render_since(since: &SinceLastSession) -> String {
    let when = match since.days_ago {
        Some(0) => "earlier today".to_string(),
        Some(1) => "yesterday".to_string(),
        Some(n) => format!("{n} days ago"),
        None => since.ended_at.clone(),
    };
    let mut parts = Vec::new();
    if since.events > 0 {
        parts.push(format!("{} recorded", plural(since.events, "event", "events")));
    }
    if since.commits > 0 {
        parts.push(format!("{} committed", plural(since.commits, "change", "changes")));
    }
    match parts.is_empty() {
        // Nothing since is a fact worth one line: it says the events below
        // are all from before, and that nobody has touched this meanwhile.
        true => format!("Last session ended {when}; nothing since\n"),
        false => format!("Last session ended {when}: {}\n", parts.join(", ")),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
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
    out.push_str(&format!(
        "{} planned, {} open\n",
        plural(p.state.versions_planned as usize, "version", "versions"),
        plural(p.state.tasks_open as usize, "task", "tasks")
    ));
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
    // Where the last sitting stopped, so that the events below can be read
    // as "since then" rather than as undated history.
    if let Some(gate) = &p.state.gate {
        out.push_str(&format!(
            "Gate: {gate}
"
        ));
    }
    if let Some(since) = &p.state.since_last_session {
        out.push_str(&render_since(since));
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
        // The box of a plan means `new`; any other word is worth a mark.
        match task.status.as_str() {
            "new" => out.push_str(&format!("- {}\n", task.title)),
            status => out.push_str(&format!("- {} ({status})\n", task.title)),
        }
    }
    out
}

fn render_items(heading: &str, items: &[Item]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = if heading.is_empty() { String::new() } else { format!("\n## {heading}\n") };
    for item in items {
        match &item.asked_by {
            Some(who) => out.push_str(&format!("- [{}] ({who} asks) {}\n", item.id, item.text)),
            None => out.push_str(&format!("- [{}] {}\n", item.id, item.text)),
        }
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
        match (packet.events_omitted, packet.events_beyond_window) {
            (0, 0) => {}
            (0, beyond) => out.push_str(&format!(
                "({beyond} older events are beyond the window the packet looks at; `rigger find` reaches them)\n"
            )),
            (dropped, 0) => out.push_str(&format!("({dropped} events left out by the budget; `--explain` names them)\n")),
            (dropped, beyond) => out.push_str(&format!(
                "({dropped} events left out by the budget - `--explain` names them - and {beyond} more are beyond the window)\n"
            )),
        }
    }
    out.push_str(&render_documents(packet));
    if let Some(next) = &packet.next_step {
        out.push_str(&format!("\n## Next step\n{next}\n"));
    }
    out
}

/// The documents section: what this project has written down.
fn render_documents(packet: &Packet) -> String {
    if packet.documents.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n## Written down\n");
    for doc in &packet.documents {
        let day = doc.updated_at.split('T').next().unwrap_or(&doc.updated_at);
        out.push_str(&format!(
            "- {} · {} · {day} — `rigger doc show {} {}`\n",
            doc.kind, doc.title, packet.project, doc.slug
        ));
    }
    out
}

/// What the budget refused, for `--explain`.
///
/// Printed after the packet rather than inside it: it is an account of
/// the packet, and an assistant reading the packet should not have to
/// step over it.
pub fn render_dropped(packet: &Packet) -> String {
    if packet.dropped.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n## Left out\n");
    for d in &packet.dropped {
        out.push_str(&format!("{}  {:<9} {:>4}t  {} — {}\n", d.date, d.kind, d.tokens, d.head, d.why));
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
