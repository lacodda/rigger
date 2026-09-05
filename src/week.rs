//! The owner's week: the Monday brief, the Friday queue, and the signals a
//! tier is meant to raise.
//!
//! The calendar module knows what a week is and where a version sits in it.
//! This one knows what the owner is supposed to do about it. Three rules of
//! the written calendar live here, and all three were rules nothing enforced:
//!
//! - Monday opens with a brief: the focus, what ships on Friday, what waits.
//! - The release goes out on Friday. A version ready on Tuesday queues; two
//!   releases on one day read as one burst, two in different weeks read as
//!   a rhythm. The shopfront wants an even trace, not even effort.
//! - A tier is a promise about pace, and each tier fails in its own way: A
//!   by missing a cycle, B by going without a turn, C by starting a second
//!   product before the first has shipped anything.

use serde::Serialize;

use crate::calendar::{Planned, Tier, Week};

/// A version that shipped inside a week, with the day its tag landed on.
#[derive(Debug, Clone, Serialize)]
pub struct Shipped {
    pub project: String,
    pub version: String,
    /// The day of the tag, as the record spells it.
    pub day: String,
    /// Whether the tag landed on the Friday its week releases on.
    pub on_release_day: bool,
}

/// A version aimed at a week and still waiting for its tag.
#[derive(Debug, Clone, Serialize)]
pub struct Queued {
    pub project: String,
    pub version: String,
    pub title: Option<String>,
}

/// What the shopfront shows for one week.
///
/// The written rule is one release a week, on a Friday. What the record
/// says about this line is a different thing entirely, and the queue exists
/// to say so rather than to pretend otherwise.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseDay {
    pub week: Week,
    pub friday: String,
    /// Versions whose tag is already in this week.
    pub shipped: Vec<Shipped>,
    /// Versions aimed at this week that have no tag yet: the queue proper.
    pub queued: Vec<Queued>,
}

impl ReleaseDay {
    /// Releases that went out before the Friday of their own week.
    ///
    /// Counted rather than judged one by one: an early tag is not a mistake,
    /// it is a release that should have waited in the queue, and the number
    /// is what says whether the shopfront rule is being kept at all.
    pub fn early(&self) -> usize {
        self.shipped.iter().filter(|s| !s.on_release_day).count()
    }

    /// How far past its one slot the week has gone.
    ///
    /// The rule the calendar wrote is one release a week: two in a day read
    /// as a single burst to anyone watching from outside, and the whole
    /// point of the shopfront is that the trace looks even.
    pub fn over_the_slot(&self) -> usize {
        self.shipped.len().saturating_sub(1)
    }
}

/// One day of the shopfront week, folded.
///
/// A day is the unit because that is the unit the rule is about: what an
/// onlooker sees is one day carrying one release, or one day carrying
/// twenty. Which twenty matters much less than that there were twenty.
#[derive(Debug, Clone, Serialize)]
pub struct Day {
    pub day: String,
    /// Whether this is the Friday the week releases on.
    pub on_release_day: bool,
    /// The projects that released on it, each with its versions.
    pub projects: Vec<DayProject>,
    pub releases: usize,
}

/// What one project put out on one day.
#[derive(Debug, Clone, Serialize)]
pub struct DayProject {
    pub project: String,
    pub versions: Vec<String>,
}

impl DayProject {
    /// The versions, named while they are few and spanned once they are not.
    ///
    /// The same fold the calendar grid needed, and for the same reason: a
    /// real week of this line holds ninety-four releases, and a screen that
    /// prints one line each is a screen nobody reads to the bottom - where
    /// the two numbers that answer the question are.
    pub fn summary(&self) -> String {
        match self.versions.len() {
            0 => String::new(),
            1..=3 => self.versions.join(" "),
            n => format!(
                "{}..{} ({n})",
                self.versions.first().map(String::as_str).unwrap_or(""),
                self.versions.last().map(String::as_str).unwrap_or("")
            ),
        }
    }
}

impl ReleaseDay {
    /// The week's releases folded into days, oldest first.
    pub fn days(&self) -> Vec<Day> {
        let mut days: Vec<Day> = Vec::new();
        for shipped in &self.shipped {
            let day = match days.iter_mut().find(|d| d.day == shipped.day) {
                Some(day) => day,
                None => {
                    days.push(Day {
                        day: shipped.day.clone(),
                        on_release_day: shipped.on_release_day,
                        projects: Vec::new(),
                        releases: 0,
                    });
                    days.last_mut().expect("just pushed")
                }
            };
            day.releases += 1;
            match day.projects.iter_mut().find(|p| p.project == shipped.project) {
                Some(project) => project.versions.push(shipped.version.clone()),
                None => day.projects.push(DayProject {
                    project: shipped.project.clone(),
                    versions: vec![shipped.version.clone()],
                }),
            }
        }
        days
    }
}

/// Gathers what a week holds on the shopfront.
pub fn release_day(week: Week, versions: &[Planned]) -> ReleaseDay {
    let friday = week.friday().to_string();
    let mut shipped = Vec::new();
    let mut queued = Vec::new();

    for version in versions {
        match (version.shipped, version.planned) {
            (Some(shipped_week), _) if shipped_week == week => {
                // The day, not the moment: the shopfront is read by date.
                let day = version.shipped_at.as_deref().and_then(|s| s.split(['T', ' ']).next()).unwrap_or("").to_string();
                let on_release_day = day == friday;
                shipped.push(Shipped {
                    project: version.project.clone(),
                    version: version.version.clone(),
                    day,
                    on_release_day,
                });
            }
            (None, Some(planned)) if planned == week => queued.push(Queued {
                project: version.project.clone(),
                version: version.version.clone(),
                title: version.title.clone(),
            }),
            _ => {}
        }
    }

    shipped.sort_by(|a, b| {
        a.day
            .cmp(&b.day)
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| crate::db::version_order(&a.version).cmp(&crate::db::version_order(&b.version)))
    });
    queued.sort_by(|a, b| {
        a.project
            .cmp(&b.project)
            .then_with(|| crate::db::version_order(&a.version).cmp(&crate::db::version_order(&b.version)))
    });

    ReleaseDay { week, friday, shipped, queued }
}

/// A tier's promise, and the way this project is breaking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Signal {
    /// Tier A: more than one cycle of its rhythm has gone without a release.
    /// The written minimum is "not more than one cycle missed in a row".
    MissedCycle,
    /// Tier B: no turn in focus for six weeks. The written minimum is that
    /// each growing product gets the focus at least that often.
    WithoutFocus,
    /// Tier C: a second declared product started while the first has still
    /// not shipped a v0.1.0. The written rule is to start them one at a time.
    SecondStart,
}

/// One project failing its tier's promise, with the number that proves it.
#[derive(Debug, Clone, Serialize)]
pub struct Raised {
    pub project: String,
    pub tier: Tier,
    pub signal: Signal,
    /// Weeks behind, for the two signals measured in weeks.
    pub weeks: Option<i64>,
    /// The other project involved, for a signal that is about a pair.
    pub alongside: Option<String>,
}

/// What one project brings to the signal check.
#[derive(Debug, Clone)]
pub struct Standing {
    pub project: String,
    pub tier: Tier,
    pub rhythm_weeks: Option<u32>,
    /// The week of its last tag, if it has ever shipped.
    pub last_shipped: Option<Week>,
    /// The week something last happened in it: a tag, a commit, a note.
    /// A tier B product gets its turn in the focus whether or not the turn
    /// ends in a release, so a release is the wrong thing to measure it by.
    pub last_touched: Option<Week>,
    /// Whether the record holds a shipped version for it at all.
    pub has_first_release: bool,
}

/// Weeks a tier B product may go without a turn before the calendar says
/// something. Written as "at least once in six weeks".
const FOCUS_WEEKS: i64 = 6;

/// Raises the signals each tier is meant to raise.
///
/// Every one of these was a rule the written calendar stated and had no way
/// to check: it named a rotation, a minimum per tier and an order for the
/// declared products, and then went stale because nothing ever compared any
/// of it to what git said.
pub fn signals(standings: &[Standing], now: Week) -> Vec<Raised> {
    let mut out = Vec::new();

    for standing in standings {
        match standing.tier {
            // A carrying product may miss one cycle of its rhythm and no
            // more: that is what the written minimum says, and it is why
            // this is not the check `next` already has, which fires at the
            // first week over.
            Tier::A => {
                let Some(rhythm) = standing.rhythm_weeks.filter(|r| *r > 0) else {
                    continue;
                };
                let allowed = i64::from(rhythm) * 2;
                let weeks = weeks_since(standing.last_shipped, now, allowed + 1);
                if weeks > allowed {
                    out.push(Raised {
                        project: standing.project.clone(),
                        tier: Tier::A,
                        signal: Signal::MissedCycle,
                        weeks: Some(weeks),
                        alongside: None,
                    });
                }
            }
            // A growing product is measured by attention, not by releases:
            // its minimum is a turn in the focus, and a turn that produced
            // no tag is still a turn.
            Tier::B => {
                let weeks = weeks_since(standing.last_touched, now, FOCUS_WEEKS + 1);
                if weeks > FOCUS_WEEKS {
                    out.push(Raised {
                        project: standing.project.clone(),
                        tier: Tier::B,
                        signal: Signal::WithoutFocus,
                        weeks: Some(weeks),
                        alongside: None,
                    });
                }
            }
            Tier::C | Tier::Out => {}
        }
    }

    // The declared products are checked against each other rather than
    // against the clock: the rule is that the second one does not begin
    // until the first has shipped, so the signal belongs to a pair.
    let mut started: Vec<&Standing> = standings
        .iter()
        .filter(|s| s.tier == Tier::C && !s.has_first_release && s.last_touched.is_some())
        .collect();
    if started.len() > 1 {
        // Oldest start first: the one that began earlier is the one meant
        // to finish, and the others are the ones that should have waited.
        started.sort_by(|a, b| a.last_touched.cmp(&b.last_touched).then_with(|| a.project.cmp(&b.project)));
        let first = started[0].project.clone();
        for standing in started.iter().skip(1) {
            out.push(Raised {
                project: standing.project.clone(),
                tier: Tier::C,
                signal: Signal::SecondStart,
                weeks: None,
                alongside: Some(first.clone()),
            });
        }
    }

    // Worst tier first, then the longest wait: a carrying product that has
    // gone quiet outranks a declared one that started out of turn.
    out.sort_by(|a, b| a.tier.cmp(&b.tier).then_with(|| b.weeks.cmp(&a.weeks)).then_with(|| a.project.cmp(&b.project)));
    out
}

/// Weeks from a week to now, with a fallback for "never".
///
/// Never is not zero: a product that has never shipped is further behind
/// than one that shipped last week, and treating the missing week as the
/// current one would silently clear the signal.
fn weeks_since(week: Option<Week>, now: Week, never: i64) -> i64 {
    week.map(|w| w.until(now)).unwrap_or(never)
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

    #[test]
    fn a_tag_on_the_friday_is_the_rule_being_kept() {
        // 2026-W37 releases on Friday 2026-09-11.
        let day = release_day(week("2026-W37"), &[version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-11T09:00:00Z"))]);
        assert_eq!(day.shipped.len(), 1);
        assert!(day.shipped[0].on_release_day);
        assert_eq!(day.early(), 0);
        assert_eq!(day.over_the_slot(), 0);
    }

    #[test]
    fn a_tag_earlier_in_the_week_is_one_that_should_have_queued() {
        let day = release_day(week("2026-W37"), &[version("alpha", "v0.1.0", Some("2026-W37"), Some("2026-09-08T09:00:00Z"))]);
        assert!(!day.shipped[0].on_release_day);
        assert_eq!(day.early(), 1);
    }

    /// The case the shopfront rule exists for: several releases inside one
    /// week read as a single burst from outside, however good each one is.
    #[test]
    fn a_week_past_its_one_slot_says_by_how_much() {
        let day = release_day(
            week("2026-W37"),
            &[
                version("alpha", "v0.1.0", None, Some("2026-09-08T09:00:00Z")),
                version("alpha", "v0.2.0", None, Some("2026-09-09T09:00:00Z")),
                version("beta", "v1.0.0", None, Some("2026-09-11T09:00:00Z")),
            ],
        );
        assert_eq!(day.shipped.len(), 3);
        assert_eq!(day.over_the_slot(), 2);
        assert_eq!(day.early(), 2);
        // Oldest tag first, so the week reads in the order it happened.
        assert_eq!(day.shipped[0].version, "v0.1.0");
        assert_eq!(day.shipped[2].version, "v1.0.0");
    }

    /// The fold the real record forced: a week of this line holds ninety-four
    /// releases, and one line each buries the two numbers that answer the
    /// question. The calendar grid learnt this at v0.10.0; the queue is the
    /// same screen with the same week behind it.
    #[test]
    fn a_busy_week_folds_into_days_rather_than_a_line_each() {
        let mut versions = Vec::new();
        for n in 1..=9 {
            versions.push(version("alpha", &format!("v0.{n}.0"), None, Some("2026-09-08T09:00:00Z")));
        }
        versions.push(version("beta", "v1.0.0", None, Some("2026-09-08T09:00:00Z")));
        versions.push(version("beta", "v1.1.0", None, Some("2026-09-11T09:00:00Z")));

        let days = release_day(week("2026-W37"), &versions).days();
        assert_eq!(days.len(), 2, "{days:?}");
        assert_eq!(days[0].day, "2026-09-08");
        assert!(!days[0].on_release_day);
        assert_eq!(days[0].releases, 10);
        // Ten releases, two projects, one line: the run is spanned and
        // counted rather than named out.
        assert_eq!(days[0].projects.len(), 2);
        assert_eq!(days[0].projects[0].summary(), "v0.1.0..v0.9.0 (9)");
        // A short run is still named, because three names read better than
        // a span of three.
        assert_eq!(days[0].projects[1].summary(), "v1.0.0");
        assert!(days[1].on_release_day);
    }

    #[test]
    fn a_span_runs_from_the_lowest_version_to_the_highest() {
        // Tags of one day arrive in whatever order git returns them; the
        // span has to read as a range regardless.
        let versions = vec![
            version("alpha", "v0.10.0", None, Some("2026-09-08T09:00:00Z")),
            version("alpha", "v0.2.0", None, Some("2026-09-08T09:00:00Z")),
            version("alpha", "v0.9.0", None, Some("2026-09-08T09:00:00Z")),
            version("alpha", "v0.1.0", None, Some("2026-09-08T09:00:00Z")),
        ];
        let days = release_day(week("2026-W37"), &versions).days();
        assert_eq!(days[0].projects[0].summary(), "v0.1.0..v0.10.0 (4)");
    }

    #[test]
    fn a_planned_version_without_a_tag_is_the_queue() {
        let day = release_day(
            week("2026-W37"),
            &[
                version("alpha", "v0.1.0", Some("2026-W37"), None),
                // Aimed at another week, so not this week's queue.
                version("beta", "v0.2.0", Some("2026-W38"), None),
                // Already out, so no longer waiting.
                version("gamma", "v0.3.0", Some("2026-W37"), Some("2026-09-11T09:00:00Z")),
            ],
        );
        let queued: Vec<&str> = day.queued.iter().map(|q| q.version.as_str()).collect();
        assert_eq!(queued, vec!["v0.1.0"]);
        assert_eq!(day.shipped.len(), 1);
    }

    fn standing(project: &str, tier: Tier, rhythm: Option<u32>, shipped: Option<&str>, touched: Option<&str>, first: bool) -> Standing {
        Standing {
            project: project.to_string(),
            tier,
            rhythm_weeks: rhythm,
            last_shipped: shipped.map(week),
            last_touched: touched.map(week),
            has_first_release: first,
        }
    }

    /// Tier A is allowed one missed cycle. The written minimum is "not more
    /// than one in a row", so the signal fires on the second - which is what
    /// makes it a different check from the rhythm lapse `next` already has.
    #[test]
    fn tier_a_fires_on_the_second_missed_cycle_not_the_first() {
        let now = week("2026-W40");
        // A rhythm of two weeks: four weeks gone is one cycle missed and
        // allowed; five is two, and not.
        let one_missed = signals(&[standing("alpha", Tier::A, Some(2), Some("2026-W36"), None, true)], now);
        assert!(one_missed.is_empty(), "{one_missed:?}");

        let two_missed = signals(&[standing("alpha", Tier::A, Some(2), Some("2026-W35"), None, true)], now);
        assert_eq!(two_missed.len(), 1);
        assert_eq!(two_missed[0].signal, Signal::MissedCycle);
        assert_eq!(two_missed[0].weeks, Some(5));
    }

    #[test]
    fn a_carrying_product_that_never_shipped_is_behind_from_the_start() {
        let now = week("2026-W40");
        let raised = signals(&[standing("alpha", Tier::A, Some(2), None, None, false)], now);
        assert_eq!(raised.len(), 1);
        assert_eq!(raised[0].signal, Signal::MissedCycle);
    }

    /// Tier B is measured by attention, not releases: a turn in the focus
    /// that ended without a tag is still a turn, and a product touched last
    /// week is not the one the rule is worried about.
    #[test]
    fn tier_b_is_measured_by_the_last_turn_not_the_last_tag() {
        let now = week("2026-W40");
        // Six weeks is the minimum, so six is still inside it.
        let inside = signals(&[standing("beta", Tier::B, Some(4), Some("2026-W10"), Some("2026-W34"), true)], now);
        assert!(inside.is_empty(), "{inside:?}");

        let outside = signals(&[standing("beta", Tier::B, Some(4), Some("2026-W10"), Some("2026-W33"), true)], now);
        assert_eq!(outside.len(), 1);
        assert_eq!(outside[0].signal, Signal::WithoutFocus);
        assert_eq!(outside[0].weeks, Some(7));
    }

    /// The tier C rule is about a pair, not a clock: the second declared
    /// product should not have begun before the first shipped anything.
    #[test]
    fn tier_c_names_the_second_start_and_the_one_it_should_have_waited_for() {
        let now = week("2026-W40");
        let raised = signals(
            &[
                standing("first", Tier::C, Some(6), None, Some("2026-W32"), false),
                standing("second", Tier::C, Some(6), None, Some("2026-W35"), false),
            ],
            now,
        );
        let out_of_turn: Vec<&Raised> = raised.iter().filter(|r| r.signal == Signal::SecondStart).collect();
        assert_eq!(out_of_turn.len(), 1, "{raised:?}");
        // The later start is the one out of turn; the earlier is what it
        // should have waited for.
        assert_eq!(out_of_turn[0].project, "second");
        assert_eq!(out_of_turn[0].alongside.as_deref(), Some("first"));
    }

    #[test]
    fn one_declared_product_at_a_time_raises_nothing() {
        let now = week("2026-W40");
        let raised = signals(
            &[
                standing("first", Tier::C, Some(6), None, Some("2026-W32"), false),
                // Started, and it has shipped - so the next one may begin.
                standing("second", Tier::C, Some(6), Some("2026-W36"), Some("2026-W36"), true),
            ],
            now,
        );
        assert!(raised.iter().all(|r| r.signal != Signal::SecondStart), "{raised:?}");
    }

    #[test]
    fn a_declared_product_nobody_has_started_is_not_a_second_start() {
        let now = week("2026-W40");
        let raised = signals(
            &[
                standing("first", Tier::C, Some(6), None, Some("2026-W32"), false),
                // Recorded and never touched: waiting its turn, as asked.
                standing("second", Tier::C, Some(6), None, None, false),
            ],
            now,
        );
        assert!(raised.is_empty(), "{raised:?}");
    }

    #[test]
    fn a_project_out_of_the_rotation_raises_nothing() {
        let now = week("2026-W40");
        let raised = signals(&[standing("aside", Tier::Out, None, None, None, false)], now);
        assert!(raised.is_empty(), "{raised:?}");
    }

    #[test]
    fn the_worst_tier_is_listed_first() {
        let now = week("2026-W40");
        let raised = signals(
            &[
                standing("beta", Tier::B, Some(4), None, Some("2026-W20"), true),
                standing("alpha", Tier::A, Some(2), Some("2026-W20"), None, true),
            ],
            now,
        );
        assert_eq!(raised[0].project, "alpha");
        assert_eq!(raised[0].tier, Tier::A);
    }
}
