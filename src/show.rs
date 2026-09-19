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

use crate::db::{Db, DocLine, Project, Task};

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

    #[test]
    fn a_long_title_is_cut_and_says_it_was() {
        let long = "я".repeat(200);
        assert_eq!(trim_to(&long, 34).chars().count(), 34);
        assert!(trim_to(&long, 34).ends_with('…'));
    }
}
