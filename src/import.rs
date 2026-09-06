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
    pub diary_added: u32,
    pub diary_updated: u32,
    /// Files whose between-stage prose the record took in.
    pub prose_files: u32,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.versions_added
            + self.versions_updated
            + self.tasks_added
            + self.tasks_updated
            + self.decisions_added
            + self.questions_added
            + self.diary_added
            + self.diary_updated
            + self.prose_files
            > 0
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
    //
    // Nor may it undo where the changelog put it. A plan keeps its shipped
    // stages in the major map - lyrn lists four of them as `###` under a
    // block heading, and the changelog writes the same four as `##` with
    // the entry about each - so a second pass that rewrote the shape moved
    // every one of them out of the changelog and into the plan's order.
    // The file a stage is being read from decides: a stage in hand from
    // the changelog owns its shape, one from the plan defers.
    let closed = hub.closed_stages.iter().map(|stage| (stage, true));
    let open = hub.open_stages.iter().map(|stage| (stage, false));
    for (stage, owns_shape) in closed.chain(open) {
        let (version_id, change) = db.upsert_version(project_id, stage, owns_shape)?;
        tally(change, &mut report.versions_added, &mut report.versions_updated);
        for (position, task) in stage.tasks.iter().enumerate() {
            let change = db.upsert_task(project_id, version_id, position, task)?;
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

    // The diary becomes sessions that already ended: an entry is one
    // sitting, written before rigger knew what a sitting was, and it has
    // nowhere else to live that an export could read back.
    for entry in &hub.diary {
        tally(db.upsert_diary_entry(project_id, entry)?, &mut report.diary_added, &mut report.diary_updated);
    }

    // Everything that belongs to no stage and no entry - a preamble, a map,
    // the headings that group stages into blocks. Without it an export would
    // generate a file that had lost most of what a person had written in it:
    // measured on this project's own hub, 88% of the changelog and 42% of
    // the plan is prose of this kind.
    let mut files: Vec<&str> = hub.prose.iter().map(|p| p.file.as_str()).collect();
    files.sort_unstable();
    files.dedup();
    for file in files {
        let runs: Vec<crate::hub::Prose> = hub.prose.iter().filter(|p| p.file == file).cloned().collect();
        if db.set_hub_prose(project_id, file, &runs)? != Change::Unchanged {
            report.prose_files += 1;
        }
    }

    // The README's state block: one dated line per thing worth telling.
    if db.set_state_lines(project_id, &hub.state)? != Change::Unchanged {
        report.prose_files += 1;
    }

    Ok(report)
}
