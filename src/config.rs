//! The numbers the owner may set for how the line grows and is kept up.
//!
//! Each has a default the line was run on, and can be set for the whole record
//! or for one project: a project that needs its plan thicker than the rest says
//! so without changing anybody else's. All of them are whole numbers, and are
//! read fresh, so a change takes effect on the next look at the queue.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::db::{Db, Project};

/// One setting: its name, its default, and what it means.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Setting {
    pub key: &'static str,
    pub default: i64,
    pub about: &'static str,
}

pub const TAIL_FLOOR: &str = "tail.floor";
pub const TAIL_CAP: &str = "tail.cap";
pub const TAIL_SORT_DAYS: &str = "tail.sort_days";
pub const IDEAS_EVERY_DAYS: &str = "ideas.every_days";
pub const REFACTOR_MIN_VERSIONS: &str = "refactor.min_versions";
pub const REFACTOR_PER_WEEK: &str = "refactor.per_week";

pub const SETTINGS: [Setting; 6] = [
    Setting {
        key: TAIL_FLOOR,
        default: 3,
        about: "With this many versions planned or fewer, a project's turn starts by growing its plan",
    },
    Setting {
        key: TAIL_CAP,
        default: 30,
        about: "With this many versions planned, small ideas stop going into the plan by themselves",
    },
    Setting {
        key: TAIL_SORT_DAYS,
        default: 30,
        about: "How many days a project's wishes and orders may wait before a turn starts by sorting them",
    },
    Setting {
        key: IDEAS_EVERY_DAYS,
        default: 30,
        about: "How many days between the questionnaires that ask about big ideas",
    },
    Setting {
        key: REFACTOR_MIN_VERSIONS,
        default: 10,
        about: "How many versions a project ships before it is considered for a refactoring",
    },
    Setting {
        key: REFACTOR_PER_WEEK,
        default: 1,
        about: "How many refactorings the whole line takes on in a week",
    },
];

fn setting(key: &str) -> Result<&'static Setting> {
    match SETTINGS.iter().find(|s| s.key == key) {
        Some(found) => Ok(found),
        None => {
            let known: Vec<&str> = SETTINGS.iter().map(|s| s.key).collect();
            bail!("there is no setting '{key}'; the settings are {}", known.join(", "))
        }
    }
}

/// A setting's value for a project: its own, or the record's, or the default.
pub fn value(db: &Db, project: Option<&Project>, key: &str) -> Result<i64> {
    let found = setting(key)?;
    if let Some(project) = project
        && let Some(own) = db.project_setting(project.id, key)?
    {
        return Ok(own);
    }
    Ok(db
        .setting(&format!("config.{key}"))?
        .and_then(|text| text.parse().ok())
        .unwrap_or(found.default))
}

/// Sets a value, for one project or for the record, or clears it back to
/// what it stood on before.
pub fn set(db: &Db, project: Option<&Project>, key: &str, to: Option<i64>) -> Result<()> {
    setting(key)?;
    if to.is_some_and(|value| value < 0) {
        bail!("{key} is a count, and cannot be below 0");
    }
    match project {
        Some(project) => db.set_project_setting(project.id, key, to),
        None => db.set_setting(&format!("config.{key}"), to.map(|value| value.to_string()).as_deref()),
    }
}

/// One line of what stands: the value, where it comes from, what it means.
#[derive(Debug, Clone, Serialize)]
pub struct Standing {
    pub key: &'static str,
    pub value: i64,
    pub default: i64,
    pub about: &'static str,
    /// The projects with a value of their own.
    pub projects: Vec<ProjectValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectValue {
    pub project: String,
    pub value: i64,
}

/// Every setting as it stands.
pub fn standing(db: &Db) -> Result<Vec<Standing>> {
    let projects = db.projects()?;
    SETTINGS
        .iter()
        .map(|setting| {
            let mut own = Vec::new();
            for project in &projects {
                if let Some(value) = db.project_setting(project.id, setting.key)? {
                    own.push(ProjectValue {
                        project: project.name.clone(),
                        value,
                    });
                }
            }
            Ok(Standing {
                key: setting.key,
                value: value(db, None, setting.key)?,
                default: setting.default,
                about: setting.about,
                projects: own,
            })
        })
        .collect()
}
