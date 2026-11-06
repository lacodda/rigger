//! What to work on now: one project of the line, the stage it is on, and
//! the one reason it comes before the others.
//!
//! The calendar answers this when the owner has aimed a version at the
//! week. Most weeks of this line have nothing aimed at them - the work goes
//! in bursts, a dozen releases a day across a dozen products - and the
//! choice was made by hand, from the inbox, the calendar, the digest and
//! memory. The pick reads the same record and says one name.
//!
//! The rule is an order of reasons rather than a sum of scores. Nobody can
//! say why a 7.3 beat a 6.9, while a reason is a sentence the owner can
//! check, and steer: aim a version at the week and it comes first; give a
//! project a tier and its rhythm is kept. The first reason that holds for a
//! project is its reason, and the reasons rank so:
//!
//! 1. `unfinished` - a turn begun and not over: work uncommitted or unpushed,
//!    a feature or a fix since the last release, a release that did not get
//!    out, a sitting left open. The line does not move on past it.
//! 2. `late` - a version aimed at a week already gone: the owner's own
//!    promise, broken.
//! 3. `due` - a version aimed at this week: the owner's own word.
//! 4. `answered` - the owner decided something about it outside a sitting,
//!    after its last one. An answer waits to be acted on.
//! 5. `behind` - its tier asks for more than it has had: a floor gone
//!    under, or a rhythm lapsed.
//! 6. `turn` - nothing presses, so the one longest without a sitting.
//!
//! What cannot be finished from here is not unfinished: a project whose tag
//! is stopped by something out of reach - a missing key, a red pipeline, a
//! broken service - says so with a blocker, and is left out of the order
//! with that reason until the blocker is cleared or a release settles it.
//! Without that, the one project waiting on the outside world would stop
//! every other.
//!
//! Questions waiting on the owner do not take a project out of the pick -
//! by the line's rules a question does not stop a stage - and neither do
//! its neighbours' orders. Both are counted on its line instead.

use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use crate::calendar::{Focus, Overdue, Tier, Week};
use crate::db::{Db, version_order};
use crate::plural;
use crate::week::{Raised, Signal};

/// Why a project comes where it does, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    Unfinished,
    Late,
    Due,
    Answered,
    Behind,
    Turn,
}

/// How a tier's promise is being broken, for a project that is `behind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lag {
    /// Longer without a release than its rhythm asks.
    Rhythm,
    /// Tier A: more than one cycle of its rhythm missed in a row.
    MissedCycle,
    /// Tier B: no turn in the focus for six weeks.
    WithoutFocus,
}

impl Lag {
    /// A floor the tier promised not to go under, rather than its pace.
    fn is_a_floor(self) -> bool {
        self != Lag::Rhythm
    }
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
            out.push(format!("a sitting open since {}", day_of(sitting)));
        }
        out
    }
}

/// What stops a project's tag and is out of reach, as it was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Blocker {
    /// The id `rigger resolve` clears it by.
    pub id: i64,
    pub head: String,
    pub since: String,
}

/// A tier asking for more, with the number that says how much.
#[derive(Debug, Clone, Serialize)]
pub struct Behind {
    pub lag: Lag,
    /// Weeks without a release, or without a turn for `without-focus`.
    pub weeks: i64,
    pub rhythm_weeks: Option<u32>,
}

/// A version the calendar holds a project to.
#[derive(Debug, Clone)]
pub struct Aimed {
    pub version: String,
    pub title: Option<String>,
    pub week: Week,
    /// Weeks its week is past; zero for this week.
    pub weeks_late: i64,
}

/// The stage a project is on.
#[derive(Debug, Clone)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    /// Every open task of it is frozen or asleep: nobody is building it.
    pub put_down: bool,
}

/// What one project brings to the pick.
#[derive(Debug, Clone, Default)]
pub struct Standing {
    pub project: String,
    pub tier: Option<Tier>,
    pub stage: Option<Stage>,
    pub aimed: Option<Aimed>,
    /// Changes since its last release, oldest first: moment and subject.
    pub untagged: Vec<(String, String)>,
    /// What git says the repository holds that has gone nowhere yet.
    pub held: crate::git::Held,
    /// How its last release fell short of getting out.
    pub undelivered: Option<String>,
    /// When the sitting left open began.
    pub sitting: Option<String>,
    /// When it last had a sitting: the open one's start, or the last end.
    pub last_turn: Option<String>,
    /// What stops its tag and is out of reach, oldest first.
    pub blockers: Vec<Blocker>,
    /// Moments of what the owner decided outside a sitting since its last.
    pub answered: Vec<String>,
    pub behind: Option<Behind>,
    pub waiting: usize,
    pub asked: usize,
}

/// One project in the order of the pick, with what put it there.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub project: String,
    pub tier: Option<Tier>,
    pub reason: Reason,
    /// The version to work on: the one the calendar aims at for `late` and
    /// `due`, the current stage otherwise.
    pub version: String,
    pub title: Option<String>,
    /// The week the version is aimed at, for `late` and `due`.
    pub planned: Option<Week>,
    pub weeks_late: Option<i64>,
    /// What it holds that has not gone out, for `unfinished`.
    pub unfinished: Option<Unfinished>,
    pub answered: usize,
    pub behind: Option<Behind>,
    /// The day of its last sitting.
    pub last_turn: Option<String>,
    pub waiting: usize,
    pub asked: usize,
    /// When the reason began to hold, for the reasons that wait.
    #[serde(skip)]
    pub(crate) since: Option<String>,
    #[serde(skip)]
    pub(crate) turn_at: Option<String>,
}

/// Why a project is not in the order at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aside {
    /// Something out of reach stops its tag.
    Blocked,
    /// Its tier is `out`: worked on when asked.
    Out,
    /// Nothing is planned for it, so there is no stage to work on.
    NoStage,
    /// Everything planned was frozen or put to sleep on purpose.
    PutDown,
}

impl Aside {
    fn describe(self) -> &'static str {
        match self {
            Aside::Blocked => "blocked",
            Aside::Out => "out of the rotation - worked on when asked",
            Aside::NoStage => "no stage planned",
            Aside::PutDown => "everything planned is put down",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LeftOut {
    pub project: String,
    pub why: Aside,
    /// What blocks it, for `blocked`.
    pub blocker: Option<Blocker>,
}

/// The whole answer: the one to work on, the order the rest wait in, and
/// what the rule did not consider.
#[derive(Debug, Clone, Serialize)]
pub struct Pick {
    pub week: Week,
    pub pick: Option<Candidate>,
    pub then: Vec<Candidate>,
    pub left_out: Vec<LeftOut>,
    /// Projects whose git did not read: their facts are as of the last
    /// time it did.
    pub unread: Vec<String>,
}

/// The pick in a line, for a screen that has room for one.
#[derive(Debug, Clone, Serialize)]
pub struct Named {
    pub project: String,
    pub version: String,
    pub title: Option<String>,
    pub reason: Reason,
}

impl Candidate {
    pub fn named(&self) -> Named {
        Named {
            project: self.project.clone(),
            version: self.version.clone(),
            title: self.title.clone(),
            reason: self.reason,
        }
    }
}

/// Reads git for every repository in the work, as `sync` does and without
/// asking GitHub, so the pick stands on today's commits rather than on the
/// last sitting's. A commit made in one project from a neighbour's sitting
/// is exactly what the record would otherwise not know. Returns the
/// projects whose git did not read.
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

/// What every repository holds, read side by side. Walking a working tree is
/// nearly all waiting on the disk, and the only slow part of a pick that a
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

/// What each project in the work brings, read from the record and from
/// the facts of the week the calendar screens already gather.
pub fn gather(db: &Db, focus: &[Focus], overdue: &[Focus], lapsed: &[Overdue], signals: &[Raised]) -> Result<Vec<Standing>> {
    let mut out = Vec::new();
    let projects = db.projects()?;
    let mut held = read_held(db, &projects)?;
    for project in projects {
        // A place the record keeps for itself is not work anyone does.
        if !project.kind.has_a_place() {
            continue;
        }
        let tier = project.tier.as_deref().and_then(|t| Tier::parse(t).ok());
        let stage = db.current_stage(project.id)?.map(|stage| {
            let open: Vec<&crate::db::Task> = stage.tasks.iter().chain(&stage.asleep).collect();
            Stage {
                put_down: !open.is_empty() && open.iter().all(|t| t.is_put_down()),
                version: stage.version,
                title: stage.title,
            }
        });

        // The latest promise first, and of two equally late the lower
        // version: a plan is a queue.
        let aimed = overdue
            .iter()
            .filter(|f| f.project == project.name)
            .min_by(|a, b| {
                b.overdue_weeks
                    .cmp(&a.overdue_weeks)
                    .then_with(|| version_order(&a.version).cmp(&version_order(&b.version)))
            })
            .or_else(|| focus.iter().filter(|f| f.project == project.name).min_by_key(|f| version_order(&f.version)))
            .map(|f| Aimed {
                version: f.version.clone(),
                title: f.title.clone(),
                week: f.planned,
                weeks_late: f.overdue_weeks.unwrap_or(0),
            });

        let released = db.last_release_moment(project.id)?;
        let open = db.open_session(project.id)?;
        let ended = db.last_ended_session(project.id)?.and_then(|s| s.ended_at);
        let last_turn = open.as_ref().map(|s| s.started_at.clone()).or_else(|| ended.clone());
        let (held, undelivered) = held.remove(&project.id).unwrap_or_default();

        out.push(Standing {
            tier,
            stage,
            aimed,
            untagged: db.changes_after(project.id, released.as_deref())?,
            held,
            undelivered,
            sitting: open.map(|s| s.started_at),
            last_turn,
            blockers: db
                .blockers(project.id)?
                .into_iter()
                .map(|b| Blocker {
                    id: b.id,
                    head: crate::context::headline(&b.body),
                    since: b.date,
                })
                .collect(),
            answered: db.owner_words_after(project.id, ended.as_deref())?,
            behind: behind_of(&project.name, project.rhythm_weeks, lapsed, signals),
            waiting: db.count_open_events(project.id, "question")? as usize,
            asked: db.asked_wishes(project.id)?.len(),
            project: project.name,
        });
    }
    Ok(out)
}

/// The way a project's tier asks for more, if it does: a floor broken
/// before a pace lapsed, because the floor is what the tier promised.
///
/// The tier C signal is left out on purpose. It says a second declared
/// product started before the first shipped, which is a reason to work on
/// the first one - not on the one it names.
fn behind_of(project: &str, rhythm: Option<u32>, lapsed: &[Overdue], signals: &[Raised]) -> Option<Behind> {
    signals
        .iter()
        .filter(|r| r.project == project)
        .find_map(|r| {
            let lag = match r.signal {
                Signal::MissedCycle => Lag::MissedCycle,
                Signal::WithoutFocus => Lag::WithoutFocus,
                Signal::SecondStart => return None,
            };
            Some(Behind {
                lag,
                weeks: r.weeks.unwrap_or_default(),
                rhythm_weeks: rhythm,
            })
        })
        .or_else(|| {
            lapsed.iter().find(|o| o.project == project).map(|o| Behind {
                lag: Lag::Rhythm,
                weeks: o.weeks,
                rhythm_weeks: Some(o.rhythm_weeks),
            })
        })
}

/// Puts every project in its place: the order, and what is left out of it.
pub fn choose(standings: &[Standing], now: Week) -> Pick {
    let mut ranked = Vec::new();
    let mut left_out = Vec::new();
    for standing in standings {
        match candidate(standing) {
            Ok(found) => ranked.push(found),
            Err(why) => left_out.push(LeftOut {
                project: standing.project.clone(),
                why,
                blocker: standing.blockers.first().filter(|_| why == Aside::Blocked).cloned(),
            }),
        }
    }
    ranked.sort_by(order);
    left_out.sort_by(|a, b| a.project.cmp(&b.project));
    let mut ranked = ranked.into_iter();
    Pick {
        week: now,
        pick: ranked.next(),
        then: ranked.collect(),
        left_out,
        unread: Vec::new(),
    }
}

/// The reason a project is in the order, or the reason it is not.
fn candidate(s: &Standing) -> Result<Candidate, Aside> {
    // Before anything else, the calendar included: nobody can ask for work
    // that something out of reach is stopping.
    if !s.blockers.is_empty() {
        return Err(Aside::Blocked);
    }
    let unfinished = Unfinished {
        changed: s.held.changed,
        untracked: s.held.untracked,
        unpushed: s.held.unpushed,
        operation: s.held.operation.clone(),
        untagged: s.untagged.iter().map(|(_, subject)| subject.clone()).collect(),
        undelivered: s.undelivered.clone(),
        sitting: s.sitting.clone(),
    };
    // A project set aside on purpose is not holding the line up, whatever
    // it was left holding. Having no plan does not set it aside: work done
    // with nothing planned still has to go out.
    let set_aside = s.tier == Some(Tier::Out) || s.stage.as_ref().is_some_and(|stage| stage.put_down);

    let unplanned = String::from("unplanned");
    let (reason, version, title, planned, weeks_late, since) = if !unfinished.is_empty() && !set_aside {
        let since = s.untagged.first().map(|(at, _)| at.clone()).or_else(|| s.last_turn.clone());
        let (version, title) = match &s.stage {
            Some(stage) => (&stage.version, &stage.title),
            None => (&unplanned, &None),
        };
        (Reason::Unfinished, version, title, None, None, since)
    } else {
        // The calendar is the owner's own word, so it holds even for a
        // project the rule would otherwise leave out: aiming a version at a
        // week is asking for it.
        match &s.aimed {
            Some(aimed) if aimed.weeks_late > 0 => (Reason::Late, &aimed.version, &aimed.title, Some(aimed.week), Some(aimed.weeks_late), None),
            Some(aimed) => (Reason::Due, &aimed.version, &aimed.title, Some(aimed.week), None, None),
            None => {
                if s.tier == Some(Tier::Out) {
                    return Err(Aside::Out);
                }
                let Some(stage) = &s.stage else {
                    return Err(Aside::NoStage);
                };
                if stage.put_down {
                    return Err(Aside::PutDown);
                }
                let (reason, since) = match (s.answered.first(), s.behind.is_some()) {
                    (Some(at), _) => (Reason::Answered, Some(at.clone())),
                    (None, true) => (Reason::Behind, None),
                    (None, false) => (Reason::Turn, None),
                };
                (reason, &stage.version, &stage.title, None, None, since)
            }
        }
    };

    Ok(Candidate {
        project: s.project.clone(),
        tier: s.tier,
        reason,
        version: version.clone(),
        title: title.clone(),
        planned,
        weeks_late,
        unfinished: (reason == Reason::Unfinished).then_some(unfinished),
        answered: s.answered.len(),
        behind: s.behind.clone(),
        last_turn: s.last_turn.as_deref().map(day_of),
        waiting: s.waiting,
        asked: s.asked,
        since,
        turn_at: s.last_turn.clone(),
    })
}

/// The order of the pick: the reason first, then what weighs within it,
/// then the name, so that two runs over one record agree.
fn order(a: &Candidate, b: &Candidate) -> Ordering {
    // A tier ranks A before B before C, and no tier after all of them.
    let tier = |c: &Candidate| c.tier.map_or(u8::MAX, |t| t as u8);
    a.reason
        .cmp(&b.reason)
        .then_with(|| match a.reason {
            // The promise broken longest ago first.
            Reason::Late => b
                .weeks_late
                .cmp(&a.weeks_late)
                .then_with(|| tier(a).cmp(&tier(b)))
                .then_with(|| a.turn_at.cmp(&b.turn_at)),
            Reason::Due => tier(a).cmp(&tier(b)).then_with(|| a.turn_at.cmp(&b.turn_at)),
            // What has waited longest first.
            Reason::Unfinished | Reason::Answered => a.since.cmp(&b.since),
            Reason::Behind => {
                let pace = |c: &Candidate| !c.behind.as_ref().is_some_and(|b| b.lag.is_a_floor());
                let weeks = |c: &Candidate| c.behind.as_ref().map_or(0, |b| b.weeks);
                pace(a).cmp(&pace(b)).then_with(|| tier(a).cmp(&tier(b))).then_with(|| weeks(b).cmp(&weeks(a)))
            }
            // Never having had a sitting is the longest wait of all, and
            // `None` sorts first.
            Reason::Turn => a.turn_at.cmp(&b.turn_at),
        })
        .then_with(|| a.project.cmp(&b.project))
}

fn day_of(moment: &str) -> String {
    moment.split(['T', ' ']).next().unwrap_or(moment).to_string()
}

/// How many of the rest are named on the screen; the JSON has them all.
const SHOWN: usize = 5;

/// What a project left unfinished holds, in a phrase.
pub fn unfinished_parts(c: &Candidate) -> String {
    c.unfinished.as_ref().map(|u| u.parts().join(", ")).unwrap_or_default()
}

/// The reason in a sentence, for the project picked.
pub fn why(c: &Candidate) -> String {
    let weeks = |n: i64| plural(n.max(0) as usize, "week", "weeks");
    match c.reason {
        Reason::Late => format!(
            "aimed at {}, {} past its week",
            c.planned.map(|w| w.to_string()).unwrap_or_default(),
            weeks(c.weeks_late.unwrap_or_default())
        ),
        Reason::Due => format!("aimed at this week, {}", c.planned.map(|w| w.to_string()).unwrap_or_default()),
        Reason::Unfinished => format!("not finished: {}", unfinished_parts(c)),
        Reason::Answered => format!(
            "the owner decided {} outside a sitting since its last one",
            plural(c.answered, "thing", "things")
        ),
        Reason::Behind => behind_line(c, weeks),
        Reason::Turn => match &c.last_turn {
            Some(day) => format!("nothing presses - its last sitting, {day}, is the longest ago"),
            None => "nothing presses - it has never had a sitting".to_string(),
        },
    }
}

fn behind_line(c: &Candidate, weeks: impl Fn(i64) -> String) -> String {
    let tier = c.tier.map(|t| t.to_string()).unwrap_or_default();
    let Some(behind) = &c.behind else {
        return format!("tier {tier} asks for more");
    };
    match behind.lag {
        Lag::Rhythm => format!(
            "tier {tier}: {} without a release, its rhythm is {}",
            weeks(behind.weeks),
            weeks(i64::from(behind.rhythm_weeks.unwrap_or_default()))
        ),
        Lag::MissedCycle => format!("tier {tier}: more than one cycle missed - {} without a release", weeks(behind.weeks)),
        Lag::WithoutFocus => format!("tier {tier}: no turn in the focus for {}", weeks(behind.weeks)),
    }
}

/// The reason in a few words, for a line among others.
pub fn short(c: &Candidate) -> String {
    match c.reason {
        Reason::Late => format!("{} late", plural(c.weeks_late.unwrap_or_default().max(0) as usize, "week", "weeks")),
        Reason::Due => "aimed at this week".to_string(),
        Reason::Unfinished => unfinished_parts(c),
        Reason::Answered => format!("{} from the owner", plural(c.answered, "answer", "answers")),
        Reason::Behind => format!("tier {} asks for more", c.tier.map(|t| t.to_string()).unwrap_or_default()),
        Reason::Turn => match &c.last_turn {
            Some(day) => format!("last sitting {day}"),
            None => "never had a sitting".to_string(),
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
    if c.waiting > 0 {
        out.push(format!("{} on the owner", waits(c.waiting, "question", "questions")));
    }
    if c.asked > 0 {
        out.push(format!("{} from neighbours", plural(c.asked, "order", "orders")));
    }
    out
}

pub fn render(pick: &Pick) -> String {
    let mut out = String::new();
    match &pick.pick {
        Some(first) => {
            let tier = first.tier.map(|t| format!(" [{t}]")).unwrap_or_default();
            let title = first.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            out.push_str(&format!("Work on {}{tier}  {}{title}\n", first.project, first.version));
            out.push_str(&format!("  {}\n", why(first)));
            let also = also(first);
            if !also.is_empty() {
                out.push_str(&format!("  {}\n", also.join(" · ")));
            }
        }
        None if pick.left_out.is_empty() => out.push_str("Nothing to pick: no project is in the work.\n"),
        None => out.push_str("Nothing to pick: every project is left out, below.\n"),
    }

    if !pick.then.is_empty() {
        let shown = &pick.then[..pick.then.len().min(SHOWN)];
        let name = shown.iter().map(|c| c.project.chars().count()).max().unwrap_or(0);
        let version = shown.iter().map(|c| c.version.chars().count()).max().unwrap_or(0);
        out.push_str("\nThen, by the same rule\n");
        for c in shown {
            out.push_str(&format!("  {:<name$}  {:<version$}  {}\n", c.project, c.version, short(c)));
        }
        if pick.then.len() > SHOWN {
            out.push_str(&format!("  and {} more - `rigger pick --json` lists every one\n", pick.then.len() - SHOWN));
        }
    }

    if !pick.left_out.is_empty() {
        let name = pick.left_out.iter().map(|l| l.project.chars().count()).max().unwrap_or(0);
        out.push_str("\nLeft out\n");
        for item in &pick.left_out {
            let why = match &item.blocker {
                Some(blocker) => format!("blocked since {}: {} [{}]", blocker.since, blocker.head, blocker.id),
                None => item.why.describe().to_string(),
            };
            out.push_str(&format!("  {:<name$}  {why}\n", item.project));
        }
    }

    if !pick.unread.is_empty() {
        out.push_str(&format!(
            "\nGit did not read for {}: what the pick knows of {} is as of the last time it did.\n",
            pick.unread.join(", "),
            if pick.unread.len() == 1 { "it" } else { "them" }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week(text: &str) -> Week {
        Week::parse(text).unwrap()
    }

    fn on_stage(project: &str, turn: Option<&str>) -> Standing {
        Standing {
            project: project.to_string(),
            stage: Some(Stage {
                version: "v0.2.0".to_string(),
                title: Some("Second".to_string()),
                put_down: false,
            }),
            last_turn: turn.map(String::from),
            ..Standing::default()
        }
    }

    fn order_of(standings: &[Standing]) -> Vec<String> {
        let pick = choose(standings, week("2026-W44"));
        pick.pick.iter().chain(&pick.then).map(|c| c.project.clone()).collect()
    }

    fn reasons(standings: &[Standing]) -> Vec<Reason> {
        let pick = choose(standings, week("2026-W44"));
        pick.pick.iter().chain(&pick.then).map(|c| c.reason).collect()
    }

    /// Nothing presses anywhere, so the pick goes round: the project whose
    /// last sitting is furthest back comes first, and one that never had a
    /// sitting is further back than any.
    #[test]
    fn with_nothing_pressing_the_longest_without_a_sitting_comes_first() {
        let order = order_of(&[
            on_stage("recent", Some("2026-10-26T10:00:00Z")),
            on_stage("older", Some("2026-10-20T10:00:00Z")),
            on_stage("never", None),
        ]);
        assert_eq!(order, vec!["never", "older", "recent"]);
        assert_eq!(reasons(&[on_stage("a", Some("2026-10-26T10:00:00Z"))]), vec![Reason::Turn]);
    }

    /// The order of the reasons is the rule itself: every one outranks the
    /// next, whatever the clock says within it.
    #[test]
    fn each_reason_outranks_the_next() {
        let mut late = on_stage("late", Some("2026-10-26T10:00:00Z"));
        late.aimed = Some(Aimed {
            version: "v0.3.0".into(),
            title: None,
            week: week("2026-W42"),
            weeks_late: 2,
        });
        let mut due = on_stage("due", Some("2026-10-26T10:00:00Z"));
        due.aimed = Some(Aimed {
            version: "v0.3.0".into(),
            title: None,
            week: week("2026-W44"),
            weeks_late: 0,
        });
        let mut unfinished = on_stage("unfinished", Some("2026-10-26T10:00:00Z"));
        unfinished.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: a thing".into())];
        let mut answered = on_stage("answered", Some("2026-10-26T10:00:00Z"));
        answered.answered = vec!["2026-10-27T08:00:00Z".into()];
        let mut behind = on_stage("behind", Some("2026-10-26T10:00:00Z"));
        behind.behind = Some(Behind {
            lag: Lag::Rhythm,
            weeks: 5,
            rhythm_weeks: Some(2),
        });
        // Longest without a sitting of them all, and still last.
        let turn = on_stage("turn", None);

        let order = order_of(&[turn, behind, answered, due, late, unfinished]);
        assert_eq!(order, vec!["unfinished", "late", "due", "answered", "behind", "turn"]);
    }

    /// Each thing a turn can be left holding is enough on its own, and says
    /// what it is.
    #[test]
    fn whatever_has_not_gone_out_leaves_a_project_unfinished() {
        type Case = (&'static str, fn(&mut Standing));
        let cases: [Case; 7] = [
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
        ];
        for (said, set) in cases {
            let mut s = on_stage("alpha", Some("2026-10-26T10:00:00Z"));
            set(&mut s);
            let first = choose(&[s], week("2026-W44")).pick.unwrap();
            assert_eq!(first.reason, Reason::Unfinished, "{said}");
            assert_eq!(why(&first), format!("not finished: {said}"));
        }
        // With nothing held, nothing is said.
        let first = choose(&[on_stage("alpha", None)], week("2026-W44")).pick.unwrap();
        assert_eq!(first.reason, Reason::Turn);
        assert_eq!(first.unfinished, None);
    }

    /// Work done with nothing planned still has to go out; work left in a
    /// project set aside on purpose holds nothing up.
    #[test]
    fn a_project_set_aside_is_not_unfinished_and_one_without_a_plan_is() {
        let mut bare = on_stage("bare", None);
        bare.stage = None;
        bare.held.changed = 1;
        let first = choose(&[bare], week("2026-W44")).pick.unwrap();
        assert_eq!(first.reason, Reason::Unfinished);
        assert_eq!(first.version, "unplanned");

        let mut out = on_stage("out", None);
        out.tier = Some(Tier::Out);
        out.held.changed = 1;
        let mut asleep = on_stage("asleep", None);
        asleep.stage.as_mut().unwrap().put_down = true;
        asleep.held.changed = 1;
        let pick = choose(&[out, asleep], week("2026-W44"));
        assert!(pick.pick.is_none(), "{:?}", pick.pick);
        let aside: Vec<Aside> = pick.left_out.iter().map(|l| l.why).collect();
        assert_eq!(aside, vec![Aside::PutDown, Aside::Out]);
    }

    /// A blocker is the way out for a project waiting on the outside world:
    /// it leaves the order with its reason, unfinished or not, whatever the
    /// calendar says, and the next project is picked.
    #[test]
    fn a_blocked_project_is_left_out_with_what_blocks_it() {
        let mut blocked = on_stage("blocked", None);
        blocked.held.unpushed = 2;
        blocked.aimed = Some(Aimed {
            version: "v0.3.0".into(),
            title: None,
            week: week("2026-W44"),
            weeks_late: 0,
        });
        blocked.blockers = vec![Blocker {
            id: 7,
            head: "The signing key has expired".into(),
            since: "2026-11-01".into(),
        }];
        let next = on_stage("next", Some("2026-10-26T10:00:00Z"));
        let pick = choose(&[blocked, next], week("2026-W44"));
        assert_eq!(pick.pick.as_ref().unwrap().project, "next");
        assert_eq!(pick.left_out[0].why, Aside::Blocked);
        let said = render(&pick);
        assert!(said.contains("blocked since 2026-11-01: The signing key has expired [7]"), "{said}");
    }

    #[test]
    fn of_two_unfinished_the_one_waiting_longer_comes_first() {
        let mut newer = on_stage("newer", Some("2026-10-20T10:00:00Z"));
        newer.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: b".into())];
        let mut older = on_stage("older", Some("2026-10-26T10:00:00Z"));
        older.untagged = vec![("2026-10-26T11:00:00Z".into(), "feat: a".into())];
        assert_eq!(order_of(&[newer, older]), vec!["older", "newer"]);
    }

    /// A floor the tier promised outranks a pace it lapsed, and then the
    /// higher tier, and then the longer wait.
    #[test]
    fn behind_a_floor_comes_before_behind_a_rhythm() {
        let lagging = |name: &str, tier: Tier, lag: Lag, weeks: i64| {
            let mut s = on_stage(name, Some("2026-10-26T10:00:00Z"));
            s.tier = Some(tier);
            s.behind = Some(Behind {
                lag,
                weeks,
                rhythm_weeks: Some(2),
            });
            s
        };
        let order = order_of(&[
            lagging("pace-a", Tier::A, Lag::Rhythm, 9),
            lagging("floor-b", Tier::B, Lag::WithoutFocus, 7),
            lagging("pace-b-long", Tier::B, Lag::Rhythm, 12),
            lagging("pace-b-short", Tier::B, Lag::Rhythm, 5),
        ]);
        assert_eq!(order, vec!["floor-b", "pace-a", "pace-b-long", "pace-b-short"]);
    }

    #[test]
    fn the_latest_promise_comes_first_and_then_the_higher_tier() {
        let aimed = |name: &str, tier: Option<Tier>, late: i64| {
            let mut s = on_stage(name, Some("2026-10-26T10:00:00Z"));
            s.tier = tier;
            s.aimed = Some(Aimed {
                version: "v0.3.0".into(),
                title: None,
                week: week("2026-W40"),
                weeks_late: late,
            });
            s
        };
        let order = order_of(&[
            aimed("one-week", Some(Tier::A), 1),
            aimed("untiered", None, 4),
            aimed("tier-b", Some(Tier::B), 4),
        ]);
        assert_eq!(order, vec!["tier-b", "untiered", "one-week"]);
    }

    /// Left out, each with its reason - and the calendar overrides all
    /// three, because aiming a version at a week is asking for it.
    #[test]
    fn out_no_stage_and_put_down_are_left_out_unless_aimed() {
        let mut out = on_stage("out", None);
        out.tier = Some(Tier::Out);
        let mut bare = on_stage("bare", None);
        bare.stage = None;
        let mut asleep = on_stage("asleep", None);
        asleep.stage.as_mut().unwrap().put_down = true;
        let mut asked = on_stage("asked", None);
        asked.tier = Some(Tier::Out);
        asked.stage = None;
        asked.aimed = Some(Aimed {
            version: "v1.0.0".into(),
            title: None,
            week: week("2026-W44"),
            weeks_late: 0,
        });

        let pick = choose(&[out, bare, asleep, asked], week("2026-W44"));
        let first = pick.pick.expect("the aimed one is picked");
        assert_eq!(first.project, "asked");
        assert_eq!(first.version, "v1.0.0");
        assert!(pick.then.is_empty());
        let aside: Vec<(&str, Aside)> = pick.left_out.iter().map(|l| (l.project.as_str(), l.why)).collect();
        assert_eq!(aside, vec![("asleep", Aside::PutDown), ("bare", Aside::NoStage), ("out", Aside::Out)]);
    }

    /// A question to the owner does not stop a stage, so it does not take
    /// a project out of the pick - it is counted on its line.
    #[test]
    fn questions_and_orders_are_counted_not_obeyed() {
        let mut s = on_stage("alpha", Some("2026-10-26T10:00:00Z"));
        s.waiting = 3;
        s.asked = 2;
        let pick = choose(&[s], week("2026-W44"));
        let first = pick.pick.unwrap();
        assert_eq!(first.reason, Reason::Turn);
        let said = render(&choose(
            &[Standing {
                waiting: 3,
                asked: 2,
                ..on_stage("alpha", Some("2026-10-26T10:00:00Z"))
            }],
            week("2026-W44"),
        ));
        assert!(said.contains("3 questions wait on the owner"), "{said}");
        assert!(said.contains("2 orders from neighbours"), "{said}");
        assert_eq!(first.waiting, 3);
    }

    #[test]
    fn the_screen_names_five_of_the_rest_and_counts_the_others() {
        let standings: Vec<Standing> = (0..9).map(|n| on_stage(&format!("p{n}"), Some(&format!("2026-10-1{n}T10:00:00Z")))).collect();
        let said = render(&choose(&standings, week("2026-W44")));
        assert!(said.starts_with("Work on p0"), "{said}");
        assert!(said.contains("p5"), "{said}");
        assert!(!said.contains("p6"), "{said}");
        assert!(said.contains("and 3 more"), "{said}");
    }

    #[test]
    fn an_empty_line_says_there_is_nothing_to_pick() {
        assert!(render(&choose(&[], week("2026-W44"))).starts_with("Nothing to pick: no project"));
        let mut bare = on_stage("bare", None);
        bare.stage = None;
        assert!(render(&choose(&[bare], week("2026-W44"))).starts_with("Nothing to pick: every project is left out"));
    }
}
