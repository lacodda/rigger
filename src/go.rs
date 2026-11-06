//! One command for a whole turn: which project, its stage, and the assistant
//! started on it.
//!
//! The record has known what comes next for a while, and sessions could be
//! started since v0.4, but the owner still looked at GitHub for what was
//! touched longest ago and called a project's skill by hand: a choice printed
//! on one screen and a session started from another is two steps, and two
//! steps is a routine that lapses. `go` is both, and it holds the line's rule
//! as it does: a project left unfinished comes back before anything new
//! starts, and only `--force` walks past it.
//!
//! The rule itself lives in `queue`, so `go`, `rigger queue` and the MCP tool
//! cannot disagree about what comes next. This module only decides what the answer
//! means for a command about to start something, and what to say before and
//! after.

use serde::Serialize;

use crate::db::Kind;
use crate::queue::{Candidate, Queue, Reason, Unfinished};

/// What `go` will start.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub project: String,
    pub kind: Kind,
    /// The stage to take to a release; `None` for a project named outright
    /// that has none planned.
    pub version: Option<String>,
    pub title: Option<String>,
    /// Why it comes now; `None` for a project named outright that the rule
    /// leaves out.
    pub reason: Option<Reason>,
    pub unfinished: Option<Unfinished>,
    /// Something asked for it to come before its turn, whatever else holds.
    #[serde(skip)]
    pub moved: bool,
    /// The sentence that says why, for the screen.
    #[serde(skip)]
    pub why: Option<String>,
}

impl Target {
    fn of(c: &Candidate) -> Target {
        Target {
            project: c.project.clone(),
            kind: c.kind,
            version: (c.version != "unplanned").then(|| c.version.clone()),
            title: c.title.clone(),
            reason: Some(c.reason),
            unfinished: c.unfinished.clone(),
            moved: c.moved.is_some(),
            why: Some(crate::queue::why(c)),
        }
    }
}

/// What the pick means for a command about to start something.
#[derive(Debug, Clone)]
pub enum Choice {
    Start(Target),
    /// The next project has a sitting open, and the record cannot tell a
    /// live one from an abandoned one. A second assistant in a working tree
    /// another is writing to is the worse of the two mistakes, so `go` says
    /// so and starts nothing.
    Busy {
        project: String,
        since: String,
    },
    /// Nothing to start, and why.
    Nothing(String),
}

/// Reads the pick for one run of `go`.
///
/// A project named outright is the owner's own word: it is started whatever
/// the rule says of it, and joins a sitting that is open. `force` passes over
/// every project left unfinished, and only those - the order of the rest is
/// the rule's.
pub fn choose(queue: &Queue, named: Option<&str>, force: bool) -> Choice {
    let order = || queue.order.iter();
    if let Some(name) = named {
        if let Some(found) = order().find(|c| c.project == name) {
            return Choice::Start(Target::of(found));
        }
        return Choice::Start(Target {
            project: name.to_string(),
            kind: Kind::Repo,
            version: None,
            title: None,
            reason: None,
            unfinished: None,
            moved: false,
            why: queue.left_out.iter().find(|l| l.project == name).map(|l| match (&l.blocker, l.why) {
                (Some(blocker), _) => format!("blocked since {}: {}", blocker.since, blocker.head),
                (None, _) => "left out of the order".to_string(),
            }),
        });
    }

    let first = match force {
        true => order().find(|c| c.unfinished.is_none()),
        false => order().next(),
    };
    let Some(first) = first else {
        return Choice::Nothing(match (force, !queue.order.is_empty()) {
            (true, true) => "Nothing to go on to: every project in the order is unfinished.".to_string(),
            _ if queue.left_out.is_empty() => "Nothing to work on: no project is in the work.".to_string(),
            _ => "Nothing to work on: every project is left out - `rigger queue` says why.".to_string(),
        });
    };
    if let Some(since) = first.unfinished.as_ref().and_then(|u| u.sitting.as_ref()) {
        return Choice::Busy {
            project: first.project.clone(),
            since: since.clone(),
        };
    }
    Choice::Start(Target::of(first))
}

/// The projects `--force` walks past to reach `target`.
pub fn passed_over(queue: &Queue, target: &Target) -> Vec<String> {
    queue
        .order
        .iter()
        .take_while(|c| c.project != target.project)
        .map(|c| c.project.clone())
        .collect()
}

/// The first message: one line, on purpose.
///
/// An assistant installed by npm is a batch file on Windows. A batch file
/// cannot be handed a line break in an argument, and Claude Code refuses to
/// start at all with its input piped, so the packet itself cannot travel
/// either way. It does not need to: the assistant reads it with its first
/// command, fresh, and this line only has to say what the sitting is for.
/// Commit subjects and stage titles stay out of it for the same reason: they
/// hold whatever their authors typed, and this passes through `cmd`.
pub fn first_message(target: &Target, tail: Option<&str>) -> String {
    let project = &target.project;
    let order = match (&target.version, target.kind) {
        // A folder has no tags: what a turn there comes to is for its rules to say.
        (_, Kind::Folder) => format!("Work on {project}: one turn, one finished piece of work, as its rules say, without stopping to ask."),
        (Some(version), _) => format!("Work on {project}: take {version} to a release - one sitting, one tag, without stopping to ask."),
        (None, _) => format!("Work on {project}: no stage is planned, so plan one and take it to a release - one sitting, one tag."),
    };
    let unfinished = match target.unfinished.as_ref().map(Unfinished::parts) {
        Some(parts) if !parts.is_empty() => format!(" It was left unfinished ({}): finish that first.", parts.join(", ")),
        _ => String::new(),
    };
    // What moved it is in the owner's or a neighbour's words, which stay out of
    // this line, and `rigger queue` says them.
    let moved = match target.moved {
        true => " It comes before its turn: `rigger queue` says why, and that comes first in this stage.",
        false => "",
    };
    let tail = tail.unwrap_or_default();
    format!(
        "{order}{unfinished}{moved}{tail} Read `rigger context {project}` and `rigger rules {project}` first: they give the stage, the gate and how the work is done here. \
         If something out of your reach stops the tag, record it with `rigger note {project} --kind blocker` and stop; the queue goes on without it."
    )
}

/// What the screen says before the assistant starts.
pub fn before(target: &Target, program: &str, dir: &str) -> String {
    let stage = match (&target.version, &target.title) {
        (Some(version), Some(title)) => format!("  {version} · {title}"),
        (Some(version), None) => format!("  {version}"),
        (None, _) => String::new(),
    };
    let why = target.why.as_deref().map(|w| format!("  {w}\n")).unwrap_or_default();
    format!("Work on {}{stage}\n{why}Starting {program} in {dir}\n", target.project)
}

/// What the screen says after it stopped: how the turn went, and what comes
/// next. Starting that is another `go`: the owner sees each turn end.
pub fn after(target: &Target, shipped: Option<&str>, queue: &Queue) -> String {
    let project = target.project.as_str();
    let mut out = String::from("\n");
    let order = || queue.order.iter();
    if let Some(version) = shipped {
        out.push_str(&format!("{project} {version} went out.\n"));
    }
    if let Some(left) = order().find(|c| c.project == project && c.unfinished.is_some()) {
        out.push_str(&format!(
            "{project} is not finished: {}. `rigger go` comes back to it; `rigger go --force` goes past it.\n",
            crate::queue::unfinished_parts(left)
        ));
    } else if let Some(blocked) = queue.left_out.iter().find(|l| l.project == project).and_then(|l| l.blocker.as_ref()) {
        out.push_str(&format!(
            "{project} is blocked: {}. The queue goes on without it; `rigger resolve {project} {}` puts it back.\n",
            blocked.head, blocked.id
        ));
    } else if shipped.is_none() {
        out.push_str(&format!("{project} has no new tag, and nothing left over: it is still its turn.\n"));
    }
    match order().next() {
        Some(next) if next.project == project => {}
        Some(next) => out.push_str(&format!("Next: {} {} - `rigger go`\n", next.project, next.version)),
        None => out.push_str("Nothing comes next: `rigger queue` says why.\n"),
    }
    out
}

/// What the screen says when the next project has a sitting open.
pub fn busy(project: &str, since: &str) -> String {
    format!("{project} has a sitting open since {since}. Finish it there, go back to it with `rigger go {project}`, or go past it with `rigger go --force`.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{LeftOut, Note};

    fn candidate(project: &str, unfinished: Option<Unfinished>) -> Candidate {
        Candidate {
            project: project.to_string(),
            kind: Kind::Repo,
            weight: 2,
            reason: if unfinished.is_some() { Reason::Unfinished } else { Reason::Turn },
            version: "v0.2.0".to_string(),
            title: Some("Second".to_string()),
            unfinished,
            moved: None,
            waited: 0,
            last_turn: None,
            waiting: 0,
            asked: 0,
            refactor: false,
            since: None,
            turn_at: None,
        }
    }

    fn pick_of(order: Vec<Candidate>, left_out: Vec<LeftOut>) -> Queue {
        Queue {
            order,
            left_out,
            parted: Vec::new(),
            unread: Vec::new(),
        }
    }

    fn dirty() -> Unfinished {
        Unfinished {
            changed: 2,
            ..Unfinished::default()
        }
    }

    fn started(choice: Choice) -> Target {
        match choice {
            Choice::Start(target) => target,
            other => panic!("expected a start, got {other:?}"),
        }
    }

    /// The owner's rule: while a tag is not out, the next project does not
    /// start. It comes back to the unfinished one, and only `--force` moves on.
    #[test]
    fn an_unfinished_project_comes_back_until_forced_past() {
        let pick = pick_of(
            vec![candidate("left", Some(dirty())), candidate("also", Some(dirty())), candidate("next", None)],
            vec![],
        );

        assert_eq!(started(choose(&pick, None, false)).project, "left");

        let forced = started(choose(&pick, None, true));
        assert_eq!(forced.project, "next");
        assert_eq!(passed_over(&pick, &forced), vec!["left", "also"]);
    }

    /// `--force` passes over what is unfinished, not over the order: with
    /// nothing unfinished it changes nothing.
    #[test]
    fn force_with_nothing_unfinished_takes_the_first() {
        let pick = pick_of(vec![candidate("first", None), candidate("second", None)], vec![]);
        assert_eq!(started(choose(&pick, None, true)).project, "first");
    }

    #[test]
    fn with_only_unfinished_projects_force_has_nowhere_to_go() {
        let pick = pick_of(vec![candidate("left", Some(dirty()))], vec![]);
        match choose(&pick, None, true) {
            Choice::Nothing(said) => assert!(said.contains("every project in the order is unfinished"), "{said}"),
            other => panic!("{other:?}"),
        }
    }

    /// A sitting left open may be someone at work: `go` starts nothing rather
    /// than put a second assistant in that tree - but the owner naming the
    /// project, or forcing past it, is the owner's word.
    #[test]
    fn an_open_sitting_is_not_walked_into() {
        let open = Unfinished {
            sitting: Some("2026-11-05T10:00:00Z".to_string()),
            ..Unfinished::default()
        };
        let pick = pick_of(vec![candidate("busy", Some(open)), candidate("next", None)], vec![]);

        match choose(&pick, None, false) {
            Choice::Busy { project, since } => {
                assert_eq!(project, "busy");
                assert!(busy(&project, &since).contains("rigger go --force"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(started(choose(&pick, Some("busy"), false)).project, "busy");
        assert_eq!(started(choose(&pick, None, true)).project, "next");
    }

    /// Named outright, a project the rule leaves out is still started: the
    /// way to work on a blocked one is to ask for it.
    #[test]
    fn a_project_named_outright_starts_though_left_out() {
        let pick = pick_of(
            vec![candidate("first", None)],
            vec![LeftOut {
                project: "bare".to_string(),
                why: crate::queue::Aside::Out,
                blocker: None,
            }],
        );
        let target = started(choose(&pick, Some("bare"), false));
        assert_eq!(target.project, "bare");
        assert_eq!(target.version, None);
        assert!(first_message(&target, None).contains("no stage is planned"), "{}", first_message(&target, None));
    }

    /// The whole of why this is one line: a line break cannot reach a batch
    /// file, and a quote or a percent sign is read by `cmd` on the way.
    /// What moved a project is told whatever else holds: a project both moved up
    /// and unfinished has to hear both, or what moved it is never done. The words
    /// stay out of the line itself, which passes through `cmd`.
    #[test]
    fn the_first_message_says_a_project_was_moved_up_whatever_else_holds() {
        let asked = Note {
            id: 1,
            head: "needed for Monday".to_string(),
            since: "2026-11-05".to_string(),
        };
        let mut moved = candidate("urgent", None);
        moved.reason = Reason::Moved;
        moved.moved = Some(asked.clone());
        let said = first_message(&Target::of(&moved), None);
        assert!(said.contains("It comes before its turn"), "{said}");
        assert!(!said.contains("needed for Monday"), "{said}");

        let mut both = candidate("urgent", Some(dirty()));
        both.moved = Some(asked);
        let said = first_message(&Target::of(&both), None);
        assert!(said.contains("It was left unfinished (2 changed files not committed)"), "{said}");
        assert!(said.contains("It comes before its turn"), "{said}");

        let said = first_message(&Target::of(&candidate("plain", None)), None);
        assert!(!said.contains("It comes before its turn"), "{said}");
    }

    #[test]
    fn the_first_message_can_pass_through_a_batch_file() {
        let target = started(choose(&pick_of(vec![candidate("left", Some(dirty()))], vec![]), None, false));
        let message = first_message(&target, None);
        assert!(!message.contains(['\n', '\r', '"', '%']), "{message}");
        assert!(message.starts_with("Work on left: take v0.2.0 to a release"), "{message}");
        assert!(
            message.contains("It was left unfinished (2 changed files not committed): finish that first."),
            "{message}"
        );
        assert!(message.contains("rigger context left"), "{message}");
        assert!(message.contains("--kind blocker"), "{message}");
        // Nothing from the title, which holds whatever its author typed.
        assert!(!message.contains("Second"), "{message}");
    }

    #[test]
    fn after_a_release_the_screen_names_what_comes_next() {
        let target = started(choose(&pick_of(vec![candidate("done", None)], vec![]), None, false));
        let pick = pick_of(vec![candidate("next", None), candidate("done", None)], vec![]);
        let said = after(&target, Some("v0.2.0"), &pick);
        assert!(said.contains("done v0.2.0 went out."), "{said}");
        assert!(said.contains("Next: next v0.2.0 - `rigger go`"), "{said}");
    }

    #[test]
    fn after_a_turn_left_unfinished_the_screen_says_it_comes_back() {
        let target = started(choose(&pick_of(vec![candidate("left", Some(dirty()))], vec![]), None, false));
        let pick = pick_of(vec![candidate("left", Some(dirty())), candidate("next", None)], vec![]);
        let said = after(&target, None, &pick);
        assert!(said.contains("left is not finished: 2 changed files not committed."), "{said}");
        assert!(said.contains("rigger go --force"), "{said}");
        // It is still what comes next, so nothing else is announced as next.
        assert!(!said.contains("Next:"), "{said}");
    }

    #[test]
    fn after_a_blocker_the_screen_says_how_it_comes_back() {
        let target = started(choose(&pick_of(vec![candidate("stuck", None)], vec![]), None, false));
        let pick = pick_of(
            vec![candidate("next", None)],
            vec![LeftOut {
                project: "stuck".to_string(),
                why: crate::queue::Aside::Blocked,
                blocker: Some(Note {
                    id: 9,
                    head: "No signing key".to_string(),
                    since: "2026-11-05".to_string(),
                }),
            }],
        );
        let said = after(&target, None, &pick);
        assert!(said.contains("stuck is blocked: No signing key."), "{said}");
        assert!(said.contains("rigger resolve stuck 9"), "{said}");
        assert!(said.contains("Next: next v0.2.0"), "{said}");
    }

    #[test]
    fn a_turn_that_changed_nothing_says_so() {
        let target = started(choose(&pick_of(vec![candidate("idle", None)], vec![]), None, false));
        let pick = pick_of(vec![candidate("next", None), candidate("idle", None)], vec![]);
        let said = after(&target, None, &pick);
        assert!(said.contains("idle has no new tag, and nothing left over: it is still its turn."), "{said}");
    }
}
