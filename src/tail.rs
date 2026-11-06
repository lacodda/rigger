//! A plan's tail: how much is left of it, and whether a turn should start by
//! doing something about that.
//!
//! A plan nobody adds to is a product that stops the day it runs out, so the
//! record watches how much is left and says so at the start of a turn. rigger
//! has no ideas of its own: it says when, and `tail.ritual.md` says how, and the
//! assistant on the turn does the thinking.
//!
//! Two different things can be due. With the plan nearly out, it has to be
//! `grow`n. With plenty left it still should be sorted now and then, or what the
//! neighbours and the sessions asked for piles up unread. A long plan gets
//! nothing heavier than that, so a monthly look costs little where little is
//! needed.

use anyhow::Result;
use serde::Serialize;

use crate::config;
use crate::db::{Db, Kind, Project};
use crate::plural;

/// The ritual, as `rigger tail ritual` prints it.
pub const RITUAL: &str = include_str!("tail.ritual.md");

/// What a turn should start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Due {
    Grow,
    Sort,
}

/// One project's tail.
#[derive(Debug, Clone, Serialize)]
pub struct Tail {
    pub project: String,
    /// Versions planned and not shipped.
    pub planned: usize,
    pub floor: i64,
    pub cap: i64,
    /// Wishes and orders waiting to be sorted.
    pub unsorted: usize,
    /// When someone last went through it.
    pub last_pass: Option<String>,
    /// How many days ago that was, or the project was added.
    pub days_since: Option<i64>,
    pub due: Option<Due>,
}

impl Tail {
    /// The sentence that goes into a first message. Counts only, and the words
    /// are fixed: this reaches an assistant's command line.
    pub fn order(&self) -> Option<String> {
        match self.due? {
            Due::Grow => Some(format!(
                " First the plan: {} left, so grow it as `rigger tail ritual` says, then take the stage.",
                plural(self.planned, "version", "versions")
            )),
            Due::Sort => Some(format!(
                " First the plan: sort what has piled up, as `rigger tail ritual` says ({}), then take the stage.",
                plural(self.unsorted, "wish", "wishes")
            )),
        }
    }

    /// The tail in a few words.
    pub fn line(&self) -> String {
        let planned = plural(self.planned, "version", "versions");
        match self.due {
            Some(Due::Grow) => format!("{planned} planned, at or under {} - grow it", self.floor),
            Some(Due::Sort) => format!("{planned} planned - sort what has piled up ({})", plural(self.unsorted, "wish", "wishes")),
            None if self.planned as i64 >= self.cap => format!("{planned} planned, at the cap of {}: small ideas wait", self.cap),
            None => format!("{planned} planned"),
        }
    }
}

/// Reads a project's tail.
///
/// A pass since the last release is enough for this turn, whatever it found: an
/// assistant that looked and honestly had nothing good to add should not be sent
/// to look again until something has shipped. A folder is left out: what it has
/// instead of a plan is for its own rules to say.
pub fn of(db: &Db, project: &Project) -> Result<Tail> {
    let planned = db.planned_count(project.id)?;
    let floor = config::value(db, Some(project), config::TAIL_FLOOR)?;
    let sort_days = config::value(db, Some(project), config::TAIL_SORT_DAYS)?;
    let last_pass = db.last_tail_pass(project.id)?;
    let released = db.last_release_moment(project.id)?;
    let passed_this_turn = match (&last_pass, &released) {
        // The record keeps seconds, and one second holding both says nothing
        // about which came first: this errs towards not asking twice.
        (Some(pass), Some(release)) => pass >= release,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let since = last_pass.as_deref().unwrap_or(project.created_at.as_str());
    let days_since = crate::days_since_utc(since);

    let due = if project.kind == Kind::Folder || project.weight == 0 {
        None
    } else if planned as i64 <= floor && !passed_this_turn {
        Some(Due::Grow)
    } else if sort_days > 0 && days_since.is_some_and(|days| days >= sort_days) {
        Some(Due::Sort)
    } else {
        None
    };
    Ok(Tail {
        project: project.name.clone(),
        planned,
        floor,
        cap: config::value(db, Some(project), config::TAIL_CAP)?,
        unsorted: db.count_open_events(project.id, "wish")? as usize,
        last_pass,
        days_since,
        due,
    })
}
