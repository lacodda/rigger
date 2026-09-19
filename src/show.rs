//! A project's screen: what it is, where it stands, what it has written.
//!
//! The hub's README was the surface a person read to remember a project -
//! and it is generated from the record, lives in a notes vault, and is
//! opened in another application. Everything on it the record already
//! holds, so it can be printed where the work happens instead.
//!
//! This is not the context packet. The packet is written for an assistant
//! and spends its budget on recent events, because that is what a session
//! has to catch up on. A person opening a project wants the opposite: what
//! the thing is, what state it is in, what has been written down about it,
//! and the one line saying what is next.

use std::fmt::Write;

use anyhow::Result;
use serde::Serialize;

use crate::db::{Asked, Db, DocLine, Project, Task};
use crate::link;

#[derive(Debug, Serialize)]
pub struct Screen {
    pub name: String,
    pub about: Option<String>,
    pub path: String,
    pub remote: Option<String>,
    pub tier: Option<String>,
    pub rhythm_weeks: Option<u32>,
    pub gate: Option<String>,
    pub on_session_end: Option<String>,
    pub hub_path: Option<String>,
    pub last_shipped: Option<(String, String)>,
    pub versions_planned: u64,
    pub tasks_open: u64,
    pub commits_since_tag: Option<u32>,
    pub stage: Option<Stage>,
    pub documents: Vec<DocLine>,
    pub questions: usize,
    pub wishes: usize,
    /// What this project is tied to, read from its own side.
    pub links: Vec<link::Link>,
    /// The open wishes a neighbour asked for.
    pub asked: Vec<Asked>,
    pub next_step: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    pub tasks: Vec<Task>,
}

pub fn build(db: &Db, project: &Project, about: Option<String>) -> Result<Screen> {
    let activity = db.activity(project.id)?;
    let stage = db.current_stage(project.id)?.map(|s| Stage {
        version: s.version,
        title: s.title,
        tasks: s.tasks,
    });
    Ok(Screen {
        name: project.name.clone(),
        about,
        path: project.path.clone(),
        remote: project.remote.clone(),
        tier: project.tier.clone(),
        rhythm_weeks: project.rhythm_weeks,
        gate: project.gate.clone(),
        on_session_end: project.on_session_end.clone(),
        hub_path: project.hub_path.clone(),
        last_shipped: db.last_shipped_version(project.id)?,
        versions_planned: db.count_versions(project.id, "planned")?,
        tasks_open: db.count_open_tasks(project.id)?,
        commits_since_tag: activity.as_ref().map(|a| a.commits_since_tag),
        stage,
        documents: db.document_lines(project.id)?,
        questions: db.open_events(project.id, "question")?.len(),
        wishes: db.open_events(project.id, "wish")?.len(),
        links: db.links_of(project.id)?,
        asked: db.asked_wishes(project.id)?,
        next_step: db.latest_event_body(project.id, "next")?,
    })
}

pub fn render(screen: &Screen) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", screen.name);
    if let Some(about) = &screen.about {
        let _ = writeln!(out, "{about}");
    }
    out.push('\n');

    let _ = writeln!(out, "{:<10} {}", "path", screen.path);
    if let Some(remote) = &screen.remote {
        let _ = writeln!(out, "{:<10} {remote}", "remote");
    }
    if let Some(hub) = &screen.hub_path {
        let _ = writeln!(out, "{:<10} {hub}", "hub");
    }
    if let Some(tier) = &screen.tier {
        let rhythm = match screen.rhythm_weeks {
            Some(1) => " · a version a week".to_string(),
            Some(n) => format!(" · a version every {n} weeks"),
            None => String::new(),
        };
        let _ = writeln!(out, "{:<10} {tier}{rhythm}", "tier");
    }
    if let Some(gate) = &screen.gate {
        let _ = writeln!(out, "{:<10} {gate}", "gate");
    }
    if let Some(hook) = &screen.on_session_end {
        let _ = writeln!(out, "{:<10} {hook}", "on end");
    }

    out.push('\n');
    match &screen.last_shipped {
        Some((version, day)) => {
            let _ = write!(out, "Last shipped {version} on {day}");
            match screen.commits_since_tag {
                Some(0) | None => out.push('\n'),
                Some(1) => out.push_str(", 1 commit since\n"),
                Some(n) => {
                    let _ = writeln!(out, ", {n} commits since");
                }
            }
        }
        None => out.push_str("Nothing shipped yet\n"),
    }
    let _ = writeln!(out, "{} versions planned, {} tasks open", screen.versions_planned, screen.tasks_open);
    if screen.questions > 0 || screen.wishes > 0 {
        let _ = writeln!(out, "{} waiting on you, {} wishes unsorted", screen.questions, screen.wishes);
    }

    if let Some(stage) = &screen.stage {
        out.push('\n');
        let _ = write!(out, "Current stage: {}", stage.version);
        if let Some(title) = &stage.title {
            let _ = write!(out, " · {title}");
        }
        out.push('\n');
        for task in &stage.tasks {
            // The first line of a task, because a task of these plans is a
            // paragraph with the point in its opening clause.
            let first = task.title.lines().next().unwrap_or("").trim();
            let _ = writeln!(out, "  {} {}", mark(&task.status), trim_to(first, 96));
        }
    }

    let neighbours = render_neighbours(screen);
    if !neighbours.is_empty() {
        out.push('\n');
        out.push_str(&neighbours);
    }

    if !screen.documents.is_empty() {
        out.push('\n');
        out.push_str("Written down:\n");
        for doc in &screen.documents {
            let _ = writeln!(
                out,
                "  {:<9} {:<34} {}  rigger doc show {} {}",
                doc.kind,
                trim_to(&doc.title, 34),
                day(&doc.updated_at),
                screen.name,
                doc.slug
            );
        }
    }

    if let Some(next) = &screen.next_step {
        out.push('\n');
        out.push_str("Next step\n");
        let _ = writeln!(out, "  {}", trim_to(next.lines().next().unwrap_or("").trim(), 96));
    }
    out
}

/// Who this project stands beside, and who is waiting on it.
///
/// This is the half of a project's README that used to be prose - "kasl and
/// kasl-server are two halves of one thing", "the HTTPS door rhapsod built
/// is what hilvan needs" - written once in each of two hubs and true in
/// neither after a month. Printed from the links instead, so it is as
/// current as the last thing recorded.
///
/// The three kinds get three headings because they are three different
/// facts. Folding them into one list of neighbours would say that lyrid
/// stands beside dowel the way kasl stands beside kasl-server, and the
/// whole point of the distinction is that it does not.
fn render_neighbours(screen: &Screen) -> String {
    let mut out = String::new();
    let heading = |out: &mut String, text: &str| {
        let _ = writeln!(out, "{text}:");
    };

    for (kind, title) in [
        (link::Kind::Pair, "Paired with"),
        (link::Kind::Consumer, "Draws on"),
        (link::Kind::Donor, "Drawn on by"),
    ] {
        let of_kind: Vec<&link::Link> = screen.links.iter().filter(|l| l.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        heading(&mut out, title);
        for found in of_kind {
            let far = match &found.far.version {
                Some(version) => format!("{} {version}", found.far.project),
                None => found.far.project.clone(),
            };
            // The near version only when there is one: the anchor is what
            // the pair was agreed at, and a blank there reads as a missing
            // version rather than as a tie that has none.
            let near = match &found.near.version {
                Some(version) => format!(" (here: {version})"),
                None => String::new(),
            };
            let note = found.note.as_deref().map(|n| format!(" — {n}")).unwrap_or_default();
            let _ = writeln!(out, "  {far}{near}{note}");
        }
    }

    if !screen.asked.is_empty() {
        heading(&mut out, "Neighbours are asking for");
        for wish in &screen.asked {
            let first = wish.body.lines().next().unwrap_or("").trim();
            let _ = writeln!(out, "  [{}] {} — {}", wish.id, trim_to(first, 72), wish.asked_by);
        }
    }
    out
}

/// A task's status as one character, so the list reads as a column.
fn mark(status: &str) -> char {
    match status {
        "done" => 'x',
        "dropped" => '-',
        "active" => '>',
        "frozen" | "waiting-handoff" => '~',
        _ => ' ',
    }
}

/// The day of a timestamp, whichever of the two forms it is in.
fn day(stamp: &str) -> &str {
    stamp.split('T').next().unwrap_or(stamp)
}

fn trim_to(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen {
            name: "sample".into(),
            about: Some("A sample product".into()),
            path: "C:\\dev\\sample".into(),
            remote: None,
            tier: Some("C".into()),
            rhythm_weeks: Some(1),
            gate: Some("cargo test".into()),
            on_session_end: None,
            hub_path: None,
            last_shipped: Some(("v0.2.0".into(), "2026-09-10".into())),
            versions_planned: 3,
            tasks_open: 4,
            commits_since_tag: Some(2),
            stage: None,
            documents: Vec::new(),
            questions: 0,
            wishes: 0,
            links: Vec::new(),
            asked: Vec::new(),
            next_step: None,
        }
    }

    #[test]
    fn the_screen_opens_with_what_the_project_is() {
        let text = render(&screen());
        assert!(text.starts_with("sample\nA sample product\n"), "{text}");
        assert!(text.contains("Last shipped v0.2.0 on 2026-09-10, 2 commits since"), "{text}");
        assert!(text.contains("cargo test"), "the gate is on the screen: {text}");
    }

    #[test]
    fn a_project_that_has_shipped_nothing_says_so_rather_than_nothing() {
        let mut s = screen();
        s.last_shipped = None;
        s.commits_since_tag = None;
        let text = render(&s);
        assert!(text.contains("Nothing shipped yet"), "{text}");
    }

    #[test]
    fn one_commit_since_the_tag_is_not_pluralised() {
        let mut s = screen();
        s.commits_since_tag = Some(1);
        assert!(render(&s).contains("1 commit since"), "{}", render(&s));
        // And a tag with nothing after it says nothing at all, rather than
        // "0 commits since", which reads as news when it is the opposite.
        s.commits_since_tag = Some(0);
        assert!(!render(&s).contains("commit"), "{}", render(&s));
    }

    #[test]
    fn a_document_carries_the_command_that_opens_it() {
        let mut s = screen();
        s.documents = vec![DocLine {
            kind: "vision".into(),
            slug: "vision".into(),
            title: "Vision of sample".into(),
            updated_at: "2026-09-12T08:00:00Z".into(),
        }];
        let text = render(&s);
        assert!(text.contains("rigger doc show sample vision"), "{text}");
        // The day, not the timestamp: a screen is read, not parsed.
        assert!(text.contains("2026-09-12") && !text.contains("08:00:00"), "{text}");
    }

    fn link_of(kind: link::Kind, far: &str, far_version: Option<&str>, near_version: Option<&str>) -> link::Link {
        link::Link {
            id: 1,
            kind,
            near: link::End {
                project: "sample".into(),
                version: near_version.map(String::from),
                shipped: near_version.map(|_| false),
            },
            far: link::End {
                project: far.into(),
                version: far_version.map(String::from),
                shipped: far_version.map(|_| false),
            },
            note: None,
        }
    }

    /// The three kinds are three facts, so they get three headings: a
    /// project that draws on another does not stand beside it.
    #[test]
    fn the_neighbours_are_told_apart_by_what_the_tie_is() {
        let mut s = screen();
        s.links = vec![
            link_of(link::Kind::Pair, "sample-server", Some("v0.3.0"), Some("v0.9.0")),
            link_of(link::Kind::Consumer, "widgets", None, None),
            link_of(link::Kind::Donor, "downstream", None, None),
        ];
        let text = render(&s);
        assert!(text.contains("Paired with:\n  sample-server v0.3.0 (here: v0.9.0)"), "{text}");
        assert!(text.contains("Draws on:\n  widgets\n"), "{text}");
        assert!(text.contains("Drawn on by:\n  downstream\n"), "{text}");
    }

    /// A project tied to nothing prints no heading at all, rather than an
    /// empty one that reads as a thing gone missing.
    #[test]
    fn a_project_with_no_ties_says_nothing_about_them() {
        let text = render(&screen());
        assert!(!text.contains("Paired with"), "{text}");
        assert!(!text.contains("Neighbours are asking"), "{text}");
    }

    /// A neighbour's wish carries the id that resolves it and the name of
    /// who asked: an order with no sender is one nobody can answer.
    #[test]
    fn a_wish_from_a_neighbour_names_the_neighbour_and_its_id() {
        let mut s = screen();
        s.asked = vec![Asked {
            id: 42,
            body: "A gate for the docs address".into(),
            asked_by: "lyrn".into(),
        }];
        let text = render(&s);
        assert!(text.contains("Neighbours are asking for:"), "{text}");
        assert!(text.contains("[42] A gate for the docs address — lyrn"), "{text}");
    }

    #[test]
    fn a_long_title_is_cut_and_says_it_was() {
        let long = "я".repeat(200);
        assert_eq!(trim_to(&long, 34).chars().count(), 34);
        assert!(trim_to(&long, 34).ends_with('…'));
    }
}
