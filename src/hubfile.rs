//! The files of a hub the record writes, and how each one stands on disk.
//!
//! A hub is a view of the record: four files generated whole, and the
//! handwritten texts written back out as they were written. Whether a file
//! on disk still is that view is a question `export`, `doctor --hubs` and
//! the end of a session all ask, and this is the one place that answers
//! it. Three answers would drift apart, and the first one to drift would
//! either overwrite an edit or nag about a file nobody had touched.
//!
//! The answer turns on what the record last accounted for in each file -
//! the digest of the text it wrote there, or read in from there. Before the
//! record kept that, a file that differed from the record could mean two
//! opposite things: the record had moved on since the export, or somebody
//! had written in the file. Measured on the line's hubs, 64 of the 66 files
//! `doctor` named were the first kind, and the two edits were lost in them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::db::{self, Db};
use crate::{export, hub};

/// One file of a hub as the record would write it.
pub struct Planned {
    /// Where it goes, relative to the hub, with `/` between directories.
    pub name: String,
    /// What it says, in LF endings: the file's own are put on as it is
    /// written.
    pub text: String,
    /// Whether it is one of the generated files, which carry the mark, or a
    /// document written back out as the owner wrote it.
    pub generated: bool,
    /// The document this is, when the record does not yet know which file
    /// it lives in: writing it, or finding it already there, tells it.
    pub unclaimed: Option<i64>,
}

/// How a file on disk stands against what the record would write there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// There is no such file.
    Missing,
    /// It says what the record says.
    Current,
    /// It is what the record last wrote or read, and the record has moved
    /// on since: writing it again loses nothing.
    Behind,
    /// It differs from what the record last wrote or read: somebody has
    /// written in it, and writing it again would throw that away.
    Edited,
    /// It differs, and the record has no note of it - the file predates
    /// the notes, so whether it holds an edit cannot be said.
    Unaccounted,
    /// A file of the generated kind with no mark on it: kept by hand, and
    /// not the record's until `export --adopt` hands it over.
    ByHand,
}

/// Every file the record can write into a project's hub: the generated
/// four, and - when `documents` is asked for - each handwritten text that
/// has a place in a hub.
///
/// A text the record holds without knowing its file - written through
/// `doc add` or an assistant's tool - goes where the hub already keeps the
/// same words, when `dir` names the hub and some unclaimed file there says
/// exactly that; only otherwise under a name made from its title. Made up
/// every time, the name was a second file beside the one that already held
/// the text, and the next import took that file in as a second document.
pub fn planned(db: &Db, project: &db::Project, documents: bool, dir: Option<&Path>) -> Result<Vec<Planned>> {
    let mut out = Vec::new();
    // The line's own place has no plan, changelog, diary or state block:
    // its hub is the directory every project's hub lies in, and writing a
    // plan of nothing into it would put four empty files above them all.
    if has_record_files(project) {
        for name in export::GENERATED {
            out.push(Planned {
                name: name.to_string(),
                text: generate(db, project, name)?,
                generated: true,
                unclaimed: None,
            });
        }
    }
    if !documents {
        return Ok(out);
    }
    let docs = db.documents(project.id, None)?;
    let known = |d: &db::Document| d.source_file.as_deref().is_some_and(|f| !f.is_empty());
    // The files a text is already known to live in, and - only when some
    // text has none - what every other markdown file of the hub says.
    let mut claimed: Vec<String> = docs.iter().filter(|d| known(d)).filter_map(|d| d.source_file.clone()).collect();
    let loose: Vec<(String, String)> = match dir {
        Some(dir) if docs.iter().any(|d| !known(d)) => hub::markdown_files(dir, &skip_inside(db, project, dir)?)
            .into_iter()
            .filter(|rel| !has_record_files(project) || !hub::is_reserved(rel))
            .filter_map(|rel| {
                let text = std::fs::read_to_string(dir.join(&rel)).ok()?;
                (!export::is_generated(&text)).then(|| (rel, db::lf(text.trim_end()).into_owned()))
            })
            .collect(),
        _ => Vec::new(),
    };
    for document in &docs {
        let Some(mut name) = file_for(document) else { continue };
        let text = document_text(&document.body);
        let unclaimed = (!known(document)).then_some(document.id);
        if unclaimed.is_some() {
            let same = loose.iter().find(|(rel, said)| !claimed.contains(rel) && said.as_str() == text.trim_end());
            if let Some((rel, _)) = same {
                name = rel.clone();
            }
            claimed.push(name.clone());
        }
        out.push(Planned {
            name,
            text,
            generated: false,
            unclaimed,
        });
    }
    Ok(out)
}

/// One generated file of a hub, from the record.
///
/// The single place a hub file is produced, so that `export` and
/// `doctor --hubs` cannot disagree about what the record says a file should
/// contain - a check that generated the file a second way would eventually
/// pass while the export wrote something else.
pub fn generate(db: &Db, project: &db::Project, name: &str) -> Result<String> {
    let prose = db.hub_prose(project.id, name)?;
    Ok(match name {
        n if n == export::GENERATED[0] => {
            let questions: Vec<String> = db.open_events(project.id, "question")?.into_iter().map(|(_, text)| text).collect();
            export::plan(&prose, &db.stages(project.id, false)?, &questions)
        }
        n if n == export::GENERATED[1] => export::changes(&prose, &db.stages(project.id, true)?),
        n if n == export::GENERATED[3] => export::readme(&prose, &db.state_lines(project.id)?),
        _ => export::diary(&prose, &db.diary_entries(project.id)?),
    })
}

/// A document as its file holds it: the body and one newline.
///
/// Without the generated mark: these are the owner's prose, and a mark
/// saying "edits here are overwritten" would be a lie - `import` reads an
/// edit back in.
pub fn document_text(body: &str) -> String {
    format!("{}\n", db::lf(body.trim_end()))
}

/// The file a document is written back out to, mirroring what `import`
/// reads. A kind with no place in a hub is not exported.
pub fn file_for(document: &db::Document) -> Option<String> {
    // The file it was read from, when the record knows it. Composing a name
    // from the title instead wrote a research note out beside the one it
    // came from, under a second name, and one note became two.
    if let Some(file) = document.source_file.as_deref()
        && !file.is_empty()
        && document.kind != "decisions"
    {
        return Some(file.to_string());
    }
    match document.kind.as_str() {
        "vision" => Some("Видение.md".to_string()),
        "rituals" => Some("Ритуалы.md".to_string()),
        // The decisions preamble is the head of a file the record also
        // writes the entries of, so it is not a file of its own; and a text
        // of no kind that never came from a hub has no name to go under.
        "decisions" | "other" => None,
        // The date the slug opens with is put back in front of the title,
        // because that is what the filename is sorted and addressed by:
        // writing out `Заметка.md` where `2026-09-04 — Заметка.md` came in
        // would give the note a different address on the next import.
        "research" => {
            let title = sanitise(&document.title);
            let date = document.slug.split('-').take(3).collect::<Vec<_>>().join("-");
            let named = match date.len() == 10 && date.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
                true if !title.starts_with(&date) => format!("{date} — {title}"),
                _ => title,
            };
            Some(format!("{}/{}.md", hub::RESEARCH_DIR, named))
        }
        _ => None,
    }
}

/// A title as a filename: what a filesystem refuses, turned into spaces.
fn sanitise(title: &str) -> String {
    let cleaned: String = title.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { ' ' } else { c }).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How a file stands, given what is on disk at its place.
pub fn state(db: &Db, project_id: i64, planned: &Planned, on_disk: Option<&str>) -> Result<State> {
    let Some(text) = on_disk else { return Ok(State::Missing) };
    if planned.generated && !export::is_generated(text) {
        return Ok(State::ByHand);
    }
    if db::lf(text) == planned.text.as_str() {
        return Ok(State::Current);
    }
    Ok(match db.hub_file_digest(project_id, &planned.name)? {
        Some(digest) if digest == export::digest(text) => State::Behind,
        Some(_) => State::Edited,
        None => State::Unaccounted,
    })
}

/// Writes a planned file in the line ending the file on disk already uses,
/// and remembers it as what the record last wrote there. Says whether any
/// byte changed.
///
/// The ending is the file's: every hub of this line is written on Windows,
/// and a generated file in LF would differ from its source on every line -
/// which is not a diff anybody reads, it is the file being rewritten.
pub fn write(db: &Db, project_id: i64, dir: &Path, planned: &Planned) -> Result<bool> {
    let path = dir.join(&planned.name);
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    let text = export::with_line_ending(&planned.text, export::line_ending(&before));
    let changed = before != text;
    if changed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("cannot make {}", parent.display()))?;
        }
        std::fs::write(&path, &text).with_context(|| format!("cannot write {}", path.display()))?;
    }
    db.set_hub_file_digest(project_id, &planned.name, &export::digest(&text))?;
    claim(db, planned)?;
    Ok(changed)
}

/// Remembers a file that already says what the record says as one the
/// record has accounted for, so that a change to it later reads as an edit
/// and not as a file of unknown history.
pub fn account(db: &Db, project_id: i64, planned: &Planned, on_disk: &str) -> Result<()> {
    let digest = export::digest(on_disk);
    if db.hub_file_digest(project_id, &planned.name)?.as_deref() != Some(digest.as_str()) {
        db.set_hub_file_digest(project_id, &planned.name, &digest)?;
    }
    claim(db, planned)
}

/// Tells a document which file it lives in, once that file holds it.
fn claim(db: &Db, planned: &Planned) -> Result<()> {
    if let Some(id) = planned.unclaimed {
        db.set_document_source(id, &planned.name)?;
    }
    Ok(())
}

/// The directory of a project's hub, when the record knows one and it is
/// there.
pub fn dir_of(project: &db::Project) -> Option<PathBuf> {
    let dir = PathBuf::from(project.hub_path.as_deref()?);
    dir.is_dir().then_some(dir)
}

/// The generated files of a hub that somebody keeps by hand and that an
/// export would change: what it refuses to overwrite until `--adopt`.
///
/// A file a person has been writing in is not overwritten because a
/// command was typed. The mark is what says the record owns it, and only an
/// explicit `--adopt` puts the mark there the first time. A document
/// carries no mark - it is the owner's prose written back out - so it is
/// never one of these.
pub fn by_hand(db: &Db, project: &db::Project, dir: &Path) -> Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for file in planned(db, project, false, Some(dir))? {
        let Ok(before) = std::fs::read_to_string(dir.join(&file.name)) else {
            continue;
        };
        let unchanged = before == export::with_line_ending(&file.text, export::line_ending(&before));
        if !unchanged && !before.is_empty() && !export::is_generated(&before) {
            out.push(file.name);
        }
    }
    Ok(out)
}

/// Whether a project's hub has the parts of the record in it - a plan, a
/// changelog, a diary, a state block. Every project has, the retired among
/// them; the line's own place has not: it plans nothing, and its hub is
/// the directory the projects' hubs lie in.
pub fn has_record_files(project: &db::Project) -> bool {
    project.kind != db::Kind::Service
}

/// How a project's hub is read: whole, or its texts and files alone, with
/// the hubs of other projects inside it left out.
///
/// Texts alone for a retired project - its history is closed, and what is
/// left in its hub to take is what it wrote - and for the line's own
/// place, which has no plan to read and whose every file is a text or a
/// file whatever it is called.
pub fn reading_for(db: &Db, project: &db::Project, dir: &Path) -> Result<hub::Reading> {
    Ok(hub::Reading {
        texts_only: project.retired_at.is_some() || !has_record_files(project),
        all_texts: !has_record_files(project),
        skip: skip_inside(db, project, dir)?,
    })
}

/// The directories under a hub that are not part of it: every hub of
/// another project that lies inside it, and - in the line's own place -
/// every directory at its top that holds a plan or a changelog, which is
/// the hub of a project the record does not know yet.
///
/// The second because the line's place takes in whatever lies in it, and
/// a project's hub the record has not been told about would otherwise be
/// filed whole under the line: its plan a text of no kind, its sittings
/// lost to the project they belong to.
pub fn skip_inside(db: &Db, project: &db::Project, dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = recorded_inside(db, project, dir)?;
    if !has_record_files(project) {
        let strays = unrecorded_hubs(&hub::canonical(dir), &out);
        out.extend(strays);
    }
    Ok(out)
}

/// The hubs of other projects that lie inside `dir`.
pub fn recorded_inside(db: &Db, project: &db::Project, dir: &Path) -> Result<Vec<PathBuf>> {
    let dir = hub::canonical(dir);
    let mut out = Vec::new();
    for other in db.all_projects()? {
        if other.id == project.id {
            continue;
        }
        let Some(hub_path) = other.hub_path.as_deref() else { continue };
        let hub_path = hub::canonical(Path::new(hub_path));
        if hub_path != dir && hub_path.starts_with(&dir) {
            out.push(hub_path);
        }
    }
    Ok(out)
}

/// The directories at the top of `dir`, outside `known`, that hold a plan
/// or a changelog: hubs no project of the record names.
pub fn unrecorded_hubs(dir: &Path, known: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| hub::canonical(&e.path()))
        .filter(|p| !known.contains(p))
        .filter(|p| ["План.md", "Изменения.md"].iter().any(|f| p.join(f).is_file()))
        .collect();
    out.sort();
    out
}

/// How a file the record keeps for a hub stands against the one on disk.
pub fn attachment_state(attachment: &db::Attachment, dir: &Path) -> State {
    let Ok(bytes) = std::fs::read(dir.join(&attachment.path)) else {
        return State::Missing;
    };
    match export::digest_bytes(&bytes) == attachment.digest {
        true => State::Current,
        false => State::Edited,
    }
}

/// Writes a file the record keeps back into the hub, byte for byte, when
/// the file there says something else. Says whether it wrote.
pub fn write_attachment(db: &Db, project_id: i64, dir: &Path, attachment: &db::Attachment) -> Result<bool> {
    if attachment_state(attachment, dir) == State::Current {
        return Ok(false);
    }
    let bytes = db
        .attachment_bytes(project_id, &attachment.path)?
        .with_context(|| format!("{} is listed and not held", attachment.path))?;
    let path = dir.join(&attachment.path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot make {}", parent.display()))?;
    }
    std::fs::write(&path, &bytes).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(true)
}

/// A file of a hub the record does not hold, and why it does not.
#[derive(Debug, Clone, Serialize)]
pub struct NotHeld {
    pub file: String,
    pub state: &'static str,
    pub why: &'static str,
}

/// The files of a hub that live only there: neither a part of the record,
/// nor a text it holds, nor a file it keeps - and the decisions journal,
/// when an entry of it is missing from the record.
///
/// What this cannot find, deleting the hub would lose. Files the record
/// holds and that differ from it are not here: they are the drift
/// `doctor --hubs` reports beside this, edits to take in or throw away.
pub fn not_held(db: &Db, project: &db::Project, dir: &Path) -> Result<Vec<NotHeld>> {
    let reading = reading_for(db, project, dir)?;
    let planned: Vec<String> = planned(db, project, true, Some(dir))?.into_iter().map(|p| p.name).collect();
    let kept: Vec<String> = db.attachments(project.id)?.into_iter().map(|a| a.path).collect();
    let mut out = Vec::new();
    for rel in hub::files(dir, &reading.skip) {
        if has_record_files(project) && hub::is_reserved(&rel) {
            // The parts of the record are views of it, and their drift is
            // reported apart; the wishes file has its own line in doctor.
            if rel == "Решения.md" && !journal_held(db, project, &dir.join(&rel))? {
                out.push(NotHeld {
                    file: rel,
                    state: "not-held",
                    why: "holds decisions the record does not; `rigger import` takes them in",
                });
            }
            continue;
        }
        if planned.contains(&rel) || kept.contains(&rel) {
            continue;
        }
        let size = std::fs::metadata(dir.join(&rel)).map(|m| m.len()).unwrap_or(0);
        out.push(match size > hub::ATTACHMENT_LIMIT {
            true => NotHeld {
                file: rel,
                state: "too-large",
                why: "larger than the record keeps; only the hub has it",
            },
            false => NotHeld {
                file: rel,
                state: "not-held",
                why: "lives only in the hub; `rigger import` takes it in",
            },
        });
    }
    // A hub no project names is not the line's, and it is not held either:
    // named whole, once, rather than file by file under the wrong project.
    if !has_record_files(project) {
        let root = hub::canonical(dir);
        for stray in unrecorded_hubs(&root, &recorded_inside(db, project, dir)?) {
            out.push(NotHeld {
                file: stray.strip_prefix(&root).unwrap_or(&stray).to_string_lossy().replace('\\', "/"),
                state: "unrecorded-hub",
                why: "a hub no project of the record names; `rigger import <project> --hub <dir>` reads it",
            });
        }
    }
    Ok(out)
}

/// Whether the record holds every entry of a decisions journal, and the
/// prose above them.
fn journal_held(db: &Db, project: &db::Project, path: &Path) -> Result<bool> {
    let Ok(text) = std::fs::read_to_string(path) else { return Ok(true) };
    let journal = hub::journal(&text);
    for decision in &journal.decisions {
        if !db.holds_decision(project.id, &decision.at(), &decision.event_body())? {
            return Ok(false);
        }
    }
    if journal.preamble.trim().is_empty() {
        return Ok(true);
    }
    Ok(db
        .documents(project.id, Some("decisions"))?
        .iter()
        .any(|d| db::lf(&d.body).trim_end() == journal.preamble.trim_end()))
}

/// What bringing a hub up to date did.
#[derive(Debug, Default, Serialize)]
pub struct Refreshed {
    /// Files written again because the record had moved on since it last
    /// wrote them.
    pub written: Vec<String>,
    /// Files left as they are because somebody has written in them.
    pub edited: Vec<String>,
}

/// Brings the files a hub already has up to date with the record, wherever
/// that loses nothing.
///
/// A file the record last wrote and nobody touched since is written again;
/// one somebody wrote in is left alone and named, for `import` to take in
/// or `export` to discard. A file of unknown history is left too - that is
/// for `doctor --hubs` to raise and a person to settle - and a file the hub
/// does not have is not made: a hub that keeps no rituals file has not
/// asked for one.
pub fn refresh(db: &Db, project: &db::Project) -> Result<Refreshed> {
    let mut out = Refreshed::default();
    let Some(dir) = dir_of(project) else { return Ok(out) };
    for planned in planned(db, project, true, Some(&dir))? {
        let on_disk = std::fs::read_to_string(dir.join(&planned.name)).ok();
        match state(db, project.id, &planned, on_disk.as_deref())? {
            State::Behind => {
                write(db, project.id, &dir, &planned)?;
                out.written.push(planned.name);
            }
            State::Current => account(db, project.id, &planned, on_disk.as_deref().unwrap_or_default())?,
            State::Edited => out.edited.push(planned.name),
            State::Missing | State::Unaccounted | State::ByHand => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_loses_what_a_filesystem_refuses() {
        assert_eq!(sanitise("A: b / c?"), "A b c");
    }

    /// The text a document's file holds carries the record's endings and
    /// one newline, whatever the body was given with.
    #[test]
    fn a_document_file_is_its_body_and_one_newline() {
        assert_eq!(document_text("line one\r\nline two\r\n\r\n"), "line one\nline two\n");
    }
}
