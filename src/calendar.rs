//! The release calendar: which week a version is aimed at, and what the
//! tags say actually happened in it.
//!
//! The calendar this replaces was a document written by hand and read by
//! nobody: it named a focus per week for seven weeks and then went stale
//! the first time a week slipped, because nothing checked it against what
//! shipped. So the calendar here holds one thing only - the week a version
//! is planned for - and gets everything else from facts already recorded:
//! a version that shipped has a tag, and the tag has a date, and the date
//! falls in a week.
//!
//! Slippage is therefore not a state anyone sets. It is the difference
//! between the week that was planned and the week the tag is in, and it
//! appears the moment `sync` reads the tag.

use std::fmt;

use anyhow::{Result, bail};
use jiff::civil::{Date, ISOWeekDate, Weekday};
use serde::Serialize;

/// A week of the calendar, as ISO-8601 spells it: `2026-W37`.
///
/// ISO weeks are used because they are the only week numbering that is the
/// same everywhere and needs no locale to read, and because a week that
/// starts on Monday is the week this line already works in - the stack
/// update is a Monday ritual and the release is a Friday one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Week {
    pub year: i16,
    pub week: i8,
}

impl Week {
    /// The week a date falls in.
    pub fn of(date: Date) -> Week {
        let iso = ISOWeekDate::from_date(date);
        Week {
            year: iso.year(),
            week: iso.week(),
        }
    }

    /// The week today falls in, by the system clock.
    pub fn current() -> Week {
        Week::of(jiff::Zoned::now().date())
    }

    /// The week a recorded date (`YYYY-MM-DD`, possibly with a time after
    /// it) falls in. `None` when the text is not a date rigger wrote.
    pub fn of_recorded(stamp: &str) -> Option<Week> {
        let day = stamp.split(['T', ' ']).next()?;
        let date: Date = day.parse().ok()?;
        Some(Week::of(date))
    }

    /// Parses `2026-W37`, and also the `2026-W7` a person types.
    pub fn parse(text: &str) -> Result<Week> {
        let malformed = || format!("{text:?} is not a week; write it as `2026-W37`");
        let (year, week) = match text.trim().split_once(['W', 'w']) {
            Some((year, week)) => (year.trim_end_matches('-'), week),
            None => bail!(malformed()),
        };
        let (Ok(year), Ok(week)) = (year.parse::<i16>(), week.parse::<i8>()) else {
            bail!(malformed());
        };
        // Rejected here rather than at the database: a week outside the year
        // is a typo, and storing it would put a row in the calendar that no
        // date can ever match.
        if ISOWeekDate::new(year, week, Weekday::Monday).is_err() {
            bail!("{text:?} is not a week of {year}");
        }
        Ok(Week { year, week })
    }

    /// The Monday this week starts on.
    pub fn monday(self) -> Date {
        // Constructed from a week that was checked when it was parsed or
        // read from a date, so the fallback is unreachable in practice.
        ISOWeekDate::new(self.year, self.week, Weekday::Monday)
            .map(|iso| iso.date())
            .unwrap_or_else(|_| Date::default())
    }

    /// The Friday of this week - the day the line releases on.
    pub fn friday(self) -> Date {
        ISOWeekDate::new(self.year, self.week, Weekday::Friday)
            .map(|iso| iso.date())
            .unwrap_or_else(|_| self.monday())
    }

    /// The week `n` weeks after this one, counted through the calendar so
    /// that a 53-week year does not lose a week.
    pub fn plus(self, n: i64) -> Week {
        let days = jiff::SignedDuration::from_hours(24 * 7 * n);
        match self.monday().checked_add(days) {
            Ok(date) => Week::of(date),
            Err(_) => self,
        }
    }

    /// Whole weeks from this week to `other`, negative when `other` is
    /// earlier. This is what slippage is measured in.
    pub fn until(self, other: Week) -> i64 {
        let days = other.monday().since(self.monday()).map(|span| span.get_days()).unwrap_or(0);
        i64::from(days) / 7
    }
}

impl fmt::Display for Week {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-W{:02}", self.year, self.week)
    }
}

impl Serialize for Week {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// How fast a project is meant to release, and how much of the line's
/// attention it gets. The tiers come from the owner's calendar: carrying
/// products that must look alive, growing ones that get a turn each cycle,
/// and declared ones that have a name and no product yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Tier {
    A,
    B,
    C,
    /// Worked on when asked, and deliberately not in the rotation.
    Out,
}

impl Tier {
    pub fn parse(text: &str) -> Result<Tier> {
        match text.trim().to_ascii_lowercase().as_str() {
            "a" => Ok(Tier::A),
            "b" => Ok(Tier::B),
            "c" => Ok(Tier::C),
            "out" | "none" => Ok(Tier::Out),
            _ => bail!("{text:?} is not a tier; write A, B, C or out"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Tier::A => "A",
            Tier::B => "B",
            Tier::C => "C",
            Tier::Out => "out",
        }
    }

    /// What the tier means, in the words the calendar used.
    pub fn describe(self) -> &'static str {
        match self {
            Tier::A => "carrying - released, in the registries, used every day",
            Tier::B => "growing - the code works, the circuit is not closed",
            Tier::C => "declared - a name and a plan, no product yet",
            Tier::Out => "out of the rotation - worked on when asked",
        }
    }

    /// The rhythm a tier keeps when nobody sets one, in weeks between
    /// releases. `Out` has none: that is what being out of the rotation is.
    pub fn default_rhythm(self) -> Option<u32> {
        match self {
            Tier::A => Some(2),
            Tier::B => Some(4),
            Tier::C => Some(6),
            Tier::Out => None,
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One version's place in the calendar: where it was aimed, and where the
/// tag put it.
#[derive(Debug, Clone, Serialize)]
pub struct Planned {
    pub project: String,
    pub version: String,
    pub title: Option<String>,
    pub planned: Option<Week>,
    /// The week the tag falls in, for a version that shipped.
    pub shipped: Option<Week>,
    pub shipped_at: Option<String>,
}

impl Planned {
    /// Weeks between the plan and the tag: positive is late, negative early.
    /// `None` when there is nothing to compare - no plan, or not shipped.
    pub fn slip(&self) -> Option<i64> {
        match (self.planned, self.shipped) {
            (Some(planned), Some(shipped)) => Some(planned.until(shipped)),
            _ => None,
        }
    }

    /// A planned version whose week has passed without a tag. Measured
    /// against the week being read, so this week is not yet late.
    pub fn overdue(&self, now: Week) -> Option<i64> {
        match (self.planned, self.shipped) {
            (Some(planned), None) if planned < now => Some(planned.until(now)),
            _ => None,
        }
    }
}

/// How a version reads on the calendar grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// Planned for this week, still to come.
    Planned,
    /// Shipped in the week it was planned for.
    Shipped,
    /// Shipped, but not in the week it was aimed at.
    Slipped,
    /// The planned week has passed and no tag exists.
    Overdue,
    /// Shipped without ever being planned.
    Unplanned,
}

impl Standing {
    /// The mark the grid prints. A grid is read by shape before it is read
    /// by word, so each standing gets one character of its own.
    pub fn mark(self) -> char {
        match self {
            Standing::Planned => '·',
            Standing::Shipped => '+',
            Standing::Slipped => '>',
            Standing::Overdue => '!',
            Standing::Unplanned => '*',
        }
    }
}

/// One cell of the grid: what a project has in a given week.
#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub week: Week,
    pub version: String,
    pub standing: Standing,
}

/// What a project shows across the weeks being read.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub project: String,
    pub tier: Option<Tier>,
    pub rhythm_weeks: Option<u32>,
    pub cells: Vec<Cell>,
}

/// Lays the versions of one project out over a span of weeks.
///
/// A version appears in the week it was planned for while it is unshipped,
/// and in the week it shipped once it has a tag - so a slipped release is
/// seen where it landed, with a mark saying it was not aimed there. Showing
/// it in its planned week instead would leave the grid claiming a release
/// happened in a week that has none.
pub fn row(project: &str, tier: Option<Tier>, rhythm_weeks: Option<u32>, versions: &[Planned], from: Week, weeks: u32, now: Week) -> Row {
    let last = from.plus(i64::from(weeks.saturating_sub(1)));
    let mut cells = Vec::new();
    for version in versions {
        let (week, standing) = match (version.planned, version.shipped) {
            (Some(planned), Some(shipped)) if planned == shipped => (shipped, Standing::Shipped),
            (Some(_), Some(shipped)) => (shipped, Standing::Slipped),
            (None, Some(shipped)) => (shipped, Standing::Unplanned),
            (Some(planned), None) if planned < now => (planned, Standing::Overdue),
            (Some(planned), None) => (planned, Standing::Planned),
            (None, None) => continue,
        };
        if week < from || week > last {
            continue;
        }
        cells.push(Cell {
            week,
            version: version.version.clone(),
            standing,
        });
    }
    cells.sort_by(|a, b| {
        a.week
            .cmp(&b.week)
            .then_with(|| crate::db::version_order(&a.version).cmp(&crate::db::version_order(&b.version)))
    });
    Row {
        project: project.to_string(),
        tier,
        rhythm_weeks,
        cells,
    }
}

/// Why a project is the focus of a week, in the words that justify it.
#[derive(Debug, Clone, Serialize)]
pub struct Focus {
    pub project: String,
    pub tier: Option<Tier>,
    pub version: String,
    pub title: Option<String>,
    pub planned: Week,
    /// Weeks the planned week is already past, for a focus that is late.
    pub overdue_weeks: Option<i64>,
}

/// The weeks a project has gone without shipping, and what its rhythm asks.
///
/// This is the one thing the written calendar could never do: it named a
/// rotation and had no way to notice when a product fell out of it.
#[derive(Debug, Clone, Serialize)]
pub struct Overdue {
    pub project: String,
    pub tier: Tier,
    pub rhythm_weeks: u32,
    pub since: Option<Week>,
    pub weeks: i64,
}

/// Projects whose rhythm has lapsed: nothing shipped for longer than the
/// tier asks. A project that has never shipped is measured from the week it
/// was recorded, because "never" and "not lately" are the same problem.
pub fn lapsed(projects: &[(String, Tier, u32, Option<Week>)], now: Week) -> Vec<Overdue> {
    let mut out: Vec<Overdue> = projects
        .iter()
        .filter_map(|(project, tier, rhythm, since)| {
            let weeks = since.map(|week| week.until(now)).unwrap_or(i64::from(*rhythm) + 1);
            (weeks > i64::from(*rhythm)).then(|| Overdue {
                project: project.clone(),
                tier: *tier,
                rhythm_weeks: *rhythm,
                since: *since,
                weeks,
            })
        })
        .collect();
    // Worst first: the tier that promised the most and delivered least.
    out.sort_by(|a, b| {
        let overshoot = |o: &Overdue| o.weeks - i64::from(o.rhythm_weeks);
        overshoot(b)
            .cmp(&overshoot(a))
            .then_with(|| a.tier.cmp(&b.tier))
            .then_with(|| a.project.cmp(&b.project))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn a_week_is_written_and_read_the_same_way() {
        let week = Week::parse("2026-W37").unwrap();
        assert_eq!(week.to_string(), "2026-W37");
        assert_eq!(week, Week { year: 2026, week: 37 });
    }

    #[test]
    fn a_single_digit_week_is_accepted_and_padded() {
        assert_eq!(Week::parse("2026-W7").unwrap().to_string(), "2026-W07");
    }

    #[test]
    fn nonsense_is_refused_rather_than_stored() {
        for text in ["", "2026", "2026-37", "W37", "2026-Wxx", "2026-W99", "2026-W0"] {
            assert!(Week::parse(text).is_err(), "{text:?} should not parse");
        }
    }

    /// The turn of the year is where a naive week number goes wrong: these
    /// dates belong to a week of the neighbouring year, and a calendar that
    /// files them under their own year puts a release in the wrong place.
    #[test]
    fn the_turn_of_the_year_follows_iso_not_the_calendar_year() {
        // 2027 opens on a Friday, so its first days are still 2026's W53.
        assert_eq!(Week::of(day("2027-01-01")).to_string(), "2026-W53");
        // And 2025 ends inside 2026's first week.
        assert_eq!(Week::of(day("2025-12-29")).to_string(), "2026-W01");
    }

    #[test]
    fn a_week_knows_its_monday_and_its_friday() {
        let week = Week::parse("2026-W37").unwrap();
        assert_eq!(week.monday().to_string(), "2026-09-07");
        assert_eq!(week.friday().to_string(), "2026-09-11");
        assert_eq!(Week::of(week.monday()), week);
        assert_eq!(Week::of(week.friday()), week);
    }

    /// Adding weeks by counting days, not by adding to the number: a year
    /// with 53 weeks would otherwise swallow one at the turn.
    #[test]
    fn adding_weeks_crosses_the_year_without_losing_one() {
        let week = Week::parse("2026-W52").unwrap();
        assert_eq!(week.plus(1).to_string(), "2026-W53");
        assert_eq!(week.plus(2).to_string(), "2027-W01");
        assert_eq!(week.plus(2).plus(-2), week);
    }

    #[test]
    fn distance_between_weeks_is_signed() {
        let from = Week::parse("2026-W37").unwrap();
        assert_eq!(from.until(Week::parse("2026-W40").unwrap()), 3);
        assert_eq!(from.until(from), 0);
        assert_eq!(from.until(Week::parse("2026-W35").unwrap()), -2);
        assert_eq!(Week::parse("2026-W52").unwrap().until(Week::parse("2027-W01").unwrap()), 2);
    }

    #[test]
    fn a_recorded_date_is_read_with_or_without_a_time() {
        assert_eq!(Week::of_recorded("2026-09-11").unwrap().to_string(), "2026-W37");
        assert_eq!(Week::of_recorded("2026-09-11T14:22:03Z").unwrap().to_string(), "2026-W37");
        assert!(Week::of_recorded("not a date").is_none());
    }

    #[test]
    fn a_tier_is_read_case_insensitively_and_carries_a_default_rhythm() {
        assert_eq!(Tier::parse("a").unwrap(), Tier::A);
        assert_eq!(Tier::parse("OUT").unwrap(), Tier::Out);
        assert!(Tier::parse("D").is_err());
        assert_eq!(Tier::A.default_rhythm(), Some(2));
        assert_eq!(Tier::Out.default_rhythm(), None);
    }

    fn planned(version: &str, planned: Option<&str>, shipped: Option<&str>) -> Planned {
        Planned {
            project: "alpha".to_string(),
            version: version.to_string(),
            title: None,
            planned: planned.map(|w| Week::parse(w).unwrap()),
            shipped: shipped.map(|d| Week::of(day(d))),
            shipped_at: shipped.map(String::from),
        }
    }

    #[test]
    fn slip_is_the_difference_between_the_plan_and_the_tag() {
        assert_eq!(planned("v1", Some("2026-W37"), Some("2026-09-18")).slip(), Some(1));
        assert_eq!(planned("v1", Some("2026-W37"), Some("2026-09-11")).slip(), Some(0));
        assert_eq!(planned("v1", Some("2026-W38"), Some("2026-09-11")).slip(), Some(-1));
        assert_eq!(planned("v1", None, Some("2026-09-11")).slip(), None);
        assert_eq!(planned("v1", Some("2026-W37"), None).slip(), None);
    }

    /// A plan for this week is not late yet - only a week already gone is.
    #[test]
    fn overdue_starts_after_the_planned_week_has_passed() {
        let now = Week::parse("2026-W38").unwrap();
        assert_eq!(planned("v1", Some("2026-W38"), None).overdue(now), None);
        assert_eq!(planned("v1", Some("2026-W37"), None).overdue(now), Some(1));
        assert_eq!(planned("v1", Some("2026-W39"), None).overdue(now), None);
        // A tag settles it, however late.
        assert_eq!(planned("v1", Some("2026-W30"), Some("2026-09-11")).overdue(now), None);
    }

    #[test]
    fn a_shipped_version_sits_in_the_week_its_tag_is_in() {
        let now = Week::parse("2026-W40").unwrap();
        let from = Week::parse("2026-W37").unwrap();
        let versions = vec![
            planned("v0.1.0", Some("2026-W37"), Some("2026-09-11")),
            planned("v0.2.0", Some("2026-W38"), Some("2026-09-25")),
            planned("v0.3.0", Some("2026-W39"), None),
            planned("v0.4.0", Some("2026-W41"), None),
            planned("v0.0.1", None, Some("2026-09-18")),
        ];
        let row = row("alpha", Some(Tier::B), Some(4), &versions, from, 4, now);
        let marks: Vec<(String, &str, char)> = row.cells.iter().map(|c| (c.week.to_string(), c.version.as_str(), c.standing.mark())).collect();
        assert_eq!(
            marks,
            vec![
                ("2026-W37".to_string(), "v0.1.0", '+'),
                ("2026-W38".to_string(), "v0.0.1", '*'),
                // v0.2.0 was aimed at W38 and its tag landed in W39, which
                // is where it is shown - beside the release still due there.
                ("2026-W39".to_string(), "v0.2.0", '>'),
                ("2026-W39".to_string(), "v0.3.0", '!'),
            ]
        );
        // W41 is outside the four weeks asked for.
        assert!(!row.cells.iter().any(|c| c.version == "v0.4.0"));
    }

    #[test]
    fn a_lapsed_rhythm_is_measured_against_the_tier() {
        let now = Week::parse("2026-W40").unwrap();
        let week = |text: &str| Some(Week::parse(text).unwrap());
        let lapsed = lapsed(
            &[
                // Two weeks asked, three weeks gone.
                ("alpha".to_string(), Tier::A, 2, week("2026-W37")),
                // Four asked, one gone.
                ("beta".to_string(), Tier::B, 4, week("2026-W39")),
                // Six asked, eight gone - the worst overshoot.
                ("gamma".to_string(), Tier::C, 6, week("2026-W32")),
                // Never shipped at all.
                ("delta".to_string(), Tier::B, 4, None),
            ],
            now,
        );
        let names: Vec<&str> = lapsed.iter().map(|o| o.project.as_str()).collect();
        assert_eq!(names, vec!["gamma", "alpha", "delta"]);
        assert_eq!(lapsed[0].weeks, 8);
    }
}
