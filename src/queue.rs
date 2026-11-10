//! What comes next: every project in the work, in the order they get a turn.
//!
//! A line of products goes round. Each project has a weight, the projects with
//! more of it come round more often, and nothing here is tied to a date: a
//! place in the queue is all there is. The order is worked out from the record
//! each time rather than kept as a list, so there is no second copy to fall out
//! of step: change a weight and the order has changed, ship a version by hand
//! and that was its turn.
//!
//! The rule is an order of reasons, and a project's first reason that holds is
//! its reason:
//!
//! 1. `unfinished` - a turn begun and not over: work uncommitted or unpushed, a
//!    feature or a fix since the last release, a release that did not get out,
//!    a sitting left open. The line does not move on past it.
//! 2. `moved` - something asked for it to come first, and it has not shipped
//!    since. The owner's word, and a neighbour's stage that cannot go on
//!    without it, move it at once. What a neighbour did that it has to take up
//!    - a release of what it builds against that broke it, an order of its own
//!    that was built - moves it once it has waited its share, below.
//! 3. `turn` - nothing presses, so it goes by waiting and weight: the highest
//!    `(waited + 1) * weight` is next, where `waited` is how many releases the
//!    line has made since this project's last one.
//!
//! Counting releases rather than days is what keeps the queue free of dates: a
//! week off changes nothing about whose turn it is. And that formula gives each
//! project exactly its weight's share of the turns, spaced evenly; without the
//! `+ 1` the lightest projects get more than their share.
//!
//! A neighbour's need does not buy a turn. A project has waited its share once
//! `(waited + 1) * weight` reaches the weight of the whole order - the point at
//! which the formula's turn comes round - and only from then does what its
//! neighbours did put it ahead of the projects in their turn, however long
//! those have waited. Moved any sooner, it would get one more turn than its
//! weight gives it each time a neighbour broke something, and two projects
//! that use each other would hand the line back and forth for good. Until then
//! it stands in its turn, and what will move it is named on its line.
//!
//! What cannot be finished from here is not unfinished: a project whose tag is
//! stopped by something out of reach - a missing key, a red pipeline, a broken
//! service - says so with a blocker, and is left out of the order with that
//! reason until the blocker is cleared or a release settles it. Without that,
//! the one project waiting on the outside world would stop every other.
//!
//! Questions waiting on the owner do not take a project out - by the line's
//! rules a question does not stop a stage - and neither do its neighbours'
//! orders. Both are counted on its line instead.

use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use crate::db::{Db, Kind};
use crate::plural;

/// Why a project comes where it does, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    Unfinished,
    Moved,
    Turn,
}

/// What a project holds that has not gone out: the turn it is on is not over.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Unfinished {
    /// Tracked files changed and not committed.
    pub changed: usize,
    /// Files git does not know, and does not ignore.
    pub untracked: usize,
    /// Commits on no remote.
    pub unpushed: usize,
    /// An operation git started and has not finished: `merge`, `rebase`.
    pub operation: Option<String>,
    /// The subjects of the features and fixes since the last release. Docs
    /// and chores are not here: they wait for the next tag without holding
    /// the line up, and are never recorded as changes at all.
    pub untagged: Vec<String>,
    /// How the last release fell short of getting out, in a few words.
    pub undelivered: Option<String>,
    /// When the sitting left open began.
    pub sitting: Option<String>,
    /// Whether anyone is at that sitting: `true` while the `go` that opened
    /// it runs, `false` once it has gone, nothing where nothing held it.
    pub sitting_live: Option<bool>,
}

impl Unfinished {
    fn is_empty(&self) -> bool {
        *self == Unfinished::default()
    }

    /// What is unfinished, a few words for each thing.
    ///
    /// Counts rather than subjects: this reaches an assistant's command
    /// line, and a commit message may hold whatever its author typed.
    pub fn parts(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.changed > 0 {
            out.push(format!("{} not committed", plural(self.changed, "changed file", "changed files")));
        }
        if self.untracked > 0 {
            out.push(plural(self.untracked, "untracked file", "untracked files"));
        }
        if let Some(operation) = &self.operation {
            out.push(format!("a {operation} left halfway"));
        }
        if self.unpushed > 0 {
            out.push(format!("{} not pushed", plural(self.unpushed, "commit", "commits")));
        }
        if !self.untagged.is_empty() {
            out.push(format!("{} since the last release", plural(self.untagged.len(), "change", "changes")));
        }
        if let Some(undelivered) = &self.undelivered {
            out.push(undelivered.clone());
        }
        if let Some(sitting) = &self.sitting {
            let day = day_of(sitting);
            out.push(match self.sitting_live {
                Some(true) => format!("a sitting at work since {day}"),
                Some(false) => format!("a sitting left open since {day} by an assistant that has gone"),
                None => format!("a sitting open since {day}"),
            });
        }
        out
    }
}

/// An event that says something about a project's place: what stops its tag,
/// or what asked for it to come first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Note {
    /// The id `rigger resolve` closes it by.
    pub id: i64,
    pub head: String,
    pub since: String,
}

/// What one project brings to the queue.
#[derive(Debug, Clone)]
pub struct Standing {
    pub project: String,
    pub kind: Kind,
    pub weight: u32,
    pub stage: Option<Stage>,
    /// Changes since its last release, oldest first: moment and subject.
    pub untagged: Vec<(String, String)>,
    /// What git says the repository holds that has gone nowhere yet.
    pub held: crate::git::Held,
    /// How its last release fell short of getting out.
    pub undelivered: Option<String>,
    /// When the sitting left open began.
    pub sitting: Option<String>,
    /// Whether anyone is at it; see `Unfinished::sitting_live`.
    pub sitting_live: Option<bool>,
    /// When it last had a turn: its last release, or for a folder, which makes
    /// none, the end of its last sitting.
    pub last_turn: Option<String>,
    /// How many releases the line has made since then.
    pub waited: usize,
    /// Its next turn is a refactoring.
    pub refactor: bool,
    /// What stops its tag and is out of reach, oldest first.
    pub blockers: Vec<Note>,
    /// What the owner asked to come first, oldest first.
    pub moves: Vec<Note>,
    /// The orders it holds that a neighbour's stage waits on: that stage cannot
    /// go on without them, so they move it at once. Unlike the owner's word
    /// they do not bring back a project set aside on purpose.
    pub awaited: Vec<Note>,
    /// What its neighbours did that it has to take up, oldest first: a break
    /// in what it uses, an order of its own built. It moves the project once
    /// that has waited its share, and brings back nothing set aside either.
    pub pulled: Vec<Note>,
    pub waiting: usize,
    pub asked: usize,
}

/// The stage a project is on.
#[derive(Debug, Clone)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    /// Every open task of it is frozen or asleep: nobody is building it.
    pub put_down: bool,
}

/// One project in the order, with what put it there.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub project: String,
    pub kind: Kind,
    pub weight: u32,
    pub reason: Reason,
    /// The current stage; `unplanned` when there is none.
    pub version: String,
    pub title: Option<String>,
    /// What it holds that has not gone out, for `unfinished`.
    pub unfinished: Option<Unfinished>,
    /// Everything that asks for it to come first, while it stands, whatever
    /// the reason: the owner's word first, oldest first, then its neighbours',
    /// oldest first. A screen names the first and counts the rest, so that a
    /// note with nothing to do in it cannot hide a release that broke the
    /// build. What a neighbour did moves it only once it has waited its share,
    /// so a project in its turn can have something here.
    pub moved: Vec<Note>,
    /// How many releases the line has made since its last turn.
    pub waited: usize,
    /// The day of its last turn.
    pub last_turn: Option<String>,
    pub waiting: usize,
    pub asked: usize,
    /// Its next turn is a refactoring instead of a stage.
    pub refactor: bool,
    /// When the reason began to hold.
    #[serde(skip)]
    pub(crate) since: Option<String>,
    #[serde(skip)]
    pub(crate) turn_at: Option<String>,
    /// What asks for it puts it before its turn: the owner's word, a stage
    /// waiting on it, or what a neighbour did, once it has waited its share.
    /// Being unfinished does not hide it.
    #[serde(skip)]
    pub(crate) early: bool,
    /// How many releases it waits, at its weight, before what a neighbour did
    /// moves it: its share of the order.
    #[serde(skip)]
    pub(crate) share: usize,
}

/// Why a project is not in the order at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aside {
    /// Something out of reach stops its tag.
    Blocked,
    /// Its weight is 0: worked on when asked.
    Out,
    /// Everything planned was frozen or put to sleep on purpose.
    PutDown,
}

impl Aside {
    fn describe(self) -> &'static str {
        match self {
            Aside::Blocked => "blocked",
            Aside::Out => "weight 0 - worked on when asked",
            Aside::PutDown => "everything planned is put down",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LeftOut {
    pub project: String,
    pub why: Aside,
    /// What blocks it, for `blocked`.
    pub blocker: Option<Note>,
}

/// The whole answer: the order, and what it does not consider.
#[derive(Debug, Clone, Serialize)]
pub struct Queue {
    pub order: Vec<Candidate>,
    pub left_out: Vec<LeftOut>,
    /// Pairs whose halves have parted company: a warning, not a place in the
    /// order. The next turn of the half behind is what puts it right.
    pub parted: Vec<crate::link::Parted>,
    /// Projects whose git did not read: their facts are as of the last
    /// time it did.
    pub unread: Vec<String>,
}

/// The queue as it stands now: git read first, so that a commit made since the
/// last sitting counts, then the rule.
pub fn now(db: &Db) -> Result<Queue> {
    let unread = read_git(db)?;
    let mut queue = choose(&gather(db)?);
    queue.parted = crate::link::parted(&db.pairs()?);
    queue.unread = unread;
    Ok(queue)
}

/// Reads git for every repository in the work, as `sync` does and without
/// asking GitHub, so the order stands on today's commits rather than on the
/// last sitting's. A commit made in one project from a neighbour's sitting is
/// exactly what the record would otherwise not know. Returns the projects
/// whose git did not read.
pub fn read_git(db: &Db) -> Result<Vec<String>> {
    let mut unread = Vec::new();
    for project in db.projects()? {
        if !project.kind.reads_git() {
            continue;
        }
        if !crate::sync::sync(db, &project)?.warnings.is_empty() {
            unread.push(project.name);
        }
    }
    Ok(unread)
}

/// What a repository holds that has gone nowhere, and how its last release
/// fell short of getting out. Nothing, for one that does not read: `read_git`
/// has said so already, and what cannot be read cannot hold the line up.
fn held_by(project: &crate::db::Project, latest: Option<&crate::db::Delivered>) -> (crate::git::Held, Option<String>) {
    if !project.kind.reads_git() {
        return Default::default();
    }
    let Ok(repo) = crate::git::open(&project.path) else {
        return Default::default();
    };
    let held = crate::git::held(&repo).unwrap_or_default();
    let undelivered = latest.and_then(|latest| match latest.delivery {
        Some(state) if state.is_short() => Some(format!("{}: {}", latest.version, state.describe())),
        Some(_) => None,
        // No word on how far it got, so the one thing that can be asked
        // cheaply is: did the tag leave this machine at all?
        None => (crate::git::tag_on_remote(&repo, &latest.version) == Some(false)).then(|| format!("{}: the tag is not on the remote", latest.version)),
    });
    (held, undelivered)
}

/// When a project last had a turn: its last release, or for a folder, which
/// makes none, the end of its last sitting.
pub fn last_turn(db: &Db, project: &crate::db::Project) -> Result<Option<String>> {
    Ok(match project.kind {
        Kind::Folder => db.last_ended_session(project.id)?.and_then(|s| s.ended_at),
        _ => db.last_release_moment(project.id)?,
    })
}

/// The moment a project's last turn settled what was said before it: the end
/// of a folder's last sitting, and for a repository, when its last release got
/// out rather than when its commit was made (see `Db::last_out_moment`). The
/// owner asking for it first while its release was on its way was answered by
/// that release, as a blocker said then was.
pub fn settled_by(db: &Db, project: &crate::db::Project) -> Result<Option<String>> {
    Ok(match project.kind {
        Kind::Folder => db.last_ended_session(project.id)?.and_then(|s| s.ended_at),
        _ => db.last_out_moment(project.id)?,
    })
}

/// A weight after `up`, `down` or a number.
pub fn changed_weight(current: u32, change: &str) -> Result<u32> {
    match change {
        "up" => Ok(current + 1),
        "down" => Ok(current.saturating_sub(1)),
        number => number
            .parse()
            .map_err(|_| anyhow::anyhow!("'{number}' is not a change of weight: say `up`, `down`, or a whole number")),
    }
}

/// One project drawing on another: what `deps` reads from the manifests, and the
/// ties the record keeps for what no manifest says, such as a command's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Use {
    pub consumer: String,
    pub supplier: String,
    /// The package, or `None` for a tie the record keeps.
    pub package: Option<String>,
    /// The version the manifest asks for.
    pub requirement: Option<String>,
    /// Where the package's manifest is in the supplier's repository, from its
    /// root, `.` for the root; `None` for a tie the record keeps.
    pub directory: Option<String>,
    /// What the record says about the tie.
    pub note: Option<String>,
}

/// Who draws on whom across the work, each pair once, a manifest over a tie.
pub fn uses(db: &Db) -> Result<Vec<Use>> {
    let projects = db.projects()?;
    let repos: Vec<(String, std::path::PathBuf)> = projects
        .iter()
        .filter(|p| p.kind.reads_git())
        .map(|p| (p.name.clone(), std::path::PathBuf::from(&p.path)))
        .collect();
    let mut out: Vec<Use> = crate::deps::uses(&repos)
        .into_iter()
        .map(|u| Use {
            consumer: u.consumer,
            supplier: u.supplier,
            package: Some(u.package),
            requirement: Some(u.requirement),
            directory: Some(u.directory),
            note: None,
        })
        .collect();
    let working = |name: &str| projects.iter().any(|p| p.name == name);
    for project in &projects {
        for link in db.links_of(project.id)? {
            // Seen from both ends, each saying which it is: the consumer's end
            // is the one to read. A manifest says more about the same two.
            if link.kind != crate::link::Kind::Consumer || !working(&link.far.project) {
                continue;
            }
            if !out.iter().any(|u| u.consumer == link.near.project && u.supplier == link.far.project) {
                out.push(Use {
                    consumer: link.near.project.clone(),
                    supplier: link.far.project.clone(),
                    package: None,
                    requirement: None,
                    directory: None,
                    note: link.note.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.consumer
            .cmp(&b.consumer)
            .then_with(|| a.supplier.cmp(&b.supplier))
            .then_with(|| a.package.cmp(&b.package))
    });
    Ok(out)
}

/// One use, in a line.
pub fn use_line(item: &Use) -> String {
    let through = match (&item.package, &item.requirement, &item.note) {
        (Some(package), Some(requirement), _) => format!("{package} {requirement}"),
        (Some(package), None, _) => package.clone(),
        (None, _, Some(note)) => note.clone(),
        (None, _, None) => "a tie in the record".to_string(),
    };
    format!("{} uses {} - {through}", item.consumer, item.supplier)
}

/// What the breaking commits of the work changed, read once each however many
/// projects use their supplier: a design system's break is asked about by
/// every product that draws on it.
#[derive(Default)]
pub struct Touched {
    /// Every package's directory, by supplier.
    homes: HashMap<i64, std::collections::BTreeSet<String>>,
    /// The paths of a commit, by supplier and commit; `None` where they could
    /// not be read.
    paths: HashMap<(i64, String), Option<Vec<String>>>,
}

impl Touched {
    /// Whether a supplier's commit changed a package whose directory is in
    /// `drawn`. A commit that does not read reaches everything.
    fn reaches(&mut self, supplier: &crate::db::Project, commit: &str, drawn: &std::collections::BTreeSet<String>) -> bool {
        let homes = self
            .homes
            .entry(supplier.id)
            .or_insert_with(|| crate::deps::read(std::path::Path::new(&supplier.path)).homes);
        let paths = self
            .paths
            .entry((supplier.id, commit.to_string()))
            .or_insert_with(|| crate::git::open(&supplier.path).ok().and_then(|repo| crate::git::paths_of(&repo, commit).ok()));
        match paths {
            Some(paths) => crate::deps::reaches(homes, drawn, paths),
            None => true,
        }
    }
}

/// The orders a project holds that a neighbour's stage is waiting for. Resolving
/// the order settles it, whichever way.
fn awaited(db: &Db, project: &crate::db::Project) -> Result<Vec<Note>> {
    Ok(db
        .waited_on(project.id)?
        .into_iter()
        .map(|order| Note {
            id: order.id,
            head: format!("{} waits on it: {}", order.asked_by, crate::context::headline(&order.body)),
            since: String::new(),
        })
        .collect())
}

/// What a project's neighbours did that it has to take up: see the head of
/// this module. Worked out each time, so it needs nobody to remember to undo
/// it: the project's next release is what settles it.
///
/// A project that has never released has nothing to catch up with yet. An order
/// built and not yet released waits for the release, like the work it is.
///
/// A breaking release reaches a project through the packages it uses, so a
/// breaking commit moves it only where it changed one of them: `feat(release)!`
/// in a workspace's release crate breaks nothing for a user of its core crate
/// alone. A tie in the record names no package, and moves its project for any
/// breaking change, as does a commit whose changes cannot be read - a missed
/// break costs more than a turn spent finding nothing to do.
fn pulls(db: &Db, project: &crate::db::Project, last_turn: Option<&str>, all: &[crate::db::Project], uses: &[Use], touched: &mut Touched) -> Result<Vec<Note>> {
    let mut out = Vec::new();
    let Some(last_turn) = last_turn else {
        return Ok(out);
    };
    let mut seen = std::collections::HashSet::new();
    for item in uses.iter().filter(|u| u.consumer == project.name) {
        let Some(supplier) = all.iter().find(|p| p.name == item.supplier) else {
            continue;
        };
        if !seen.insert(supplier.id) {
            continue;
        }
        // The directories of every package it uses from this supplier; none
        // for a tie, which reaches the whole of it.
        let drawn: Option<std::collections::BTreeSet<String>> = uses
            .iter()
            .filter(|u| u.consumer == project.name && u.supplier == supplier.name)
            .map(|u| u.directory.clone())
            .collect();
        for change in db.breaking_released(supplier.id, last_turn)? {
            if let Some(drawn) = &drawn
                && !touched.reaches(supplier, &change.commit, drawn)
            {
                continue;
            }
            let subject = change.body.lines().next().unwrap_or_default().trim_start_matches("breaking: ");
            out.push(Note {
                id: change.id,
                head: format!("{} broke what it uses: {subject}", supplier.name),
                since: change.date,
            });
        }
        let released = db.last_release_moment(supplier.id)?;
        if db.changes_after(supplier.id, released.as_deref())?.is_empty() {
            for built in db.built_orders(supplier.id, project.id, last_turn)? {
                out.push(Note {
                    id: built.id,
                    head: format!("{} built what it asked for: {}", supplier.name, crate::context::headline(&built.body)),
                    since: built.date,
                });
            }
        }
    }
    out.sort_by(|a, b| a.since.cmp(&b.since).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

/// What every repository holds, read side by side. Walking a working tree is
/// nearly all waiting on the disk, and the only slow part of this that a
/// person is sitting in front of, so nothing is gained by taking turns.
fn read_held(db: &Db, projects: &[crate::db::Project]) -> Result<HashMap<i64, (crate::git::Held, Option<String>)>> {
    // The record first: its connection stays on this thread.
    let asked = projects
        .iter()
        .map(|project| Ok((project, db.latest_delivery(project.id)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(std::thread::scope(|scope| {
        let readers: Vec<_> = asked
            .iter()
            .map(|(project, latest)| scope.spawn(move || (project.id, held_by(project, latest.as_ref()))))
            .collect();
        readers
            .into_iter()
            .map(|reader| reader.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect()
    }))
}

fn notes(events: Vec<crate::db::Queued>) -> Vec<Note> {
    events
        .into_iter()
        .map(|event| Note {
            id: event.id,
            head: crate::context::headline(&event.body),
            since: event.date,
        })
        .collect()
}

/// What each project in the work brings, read from the record and from git.
pub fn gather(db: &Db) -> Result<Vec<Standing>> {
    let mut out = Vec::new();
    let projects = db.projects()?;
    let mut held = read_held(db, &projects)?;
    let releases = db.release_moments()?;
    let uses = uses(db)?;
    let rota = crate::refactor::rota(db)?;
    let mut touched = Touched::default();
    for project in &projects {
        // A place the record keeps for itself is not work anyone does.
        if !project.kind.has_a_place() {
            continue;
        }
        let last_turn = last_turn(db, project)?;
        let pulled = pulls(db, project, last_turn.as_deref(), &projects, &uses, &mut touched)?;
        let stage = db.current_stage(project.id)?.map(|stage| {
            let open: Vec<&crate::db::Task> = stage.tasks.iter().chain(&stage.asleep).collect();
            Stage {
                put_down: !open.is_empty() && open.iter().all(|t| t.is_put_down()),
                version: stage.version,
                title: stage.title,
            }
        });

        let released = db.last_release_moment(project.id)?;
        let open = db.open_session(project.id)?;
        let waited = last_turn
            .as_deref()
            .map_or(releases.len(), |at| releases.iter().filter(|moment| moment.as_str() > at).count());
        let (held, undelivered) = held.remove(&project.id).unwrap_or_default();

        out.push(Standing {
            kind: project.kind,
            weight: project.weight,
            stage,
            untagged: db.changes_after(project.id, released.as_deref())?,
            held,
            undelivered,
            sitting_live: open.as_ref().and_then(|s| crate::hold::presence(db.path(), s.id).live()),
            sitting: open.map(|s| s.started_at),
            blockers: notes(db.blockers(project.id)?),
            moves: notes(db.moves(project.id, settled_by(db, project)?.as_deref())?),
            awaited: awaited(db, project)?,
            pulled,
            last_turn,
            waited,
            refactor: rota.is_next(&project.name),
            waiting: db.count_open_events(project.id, "question")? as usize,
            asked: db.asked_wishes(project.id)?.len(),
            project: project.name.clone(),
        });
    }
    Ok(out)
}

/// Puts every project in its place: the order, and what is left out of it.
pub fn choose(standings: &[Standing]) -> Queue {
    // What a project's share is measured against. Who is in the order does not
    // depend on what the neighbours did - that brings back nothing set aside -
    // so this can be known before any place is.
    let total: usize = standings.iter().filter(|s| in_the_order(s).is_ok()).map(|s| s.weight as usize).sum();
    let mut order = Vec::new();
    let mut left_out = Vec::new();
    for standing in standings {
        match candidate(standing, total) {
            Ok(found) => order.push(found),
            Err(why) => left_out.push(LeftOut {
                project: standing.project.clone(),
                why,
                blocker: standing.blockers.first().filter(|_| why == Aside::Blocked).cloned(),
            }),
        }
    }
    order.sort_by(in_order);
    left_out.sort_by(|a, b| a.project.cmp(&b.project));
    Queue {
        order,
        left_out,
        parted: Vec::new(),
        unread: Vec::new(),
    }
}

/// Set aside on purpose: weight 0, or everything planned put down.
fn set_aside(s: &Standing) -> bool {
    s.weight == 0 || s.stage.as_ref().is_some_and(|stage| stage.put_down)
}

/// Whether a project is in the order at all, and if not, why not.
fn in_the_order(s: &Standing) -> Result<(), Aside> {
    // Before anything else: nobody can ask for work that something out of
    // reach is stopping.
    if !s.blockers.is_empty() {
        return Err(Aside::Blocked);
    }
    // Set aside on purpose, a project is not holding the line up, whatever it
    // was left holding - unless the owner moved it up, which is asking for it.
    if set_aside(s) && s.moves.is_empty() {
        return Err(if s.weight == 0 { Aside::Out } else { Aside::PutDown });
    }
    Ok(())
}

/// How many releases a project waits, at its weight, before what a neighbour
/// did moves it: the least `waited` at which `(waited + 1) * weight` reaches
/// the weight of the whole order. A project at weight 0 never gets there.
pub fn share(weight: u32, total: usize) -> usize {
    match weight {
        0 => usize::MAX,
        weight => total.div_ceil(weight as usize).saturating_sub(1),
    }
}

/// The reason a project is in the order, or the reason it is not. `total` is
/// the weight of the whole order.
fn candidate(s: &Standing, total: usize) -> Result<Candidate, Aside> {
    in_the_order(s)?;
    let unfinished = Unfinished {
        changed: s.held.changed,
        untracked: s.held.untracked,
        unpushed: s.held.unpushed,
        operation: s.held.operation.clone(),
        untagged: s.untagged.iter().map(|(_, subject)| subject.clone()).collect(),
        undelivered: s.undelivered.clone(),
        sitting: s.sitting.clone(),
        sitting_live: s.sitting_live,
    };
    let aside = set_aside(s);
    let is_unfinished = !unfinished.is_empty() && !aside;
    // Only the owner's word reaches a project set aside.
    let mut moved = s.moves.clone();
    if !aside {
        moved.extend(s.awaited.iter().cloned());
        moved.extend(s.pulled.iter().cloned());
    }
    let share = share(s.weight, total);
    let early = !s.moves.is_empty() || (!aside && (!s.awaited.is_empty() || (!s.pulled.is_empty() && s.waited >= share)));

    let (reason, since) = if is_unfinished {
        let since = s.untagged.first().map(|(at, _)| at.clone()).or_else(|| s.last_turn.clone());
        (Reason::Unfinished, since)
    } else if early {
        // The owner's word, a stage waiting, then what a neighbour did: the
        // first of them is what moves it.
        (Reason::Moved, moved.first().map(|first| first.since.clone()))
    } else {
        (Reason::Turn, None)
    };

    Ok(Candidate {
        project: s.project.clone(),
        kind: s.kind,
        weight: s.weight,
        reason,
        version: s.stage.as_ref().map_or_else(|| "unplanned".to_string(), |stage| stage.version.clone()),
        title: s.stage.as_ref().and_then(|stage| stage.title.clone()),
        unfinished: (reason == Reason::Unfinished).then_some(unfinished),
        moved,
        waited: s.waited,
        last_turn: s.last_turn.as_deref().map(day_of),
        waiting: s.waiting,
        asked: s.asked,
        refactor: s.refactor,
        since,
        turn_at: s.last_turn.clone(),
        early,
        share,
    })
}

/// How strongly a turn is due: see the head of this module.
fn due(c: &Candidate) -> usize {
    (c.waited + 1) * c.weight as usize
}

/// The order: the reason first, then what weighs within it, then the name, so
/// that two runs over one record agree.
fn in_order(a: &Candidate, b: &Candidate) -> Ordering {
    a.reason
        .cmp(&b.reason)
        .then_with(|| match a.reason {
            // What has waited longest first.
            Reason::Unfinished | Reason::Moved => a.since.cmp(&b.since),
            // On a tie the heavier one: it comes round sooner anyway, and the
            // lighter one's next look at the queue has one more to its count.
            Reason::Turn => due(b)
                .cmp(&due(a))
                .then_with(|| b.weight.cmp(&a.weight))
                .then_with(|| a.turn_at.cmp(&b.turn_at)),
        })
        .then_with(|| a.project.cmp(&b.project))
}

fn day_of(moment: &str) -> String {
    moment.split(['T', ' ']).next().unwrap_or(moment).to_string()
}

/// What a project left unfinished holds, in a phrase.
pub fn unfinished_parts(c: &Candidate) -> String {
    c.unfinished.as_ref().map(|u| u.parts().join(", ")).unwrap_or_default()
}

/// One turn ahead: who takes it, and on what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Turn {
    pub turn: usize,
    pub project: String,
    /// The stage it would be on by then; `unplanned` when the plan has run out.
    pub version: String,
    pub title: Option<String>,
    /// What this turn is besides a stage: `unfinished`, `moved`, `refactor`, or
    /// `grow` where the plan is short enough that the turn would start by growing it.
    pub note: Option<&'static str>,
}

/// What a project has ahead of it, for playing the queue forward.
#[derive(Debug, Clone, Default)]
pub struct Ahead {
    /// Its planned stages, in order.
    pub stages: Vec<(String, Option<String>)>,
    /// With this many stages left or fewer, a turn starts by growing the plan.
    pub floor: i64,
}

/// Plays the queue forward: the same rule, assuming each turn ends in a release
/// and nothing else changes. It is a forecast and says only that: a release out
/// of turn, a changed weight or a neighbour's breaking change redraws it, which
/// is the point of working it out rather than keeping it.
pub fn ahead(queue: &Queue, plans: &HashMap<String, Ahead>, refactor: Option<&str>, turns: usize) -> Vec<Turn> {
    struct Walk<'a> {
        candidate: &'a Candidate,
        stages: std::collections::VecDeque<(String, Option<String>)>,
        floor: i64,
        waited: usize,
        /// Which simulated turn it last had: none yet is the longest ago.
        played: usize,
        pressing: Option<&'static str>,
        /// What a neighbour did is waiting for it to have waited its share.
        lifted: bool,
        refactor: bool,
    }
    let total: usize = queue.order.iter().map(|c| c.weight as usize).sum();
    let mut walks: Vec<Walk> = queue
        .order
        .iter()
        .map(|candidate| {
            let plan = plans.get(&candidate.project).cloned().unwrap_or_default();
            // The stage it is on, not the lowest number planned: a version left
            // behind with nothing open in it is not what comes next.
            let from = plan.stages.iter().position(|(version, _)| *version == candidate.version).unwrap_or(0);
            Walk {
                candidate,
                stages: plan.stages.into_iter().skip(from).collect(),
                floor: plan.floor,
                waited: candidate.waited,
                played: 0,
                pressing: match candidate.reason {
                    Reason::Unfinished => Some("unfinished"),
                    Reason::Moved => Some("moved"),
                    Reason::Turn => None,
                },
                lifted: candidate.reason == Reason::Turn && !candidate.moved.is_empty(),
                refactor: refactor == Some(candidate.project.as_str()),
            }
        })
        .collect();

    let mut out = Vec::new();
    for turn in 1..=turns {
        // What presses goes first, in the order the queue already put it in;
        // then what a neighbour did, once its project has waited its share,
        // longest asked first; then the rule. A project at weight 0 is here
        // only for what pressed.
        let pressed = walks.iter().position(|w| w.pressing.is_some());
        let lifted = || {
            walks
                .iter()
                .enumerate()
                .filter(|(_, w)| w.lifted && w.waited >= share(w.candidate.weight, total))
                .min_by(|(_, a), (_, b)| {
                    let asked = |w: &Walk| w.candidate.moved.first().map(|n| n.since.clone());
                    asked(a).cmp(&asked(b)).then_with(|| a.candidate.project.cmp(&b.candidate.project))
                })
                .map(|(index, _)| index)
        };
        let by_turn = || {
            walks
                .iter()
                .enumerate()
                .filter(|(_, w)| w.candidate.weight > 0)
                .min_by(|(_, a), (_, b)| {
                    let due = |w: &Walk| (w.waited + 1) * w.candidate.weight as usize;
                    due(b)
                        .cmp(&due(a))
                        .then_with(|| b.candidate.weight.cmp(&a.candidate.weight))
                        .then_with(|| a.played.cmp(&b.played))
                        .then_with(|| a.candidate.turn_at.cmp(&b.candidate.turn_at))
                        .then_with(|| a.candidate.project.cmp(&b.candidate.project))
                })
                .map(|(index, _)| index)
        };
        let (next, moved) = match pressed {
            Some(next) => (Some(next), false),
            None => match lifted() {
                Some(next) => (Some(next), true),
                None => (by_turn(), false),
            },
        };
        let Some(next) = next else {
            break;
        };
        for walk in &mut walks {
            walk.waited += 1;
        }
        let walk = &mut walks[next];
        walk.waited = 0;
        walk.played = turn;
        // Its release settles what its neighbours did, however it got the turn.
        walk.lifted = false;

        let pressing = walk.pressing.take().or(moved.then_some("moved"));
        let refactoring = std::mem::take(&mut walk.refactor);
        let folder = walk.candidate.kind == Kind::Folder;
        // A refactoring ships no stage, and a folder's turn takes none off a plan.
        let short = !folder && walk.stages.len() as i64 <= walk.floor;
        let (version, title) = if refactoring {
            ("refactoring".to_string(), None)
        } else if folder {
            ("unplanned".to_string(), None)
        } else {
            walk.stages.pop_front().unwrap_or_else(|| ("unplanned".to_string(), None))
        };
        let note = pressing
            .or(if refactoring { Some("refactor") } else { None })
            .or(if short { Some("grow") } else { None });
        out.push(Turn {
            turn,
            project: walk.candidate.project.clone(),
            version,
            title,
            note,
        });
    }
    out
}

/// The forecast on a screen.
pub fn render_ahead(turns: &[Turn]) -> String {
    let name = turns.iter().map(|t| t.project.chars().count()).max().unwrap_or(0);
    let version = turns.iter().map(|t| t.version.chars().count()).max().unwrap_or(0);
    let number = turns.len().to_string().len();
    let mut out = String::new();
    for turn in turns {
        let title = turn.title.as_deref().map(|t| format!("  {t}")).unwrap_or_default();
        let note = turn.note.map(|n| format!("  ({n})")).unwrap_or_default();
        out.push_str(&format!(
            "{:>number$}  {:<name$}  {:<version$}{title}{note}\n",
            turn.turn, turn.project, turn.version
        ));
    }
    out
}

/// The reason in a sentence.
pub fn why(c: &Candidate) -> String {
    match c.reason {
        Reason::Unfinished => format!("not finished: {}{}", unfinished_parts(c), moved_too(c)),
        Reason::Moved => moved_up(&c.moved),
        Reason::Turn => {
            let turn = match &c.last_turn {
                Some(_) => format!("its turn: {} since its last, at weight {}", plural(c.waited, "release", "releases"), c.weight),
                None => format!("its turn: it has had none yet, at weight {}", c.weight),
            };
            match c.moved.is_empty() {
                true => turn,
                false => format!("{turn}; to take up: {}", heads(&c.moved)),
            }
        }
    }
}

/// The first thing that asks, in full, and how many more there are. `--json`
/// has every one of them.
fn heads(moved: &[Note]) -> String {
    match moved {
        [] => String::new(),
        [first] => first.head.clone(),
        [first, rest @ ..] => format!("{} (and {} more)", first.head, rest.len()),
    }
}

/// What moved a project up, in a line.
pub fn moved_up(moved: &[Note]) -> String {
    match moved {
        [] => "moved up".to_string(),
        _ => format!("moved up: {}", heads(moved)),
    }
}

/// What a neighbour did, for a project in its turn that has not waited its
/// share yet: how far it has got, and what moves it once it has.
pub fn moves_once_due(c: &Candidate) -> String {
    format!(
        "waited {} of {} before moving up: {}",
        c.waited,
        plural(c.share, "release", "releases"),
        heads(&c.moved)
    )
}

/// What moved a project that is also unfinished, said after the first reason.
///
/// `rigger go` opens a sitting and only then starts the assistant, so a project
/// moved up reads as unfinished from the moment its turn begins. Its first
/// message sends the assistant here for what moved it, and this is that answer.
/// What a neighbour did that has not moved it up yet is still there to take up.
fn moved_too(c: &Candidate) -> String {
    match (c.moved.is_empty(), c.early) {
        (true, _) => String::new(),
        (false, true) => format!("; {}", moved_up(&c.moved)),
        (false, false) => format!("; to take up: {}", heads(&c.moved)),
    }
}

/// The reason in a few words, for a line among others.
fn short(c: &Candidate) -> String {
    match c.reason {
        Reason::Unfinished => format!("{}{}", unfinished_parts(c), moved_too(c)),
        Reason::Moved => moved_up(&c.moved),
        Reason::Turn if !c.moved.is_empty() => moves_once_due(c),
        Reason::Turn => match &c.last_turn {
            Some(_) => format!("waited {}", plural(c.waited, "release", "releases")),
            None => "no turn yet".to_string(),
        },
    }
}

/// "1 question waits", "2 questions wait".
fn waits(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", plural(n, one, many), if n == 1 { "waits" } else { "wait" })
}

/// What else is true of a project, beside its reason.
fn also(c: &Candidate) -> Vec<String> {
    let mut out = Vec::new();
    if c.refactor {
        out.push("this turn is a refactoring".to_string());
    }
    if c.waiting > 0 {
        out.push(format!("{} on the owner", waits(c.waiting, "question", "questions")));
    }
    if c.asked > 0 {
        out.push(format!("{} from neighbours", plural(c.asked, "order", "orders")));
    }
    out
}

/// The screen: the next project with all it carries, then the order of the rest.
pub fn render(queue: &Queue) -> String {
    let mut out = String::new();
    match queue.order.first() {
        Some(first) => {
            let title = first.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            out.push_str(&format!("Next: {}  {}{title}\n", first.project, first.version));
            out.push_str(&format!("  {}\n", why(first)));
            let also = also(first);
            if !also.is_empty() {
                out.push_str(&format!("  {}\n", also.join(" · ")));
            }
        }
        None if queue.left_out.is_empty() => out.push_str("Nothing in the queue: no project is in the work.\n"),
        None => out.push_str("Nothing in the queue: every project is left out, below.\n"),
    }

    let rest = queue.order.get(1..).unwrap_or_default();
    if !rest.is_empty() {
        let name = rest.iter().map(|c| c.project.chars().count()).max().unwrap_or(0);
        let version = rest.iter().map(|c| c.version.chars().count()).max().unwrap_or(0);
        out.push_str("\nThen\n");
        for c in rest {
            out.push_str(&format!("  {:<name$}  {:<version$}  {}\n", c.project, c.version, short(c)));
        }
    }

    if !queue.left_out.is_empty() {
        let name = queue.left_out.iter().map(|l| l.project.chars().count()).max().unwrap_or(0);
        out.push_str("\nLeft out\n");
        for item in &queue.left_out {
            let why = match &item.blocker {
                Some(blocker) => format!("blocked since {}: {} [{}]", blocker.since, blocker.head, blocker.id),
                None => item.why.describe().to_string(),
            };
            out.push_str(&format!("  {:<name$}  {why}\n", item.project));
        }
    }

    if !queue.parted.is_empty() {
        out.push_str("\nPairs out of step\n");
        for item in &queue.parted {
            out.push_str(&format!("  {}\n", crate::drift_line(item)));
        }
    }

    if !queue.unread.is_empty() {
        out.push_str(&format!(
            "\nGit did not read for {}: what this knows of {} is as of the last time it did.\n",
            queue.unread.join(", "),
            if queue.unread.len() == 1 { "it" } else { "them" }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    fn on_stage(project: &str, weight: u32, waited: usize) -> Standing {
        Standing {
            project: project.to_string(),
            kind: Kind::Repo,
            weight,
            stage: Some(Stage {
                version: "v0.2.0".to_string(),
                title: Some("Second".to_string()),
                put_down: false,
            }),
            untagged: Vec::new(),
            held: Default::default(),
            undelivered: None,
            sitting: None,
            sitting_live: None,
            last_turn: Some("2026-10-26T10:00:00Z".to_string()),
            waited,
            refactor: false,
            blockers: Vec::new(),
            moves: Vec::new(),
            awaited: Vec::new(),
            pulled: Vec::new(),
            waiting: 0,
            asked: 0,
        }
    }

    fn note(id: i64, head: &str, since: &str) -> Note {
        Note {
            id,
            head: head.to_string(),
            since: since.to_string(),
        }
    }

    fn order_of(standings: &[Standing]) -> Vec<String> {
        choose(standings).order.iter().map(|c| c.project.clone()).collect()
    }

    /// The whole point of the formula: played out over many turns, each project
    /// gets its weight's share, and no long gaps open between its turns.
    #[test]
    fn every_project_gets_its_weights_share_of_the_turns() {
        let weights = [("a", 6), ("b", 3), ("c", 3), ("d", 2), ("e", 2), ("f", 1), ("g", 1), ("h", 1), ("i", 1)];
        let total: usize = weights.iter().map(|(_, w)| *w as usize).sum();
        let mut waited: BTreeMap<&str, usize> = weights.iter().enumerate().map(|(i, (name, _))| (*name, i)).collect();
        let mut turns: Vec<String> = Vec::new();
        let rounds = 40;
        for _ in 0..total * rounds {
            let standings: Vec<Standing> = weights.iter().map(|(name, w)| on_stage(name, *w, waited[name])).collect();
            let first = choose(&standings).order[0].project.clone();
            for count in waited.values_mut() {
                *count += 1;
            }
            *waited.get_mut(first.as_str()).unwrap() = 0;
            turns.push(first);
        }
        // Past the first few rounds, where the starting order still shows.
        let settled = &turns[total * 4..];
        for (name, weight) in weights {
            let got = settled.iter().filter(|t| t.as_str() == name).count();
            let wanted = (rounds - 4) * weight as usize;
            assert!(got.abs_diff(wanted) <= 1, "{name} at weight {weight}: {got} turns, wanted {wanted}");
            let at: Vec<usize> = settled.iter().enumerate().filter(|(_, t)| t.as_str() == name).map(|(i, _)| i).collect();
            let widest = at.windows(2).map(|w| w[1] - w[0]).max().unwrap();
            assert!(widest <= total / weight as usize + 2, "{name}: a gap of {widest} in a round of {total}");
        }
    }

    #[test]
    fn a_higher_weight_comes_first_and_waiting_catches_it_up() {
        assert_eq!(order_of(&[on_stage("light", 1, 3), on_stage("heavy", 3, 3)]), vec!["heavy", "light"]);
        // Six releases at weight 1 outweigh one at weight 3.
        assert_eq!(order_of(&[on_stage("light", 1, 6), on_stage("heavy", 3, 1)]), vec!["light", "heavy"]);
        // Level: the heavier one, then the older turn, then the name.
        assert_eq!(order_of(&[on_stage("light", 1, 5), on_stage("heavy", 3, 1)]), vec!["heavy", "light"]);
        let mut older = on_stage("older", 2, 2);
        older.last_turn = Some("2026-10-01T10:00:00Z".into());
        assert_eq!(order_of(&[on_stage("newer", 2, 2), older.clone()]), vec!["older", "newer"]);
        assert_eq!(order_of(&[on_stage("b", 2, 2), on_stage("a", 2, 2)]), vec!["a", "b"]);
    }

    /// The order of the reasons is the rule itself, whatever the waiting says.
    #[test]
    fn each_reason_outranks_the_next() {
        let turn = on_stage("turn", 9, 99);
        let mut moved = on_stage("moved", 1, 0);
        moved.moves = vec![note(1, "the owner put it first", "2026-11-01")];
        let mut unfinished = on_stage("unfinished", 1, 0);
        unfinished.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: a thing".into())];
        assert_eq!(order_of(&[turn, moved, unfinished]), vec!["unfinished", "moved", "turn"]);
    }

    /// Each thing a turn can be left holding is enough on its own, and says
    /// what it is.
    #[test]
    fn whatever_has_not_gone_out_leaves_a_project_unfinished() {
        type Case = (&'static str, fn(&mut Standing));
        let cases: [Case; 9] = [
            ("1 changed file not committed", |s| s.held.changed = 1),
            ("2 untracked files", |s| s.held.untracked = 2),
            ("a merge left halfway", |s| s.held.operation = Some("merge".into())),
            ("3 commits not pushed", |s| s.held.unpushed = 3),
            ("1 change since the last release", |s| {
                s.untagged = vec![("2026-10-27T09:00:00Z".into(), "fix: a thing".into())]
            }),
            ("v0.1.0: released, publish failed", |s| {
                s.undelivered = Some("v0.1.0: released, publish failed".into())
            }),
            ("a sitting open since 2026-11-05", |s| s.sitting = Some("2026-11-05T10:00:00Z".into())),
            // Who is at it, when the record can tell.
            ("a sitting at work since 2026-11-05", |s| {
                s.sitting = Some("2026-11-05T10:00:00Z".into());
                s.sitting_live = Some(true);
            }),
            ("a sitting left open since 2026-11-05 by an assistant that has gone", |s| {
                s.sitting = Some("2026-11-05T10:00:00Z".into());
                s.sitting_live = Some(false);
            }),
        ];
        for (said, set) in cases {
            let mut s = on_stage("alpha", 2, 0);
            set(&mut s);
            let queue = choose(&[s]);
            assert_eq!(queue.order[0].reason, Reason::Unfinished, "{said}");
            assert_eq!(why(&queue.order[0]), format!("not finished: {said}"));
        }
        // With nothing held, nothing is said.
        let queue = choose(&[on_stage("alpha", 2, 0)]);
        assert_eq!(queue.order[0].reason, Reason::Turn);
        assert_eq!(queue.order[0].unfinished, None);
    }

    /// Work done with nothing planned still has to go out; work left in a
    /// project set aside on purpose holds nothing up.
    #[test]
    fn a_project_set_aside_is_not_unfinished_and_one_without_a_plan_is() {
        let mut bare = on_stage("bare", 2, 0);
        bare.stage = None;
        bare.held.changed = 1;
        let queue = choose(&[bare]);
        assert_eq!(queue.order[0].reason, Reason::Unfinished);
        assert_eq!(queue.order[0].version, "unplanned");

        let mut out = on_stage("out", 0, 0);
        out.held.changed = 1;
        let mut asleep = on_stage("asleep", 2, 0);
        asleep.stage.as_mut().unwrap().put_down = true;
        asleep.held.changed = 1;
        let queue = choose(&[out, asleep]);
        assert!(queue.order.is_empty(), "{:?}", queue.order);
        let aside: Vec<Aside> = queue.left_out.iter().map(|l| l.why).collect();
        assert_eq!(aside, vec![Aside::PutDown, Aside::Out]);
    }

    /// A blocker is the way out for a project waiting on the outside world: it
    /// leaves the order with its reason, unfinished or moved or not.
    #[test]
    fn a_blocked_project_is_left_out_with_what_blocks_it() {
        let mut blocked = on_stage("blocked", 9, 99);
        blocked.held.unpushed = 2;
        blocked.moves = vec![note(2, "asked for", "2026-11-02")];
        blocked.blockers = vec![note(7, "The signing key has expired", "2026-11-01")];
        let queue = choose(&[blocked, on_stage("next", 1, 0)]);
        assert_eq!(queue.order[0].project, "next");
        assert_eq!(queue.left_out[0].why, Aside::Blocked);
        let said = render(&queue);
        assert!(said.contains("blocked since 2026-11-01: The signing key has expired [7]"), "{said}");
    }

    /// Weight 0 and everything put down are left out, each with its reason, and
    /// being moved up brings either back, because that is asking for it. A project
    /// with nothing planned is in the order: its turn is what plans it.
    #[test]
    fn out_and_put_down_are_left_out_unless_moved() {
        let out = on_stage("out", 0, 9);
        let mut bare = on_stage("bare", 2, 0);
        bare.stage = None;
        let mut asleep = on_stage("asleep", 2, 9);
        asleep.stage.as_mut().unwrap().put_down = true;
        let mut asked = on_stage("asked", 0, 9);
        asked.stage = None;
        asked.moves = vec![note(1, "the owner put it first", "2026-11-01")];
        let mut folder = on_stage("folder", 1, 0);
        folder.kind = Kind::Folder;
        folder.stage = None;

        let queue = choose(&[out, bare, asleep, asked, folder]);
        let order: Vec<(&str, Reason)> = queue.order.iter().map(|c| (c.project.as_str(), c.reason)).collect();
        assert_eq!(order, vec![("asked", Reason::Moved), ("bare", Reason::Turn), ("folder", Reason::Turn)]);
        assert_eq!(queue.order[0].version, "unplanned");
        let aside: Vec<(&str, Aside)> = queue.left_out.iter().map(|l| (l.project.as_str(), l.why)).collect();
        assert_eq!(aside, vec![("asleep", Aside::PutDown), ("out", Aside::Out)]);
    }

    #[test]
    fn of_two_moved_or_unfinished_the_one_waiting_longer_comes_first() {
        let mut newer = on_stage("newer", 9, 9);
        newer.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: b".into())];
        let mut older = on_stage("older", 1, 0);
        older.untagged = vec![("2026-10-26T11:00:00Z".into(), "feat: a".into())];
        assert_eq!(order_of(&[newer, older]), vec!["older", "newer"]);

        let mut newer = on_stage("newer", 9, 9);
        newer.moves = vec![note(2, "b", "2026-11-02")];
        let mut older = on_stage("older", 1, 0);
        older.moves = vec![note(1, "a", "2026-11-01")];
        assert_eq!(order_of(&[newer, older]), vec!["older", "newer"]);
    }

    /// What a neighbour did moves a project only once it has waited its share:
    /// before that it stands in its turn, behind the ones that have waited
    /// longer, and says how far it has got. The owner's word and a stage
    /// waiting on it do not wait for a share.
    #[test]
    fn a_neighbours_need_moves_a_project_once_it_has_waited_its_share() {
        // The whole order weighs 5, so at weight 2 the share is 2 releases.
        let broke = note(4, "lib broke what it uses: feat!: a part", "2026-11-03");
        let mut pulled = on_stage("pulled", 2, 1);
        pulled.pulled = vec![broke.clone()];
        let line = |pulled: Standing| vec![pulled, on_stage("heavy", 2, 9), on_stage("light", 1, 30)];

        let queue = choose(&line(pulled.clone()));
        let order: Vec<(&str, Reason)> = queue.order.iter().map(|c| (c.project.as_str(), c.reason)).collect();
        assert_eq!(order, vec![("light", Reason::Turn), ("heavy", Reason::Turn), ("pulled", Reason::Turn)]);
        assert_eq!(queue.order[2].moved, vec![broke.clone()], "what will move it is named all the same");
        let said = render(&queue);
        assert!(
            said.contains("pulled  v0.2.0  waited 1 of 2 releases before moving up: lib broke what it uses: feat!: a part"),
            "{said}"
        );
        // What is left out of the order is not part of what the share is of.
        let mut blocked = on_stage("blocked", 6, 0);
        blocked.blockers = vec![note(7, "The signing key has expired", "2026-11-01")];
        let mut with_blocked = line(pulled.clone());
        with_blocked.push(blocked);
        assert_eq!(choose(&with_blocked).order[2].share, 2);

        // One more release, and it has: ahead of everyone in their turn.
        pulled.waited = 2;
        let queue = choose(&line(pulled.clone()));
        assert_eq!(queue.order[0].project, "pulled");
        assert_eq!(queue.order[0].reason, Reason::Moved);
        assert_eq!(why(&queue.order[0]), "moved up: lib broke what it uses: feat!: a part");

        // The owner's word, and a stage that cannot go on, move it at once.
        let asks: [fn(&mut Standing); 2] = [
            |s| s.moves = vec![note(5, "the owner asked", "2026-11-04")],
            |s| s.awaited = vec![note(6, "app waits on it: a part", "")],
        ];
        for asked in asks {
            let mut now = on_stage("pulled", 2, 0);
            asked(&mut now);
            let queue = choose(&line(now));
            assert_eq!(
                (queue.order[0].project.as_str(), queue.order[0].reason),
                ("pulled", Reason::Moved),
                "{:?}",
                queue.order[0]
            );
        }

        // Unfinished, it says what is still there to take up.
        pulled.waited = 0;
        pulled.held.changed = 1;
        let queue = choose(&line(pulled));
        assert_eq!(
            why(&queue.order[0]),
            "not finished: 1 changed file not committed; to take up: lib broke what it uses: feat!: a part"
        );
    }

    /// The loop that took the line over: two projects that use each other, each
    /// release of one breaking the other. Played out over many turns, everyone
    /// still gets the weight's share, and no gap opens wider than a round.
    #[test]
    fn two_projects_that_break_each_other_cannot_take_the_line_over() {
        let weights = [("a", 2), ("b", 2), ("c", 3), ("d", 2), ("e", 1), ("f", 1), ("g", 1)];
        let total: usize = weights.iter().map(|(_, w)| *w as usize).sum();
        let mut waited: BTreeMap<&str, usize> = weights.iter().enumerate().map(|(i, (name, _))| (*name, i)).collect();
        // What the other one broke since its own last release.
        let mut broken: BTreeMap<&str, Option<usize>> = weights.iter().map(|(name, _)| (*name, None)).collect();
        let mut turns: Vec<String> = Vec::new();
        let rounds = 40;
        for turn in 0..total * rounds {
            let standings: Vec<Standing> = weights
                .iter()
                .map(|(name, w)| {
                    let mut s = on_stage(name, *w, waited[name]);
                    if let Some(at) = broken[name] {
                        s.pulled = vec![note(at as i64, "the other broke what it uses", &format!("{at:05}"))];
                    }
                    s
                })
                .collect();
            let first = choose(&standings).order[0].project.clone();
            for count in waited.values_mut() {
                *count += 1;
            }
            *waited.get_mut(first.as_str()).unwrap() = 0;
            *broken.get_mut(first.as_str()).unwrap() = None;
            let other = match first.as_str() {
                "a" => Some("b"),
                "b" => Some("a"),
                _ => None,
            };
            if let Some(other) = other {
                broken.get_mut(other).unwrap().get_or_insert(turn);
            }
            turns.push(first);
        }
        let settled = &turns[total * 4..];
        for (name, weight) in weights {
            let got = settled.iter().filter(|t| t.as_str() == name).count();
            let wanted = (rounds - 4) * weight as usize;
            assert!(got.abs_diff(wanted) <= 1, "{name} at weight {weight}: {got} turns, wanted {wanted}");
            let at: Vec<usize> = settled.iter().enumerate().filter(|(_, t)| t.as_str() == name).map(|(i, _)| i).collect();
            let widest = at.windows(2).map(|w| w[1] - w[0]).max().unwrap();
            assert!(widest <= total / weight as usize + 2, "{name}: a gap of {widest} in a round of {total}");
        }
    }

    /// A question to the owner does not stop a stage, so it does not take a
    /// project out - it is counted on its line.
    #[test]
    fn questions_and_orders_are_counted_not_obeyed() {
        let mut s = on_stage("alpha", 2, 0);
        s.waiting = 3;
        s.asked = 2;
        let queue = choose(&[s]);
        assert_eq!(queue.order[0].reason, Reason::Turn);
        let said = render(&queue);
        assert!(said.contains("3 questions wait on the owner"), "{said}");
        assert!(said.contains("2 orders from neighbours"), "{said}");
    }

    #[test]
    fn the_screen_says_why_and_lists_every_project() {
        let standings: Vec<Standing> = (0..9).map(|n| on_stage(&format!("p{n}"), 1, 20 - n)).collect();
        let said = render(&choose(&standings));
        assert!(
            said.starts_with("Next: p0  v0.2.0 · Second\n  its turn: 20 releases since its last, at weight 1\n"),
            "{said}"
        );
        let named: HashSet<&str> = said.split_whitespace().collect();
        for n in 0..9 {
            assert!(named.contains(format!("p{n}").as_str()), "{said}");
        }
        assert!(said.contains("waited 12 releases"), "{said}");
    }

    /// The forecast is the same rule, so it comes out in the weights' shares, each
    /// project moving on through its own plan.
    #[test]
    fn the_forecast_plays_the_rule_forward() {
        let queue = choose(&[on_stage("heavy", 3, 0), on_stage("light", 1, 0)]);
        let plan = |versions: &[&str]| Ahead {
            stages: versions.iter().map(|v| (v.to_string(), None)).collect(),
            floor: 0,
        };
        let plans = HashMap::from([
            ("heavy".to_string(), plan(&["v1", "v2", "v3", "v4", "v5", "v6"])),
            ("light".to_string(), plan(&["v7", "v8"])),
        ]);
        let turns = ahead(&queue, &plans, None, 8);
        let said: Vec<(&str, &str)> = turns.iter().map(|t| (t.project.as_str(), t.version.as_str())).collect();
        assert_eq!(
            said,
            vec![
                ("heavy", "v1"),
                ("heavy", "v2"),
                ("heavy", "v3"),
                ("light", "v7"),
                ("heavy", "v4"),
                ("heavy", "v5"),
                ("heavy", "v6"),
                ("light", "v8"),
            ]
        );
        assert_eq!(turns[0].turn, 1);
        assert_eq!(turns[7].turn, 8);
    }

    /// The forecast plays the share too: what a neighbour did moves its project
    /// ahead of the ones that waited longer, but not before it has waited its
    /// share, and only once.
    #[test]
    fn the_forecast_moves_what_a_neighbour_needs_once_the_share_is_waited() {
        let mut pulled = on_stage("pulled", 2, 1);
        pulled.pulled = vec![note(4, "lib broke what it uses", "2026-11-03")];
        let queue = choose(&[pulled, on_stage("heavy", 2, 30), on_stage("light", 1, 30)]);
        let plan = Ahead {
            stages: ["v1", "v2", "v3", "v4"].iter().map(|v| (v.to_string(), None)).collect(),
            floor: 0,
        };
        let plans: HashMap<String, Ahead> = ["pulled", "heavy", "light"].iter().map(|p| (p.to_string(), plan.clone())).collect();
        let turns = ahead(&queue, &plans, None, 6);
        let said: Vec<(&str, Option<&str>)> = turns.iter().map(|t| (t.project.as_str(), t.note)).collect();
        assert_eq!(said[..3], [("heavy", None), ("pulled", Some("moved")), ("light", None)], "{said:?}");
        assert!(said[3..].iter().all(|t| t.1.is_none()), "its release settled it: {said:?}");
    }

    /// What presses goes first, once; a refactoring takes a turn and no stage; a
    /// plan that runs short and then out says so.
    #[test]
    fn the_forecast_says_what_each_turn_is() {
        let mut left = on_stage("left", 1, 0);
        left.held.changed = 1;
        let mut moved = on_stage("moved", 1, 0);
        moved.moves = vec![note(1, "asked for", "2026-11-01")];
        let plain = on_stage("plain", 1, 9);
        let queue = choose(&[plain, moved, left]);
        let plan = |versions: &[&str], floor: i64| Ahead {
            stages: versions.iter().map(|v| (v.to_string(), None)).collect(),
            floor,
        };
        let plans = HashMap::from([
            ("left".to_string(), plan(&["l1", "l2", "l3"], 0)),
            ("moved".to_string(), plan(&["m1", "m2", "m3"], 0)),
            ("plain".to_string(), plan(&["p1"], 1)),
        ]);
        let turns = ahead(&queue, &plans, Some("plain"), 6);
        let said: Vec<(&str, &str, Option<&str>)> = turns.iter().map(|t| (t.project.as_str(), t.version.as_str(), t.note)).collect();
        assert_eq!(said[0], ("left", "l1", Some("unfinished")));
        assert_eq!(said[1], ("moved", "m1", Some("moved")));
        assert_eq!(said[2], ("plain", "refactoring", Some("refactor")));
        let plain: Vec<_> = said.iter().filter(|t| t.0 == "plain").collect();
        assert_eq!(*plain[1], ("plain", "p1", Some("grow")), "{said:?}");
        assert!(said[3..].iter().all(|t| t.2 != Some("unfinished") && t.2 != Some("moved")), "{said:?}");
        assert!(render_ahead(&turns).contains("(refactor)"));
    }

    /// A version left behind in the plan is not what comes next: the stage the
    /// project is on is where the forecast starts.
    #[test]
    fn the_forecast_starts_from_the_stage_it_is_on() {
        let queue = choose(&[on_stage("alpha", 1, 0)]);
        let plan = Ahead {
            stages: ["v0.1.5", "v0.2.0", "v0.3.0"].iter().map(|v| (v.to_string(), None)).collect(),
            floor: 0,
        };
        let plans = HashMap::from([("alpha".to_string(), plan)]);
        let turns = ahead(&queue, &plans, None, 2);
        assert_eq!(turns[0].version, "v0.2.0");
        assert_eq!(turns[1].version, "v0.3.0");
    }

    #[test]
    fn a_plan_that_has_run_out_is_said_and_nothing_loops_on_nothing() {
        let queue = choose(&[on_stage("only", 1, 0)]);
        let plans = HashMap::from([("only".to_string(), Ahead::default())]);
        let turns = ahead(&queue, &plans, None, 2);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[1].version, "unplanned");
        assert_eq!(turns[1].note, Some("grow"));

        let mut out = on_stage("out", 0, 0);
        out.moves = vec![note(1, "asked for", "2026-11-01")];
        let queue = choose(&[out]);
        assert_eq!(ahead(&queue, &HashMap::new(), None, 5).len(), 1, "weight 0 has only what pressed");
        assert!(ahead(&choose(&[]), &HashMap::new(), None, 5).is_empty());
    }

    #[test]
    fn an_empty_queue_says_so() {
        assert!(render(&choose(&[])).starts_with("Nothing in the queue: no project"));
        assert!(render(&choose(&[on_stage("out", 0, 0)])).starts_with("Nothing in the queue: every project is left out"));
    }
}
