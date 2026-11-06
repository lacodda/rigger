//! Whose turn is it to be refactored.
//!
//! Every project needs it now and then, and it is the kind of work that never
//! comes first by itself. The line can afford only so much of it at a time, so
//! it is rationed across all the projects rather than per project: a few a week
//! at most, whoever has gone longest. "Longest" is in versions, not in days: what
//! makes code harder to work on is the work done to it, so a project that ships
//! twenty versions a month comes round sooner than one that ships two.
//!
//! It takes the project's own turn, not an extra one, and moves nothing in the
//! queue: when the project next comes up, that turn is the refactoring.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config;
use crate::db::{Db, Kind, Project};
use crate::plural;

/// The ritual, as `rigger refactor ritual` prints it.
pub const RITUAL: &str = include_str!("refactor.ritual.md");

/// One project's standing.
#[derive(Debug, Clone, Serialize)]
pub struct Standing {
    pub project: String,
    /// Versions shipped since its last refactoring, or ever.
    pub versions: usize,
    /// How many it takes to be considered.
    pub min_versions: i64,
    /// When it was last done.
    pub last: Option<String>,
}

/// Where the line stands.
#[derive(Debug, Clone, Serialize)]
pub struct Rota {
    /// The project whose next turn is a refactoring; `None` when the week's slot
    /// is taken or nobody has gone long enough.
    pub next: Option<String>,
    /// Refactorings done this week, and how many a week are allowed.
    pub this_week: Vec<Done>,
    pub per_week: i64,
    /// Every project it applies to, the longest gone first.
    pub standings: Vec<Standing>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Done {
    pub project: String,
    pub at: String,
}

/// The ISO week a moment fell in, by the machine's clock: "this week" is the
/// owner's week.
fn week_of(moment: &str) -> Option<(i16, i8)> {
    let at: jiff::Timestamp = moment.parse().ok()?;
    let week = at.to_zoned(jiff::tz::TimeZone::system()).date().iso_week_date();
    Some((week.year(), week.week()))
}

fn this_week() -> (i16, i8) {
    let week = jiff::Zoned::now().date().iso_week_date();
    (week.year(), week.week())
}

/// Reads the rota. A folder has no code to refactor, and a project at weight 0
/// or blocked is not coming up.
pub fn rota(db: &Db) -> Result<Rota> {
    let mut standings = Vec::new();
    let mut this_week_done = Vec::new();
    let now = this_week();
    for project in db.projects()? {
        let last = db.last_refactor(project.id)?;
        if let Some(at) = &last
            && week_of(at) == Some(now)
        {
            this_week_done.push(Done {
                project: project.name.clone(),
                at: at.clone(),
            });
        }
        if project.kind != Kind::Repo || project.weight == 0 || !db.blockers(project.id)?.is_empty() {
            continue;
        }
        standings.push(Standing {
            versions: db.versions_shipped_since(project.id, last.as_deref())?,
            min_versions: config::value(db, Some(&project), config::REFACTOR_MIN_VERSIONS)?,
            last,
            project: project.name,
        });
    }
    standings.sort_by(|a, b| b.versions.cmp(&a.versions).then_with(|| a.project.cmp(&b.project)));
    let per_week = config::value(db, None, config::REFACTOR_PER_WEEK)?;
    let room = (this_week_done.len() as i64) < per_week;
    let next = standings
        .iter()
        .find(|s| room && s.versions as i64 >= s.min_versions)
        .map(|s| s.project.clone());
    Ok(Rota {
        next,
        this_week: this_week_done,
        per_week,
        standings,
    })
}

impl Rota {
    /// Whether this project's next turn is the refactoring.
    pub fn is_next(&self, project: &str) -> bool {
        self.next.as_deref() == Some(project)
    }

    /// The sentence that goes into a first message: fixed words and a count.
    pub fn order(&self, project: &str) -> Option<String> {
        if !self.is_next(project) {
            return None;
        }
        let versions = self.standings.iter().find(|s| s.project == project).map_or(0, |s| s.versions);
        Some(format!(
            " This turn is a refactoring instead of the stage, {} since the last: follow `rigger refactor ritual`, and finish with `rigger refactor done {project}`.",
            plural(versions, "version", "versions")
        ))
    }

    /// The screen.
    pub fn render(&self) -> String {
        let mut out = String::new();
        match &self.next {
            Some(next) => out.push_str(&format!("Next to refactor: {next}, on its next turn\n")),
            None if (self.this_week.len() as i64) >= self.per_week => {
                let done: Vec<&str> = self.this_week.iter().map(|d| d.project.as_str()).collect();
                out.push_str(&format!(
                    "This week's slot is taken: {}\n",
                    if done.is_empty() { "none allowed".to_string() } else { done.join(", ") }
                ));
            }
            None => out.push_str("Nobody has gone long enough to be refactored yet\n"),
        }
        if !self.standings.is_empty() {
            let name = self.standings.iter().map(|s| s.project.chars().count()).max().unwrap_or(0);
            out.push('\n');
            for standing in &self.standings {
                let last = standing.last.as_deref().map(|at| at.split('T').next().unwrap_or(at).to_string());
                out.push_str(&format!(
                    "  {:<name$}  {}{}\n",
                    standing.project,
                    plural(standing.versions, "version", "versions"),
                    last.map(|day| format!(" since {day}")).unwrap_or_else(|| " and none yet".to_string()),
                ));
            }
        }
        out
    }
}

/// Records that a project was refactored, or looked at and did not need it.
pub fn done(db: &Db, project: &Project, text: &str) -> Result<()> {
    db.record_event(project.id, "refactor", text, &crate::db::now(), "assistant")
        .with_context(|| format!("cannot record the refactoring of {}", project.name))?;
    Ok(())
}
