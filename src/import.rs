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
    /// Struck from the hub since it was last read.
    pub tasks_dropped: u32,
    pub versions_dropped: u32,
    pub questions_withdrawn: u32,
    pub decisions_added: u32,
    pub questions_added: u32,
    /// Wishes taken out of the wishes file and into the record.
    pub wishes_added: u32,
    pub diary_added: u32,
    pub diary_updated: u32,
    /// Files whose between-stage prose the record took in.
    pub prose_files: u32,
    /// Handwritten texts taken into the record: vision, rituals, research,
    /// and every other `.md` of the hub.
    pub documents_added: u32,
    pub documents_updated: u32,
    /// Generated files that were not read back, because they are still
    /// exactly what the record last wrote: reading them could only take the
    /// record back to how it stood at that export.
    pub untouched: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.documents_added
            + self.documents_updated
            + self.versions_added
            + self.versions_updated
            + self.tasks_added
            + self.tasks_updated
            + self.tasks_dropped
            + self.versions_dropped
            + self.questions_withdrawn
            + self.decisions_added
            + self.questions_added
            + self.wishes_added
            + self.diary_added
            + self.diary_updated
            + self.prose_files
            > 0
    }
}

/// A slug no document of the project holds yet: the one asked for, or the
/// first free number after it. The hub keeps its own texts apart; this
/// keeps them apart from texts the record has from anywhere else.
fn free_slug(db: &Db, project_id: i64, wanted: &str) -> Result<String> {
    let mut slug = wanted.to_string();
    for n in 2.. {
        if db.document(project_id, &slug)?.is_none() {
            break;
        }
        slug = format!("{wanted}-{n}");
    }
    Ok(slug)
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

    // Which of the hub's files speak in this import. One that was not there
    // says nothing - a missing plan is not a plan with no stages, and a
    // missing README is not one with no state lines. One the record wrote
    // and nobody has touched since says nothing either: it is the record as
    // it stood at that export, and reading it back would undo whatever the
    // record has learnt after - a ticked box, a state line, a sitting.
    let mut speaking: Vec<&str> = Vec::new();
    for file in crate::hub::FILES {
        let Some(digest) = hub.digest_of(file) else { continue };
        let generated = crate::export::GENERATED.contains(&file);
        if generated && db.hub_file_digest(project_id, file)?.as_deref() == Some(digest) {
            report.untouched.push(file.to_string());
            continue;
        }
        speaking.push(file);
    }
    let speaks = |file: &str| speaking.contains(&file);

    // Closed stages first: when a version appears in both files, the changelog
    // is the one that knows it shipped, and the plan must not undo that.
    //
    // Nor may it undo where the changelog put it. A plan keeps its shipped
    // stages in the major map - lyrn lists four of them as `###` under a
    // block heading, and the changelog writes the same four as `##` with
    // the entry about each - so a second pass that rewrote the shape moved
    // every one of them out of the changelog and into the plan's order.
    // The file a stage is being read from decides: a stage in hand from
    // the changelog owns its shape; one from the plan owns it only while
    // the stage has not shipped. A stage that shipped is the changelog's
    // - or, when the tag closed it before the plan was told, nobody's yet
    // - and the plan's boxes no longer speak for its tasks either: they
    // are the state of the work before the tag, and a task the record
    // closed since is not reopened by them.
    let closed = hub.closed_stages.iter().map(|stage| (stage, true));
    let open = hub.open_stages.iter().map(|stage| (stage, false));
    let mut named = Vec::new();
    for (stage, from_changelog) in closed.chain(open) {
        // Named whether or not its file speaks: a stage the file holds is
        // not one the hub struck, even when the file is not read back.
        named.push(stage.version.as_str());
        if !speaks(if from_changelog { "Изменения.md" } else { "План.md" }) {
            continue;
        }
        let version = db.upsert_version(project_id, stage, from_changelog)?;
        tally(version.change, &mut report.versions_added, &mut report.versions_updated);
        // A plan the record wrote cannot overrule the record. Its empty
        // boxes are a picture of how things stood at the last export, and
        // a task closed since is closed - found when importing this
        // project's own hub silently reopened three tasks closed minutes
        // before, on a version that had not shipped yet so the guard for
        // shipped ones did not cover them.
        let may_reopen = (from_changelog || !version.shipped) && !hub.plan_is_generated;
        let mut kept = Vec::new();
        for (position, task) in stage.tasks.iter().enumerate() {
            let (id, change) = db.upsert_task(project_id, version.id, position, task, may_reopen)?;
            tally(change, &mut report.tasks_added, &mut report.tasks_updated);
            kept.push(id);
        }
        // What the reading no longer lists is struck, when the reading is
        // the one that speaks for the stage. A plan behind a tag does not.
        if may_reopen {
            report.tasks_dropped += db.drop_tasks_not_in(version.id, &kept)?;
        }
    }
    // A stage neither file names any more was renumbered or struck by
    // hand; a tag would have shipped it, and a shipped stage is kept.
    //
    // Only when the plan is a person's writing. A generated plan lists the
    // open stages and nothing else - it says so in its own first lines -
    // so "neither file names it" means "it shipped and scrolled out of the
    // export", not "somebody struck it". Importing sixteen generated hubs
    // once struck 367 versions and 976 tasks across the line on exactly
    // this reasoning. The task half of this guard was already here; the
    // version half was not, which is how a class of defect gets fixed in
    // one of the two places it lives.
    //
    // And only when there was a plan to read. A hub without one - a folder
    // holding a diary and nothing else - named no stage because it had no
    // file to name them in, and striking on that reasoning would empty the
    // plan of every project whose hub was read in part.
    if speaks("План.md") && !hub.plan_is_generated {
        report.versions_dropped += db.drop_versions_not_in(project_id, &named)?;
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

    // The questions live in the plan, so only a plan that speaks can add
    // one or strike one.
    if speaks("План.md") {
        for question in &hub.questions {
            // A question has no date of its own; it is open now, which is
            // what the inbox will ask for later.
            if db.record_event(project_id, "question", question, &crate::db::now(), "owner")? == Change::Added {
                report.questions_added += 1;
            }
        }
        // One the owner struck from the hub by hand is withdrawn; one an
        // assistant asked was never in the file and stays.
        report.questions_withdrawn += db.withdraw_questions_not_in(project_id, &hub.questions)?;
    }

    // The wishes file, read into the record so that it can stop being a
    // place the record has to be told about. Not struck the way questions
    // are: a wish is sorted into the plan, and the file is emptied by the
    // person doing the sorting - nothing in the file says a wish was
    // withdrawn rather than dealt with.
    for wish in &hub.wishes {
        if db.holds_wish(project_id, wish)? {
            continue;
        }
        if db.record_event(project_id, "wish", wish, &crate::db::now(), "owner")? == Change::Added {
            report.wishes_added += 1;
        }
    }

    // The diary becomes sessions that already ended: an entry is one
    // sitting, written before rigger knew what a sitting was, and it has
    // nowhere else to live that an export could read back.
    if speaks("Дневник.md") {
        let (added, updated) = db.import_diary(project_id, &hub.diary)?;
        report.diary_added += added;
        report.diary_updated += updated;
    }

    // Everything that belongs to no stage and no entry - a preamble, a map,
    // the headings that group stages into blocks. Without it an export would
    // generate a file that had lost most of what a person had written in it:
    // measured on this project's own hub, 88% of the changelog and 42% of
    // the plan is prose of this kind.
    let mut files: Vec<&str> = hub.prose.iter().map(|p| p.file.as_str()).filter(|f| speaks(f)).collect();
    files.sort_unstable();
    files.dedup();
    for file in files {
        let runs: Vec<crate::hub::Prose> = hub.prose.iter().filter(|p| p.file == file).cloned().collect();
        if db.set_hub_prose(project_id, file, &runs)? != Change::Unchanged {
            report.prose_files += 1;
        }
    }

    // The README's state block: one dated line per thing worth telling.
    // Replaced wholesale, so only by a README that was there to read.
    if speaks("README.md") && db.set_state_lines(project_id, &hub.state)? != Change::Unchanged {
        report.prose_files += 1;
    }

    // The handwritten texts: the vision, the rituals, the preamble of the
    // decisions journal, the research notes. These are what a hub kept that
    // the record could not rebuild, and taking them in is what lets a hub
    // become an export rather than half the truth.
    //
    // A text already in the record is only rewritten when the file differs,
    // so re-importing an unchanged hub reports nothing - and a document
    // written through `doc edit` and not yet exported is not silently
    // replaced by the older file it came from.
    for document in &hub.documents {
        // Found by the file it came from first: that is the one name a
        // text has that cannot collide within a hub. By its slug second,
        // for what was read before the record kept the file - and only a
        // slug held by no other file, or the text of one file would be
        // written over the text of another.
        //
        // And last by what it says: a text written through `doc add` or an
        // assistant's tool has no file and an address of its own choosing,
        // and the same words in the hub - put there by an export, or by hand
        // as well - are that text, not a second one.
        let unfiled = |held: &crate::db::Document| held.source_file.as_deref().is_none_or(str::is_empty);
        let existing = match db.document_by_source(project_id, &document.source_file)? {
            Some(held) => Some(held),
            None => match db
                .document(project_id, &document.slug)?
                .filter(|held| unfiled(held) || held.source_file.as_deref() == Some(document.source_file.as_str()))
            {
                Some(held) => Some(held),
                None => db
                    .documents(project_id, Some(&document.kind))?
                    .into_iter()
                    .find(|held| unfiled(held) && crate::db::lf(&held.body).trim_end() == document.body),
            },
        };
        match &existing {
            // Unchanged in substance, but the record may not yet know which
            // file it came from - documents predate that column. Learning it
            // is not a change worth reporting; not learning it makes the
            // export invent a filename and write the note out twice.
            //
            // The title is not part of the substance: a text written through
            // the tool carries a title its first heading need not repeat,
            // and reading the file back renamed it after that heading.
            Some(held) if crate::db::lf(&held.body).trim_end() == document.body => {
                if held.source_file.as_deref() != Some(document.source_file.as_str()) {
                    db.set_document_source(held.id, &document.source_file)?;
                }
            }
            held => {
                let slug = match held {
                    Some(held) => {
                        report.documents_updated += 1;
                        held.slug.clone()
                    }
                    None => {
                        report.documents_added += 1;
                        free_slug(db, project_id, &document.slug)?
                    }
                };
                db.write_document_from(project_id, &document.kind, &slug, &document.title, &document.body, Some(&document.source_file))?;
            }
        }
        // The record now holds what the file says, so the file is one it
        // has accounted for, and a change after this is an edit. Not for
        // the decisions preamble: its file is the journal, whose entries
        // are events, and no export writes it.
        if document.kind != "decisions" {
            db.set_hub_file_digest(project_id, &document.source_file, &document.digest)?;
        }
    }

    Ok(report)
}
