//! The cycle looked back on: what the plan said, what the tags say, and
//! where the two parted company.
//!
//! The written calendar asked for this every seven weeks - "a check of the
//! calendar against reality: what slipped, what turned out dearer, whether
//! the tiers need moving" - and had no way to do it, because nothing there
//! ever read a tag. So the check was a thing to remember, and a thing to
//! remember is a thing that stops happening.
//!
//! Nothing here is a new fact. A retro is the facts already recorded, read
//! backwards over a window instead of forwards over a week: `calendar` and
//! `next` ask what is coming, this asks what happened. The answer is worth
//! having precisely because it is not the answer anyone expected - the line
//! that planned seven versions in seven weeks shipped ninety-four in one.

use serde::Serialize;

use crate::calendar::{Planned, Tier, Week};

/// The length of a cycle, in weeks, as the owner's calendar set it: seven,
/// so that tier A gets two turns, tier B one each and tier C one.
pub const CYCLE_WEEKS: u32 = 7;

/// A release that happened inside the window, and what the plan had said
/// about it.
#[derive(Debug, Clone, Serialize)]
pub struct Shipped {
    pub project: String,
    pub version: String,
    pub week: Week,
    /// The week it was aimed at, when it was aimed at all.
    pub planned: Option<Week>,
    /// Weeks between the plan and the tag; positive is late.
    pub slip: Option<i64>,
}

impl Shipped {
    /// Shipped in the week it was aimed at.
    pub fn on_time(&self) -> bool {
        self.slip == Some(0)
    }

    /// Shipped without ever having been aimed anywhere.
    pub fn unplanned(&self) -> bool {
        self.planned.is_none()
    }
}

/// A version aimed at a week inside the window that never got a tag.
#[derive(Debug, Clone, Serialize)]
pub struct Missed {
    pub project: String,
    pub version: String,
    pub planned: Week,
    /// Weeks from the planned week to the end of the window.
    pub weeks: i64,
}

/// What one project did with the window it was given.
#[derive(Debug, Clone, Serialize)]
pub struct Standing {
    pub project: String,
    pub tier: Option<Tier>,
    pub rhythm_weeks: Option<u32>,
    pub shipped: usize,
    pub planned_and_shipped: usize,
    pub missed: usize,
    /// Releases the tier's rhythm asked for over this window.
    pub expected: Option<u32>,
}

/// The way a tier has stopped describing a project.
///
/// The two directions are kept apart because they are different problems
/// and want different answers. A product shipping twenty times what its
/// tier asks is not behind, it has outgrown the tier; a product shipping
/// nothing is not busy, it is stalled. Naming both "misfit" in one list
/// loses exactly the distinction the owner needs to act on - which was
/// visible the first time this ran on the real record, where seven of
/// fifteen projects were called misfits and the two that were stalled sat
/// buried among five that were racing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Misfit {
    /// Shipping far above what the tier asks for.
    Outgrown,
    /// Nothing shipped at all where the tier asked for something.
    Stalled,
}

impl Standing {
    /// How the tier has stopped describing this project, if it has.
    ///
    /// Not a verdict: the calendar asked "do the tiers need moving", and
    /// the honest answer is a comparison between what a tier asked for and
    /// what happened, held up for the owner to read.
    pub fn misfit(&self) -> Option<Misfit> {
        let expected = self.expected?;
        if self.shipped == 0 {
            return Some(Misfit::Stalled);
        }
        // Twice the asked-for pace is where a tier stops being a
        // description and starts being a label. Anything under is the tier
        // working, however lumpy the weeks were.
        (self.shipped as u32 >= expected.saturating_mul(2)).then_some(Misfit::Outgrown)
    }

    /// How many times over the asked-for pace this project went.
    pub fn times_over(&self) -> Option<u32> {
        let expected = self.expected.filter(|e| *e > 0)?;
        Some(self.shipped as u32 / expected)
    }
}

/// The whole look back.
#[derive(Debug, Clone, Serialize)]
pub struct Retro {
    pub from: Week,
    pub to: Week,
    pub shipped: Vec<Shipped>,
    pub missed: Vec<Missed>,
    pub standings: Vec<Standing>,
}

impl Retro {
    pub fn weeks(&self) -> i64 {
        self.from.until(self.to) + 1
    }

    /// Releases that landed in the week they were aimed at.
    pub fn on_time(&self) -> usize {
        self.shipped.iter().filter(|s| s.on_time()).count()
    }

    /// Releases that were aimed somewhere and landed somewhere else.
    pub fn slipped(&self) -> usize {
        self.shipped.iter().filter(|s| s.slip.is_some_and(|n| n != 0)).count()
    }

    /// Releases nobody aimed at all.
    pub fn unplanned(&self) -> usize {
        self.shipped.iter().filter(|s| s.unplanned()).count()
    }

    /// How much the plan actually covered, as a share of what shipped.
    ///
    /// This is the number the written calendar could never produce, and the
    /// one that says whether planning is happening at all: a line where
    /// every release is unplanned has a calendar in name only.
    pub fn planned_share(&self) -> Option<u32> {
        let total = self.shipped.len();
        if total == 0 {
            return None;
        }
        let planned = total - self.unplanned();
        Some(((planned * 100) / total) as u32)
    }

    /// The worst slip in the window, for the line that names it.
    pub fn worst_slip(&self) -> Option<&Shipped> {
        self.shipped.iter().filter(|s| s.slip.is_some_and(|n| n > 0)).max_by_key(|s| s.slip)
    }

    /// Projects whose tier no longer describes what they do, in the
    /// direction it stopped describing them.
    pub fn misfits(&self, kind: Misfit) -> Vec<&Standing> {
        let mut out: Vec<&Standing> = self.standings.iter().filter(|s| s.misfit() == Some(kind)).collect();
        // Furthest from its tier first, so a list that has to be cut keeps
        // the projects the tier fits worst.
        out.sort_by(|a, b| match kind {
            Misfit::Outgrown => b.times_over().cmp(&a.times_over()).then_with(|| a.project.cmp(&b.project)),
            Misfit::Stalled => b.expected.cmp(&a.expected).then_with(|| a.project.cmp(&b.project)),
        });
        out
    }
}

/// Reads a window of the record back.
///
/// `versions` is every project's calendar versions; `projects` carries the
/// tier and rhythm each was working under.
pub fn look_back(from: Week, to: Week, versions: &[Planned], projects: &[(String, Option<Tier>, Option<u32>)]) -> Retro {
    let mut shipped = Vec::new();
    let mut missed = Vec::new();

    for version in versions {
        match (version.shipped, version.planned) {
            (Some(week), planned) if week >= from && week <= to => shipped.push(Shipped {
                project: version.project.clone(),
                version: version.version.clone(),
                week,
                planned,
                slip: version.slip(),
            }),
            // A version aimed inside the window with no tag anywhere. The
            // window's end is what it is late against, not today: a retro
            // read months later would otherwise grow the number every time
            // it was run, and a look back has to give the same answer twice.
            (None, Some(planned)) if planned >= from && planned <= to => missed.push(Missed {
                project: version.project.clone(),
                version: version.version.clone(),
                planned,
                weeks: planned.until(to),
            }),
            _ => {}
        }
    }

    shipped.sort_by(|a, b| {
        a.week
            .cmp(&b.week)
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| crate::db::version_order(&a.version).cmp(&crate::db::version_order(&b.version)))
    });
    missed.sort_by(|a, b| b.weeks.cmp(&a.weeks).then_with(|| a.project.cmp(&b.project)));

    let span = from.until(to) + 1;
    let mut standings: Vec<Standing> = projects
        .iter()
        .map(|(project, tier, rhythm)| {
            let mine: Vec<&Shipped> = shipped.iter().filter(|s| &s.project == project).collect();
            Standing {
                project: project.clone(),
                tier: *tier,
                rhythm_weeks: *rhythm,
                shipped: mine.len(),
                planned_and_shipped: mine.iter().filter(|s| !s.unplanned()).count(),
                missed: missed.iter().filter(|m| &m.project == project).count(),
                // A rhythm of two weeks over a seven-week window asks for
                // three releases, not three and a half: the calendar counts
                // releases, and there is no half of one.
                expected: expected_releases(*tier, *rhythm, span),
            }
        })
        .filter(|s| s.shipped > 0 || s.missed > 0 || s.tier.is_some_and(|t| t != Tier::Out))
        .collect();

    // Busiest first: a retro is read to find where the work went.
    standings.sort_by(|a, b| {
        b.shipped
            .cmp(&a.shipped)
            .then_with(|| b.missed.cmp(&a.missed))
            .then_with(|| a.project.cmp(&b.project))
    });

    Retro {
        from,
        to,
        shipped,
        missed,
        standings,
    }
}

/// How many releases a rhythm asks for over a span of weeks.
///
/// `None` when nothing was promised - no tier, or a tier that is out of the
/// rotation. A promise nobody made cannot be broken, and inventing one here
/// would have the retro grading projects the owner deliberately set aside.
fn expected_releases(tier: Option<Tier>, rhythm: Option<u32>, span: i64) -> Option<u32> {
    let tier = tier?;
    if tier == Tier::Out {
        return None;
    }
    let rhythm = rhythm.or_else(|| tier.default_rhythm())?;
    if rhythm == 0 || span <= 0 {
        return None;
    }
    Some((span as u32) / rhythm)
}

/// The one-paragraph summary a retro leaves behind in the record.
///
/// Written as an event so that the next look back can find it: a retro that
/// is only ever printed leaves the same hole the written calendar had, where
/// the check happened and nobody could tell afterwards that it did.
pub fn summary(retro: &Retro) -> String {
    let mut parts = vec![format!(
        "{} to {} ({} weeks): {} shipped",
        retro.from,
        retro.to,
        retro.weeks(),
        retro.shipped.len()
    )];

    if !retro.shipped.is_empty() {
        parts.push(format!(
            "{} on time, {} slipped, {} unplanned",
            retro.on_time(),
            retro.slipped(),
            retro.unplanned()
        ));
        if let Some(share) = retro.planned_share() {
            parts.push(format!("{share}% of releases were planned"));
        }
    }
    if !retro.missed.is_empty() {
        parts.push(format!("{} planned and not shipped", retro.missed.len()));
    }
    if let Some(worst) = retro.worst_slip() {
        parts.push(format!("worst slip {} {} by {} weeks", worst.project, worst.version, worst.slip.unwrap_or(0)));
    }
    let stalled = retro.misfits(Misfit::Stalled);
    if !stalled.is_empty() {
        let named: Vec<String> = stalled.iter().map(|s| s.project.clone()).collect();
        parts.push(format!("stalled against their tier: {}", named.join(", ")));
    }
    let outgrown = retro.misfits(Misfit::Outgrown);
    if !outgrown.is_empty() {
        let named: Vec<String> = outgrown.iter().map(|s| s.project.clone()).collect();
        parts.push(format!("shipping past their tier: {}", named.join(", ")));
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week(text: &str) -> Week {
        Week::parse(text).unwrap()
    }

    fn version(project: &str, name: &str, planned: Option<&str>, shipped_at: Option<&str>) -> Planned {
        Planned {
            project: project.to_string(),
            version: name.to_string(),
            title: None,
            planned: planned.map(week),
            shipped: shipped_at.and_then(Week::of_recorded),
            shipped_at: shipped_at.map(String::from),
        }
    }

    fn projects(entries: &[(&str, Option<Tier>, Option<u32>)]) -> Vec<(String, Option<Tier>, Option<u32>)> {
        entries.iter().map(|(n, t, r)| (n.to_string(), *t, *r)).collect()
    }

    #[test]
    fn a_window_holds_only_what_happened_inside_it() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W40"),
            &[
                // Inside.
                version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-11")),
                // Before the window.
                version("alpha", "v0.0.1", None, Some("2026-08-28")),
                // After it.
                version("alpha", "v0.2.0", None, Some("2026-10-09")),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2))]),
        );
        let names: Vec<&str> = retro.shipped.iter().map(|s| s.version.as_str()).collect();
        assert_eq!(names, vec!["v0.1.0"]);
        assert_eq!(retro.weeks(), 4);
    }

    /// The three ways a release relates to its plan, which is the whole of
    /// what a retro is for.
    #[test]
    fn every_release_is_on_time_slipped_or_unplanned() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W40"),
            &[
                version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-11")),
                version("alpha", "v0.2.0", Some("2026-W38"), Some("2026-09-25")),
                version("beta", "v1.0.0", None, Some("2026-09-25")),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2)), ("beta", Some(Tier::B), Some(4))]),
        );
        assert_eq!(retro.on_time(), 1);
        assert_eq!(retro.slipped(), 1);
        assert_eq!(retro.unplanned(), 1);
        // Every release falls in exactly one of the three.
        assert_eq!(retro.on_time() + retro.slipped() + retro.unplanned(), retro.shipped.len());
    }

    /// The number the written calendar could never produce: how much of what
    /// shipped was ever planned. A line where nothing was planned has a
    /// calendar in name only, and that is worth being told plainly.
    #[test]
    fn the_planned_share_says_whether_planning_happened_at_all() {
        let all_unplanned = look_back(
            week("2026-W37"),
            week("2026-W40"),
            &[
                version("alpha", "v0.1.0", None, Some("2026-09-11")),
                version("alpha", "v0.2.0", None, Some("2026-09-18")),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2))]),
        );
        assert_eq!(all_unplanned.planned_share(), Some(0));

        let half = look_back(
            week("2026-W37"),
            week("2026-W40"),
            &[
                version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-11")),
                version("alpha", "v0.2.0", None, Some("2026-09-18")),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2))]),
        );
        assert_eq!(half.planned_share(), Some(50));

        // Nothing shipped is not nought per cent - it is no answer, and
        // printing 0% would read as a judgement on a quiet window.
        let empty = look_back(week("2026-W37"), week("2026-W40"), &[], &projects(&[("alpha", Some(Tier::A), Some(2))]));
        assert_eq!(empty.planned_share(), None);
    }

    /// A version aimed inside the window with no tag is measured against the
    /// window's end, not against today: a look back has to give the same
    /// answer whenever it is run.
    #[test]
    fn a_missed_release_is_measured_against_the_window_not_the_clock() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W40"),
            &[
                version("alpha", "v0.1.0", Some("2026-W37"), None),
                // Aimed after the window: not this retro's business.
                version("alpha", "v0.2.0", Some("2026-W44"), None),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2))]),
        );
        assert_eq!(retro.missed.len(), 1);
        assert_eq!(retro.missed[0].version, "v0.1.0");
        assert_eq!(retro.missed[0].weeks, 3);
    }

    #[test]
    fn a_rhythm_asks_for_whole_releases_over_a_window() {
        // Seven weeks at a release every two: three, not three and a half.
        assert_eq!(expected_releases(Some(Tier::A), Some(2), 7), Some(3));
        assert_eq!(expected_releases(Some(Tier::B), Some(4), 7), Some(1));
        // A tier with no rhythm of its own takes the tier's.
        assert_eq!(expected_releases(Some(Tier::C), None, 7), Some(1));
        // Out of the rotation promised nothing, so nothing is expected.
        assert_eq!(expected_releases(Some(Tier::Out), Some(2), 7), None);
        assert_eq!(expected_releases(None, Some(2), 7), None);
    }

    /// "Do the tiers need moving" was one of the three questions the written
    /// calendar asked. The answer is a comparison, not a verdict.
    #[test]
    fn a_tier_that_no_longer_describes_the_project_is_named() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W43"),
            &[
                // Tier C asks for one release in seven weeks; this shipped
                // six, which is not a declared product any more.
                version("busy", "v0.1.0", None, Some("2026-09-11")),
                version("busy", "v0.2.0", None, Some("2026-09-11")),
                version("busy", "v0.3.0", None, Some("2026-09-18")),
                version("busy", "v0.4.0", None, Some("2026-09-18")),
                version("busy", "v0.5.0", None, Some("2026-09-25")),
                version("busy", "v0.6.0", None, Some("2026-09-25")),
                // Tier A asks for three and got exactly three: a tier that
                // is working, and must not be named.
                version("steady", "v1.0.0", None, Some("2026-09-11")),
                version("steady", "v1.1.0", None, Some("2026-09-18")),
                version("steady", "v1.2.0", None, Some("2026-09-25")),
            ],
            &projects(&[
                ("busy", Some(Tier::C), Some(6)),
                ("steady", Some(Tier::A), Some(2)),
                ("silent", Some(Tier::A), Some(2)),
            ]),
        );
        // The two directions are different problems and are kept apart:
        // busy shipped six against one asked, silent shipped nothing.
        let outgrown: Vec<&str> = retro.misfits(Misfit::Outgrown).iter().map(|s| s.project.as_str()).collect();
        let stalled: Vec<&str> = retro.misfits(Misfit::Stalled).iter().map(|s| s.project.as_str()).collect();
        assert_eq!(outgrown, vec!["busy"], "{outgrown:?}");
        assert_eq!(stalled, vec!["silent"], "{stalled:?}");
        // A tier that is working is named in neither list.
        assert!(
            !outgrown.contains(&"steady") && !stalled.contains(&"steady"),
            "a working tier was called a misfit"
        );
    }

    /// Shipping nothing and shipping twenty times the pace are not the same
    /// problem, and a single list would bury the first among the second.
    /// The real record made this plain: seven of fifteen projects came back
    /// as misfits, and the two that were stalled sat among five racing.
    #[test]
    fn the_two_directions_of_a_misfit_are_told_apart() {
        let standing = |shipped: usize, expected: Option<u32>| Standing {
            project: "p".to_string(),
            tier: Some(Tier::B),
            rhythm_weeks: Some(4),
            shipped,
            planned_and_shipped: 0,
            missed: 0,
            expected,
        };
        assert_eq!(standing(0, Some(1)).misfit(), Some(Misfit::Stalled));
        assert_eq!(standing(2, Some(1)).misfit(), Some(Misfit::Outgrown));
        // Exactly what was asked, and just under twice it: the tier works.
        assert_eq!(standing(1, Some(1)).misfit(), None);
        assert_eq!(standing(3, Some(2)).misfit(), None);
        // Nothing promised, so nothing to fall short of - including a
        // project that shipped nothing.
        assert_eq!(standing(0, None).misfit(), None);
        assert_eq!(standing(9, None).misfit(), None);
    }

    #[test]
    fn a_project_out_of_the_rotation_is_never_a_misfit() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W43"),
            &[],
            &projects(&[("aside", Some(Tier::Out), Some(2)), ("untiered", None, None)]),
        );
        assert!(retro.misfits(Misfit::Stalled).is_empty(), "{:?}", retro.misfits(Misfit::Stalled));
        assert!(retro.misfits(Misfit::Outgrown).is_empty(), "{:?}", retro.misfits(Misfit::Outgrown));
        // And neither is listed at all: a retro of nothing is not a list of
        // every project that did nothing on purpose.
        assert!(retro.standings.is_empty(), "{:?}", retro.standings);
    }

    #[test]
    fn the_summary_says_the_numbers_a_retro_is_read_for() {
        let retro = look_back(
            week("2026-W37"),
            week("2026-W43"),
            &[
                version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-11")),
                version("alpha", "v0.2.0", Some("2026-W38"), Some("2026-10-02")),
                version("beta", "v1.0.0", None, Some("2026-09-25")),
                version("beta", "v1.1.0", Some("2026-W40"), None),
            ],
            &projects(&[("alpha", Some(Tier::A), Some(2)), ("beta", Some(Tier::B), Some(4))]),
        );
        let text = summary(&retro);
        assert!(text.contains("3 shipped"), "{text}");
        assert!(text.contains("1 on time"), "{text}");
        assert!(text.contains("1 planned and not shipped"), "{text}");
        // Two of three, truncated rather than rounded: a share that rounds
        // up would overstate how much planning happened, which is the one
        // direction this number must never err in.
        assert!(text.contains("66% of releases were planned"), "{text}");
        assert!(text.contains("worst slip alpha v0.2.0 by 2 weeks"), "{text}");
    }

    #[test]
    fn a_quiet_window_summarises_without_inventing_numbers() {
        let retro = look_back(week("2026-W37"), week("2026-W43"), &[], &projects(&[("alpha", Some(Tier::A), Some(2))]));
        let text = summary(&retro);
        assert!(text.contains("0 shipped"), "{text}");
        // No share, no slip, no on-time count: there is nothing to be a
        // share of, and a zero here would read as a measurement.
        assert!(!text.contains('%'), "{text}");
        assert!(!text.contains("on time"), "{text}");
    }
}
