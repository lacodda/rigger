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
//!
//! Every line that stands for a longer text carries the id it is read by:
//! the packet names, `rigger event` reads.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::db::{Db, DocLine, EventWritten, Project, Task};
use crate::plural;

/// Tokens are approximated from characters. A tokeniser would be exact for
/// one model and wrong for the next, and this number decides only how much
/// history to include - four characters per token is the usual rule of thumb
/// for English, and Cyrillic runs denser, so the estimate errs on the safe
/// side by counting characters, not bytes.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

pub const DEFAULT_BUDGET: usize = 3000;

/// How long the line a question or a wish is known by may run, in
/// characters. Long enough for a heading or a first sentence to say what it
/// is about; short enough that forty of them cost less than one of them
/// used to.
const HEAD_CHARS: usize = 120;

/// A line says how much more there is only when the rest is worth going
/// for: a dangling clause of a few words is not.
const POINTER_AT: usize = 40;

#[derive(Debug, Serialize)]
pub struct Packet {
    pub project: String,
    pub state: State,
    pub current: Option<Stage>,
    pub questions: Vec<Item>,
    /// How many questions the budget left out - the newest; `inbox` lists
    /// every one.
    pub questions_omitted: usize,
    pub wishes: Vec<Item>,
    /// How many wishes the budget left out - the oldest of those no
    /// neighbour is waiting on.
    pub wishes_omitted: usize,
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
    /// The id `rigger event` reads it whole by.
    pub id: i64,
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
    /// How far the newest shipped version got past its tag, when that falls
    /// short of anyone being able to install it. Only the shortfall is
    /// carried: a full delivery is the ordinary case and costs the packet a
    /// line that says nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_short: Option<String>,
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
    /// Open tasks asleep until a day; counted in the packet, not listed.
    pub asleep: Vec<Task>,
    /// Earlier versions whose open work was all put down.
    pub set_aside: Vec<crate::db::SetAside>,
}

impl From<crate::db::CurrentStage> for Stage {
    fn from(s: crate::db::CurrentStage) -> Stage {
        Stage {
            version: s.version,
            title: s.title,
            tasks: s.tasks,
            asleep: s.asleep,
            set_aside: s.set_aside,
        }
    }
}

/// What was put down, in a line each: the tasks asleep, counted with the
/// first day one comes back, and the versions set aside. Counted and not
/// listed, because out of sight is what sleeping is for - but not out of
/// mind: a stage that silently lost a task reads as a stage with less to do.
pub fn put_down_lines(asleep: &[Task], set_aside: &[crate::db::SetAside]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(first) = asleep.iter().filter_map(|t| t.snoozed_until.as_deref()).min() {
        out.push(match asleep.len() {
            1 => format!("1 task asleep until {first}"),
            n => format!("{n} tasks asleep, the first back {first}"),
        });
    }
    for v in set_aside {
        let mut why = Vec::new();
        if v.frozen > 0 {
            why.push(format!("{} frozen", v.frozen));
        }
        if v.asleep > 0 {
            why.push(format!("{} asleep", v.asleep));
        }
        let title = v.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
        out.push(format!("set aside: {}{title} ({})", v.version, why.join(", ")));
    }
    out
}

/// A question or a wish, by the line it is known by.
///
/// The whole text used to go in. Wishes are written to be acted on cold, so
/// they run to paragraphs - a thousand characters is ordinary and three
/// thousand happens - and on a real record forty of them came to six
/// thousand tokens: twice the budget, before a single event. The packet was
/// still "in budget" by its own account, because the queue was never
/// counted against it, and every session on that project started without
/// one line of what had been decided lately. A session needs to know what
/// is queued, not to read the queue; the one it picks up it reads whole.
#[derive(Debug, Serialize)]
pub struct Item {
    pub id: i64,
    /// Its heading when it opens with one, else its first sentence.
    pub head: String,
    /// How many characters the whole holds beyond `head`; 0 when the head
    /// is all of it.
    pub more: usize,
    /// The neighbour who asked for it, when a wish came from one.
    ///
    /// On the line rather than in a list of its own: an order from a
    /// neighbour is a wish like any other - it is sorted into the plan by
    /// the same `resolve` and competes with the rest for the same stages -
    /// and a second list would invite it to be read as a second queue.
    /// What changes is only that somebody is waiting for the answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked_by: Option<String>,
    /// The day a question needs its answer by, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub id: i64,
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
        delivery_short: db
            .latest_delivery(project.id)?
            .and_then(|d| d.delivery)
            .filter(|d| d.is_short())
            .map(|d| d.describe().to_string()),
        versions_planned: db.count_versions(project.id, "planned")?,
        tasks_open: db.count_open_tasks(project.id)?,
        days_quiet: db.last_event_at(project.id)?.as_deref().and_then(days_since),
        commits_since_tag: activity.as_ref().map(|a| a.commits_since_tag),
        days_since_commit: activity.as_ref().and_then(|a| a.last_commit_at.as_deref()).and_then(days_since_day),
        since_last_session: since_last_session(db, project)?,
        gate: project.gate.clone(),
    };

    let current = db.current_stage(project.id)?.map(Stage::from);
    // One query for both lists, with who asked joined in: this is the screen
    // read at the start of every session, and a query per wish to name its
    // neighbour would be paid on every one of them.
    let queue = db.queue(project.id)?;
    let next_step = db.latest_event_body(project.id, "next")?;

    let mut packet = Packet {
        project: project.name.clone(),
        state,
        current,
        questions: Vec::new(),
        questions_omitted: 0,
        wishes: Vec::new(),
        wishes_omitted: 0,
        events: Vec::new(),
        documents: db.document_lines(project.id)?,
        next_step,
        events_omitted: 0,
        events_beyond_window: 0,
        dropped: Vec::new(),
    };

    // The window is wider than any packet can hold, so that "left out" counts
    // events the budget refused rather than events this query never asked
    // for: a packet that drops history silently is the failure this whole
    // command exists to avoid. Older events beyond the window are the job of
    // `find` and `why`, not of a session's first screen.
    const WINDOW: u32 = 200;
    let recent = db.recent_events(project.id, WINDOW)?;
    let beyond_window = db.count_recent_events(project.id)?.saturating_sub(recent.len() as u64) as usize;

    // What is in the packet so far - the state, the stage, the documents and
    // the next step - is what a session cannot start without, and is never
    // dropped. The lines that say what was left out, and the one that says
    // how to read a line whole, cost tokens of their own; they are reserved
    // at their longest before anything else is added, so that a packet never
    // ends up over the budget it reports.
    let reserve = estimate_tokens(&format!("\n## Recent\n{}", events_footer(999, 9999)))
        + estimate_tokens(&questions_footer(999, &project.name))
        + estimate_tokens(&wishes_footer(999))
        + estimate_tokens(&read_whole(&project.name));
    let mut spent = estimate_tokens(&render(&packet)) + reserve;
    packet.events_beyond_window = beyond_window;

    // Summarised once, up front: what the events would cost decides how
    // much the queue may take.
    let recent: Vec<(Event, bool, usize)> = recent
        .into_iter()
        .map(|recent| {
            let event = Event {
                id: recent.id,
                kind: recent.kind,
                date: recent.date,
                body: summarise(&recent.body),
            };
            let cost = estimate_tokens(&render_event(&event));
            (event, recent.from_git, cost)
        })
        .collect();

    // The owner's queue and the events share what is left. Without a share
    // of its own the queue took everything: it was counted as essential,
    // and on a project with forty wishes it left the events not one line.
    // Half is the events' floor, not the queue's ceiling - when the events
    // need less, the queue may have the rest.
    let left = budget.saturating_sub(spent);
    let wanted: usize = recent.iter().map(|(_, _, cost)| cost).sum();
    let share = (left / 2).max(left.saturating_sub(wanted));
    spent += fill_queue(&mut packet, queue, share);

    // Changes read from commits get a share of what is left; everything
    // written by a person competes for the rest.
    //
    // Without this the chronicle crowds out the reasoning: kasl's packet came
    // back as seventy commit lines against nineteen decisions, with ninety-one
    // events dropped. A commit can always be read again in git; a decision or
    // a pitfall exists nowhere else, and losing one costs a session the
    // argument behind the code it is about to change.
    let mut git_left = budget.saturating_sub(spent) / 4;

    for (event, from_git, cost) in recent {
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
                id: event.id,
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

const QUESTIONS: &str = "Waiting for the owner";
const WISHES: &str = "Wishes, not yet sorted";

impl Item {
    fn of(queued: &crate::db::Queued) -> Item {
        let head = headline(&queued.body);
        let more = queued.body.trim().chars().count().saturating_sub(head.chars().count());
        Item {
            id: queued.id,
            head,
            more,
            asked_by: queued.asked_by.clone(),
            due: queued.due.clone(),
        }
    }
}

/// Puts the owner's queue into the packet within its share, and returns
/// what it spent.
///
/// Questions come first, in the order they were asked: a session that does
/// not know a question is open may settle it by itself. Then the wishes a
/// neighbour placed, because somebody is waiting on each. Then the rest of
/// the wishes, newest first - the newest is the tail of the work just done,
/// and the oldest has waited this long already. The first line that does
/// not fit ends the queue, so that what is left out is always the end of
/// that order, and the packet can say so in a few words.
///
/// Each list is printed in the order it was recorded, whatever order paid
/// for it.
fn fill_queue(packet: &mut Packet, queue: Vec<crate::db::Queued>, share: usize) -> usize {
    let mut left = share;
    // A heading is paid for once, by the list under it.
    let has = |kind: &str| queue.iter().any(|q| q.kind == kind);
    if has("question") {
        left = left.saturating_sub(estimate_tokens(&format!("\n## {QUESTIONS}\n")));
    }
    if has("wish") {
        left = left.saturating_sub(estimate_tokens(&format!("\n## {WISHES}\n")));
    }

    let at = |kind: &'static str, asked: bool| {
        queue
            .iter()
            .enumerate()
            .filter(move |(_, q)| q.kind == kind && q.asked_by.is_some() == asked)
            .map(|(i, _)| i)
    };
    let order: Vec<usize> = at("question", false).chain(at("wish", true)).chain(at("wish", false).rev()).collect();

    let mut kept = vec![false; queue.len()];
    let mut full = false;
    for i in order {
        let queued = &queue[i];
        let item = Item::of(queued);
        let cost = estimate_tokens(&render_item(&item));
        if !full && cost <= left {
            left -= cost;
            kept[i] = true;
            continue;
        }
        full = true;
        match queued.kind.as_str() {
            "question" => packet.questions_omitted += 1,
            _ => packet.wishes_omitted += 1,
        }
        packet.dropped.push(Dropped {
            id: item.id,
            kind: queued.kind.clone(),
            date: queued.date.clone(),
            head: item.head,
            tokens: cost,
            why: "the queue's share of the budget is spent",
        });
    }
    for (queued, kept) in queue.iter().zip(kept) {
        if kept {
            match queued.kind.as_str() {
                "question" => packet.questions.push(Item::of(queued)),
                _ => packet.wishes.push(Item::of(queued)),
            }
        }
    }
    share - left
}

/// The line a question or a wish is known by: its bold heading when it
/// opens with one, else its first sentence, cut to what a list can carry.
///
/// A heading that only announces what follows - `**Вопрос:**` - names
/// nothing, so a heading ending in a colon gives way to the sentence it
/// opens.
fn headline(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first = first.trim_start_matches('#').trim_start();
    if let Some(rest) = first.strip_prefix("**")
        && let Some(end) = rest.find("**")
    {
        let heading = rest[..end].trim();
        if !heading.is_empty() && !heading.ends_with(':') {
            return shorten(heading, HEAD_CHARS);
        }
    }
    shorten(&first_sentence(&first.replace("**", "")), HEAD_CHARS)
}

/// What the packet says when the budget left questions out. They are the
/// owner's and the inbox lists every one, so it points there.
fn questions_footer(omitted: usize, project: &str) -> String {
    let word = if omitted == 1 { "question" } else { "questions" };
    format!("({omitted} more {word} left out by the budget; `rigger inbox --project {project}` lists them all)\n")
}

fn wishes_footer(omitted: usize) -> String {
    let word = if omitted == 1 { "wish" } else { "wishes" };
    format!("({omitted} more {word} left out by the budget; `--explain` names them)\n")
}

/// The line that says how to read whole what the packet only names.
fn read_whole(project: &str) -> String {
    format!("\n(Any [id] above reads whole with `rigger event {project} <id>`.)\n")
}

/// What the packet says about the events it did not print. Said even when
/// nothing fit at all: a section that is simply absent reads as "nothing
/// happened", which is the opposite of the truth.
fn events_footer(dropped: usize, beyond: usize) -> String {
    match (dropped, beyond) {
        (0, 0) => String::new(),
        (0, beyond) => format!("({beyond} older events are beyond the window the packet looks at; `rigger find` reaches them)\n"),
        (dropped, 0) => format!("({dropped} events left out by the budget; `--explain` names them)\n"),
        (dropped, beyond) => {
            format!("({dropped} events left out by the budget - `--explain` names them - and {beyond} more are beyond the window)\n")
        }
    }
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
            tokens: estimate_tokens(&render_questions(packet)),
        },
        Cost {
            section: "wishes",
            tokens: estimate_tokens(&render_wishes(packet)),
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

fn render_state(p: &Packet) -> String {
    let mut out = format!("# {}\n\n{}\n", p.project, p.state.path);
    if let Some(remote) = &p.state.remote {
        out.push_str(remote);
        out.push('\n');
    }
    match (&p.state.last_shipped, &p.state.last_shipped_on) {
        (Some(v), Some(on)) => match &p.state.delivery_short {
            Some(short) => out.push_str(&format!("Last shipped: {v} on {on} - {short}\n")),
            None => out.push_str(&format!("Last shipped: {v} on {on}\n")),
        },
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
    for line in put_down_lines(&stage.asleep, &stage.set_aside) {
        out.push_str(&format!("({line})\n"));
    }
    out
}

/// One line of the queue: the id, the day it is due, who asked, the line it
/// is known by, and how much more there is.
fn render_item(item: &Item) -> String {
    // The day goes in front of the text, where a long question cannot push
    // it out of sight - and says "overdue" in words, because the packet is
    // read by a model that sees no colour.
    let due = match &item.due {
        Some(day) if crate::db::is_past(day, &crate::db::today()) => format!("(overdue since {day}) "),
        Some(day) => format!("(due {day}) "),
        None => String::new(),
    };
    let who = item.asked_by.as_deref().map(|who| format!("({who} asks) ")).unwrap_or_default();
    let more = match item.more > POINTER_AT {
        true => format!(" (+{} chars)", item.more),
        false => String::new(),
    };
    format!("- [{}] {due}{who}{}{more}\n", item.id, item.head)
}

/// A list of the queue under its heading, with what the budget left out
/// counted at its foot. Absent only when there is nothing either way.
fn render_list(heading: &str, items: &[Item], footer: Option<String>) -> String {
    if items.is_empty() && footer.is_none() {
        return String::new();
    }
    let mut out = format!("\n## {heading}\n");
    for item in items {
        out.push_str(&render_item(item));
    }
    out.push_str(&footer.unwrap_or_default());
    out
}

fn render_questions(packet: &Packet) -> String {
    let footer = (packet.questions_omitted > 0).then(|| questions_footer(packet.questions_omitted, &packet.project));
    render_list(QUESTIONS, &packet.questions, footer)
}

fn render_wishes(packet: &Packet) -> String {
    let footer = (packet.wishes_omitted > 0).then(|| wishes_footer(packet.wishes_omitted));
    render_list(WISHES, &packet.wishes, footer)
}

fn render_event(event: &Event) -> String {
    format!("- [{}] {} · {} · {}\n", event.id, event.date, event.kind, event.body)
}

/// The packet as the assistant reads it.
pub fn render(packet: &Packet) -> String {
    let mut out = render_state(packet);
    if let Some(stage) = &packet.current {
        out.push_str(&render_stage(stage));
    }
    out.push_str(&render_questions(packet));
    out.push_str(&render_wishes(packet));
    if !packet.events.is_empty() || packet.events_omitted > 0 {
        out.push_str("\n## Recent\n");
        for event in &packet.events {
            out.push_str(&render_event(event));
        }
        out.push_str(&events_footer(packet.events_omitted, packet.events_beyond_window));
    }
    // Said once, under the last list that carries ids: a line that names
    // a text is only as good as the way to read it.
    if !packet.questions.is_empty() || !packet.wishes.is_empty() || !packet.events.is_empty() {
        out.push_str(&read_whole(&packet.project));
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

/// One event whole, by the id a packet printed.
///
/// Refused when the id is another project's: ids are counted across the
/// whole record, and a number copied from the wrong packet should read as a
/// mistake rather than as somebody else's text taken for this project's.
pub fn read(db: &Db, project: &Project, id: i64) -> Result<EventWritten> {
    let Some(event) = db.event_written(id)? else {
        bail!("there is no [{id}] in the record; the packet lists the ids");
    };
    if event.project != project.name {
        bail!(
            "[{id}] belongs to {}, not to {}; `rigger event {} {id}` reads it",
            event.project,
            project.name,
            event.project
        );
    }
    Ok(event)
}

/// An event whole, as `rigger event` prints it: what it is and whose on the
/// first line, what else the record knows about it under that, then the
/// text as it was written.
pub fn render_whole(event: &EventWritten) -> String {
    let day = event.created_at.split('T').next().unwrap_or(&event.created_at);
    let mut out = format!("[{}] {} · {} · {day}\n", event.id, event.kind, event.project);
    if let Some(card) = &event.card {
        out.push_str(&format!("on the card {card}\n"));
    }
    if let Some(who) = &event.asked_by {
        out.push_str(&format!("asked by {who}\n"));
    }
    if let Some(due) = &event.due {
        out.push_str(&format!("due {due}\n"));
    }
    if let Some(principle) = &event.principle {
        out.push_str(&format!("on the principle {principle:?}\n"));
    }
    out.push('\n');
    out.push_str(event.body.trim_end());
    out.push('\n');
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
        out.push_str(&format!("{}  {:<9} {:>4}t  [{}] {} — {}\n", d.date, d.kind, d.tokens, d.id, d.head, d.why));
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
    fn a_wish_is_known_by_its_bold_heading() {
        let wish = "**05.09.2026 · Read ODS too** (from the review)\n\nSome of the sheets arrive from LibreOffice.";
        assert_eq!(headline(wish), "05.09.2026 · Read ODS too");
    }

    #[test]
    fn a_heading_that_only_announces_gives_way_to_its_sentence() {
        let question = "**Вопрос:** какой формат первым? Остальное потом.";
        assert_eq!(headline(question), "Вопрос: какой формат первым?");
    }

    #[test]
    fn without_a_heading_the_first_sentence_names_it() {
        let wish = "`rigger why <task>` - only versions today. A task has no bounds in time.\n\nMore detail.";
        assert_eq!(headline(wish), "`rigger why <task>` - only versions today.");
        assert_eq!(headline("# A title line\n\nbody"), "A title line");
    }

    #[test]
    fn a_long_first_sentence_is_cut_to_what_a_list_carries() {
        let wish = "word ".repeat(100);
        let head = headline(&wish);
        assert!(head.chars().count() <= HEAD_CHARS + 1, "{head}");
        assert!(head.ends_with('…'), "{head}");
    }

    #[test]
    fn a_line_points_to_the_rest_only_when_there_is_much_of_it() {
        let item = |more| Item {
            id: 7,
            head: "Read ODS too".to_string(),
            more,
            asked_by: None,
            due: None,
        };
        assert_eq!(render_item(&item(12)), "- [7] Read ODS too\n");
        assert_eq!(render_item(&item(1200)), "- [7] Read ODS too (+1200 chars)\n");
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
