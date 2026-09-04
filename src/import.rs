//! Importing a hub into the record.
//!
//! The hub is the history of a project written by hand; this reads it once so
//! that everything after it can be a query. Importing the same hub again must
//! be safe and quiet: a stage is identified by its version, a task by its
//! text within a stage, a decision by its date and body. Nothing is deleted -
//! a line dropped from the plan stays in the record as the task it was.

use anyhow::Result;
use serde::Serialize;

use crate::db::{Change, Db};
use crate::hub::Hub;

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub versions_added: u32,
    pub versions_updated: u32,
    pub tasks_added: u32,
    pub tasks_updated: u32,
    pub decisions_added: u32,
    pub questions_added: u32,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.versions_added + self.versions_updated + self.tasks_added + self.tasks_updated + self.decisions_added + self.questions_added > 0
    }
}

fn tally(change: Change, added: &mut u32, updated: &mut u32) {
    match change {
        Change::Added => *added += 1,
        Change::Updated => *updated += 1,
        Change::Unchanged => {}
    }
}

pub fn import(db: &Db, project_id: i64, hub: &Hub) -> Result<Report> {
    let mut report = Report {
        warnings: hub.warnings.clone(),
        ..Report::default()
    };

    // Closed stages first: when a version appears in both files, the changelog
    // is the one that knows it shipped, and the plan must not undo that.
    for stage in hub.closed_stages.iter().chain(hub.open_stages.iter()) {
        let (version_id, change) = db.upsert_version(project_id, stage)?;
        tally(change, &mut report.versions_added, &mut report.versions_updated);
        for task in &stage.tasks {
            let change = db.upsert_task(project_id, version_id, task)?;
            tally(change, &mut report.tasks_added, &mut report.tasks_updated);
        }
    }

    for decision in &hub.decisions {
        // Decisions are dated by day in the hub; the record keeps timestamps.
        let at = format!("{}T00:00:00Z", decision.date);
        let body = if decision.body.is_empty() {
            decision.title.clone()
        } else {
            format!("{}\n\n{}", decision.title, decision.body)
        };
        if db.record_event(project_id, "decision", &body, &at, "owner")? == Change::Added {
            report.decisions_added += 1;
        }
    }

    for question in &hub.questions {
        // A question has no date of its own; it is open now, which is what
        // the inbox will ask for later.
        if db.record_event(project_id, "question", question, &crate::db::now(), "owner")? == Change::Added {
            report.questions_added += 1;
        }
    }

    Ok(report)
}
