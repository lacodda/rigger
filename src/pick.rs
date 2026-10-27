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
//! 1. `late` - a version aimed at a week already gone: the owner's own
//!    promise, broken.
//! 2. `due` - a version aimed at this week: the owner's own word.
//! 3. `unseen` - changes since the last release that no finished sitting of
//!    the project has seen: a commit made from a neighbour's sitting, or a
//!    sitting left open. The record has lost the thread there, and work
//!    nobody is following is the work that never ships.
//! 4. `answered` - the owner decided something about it outside a sitting,
//!    after its last one. An answer waits to be acted on.
//! 5. `behind` - its tier asks for more than it has had: a floor gone
//!    under, or a rhythm lapsed.
//! 6. `turn` - nothing presses, so the one longest without a sitting.
//!
//! Changes a sitting saw and left untagged are not a reason of their own:
//! the sitting knew, and chose. Counting them as unfinished would pick the
//! same project on every run while it waits on something outside it.
//!
//! Questions waiting on the owner do not take a project out of the pick -
//! by the line's rules a question does not stop a stage - and neither do
//! its neighbours' orders. Both are counted on its line instead.

use std::cmp::Ordering;

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
    Late,
    Due,
    Unseen,
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
    /// When its last finished sitting ended.
    pub seen_until: Option<String>,
    pub sitting_open: bool,
    /// When it last had a sitting: the open one's start, or the last end.
    pub last_turn: Option<String>,
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
    /// Changes since the last release that no finished sitting has seen.
    pub unseen: Vec<String>,
    /// Changes since the last release, seen or not.
    pub untagged: usize,
    pub answered: usize,
    pub behind: Option<Behind>,
    /// The day of its last sitting.
    pub last_turn: Option<String>,
    pub sitting_open: bool,
    pub waiting: usize,
    pub asked: usize,
    /// When the reason began to hold, for the two reasons that wait.
    #[serde(skip)]
    since: Option<String>,
    #[serde(skip)]
    turn_at: Option<String>,
}

/// Why a project is not in the order at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aside {
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

/// What each project in the work brings, read from the record and from
/// the facts of the week the calendar screens already gather.
pub fn gather(db: &Db, focus: &[Focus], overdue: &[Focus], lapsed: &[Overdue], signals: &[Raised]) -> Result<Vec<Standing>> {
    let mut out = Vec::new();
    for project in db.projects()? {
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
        let seen_until = db.last_ended_session(project.id)?.and_then(|s| s.ended_at);
        let last_turn = open.as_ref().map(|s| s.started_at.clone()).or_else(|| seen_until.clone());

        out.push(Standing {
            tier,
            stage,
            aimed,
            untagged: db.changes_after(project.id, released.as_deref())?,
            sitting_open: open.is_some(),
            last_turn,
            answered: db.owner_words_after(project.id, seen_until.as_deref())?,
            behind: behind_of(&project.name, project.rhythm_weeks, lapsed, signals),
            seen_until,
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
    let unseen: Vec<&(String, String)> = s
        .untagged
        .iter()
        .filter(|(at, _)| s.seen_until.as_deref().is_none_or(|end| at.as_str() > end))
        .collect();

    // The calendar is the owner's own word, so it holds even for a project
    // the rule would otherwise leave out: aiming a version at a week is
    // asking for it.
    let (reason, version, title, planned, weeks_late, since) = match &s.aimed {
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
            let (reason, since) = match (unseen.first(), s.answered.first()) {
                (Some((at, _)), _) => (Reason::Unseen, Some(at.clone())),
                (None, Some(at)) => (Reason::Answered, Some(at.clone())),
                (None, None) if s.behind.is_some() => (Reason::Behind, None),
                (None, None) => (Reason::Turn, None),
            };
            (reason, &stage.version, &stage.title, None, None, since)
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
        unseen: unseen.iter().map(|(_, subject)| subject.clone()).collect(),
        untagged: s.untagged.len(),
        answered: s.answered.len(),
        behind: s.behind.clone(),
        last_turn: s.last_turn.as_deref().map(day_of),
        sitting_open: s.sitting_open,
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
            Reason::Unseen | Reason::Answered => a.since.cmp(&b.since),
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
        Reason::Unseen => {
            let first = c.unseen.first().map(String::as_str).unwrap_or_default();
            let more = match c.unseen.len() {
                0 | 1 => String::new(),
                n => format!(" - and {} more", n - 1),
            };
            format!(
                "{} since the last release that no finished sitting has seen: {first}{more}",
                plural(c.unseen.len(), "change", "changes")
            )
        }
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
        Reason::Unseen => format!("{} unseen", plural(c.unseen.len(), "change", "changes")),
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
    if c.sitting_open {
        out.push("a sitting is open".to_string());
    }
    if c.reason != Reason::Unseen && c.untagged > 0 {
        out.push(format!("{} for the next tag", waits(c.untagged, "change", "changes")));
    }
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
            out.push_str(&format!("  {:<name$}  {}\n", item.project, item.why.describe()));
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
            seen_until: turn.map(String::from),
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
        let mut unseen = on_stage("unseen", Some("2026-10-26T10:00:00Z"));
        unseen.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: a thing".into())];
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

        let order = order_of(&[turn, behind, answered, unseen, due, late]);
        assert_eq!(order, vec!["late", "due", "unseen", "answered", "behind", "turn"]);
    }

    /// The case the reason exists for: a change made after the project's
    /// last sitting ended - from a neighbour's sitting, say - is work the
    /// record lost the thread of.
    #[test]
    fn a_change_after_the_last_sitting_is_unseen() {
        let mut s = on_stage("alpha", Some("2026-10-26T10:00:00Z"));
        s.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: generate plugins".into())];
        let pick = choose(&[s], week("2026-W44"));
        let first = pick.pick.unwrap();
        assert_eq!(first.reason, Reason::Unseen);
        assert_eq!(first.unseen, vec!["feat: generate plugins"]);
        assert!(why(&first).contains("feat: generate plugins"), "{}", why(&first));
    }

    /// And the case it must not catch: a fix the sitting made a minute
    /// before it ended and chose not to tag. The sitting knew; picking the
    /// project for it would pick it again on every run while it waits on
    /// something outside it.
    #[test]
    fn a_change_a_sitting_saw_and_left_untagged_is_not_a_reason() {
        let mut s = on_stage("alpha", Some("2026-10-22T12:37:25Z"));
        s.untagged = vec![("2026-10-22T12:36:34Z".into(), "fix: load the rows".into())];
        let pick = choose(&[s], week("2026-W44"));
        let first = pick.pick.unwrap();
        assert_eq!(first.reason, Reason::Turn);
        assert!(first.unseen.is_empty());
        assert_eq!(first.untagged, 1);
        assert!(also(&first).iter().any(|a| a == "1 change waits for the next tag"), "{:?}", also(&first));
    }

    /// A project that never had a finished sitting has seen none of its
    /// changes.
    #[test]
    fn with_no_finished_sitting_every_untagged_change_is_unseen() {
        let mut s = on_stage("alpha", None);
        s.untagged = vec![("2026-10-27T09:00:00Z".into(), "feat: a".into())];
        assert_eq!(reasons(&[s]), vec![Reason::Unseen]);
    }

    #[test]
    fn of_two_unseen_the_one_waiting_longer_comes_first() {
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
