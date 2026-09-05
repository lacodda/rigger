//! The record: one SQLite file, migrated forward by version.
//!
//! Every fact rigger shows is a query over these tables. The schema version
//! lives in SQLite's `user_version` pragma; migrations are applied in order
//! and never edited once released - a change is a new migration.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// Migrations in order. Index + 1 is the schema version they bring the
/// database to.
const MIGRATIONS: &[&str] = &[
    // v1: the five entities of the model.
    "
    CREATE TABLE projects (
        id         INTEGER PRIMARY KEY,
        name       TEXT NOT NULL UNIQUE,
        path       TEXT NOT NULL UNIQUE,
        remote     TEXT,
        tier       TEXT,
        created_at TEXT NOT NULL
    );
    CREATE TABLE versions (
        id           INTEGER PRIMARY KEY,
        project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name         TEXT NOT NULL,
        title        TEXT,
        status       TEXT NOT NULL DEFAULT 'planned',
        planned_week TEXT,
        shipped_at   TEXT,
        UNIQUE (project_id, name)
    );
    CREATE TABLE tasks (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        version_id INTEGER REFERENCES versions(id) ON DELETE SET NULL,
        title      TEXT NOT NULL,
        status     TEXT NOT NULL DEFAULT 'open',
        created_at TEXT NOT NULL,
        closed_at  TEXT
    );
    CREATE TABLE sessions (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        started_at TEXT NOT NULL,
        ended_at   TEXT
    );
    CREATE TABLE events (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
        version_id INTEGER REFERENCES versions(id) ON DELETE SET NULL,
        task_id    INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
        kind       TEXT NOT NULL,
        body       TEXT NOT NULL,
        author     TEXT NOT NULL DEFAULT 'assistant',
        created_at TEXT NOT NULL
    );
    CREATE INDEX events_by_project ON events (project_id, created_at);
    ",
    // v2: what git says about a project, refreshed by `sync`. Kept on the
    // project rather than derived on every read: a walk of history costs
    // more than a column, and the packet wants the number, not the walk.
    "
    ALTER TABLE projects ADD COLUMN commits_since_tag INTEGER;
    ALTER TABLE projects ADD COLUMN last_commit_at TEXT;
    ALTER TABLE projects ADD COLUMN synced_at TEXT;
    -- Where a shipped version's date came from. 'tag' is proof; anything
    -- else is a claim read from a plan, and the difference is the whole
    -- point of `sync` - a date alone cannot tell them apart, because an
    -- imported hub carries dates too.
    ALTER TABLE versions ADD COLUMN shipped_source TEXT;
    ",
    // v3: the commit an event came from. A commit is read again on every
    // sync, so the hash - not the text - is what makes recording it twice
    // impossible; a rewritten message would otherwise arrive as a second
    // event about the same change.
    "
    ALTER TABLE events ADD COLUMN commit_hash TEXT;
    CREATE UNIQUE INDEX events_by_commit ON events (project_id, commit_hash) WHERE commit_hash IS NOT NULL;
    -- Where a task sits in its stage. A plan is edited: rewording a line
    -- used to add a second task and leave the first one open for ever,
    -- because the text was the only thing identifying it.
    ALTER TABLE tasks ADD COLUMN position INTEGER;
    ",
    // v4: a full-text index over event bodies, kept in step with the table
    // by triggers rather than by remembering to write to both.
    //
    // `content=` makes it an external-content index: the text is not stored
    // twice, the index holds only what it needs to search. `unicode61`
    // folds case and strips diacritics for Cyrillic as well as Latin, which
    // matters because these events are written in both.
    "
    -- The moment a version shipped, beside the day it shipped on.
    -- `shipped_at` is a day because that is what a changelog and a calendar
    -- speak in; but this line ships several versions on one day, and `why`
    -- has to know which events belong to which release. The day cannot tell
    -- them apart, so the tag's own timestamp is kept as well.
    ALTER TABLE versions ADD COLUMN shipped_ts TEXT;
    CREATE VIRTUAL TABLE events_fts USING fts5(
        body,
        content='events',
        content_rowid='id',
        tokenize='unicode61'
    );
    INSERT INTO events_fts(rowid, body) SELECT id, body FROM events;
    CREATE TRIGGER events_fts_insert AFTER INSERT ON events BEGIN
        INSERT INTO events_fts(rowid, body) VALUES (new.id, new.body);
    END;
    CREATE TRIGGER events_fts_delete AFTER DELETE ON events BEGIN
        INSERT INTO events_fts(events_fts, rowid, body) VALUES ('delete', old.id, old.body);
    END;
    CREATE TRIGGER events_fts_update AFTER UPDATE OF body ON events BEGIN
        INSERT INTO events_fts(events_fts, rowid, body) VALUES ('delete', old.id, old.body);
        INSERT INTO events_fts(rowid, body) VALUES (new.id, new.body);
    END;
    ",
    // v5: how fast a project is meant to release. `tier` has been a column
    // since v1 and was never written to - a place kept for the rotation the
    // owner's calendar described. The rhythm joins it, because a tier alone
    // cannot say a project has fallen behind: A is "every two or three
    // weeks", and the number is what a check can be made of.
    //
    // Nullable on purpose. A project with no tier is not in the rotation by
    // omission rather than by decision, and the calendar says so instead of
    // inventing a default that would make it look scheduled.
    "
    ALTER TABLE projects ADD COLUMN rhythm_weeks INTEGER;
    ",
    // v6: what kind of thing a project is. Everything recorded so far has
    // been a repository, and every part of rigger assumed so - `sync` asks
    // git about it, `doctor` lists it as never synced until git answers.
    //
    // A retro needs somewhere to leave its summary that is not one of the
    // projects it looked at, and that place has no repository and never
    // will. Recording it as a repo-less repository made `sync` warn about
    // it on every run and `doctor` advise a command that could not help:
    // the record would have been nagging about a project working exactly
    // as intended. So the kind is written down instead of assumed.
    //
    // `NOT NULL DEFAULT` rather than nullable: every existing row is a
    // repository, and there is no third state where the kind is unknown.
    "
    ALTER TABLE projects ADD COLUMN kind TEXT NOT NULL DEFAULT 'repo';
    ",
    // v7: the prose of a hub, which until now lived only in markdown.
    //
    // The import read a hub's skeleton - version headings, checkboxes,
    // dated decisions, questions - and left the rest on disk. Measured on
    // this project's own hub before the export was written: the changelog
    // is 88% prose the record had never held, and the plan 42%. Generating
    // those files from the record would therefore have deleted most of
    // them, which is the opposite of what an export is for.
    //
    // So the prose comes into the record first. A version keeps the entry
    // written about it, a session keeps its diary entry, and the plan keeps
    // the parts that are neither a stage nor a task - a preamble, a map,
    // the headings that group stages into blocks.
    "
    ALTER TABLE versions ADD COLUMN notes TEXT;
    -- How the hub wrote the stage: how deep its heading sat, and whether
    -- its prose came before the tasks or after them. Both are shape rather
    -- than content, and both are lost the moment an export guesses.
    ALTER TABLE versions ADD COLUMN heading_depth INTEGER;
    ALTER TABLE versions ADD COLUMN notes_first INTEGER;
    -- The stage heading exactly as the hub wrote it. An export cannot
    -- compose one: the same hub writes both `выпущен` and `выпущена`,
    -- because the word agrees with whatever noun the owner had in mind.
    ALTER TABLE versions ADD COLUMN heading TEXT;
    -- How many runs of prose stood before the stage in its file. A plan
    -- groups stages under block headings, and the blocks are prose; without
    -- this an export can only pile every stage below every heading.
    ALTER TABLE versions ADD COLUMN after_prose INTEGER;
    ALTER TABLE sessions ADD COLUMN heading TEXT;
    ALTER TABLE sessions ADD COLUMN notes TEXT;
    -- Whether a rule stood between this entry and the next. It belongs to
    -- neither entry, and dropping it lost a separator from every busy day.
    ALTER TABLE sessions ADD COLUMN followed_by_rule INTEGER;
    -- Prose of a project's plan that belongs to no single stage, kept by
    -- the section it was found in so that the export can put it back where
    -- it was rather than in one lump at the top.
    CREATE TABLE hub_prose (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        file       TEXT NOT NULL,
        position   INTEGER NOT NULL,
        heading    TEXT,
        body       TEXT NOT NULL,
        UNIQUE (project_id, file, position)
    );
    ",
    // Prose written after a stage's list, kept apart from the prose written
    // before it. A stage is explained first and concluded afterwards, and
    // 123 stages across this line's hubs do both; a single field with a
    // flag for its side could hold one half or the other, so the closing
    // line of each of them was written above its own list. The flag stays
    // in the table, unread: it belongs to a shipped migration.
    "
    ALTER TABLE versions ADD COLUMN notes_after TEXT;
    ",
    // Where a stage stood among the stages of its file, so an export can
    // put the stages of one block back in the order they were written in.
    // `after_prose` names the block and no more: all seven stages of one
    // hub's changelog follow the same single run, and without this the
    // export fell back on the order the rows happened to come out of the
    // table - which is not an order, and wrote that changelog backwards.
    // A rank rather than a line number: the export writes its marker above
    // everything, so line numbers would drift by two on every run.
    "
    ALTER TABLE versions ADD COLUMN rank INTEGER;
    ",
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub struct Db {
    conn: Connection,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub remote: Option<String>,
    pub created_at: String,
    /// Where the project sits in the release rotation, once someone says.
    pub tier: Option<String>,
    /// Weeks it is meant to go between releases.
    pub rhythm_weeks: Option<u32>,
    /// What kind of thing this is: a repository, or a place the record
    /// keeps for itself.
    pub kind: Kind,
}

/// What a project is, as far as the parts of rigger that read git care.
///
/// Almost everything recorded is a repository. The exception is the line
/// itself - somewhere for a retro to leave its summary that is not one of
/// the projects it looked at. Such a place has no repository and never
/// will, and saying so is cheaper than every reader guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A checkout on disk, which `sync` reads.
    Repo,
    /// A place the record keeps for itself; git is never asked about it.
    Service,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Repo => "repo",
            Kind::Service => "service",
        }
    }

    /// Whether `sync` and `doctor` should expect git to answer for it.
    pub fn reads_git(self) -> bool {
        self == Kind::Repo
    }
}

impl From<String> for Kind {
    /// An unknown kind reads as a repository: that is what every row was
    /// before the column existed, and a newer rigger writing a kind this
    /// one does not know should not make the project vanish from a list.
    fn from(text: String) -> Kind {
        match text.as_str() {
            "service" => Kind::Service,
            _ => Kind::Repo,
        }
    }
}

/// What writing a record did. An import reports the three apart, so that
/// running it twice on an unchanged hub visibly does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Updated,
    Unchanged,
}

/// The version being built and the tasks still open under it.
#[derive(Debug)]
pub struct CurrentStage {
    pub version: String,
    pub title: Option<String>,
    pub tasks: Vec<Task>,
}

/// A task as the plan holds it. The id travels with the title because an
/// assistant that is shown the stage is the one that closes its lines, and
/// a title is not a name: two stages can spell the same step.
#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
}

/// A question waiting for the owner, and where it came from.
#[derive(Debug, Clone, Serialize)]
pub struct Waiting {
    pub project: String,
    pub id: i64,
    pub date: String,
    pub body: String,
}

/// What a window holds for one project.
#[derive(Debug, Clone, Serialize)]
pub struct DigestFacts {
    pub shipped: Vec<String>,
    pub decisions: u32,
    pub findings: u32,
    pub changes: u32,
    pub waiting: u32,
}

/// An event as a search returns it.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub project: String,
    pub kind: String,
    pub date: String,
    pub body: String,
    pub from_git: bool,
    pub commit_hash: Option<String>,
}

/// What the record knows about one version.
#[derive(Debug, Clone, Serialize)]
pub struct VersionFacts {
    pub name: String,
    pub title: Option<String>,
    pub status: String,
    pub shipped_at: Option<String>,
    /// The moment the tag was made, where one is known. Several versions of
    /// this line ship on one day, so the day cannot bound their work.
    pub shipped_ts: Option<String>,
}

/// An event as the packet reads it back.
pub struct RecentEvent {
    pub kind: String,
    pub date: String,
    pub body: String,
    /// Read from a commit message rather than written by a person.
    pub from_git: bool,
}

/// How a hub wrote a stage's heading, as against what it said.
///
/// Kept apart from the content because it is the part an export cannot
/// invent: the same hub writes both `выпущен` and `выпущена`, nests some
/// stages deeper than others, and puts its prose before the tasks in a plan
/// and after them in a changelog. Guessing any of it rewrites real files.
#[derive(Debug, Clone, Default, PartialEq)]
struct Shape {
    heading: Option<String>,
    depth: Option<i64>,
    after_prose: Option<i64>,
    rank: Option<i64>,
}

/// A stage as the record already holds it: its id, title, status, the
/// date it shipped, the prose written before and after its list, and the
/// shape its heading had. Named because an upsert compares all of it at
/// once.
type Recorded = (i64, Option<String>, String, Option<String>, Option<String>, Option<String>, Shape);

impl Shape {
    fn of(stage: &crate::hub::Stage) -> Shape {
        Shape {
            heading: (!stage.heading.trim().is_empty()).then(|| stage.heading.clone()),
            depth: Some(stage.depth as i64),
            after_prose: Some(stage.after_prose as i64),
            rank: Some(stage.rank as i64),
        }
    }
}

/// A version row as `mark_shipped` needs to see it: what the record
/// currently holds, before a tag overrules it.
struct VersionRow {
    id: i64,
    status: String,
    shipped_at: Option<String>,
    shipped_ts: Option<String>,
    source: Option<String>,
}

/// One sitting, and the events written while it was open.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: i64,
    pub project_id: i64,
    pub started_at: String,
    /// `None` while it is still open.
    pub ended_at: Option<String>,
}

/// What git said about a project when it was last read.
#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub commits_since_tag: u32,
    pub last_commit_at: Option<String>,
    pub synced_at: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Counts {
    pub projects: u64,
    pub versions: u64,
    pub tasks: u64,
    pub sessions: u64,
    pub events: u64,
}

impl Db {
    /// Creates the database (and its directory) if needed and migrates it
    /// to the current schema. Used by `rigger init`.
    pub fn create(path: &Path) -> Result<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let db = Db::connect(path)?;
        db.migrate()?;
        Ok(db)
    }

    /// Opens an existing database; a missing file is the "run `rigger init`
    /// first" case, reported as such rather than as a fresh empty database.
    pub fn open(path: &Path) -> Result<Db> {
        if !path.exists() {
            bail!("no database at {} - run `rigger init` first", path.display());
        }
        let db = Db::connect(path)?;
        db.migrate()?;
        Ok(db)
    }

    fn connect(path: &Path) -> Result<Db> {
        let conn = Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Db {
            conn,
            path: path.to_path_buf(),
        })
    }

    fn migrate(&self) -> Result<()> {
        let current = self.schema_version()?;
        if current > SCHEMA_VERSION {
            bail!(
                "{} is at schema version {current}, newer than this rigger understands ({SCHEMA_VERSION}); update rigger",
                self.path.display()
            );
        }
        // A migration rewrites the record, and the record is the truth here:
        // copy it aside first, so a migration that goes wrong costs nothing.
        // Copying a fresh empty database would only be noise.
        if current > 0 && current < SCHEMA_VERSION {
            let backup = self.backup()?;
            eprintln!(
                "Migrating schema {current} -> {SCHEMA_VERSION}; the previous database is saved as {}",
                backup.display()
            );
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let target = i as u32 + 1;
            self.conn
                .execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {target}; COMMIT;"))
                .with_context(|| format!("migration to schema version {target} failed"))?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Copies the database beside itself, stamped with the moment and the
    /// schema it holds. SQLite's own backup API is used rather than a file
    /// copy: it is consistent even while something else is connected.
    pub fn backup(&self) -> Result<PathBuf> {
        let stamp = now().replace([':', '-'], "").replace('T', "-").replace('Z', "");
        let name = format!(
            "{}.v{}-{stamp}.bak",
            self.path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_else(|| "rigger".into()),
            self.schema_version()?
        );
        let target = self.path.with_file_name(name);
        let mut out = Connection::open(&target).with_context(|| format!("cannot create {}", target.display()))?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut out).context("cannot start the backup")?;
        backup.step(-1).context("the backup did not finish")?;
        Ok(target)
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    pub fn add_project(&self, name: &str, path: &str, remote: Option<&str>, kind: Kind) -> Result<Project> {
        if let Some(existing) = self.project_by_path(path)? {
            bail!("{} is already recorded as project '{}'", path, existing.name);
        }
        if self.project_by_name(name)?.is_some() {
            bail!("a project named '{name}' already exists; pick another with --name");
        }
        let created_at = now();
        self.conn.execute(
            "INSERT INTO projects (name, path, remote, created_at, kind) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, path, remote, created_at, kind.as_str()],
        )?;
        Ok(Project {
            id: self.conn.last_insert_rowid(),
            name: name.to_string(),
            path: path.to_string(),
            remote: remote.map(str::to_string),
            created_at,
            tier: None,
            rhythm_weeks: None,
            kind,
        })
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, path, remote, created_at, tier, rhythm_weeks, kind FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], row_to_project)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn project_by_name(&self, name: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, path, remote, created_at, tier, rhythm_weeks, kind FROM projects WHERE name = ?1",
                [name],
                row_to_project,
            )
            .optional()?)
    }

    /// Records a diary entry read from a hub as a session that already ended.
    ///
    /// A hub's diary is one entry per sitting, written before rigger knew
    /// what a sitting was. Importing them as sessions is what lets an export
    /// put the diary back: the entry has nowhere else to live, and inventing
    /// a second table for "diary entries" beside `sessions` would leave the
    /// record with two answers to what a sitting is.
    ///
    /// Identified by its day and heading, so importing the same hub twice is
    /// quiet - a day may hold two entries, but not two with the same title.
    pub fn upsert_diary_entry(&self, project_id: i64, entry: &crate::hub::DiaryEntry) -> Result<Change> {
        let at = format!("{}T00:00:00Z", entry.date);
        let existing: Option<(i64, Option<String>, Option<bool>)> = self
            .conn
            .query_row(
                "SELECT id, notes, followed_by_rule FROM sessions WHERE project_id = ?1 AND started_at = ?2 \
                 AND ((heading IS NULL AND ?3 IS NULL) OR heading = ?3)",
                params![project_id, at, entry.heading],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let notes = (!entry.body.trim().is_empty()).then(|| entry.body.clone());
        match existing {
            Some((_, was, rule)) if was == notes && rule == Some(entry.followed_by_rule) => Ok(Change::Unchanged),
            Some((id, _, _)) => {
                self.conn.execute(
                    "UPDATE sessions SET notes = ?2, followed_by_rule = ?3 WHERE id = ?1",
                    params![id, notes, entry.followed_by_rule],
                )?;
                Ok(Change::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO sessions (project_id, started_at, ended_at, heading, notes, followed_by_rule)                      VALUES (?1, ?2, ?2, ?3, ?4, ?5)",
                    params![project_id, at, entry.heading, notes, entry.followed_by_rule],
                )?;
                Ok(Change::Added)
            }
        }
    }

    /// Every diary entry the record holds for a project, newest first.
    ///
    /// A day holds several entries - a hub writes `2026-09-03 (вечер)` and
    /// `2026-09-03 (ночь, позже)` on the same date - and they are all
    /// stamped with that day's midnight, so the day cannot order them. The
    /// row id can: entries were read from the file top-down, so ascending
    /// id within a day is the order they were written in. Descending would
    /// reverse every busy day, which is what the first live run did.
    pub fn diary_entries(&self, project_id: i64) -> Result<Vec<crate::hub::DiaryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(started_at, 1, 10), heading, notes, followed_by_rule FROM sessions \
             WHERE project_id = ?1 AND notes IS NOT NULL ORDER BY started_at DESC, id ASC",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            Ok(crate::hub::DiaryEntry {
                date: r.get(0)?,
                heading: r.get(1)?,
                body: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                followed_by_rule: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Replaces the prose the record holds for one file of a hub.
    ///
    /// Replaced wholesale rather than merged: these runs are identified by
    /// where they sit in a file, and a paragraph inserted at the top would
    /// otherwise renumber everything below it into a pile of updates. The
    /// file is the unit a person edits, so the file is the unit that is
    /// written back.
    pub fn set_hub_prose(&self, project_id: i64, file: &str, runs: &[crate::hub::Prose]) -> Result<Change> {
        let before = self.hub_prose(project_id, file)?;
        if before == runs {
            return Ok(Change::Unchanged);
        }
        self.conn
            .execute("DELETE FROM hub_prose WHERE project_id = ?1 AND file = ?2", params![project_id, file])?;
        for run in runs {
            self.conn.execute(
                "INSERT INTO hub_prose (project_id, file, position, heading, body) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![project_id, file, run.position as i64, run.heading, run.body],
            )?;
        }
        Ok(if before.is_empty() { Change::Added } else { Change::Updated })
    }

    /// The prose the record holds for one file of a hub, in file order.
    pub fn hub_prose(&self, project_id: i64, file: &str) -> Result<Vec<crate::hub::Prose>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file, position, heading, body FROM hub_prose WHERE project_id = ?1 AND file = ?2 ORDER BY position")?;
        let rows = stmt.query_map(params![project_id, file], |r| {
            Ok(crate::hub::Prose {
                file: r.get(0)?,
                position: r.get::<_, i64>(1)? as usize,
                heading: r.get(2)?,
                body: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every version of a project as a stage, for the export to write back.
    pub fn stages(&self, project_id: i64, shipped: bool) -> Result<Vec<crate::hub::Stage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, title, shipped_at, notes, heading_depth, notes_after, heading, after_prose, rank FROM versions \
             WHERE project_id = ?1 AND (status = 'shipped') = ?2 \
             ORDER BY COALESCE(rank, 0), id",
        )?;
        let rows = stmt.query_map(params![project_id, shipped], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                crate::hub::Stage {
                    version: r.get(1)?,
                    title: r.get(2)?,
                    shipped_on: r.get(3)?,
                    tasks: Vec::new(),
                    notes: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    // A stage recorded before the hub was read for shape
                    // has no depth of its own: `##` is what a hub writes
                    // by default.
                    depth: r.get::<_, Option<i64>>(5)?.unwrap_or(2) as usize,
                    notes_after: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    heading: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    after_prose: r.get::<_, Option<i64>>(8)?.unwrap_or(i64::MAX) as usize,
                    rank: r.get::<_, Option<i64>>(9)?.unwrap_or(0) as usize,
                },
            ))
        })?;

        let mut stages: Vec<(i64, crate::hub::Stage)> = rows.collect::<rusqlite::Result<_>>()?;
        // Newest first for the changelog, oldest first for the plan: a hub
        // reads its history backwards and its future forwards.
        stages.sort_by_key(|(_, s)| version_order(&s.version));
        if shipped {
            stages.reverse();
        }
        let mut out = Vec::new();
        for (id, mut stage) in stages {
            stage.tasks = self.tasks_of_version(id)?;
            out.push(stage);
        }
        Ok(out)
    }

    /// The tasks of one version, in the order the plan listed them.
    fn tasks_of_version(&self, version_id: i64) -> Result<Vec<crate::hub::Task>> {
        let mut stmt = self
            .conn
            .prepare("SELECT title, status FROM tasks WHERE version_id = ?1 ORDER BY COALESCE(position, id), id")?;
        let rows = stmt.query_map([version_id], |r| {
            Ok(crate::hub::Task {
                title: r.get(0)?,
                done: r.get::<_, String>(1)? != "open",
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The place the record keeps for itself, if one has been made.
    ///
    /// There is at most one in practice and the code does not enforce it:
    /// a second would be a decision the owner made, and refusing it here
    /// would be the record arguing with them about their own filing.
    pub fn service_project(&self) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, path, remote, created_at, tier, rhythm_weeks, kind FROM projects                  WHERE kind = 'service' ORDER BY id LIMIT 1",
                [],
                row_to_project,
            )
            .optional()?)
    }

    pub fn project_by_path(&self, path: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, path, remote, created_at, tier, rhythm_weeks, kind FROM projects WHERE path = ?1",
                [path],
                row_to_project,
            )
            .optional()?)
    }

    /// Records a stage, or updates the one already recorded under that
    /// version. Returns its id and whether anything changed - the import
    /// report counts on the difference, and a second import of an unchanged
    /// hub must report nothing.
    ///
    /// `owns_shape` says whether this reading of the stage may say how it
    /// is written. A version appears in both files of a hub - the plan
    /// keeps its shipped stages in the major map, the changelog holds the
    /// entry about each - and only one of them can decide the heading, the
    /// depth and the place. The changelog does, for a stage that shipped.
    pub fn upsert_version(&self, project_id: i64, stage: &crate::hub::Stage, owns_shape: bool) -> Result<(i64, Change)> {
        let mut existing: Option<Recorded> = self
            .conn
            .query_row(
                "SELECT id, title, status, shipped_at, notes, notes_after, heading, heading_depth, after_prose, rank \
                 FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, stage.version],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        Shape {
                            heading: r.get(6)?,
                            depth: r.get(7)?,
                            after_prose: r.get(8)?,
                            rank: r.get(9)?,
                        },
                    ))
                },
            )
            .optional()?;

        // A stage recorded under its own number before rigger learnt to read
        // the number it shipped as (`v1.9` heading, `v1.9.0` release) is the
        // same stage: same title, same numbers. Two rows for one stage
        // inflate every count, so the older spelling goes - whether or not
        // the new one is already there.
        if let Some((twin_id, _)) = self.version_twin(project_id, stage)? {
            if existing.is_some() {
                // Both spellings present: the stage was imported twice, once
                // before this rule existed. Keep the row the record points at.
                self.conn.execute("DELETE FROM versions WHERE id = ?1", [twin_id])?;
            } else {
                // Only the old spelling: rename it, so the tasks and events
                // hanging off it stay attached.
                self.conn
                    .execute("UPDATE versions SET name = ?1 WHERE id = ?2", params![stage.version, twin_id])?;
                existing = Some((twin_id, stage.title.clone(), String::new(), None, None, None, Shape::default()));
            }
        }
        let status = if stage.shipped_on.is_some() { "shipped" } else { "planned" };
        // A reading that does not own the shape keeps the one already
        // recorded, so the plan's copy of a shipped stage cannot move it
        // out of the changelog it was written in.
        let shape = match owns_shape {
            true => Shape::of(stage),
            false => existing.as_ref().map(|(.., recorded)| recorded.clone()).unwrap_or_else(|| Shape::of(stage)),
        };
        // An empty body is no body: a stage written without prose and one
        // whose prose was deleted are the same thing to the record, and
        // storing "" would make an export print a blank line for it.
        let notes = (!stage.notes.trim().is_empty()).then(|| stage.notes.clone());
        let notes_after = (!stage.notes_after.trim().is_empty()).then(|| stage.notes_after.clone());
        match existing {
            Some((id, title, was_status, shipped_at, was_notes, was_after, was_shape)) => {
                // The prose and the shape join the comparison, or a hub whose
                // entry was rewritten would import as "nothing changed" and
                // the record would keep serving the old words to an export.
                // The heading is part of that: it is what an export writes,
                // and it cannot be composed from the other fields.
                if title.as_deref() == stage.title.as_deref()
                    && was_status == status
                    && shipped_at == stage.shipped_on
                    && was_notes == notes
                    && was_after == notes_after
                    && was_shape == shape
                {
                    return Ok((id, Change::Unchanged));
                }
                self.conn.execute(
                    "UPDATE versions SET title = ?1, status = ?2, shipped_at = ?3, notes = ?4, \
                     notes_after = ?5, heading_depth = ?6, heading = ?7, after_prose = ?8, rank = ?9 \
                     WHERE id = ?10",
                    params![
                        stage.title,
                        status,
                        stage.shipped_on,
                        notes,
                        notes_after,
                        shape.depth,
                        shape.heading,
                        shape.after_prose,
                        shape.rank,
                        id
                    ],
                )?;
                Ok((id, Change::Updated))
            }
            None => {
                self.conn.execute(
                    "INSERT INTO versions (project_id, name, title, status, shipped_at, notes, notes_after, heading_depth, heading, after_prose, rank) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        project_id,
                        stage.version,
                        stage.title,
                        status,
                        stage.shipped_on,
                        notes,
                        notes_after,
                        shape.depth,
                        shape.heading,
                        shape.after_prose,
                        shape.rank
                    ],
                )?;
                Ok((self.conn.last_insert_rowid(), Change::Added))
            }
        }
    }

    /// The same stage recorded under a different spelling of its number:
    /// same title, same numbers, different text (`v1.9` against `v1.9.0`).
    fn version_twin(&self, project_id: i64, stage: &crate::hub::Stage) -> Result<Option<(i64, String)>> {
        let Some(title) = stage.title.as_deref() else { return Ok(None) };
        let candidate: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT id, name FROM versions WHERE project_id = ?1 AND title = ?2 AND name <> ?3",
                params![project_id, title, stage.version],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(candidate.filter(|(_, name)| version_order(name) == version_order(&stage.version)))
    }

    /// Records a task of a version. The pair (version, title) identifies it:
    /// the hub has no ids, and the text of a line is what the owner edits
    /// least once a stage is written.
    pub fn upsert_task(&self, project_id: i64, version_id: i64, position: usize, task: &crate::hub::Task) -> Result<Change> {
        let status = if task.done { "done" } else { "open" };
        let position = position as i64;

        // The text first: a line that moved within its stage is the same
        // line, and matching it by text keeps its history when a task is
        // inserted above it.
        let existing: Option<(i64, String, String, Option<i64>)> = self
            .conn
            .query_row(
                "SELECT id, status, title, position FROM tasks WHERE version_id = ?1 AND title = ?2",
                params![version_id, task.title],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            // Then the position: a line that was reworded in place is also
            // the same line. Without this, editing a plan added a second
            // task and left the first open for ever - and a packet went on
            // showing the old wording of work already under way.
            .or(self
                .conn
                .query_row(
                    "SELECT id, status, title, position FROM tasks WHERE version_id = ?1 AND position = ?2",
                    params![version_id, position],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?);

        match existing {
            Some((id, was, title, at)) => {
                if was == status && title == task.title && at == Some(position) {
                    return Ok(Change::Unchanged);
                }
                let closed_at = task.done.then(now);
                self.conn.execute(
                    "UPDATE tasks SET status = ?1, closed_at = ?2, title = ?3, position = ?4 WHERE id = ?5",
                    params![status, closed_at, task.title, position, id],
                )?;
                Ok(Change::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO tasks (project_id, version_id, title, status, created_at, closed_at, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![project_id, version_id, task.title, status, now(), task.done.then(now), position],
                )?;
                Ok(Change::Added)
            }
        }
    }

    /// Records an event unless the same one is already there.
    ///
    /// Events carry no natural key, and the right one depends on where the
    /// event's date comes from. A decision read from the hub carries the date
    /// the owner wrote, so project, kind, date and body identify it, and a
    /// re-import produces the same four. A question or a wish has no date of
    /// its own - the timestamp is the moment it was recorded, which differs
    /// on every run - so the text alone identifies it, and re-importing a hub
    /// does not pile up copies of the same open question.
    pub fn record_event(&self, project_id: i64, kind: &str, body: &str, created_at: &str, author: &str) -> Result<Change> {
        let dated = !matches!(kind, "question" | "wish");
        let seen: Option<i64> = if dated {
            self.conn
                .query_row(
                    "SELECT id FROM events WHERE project_id = ?1 AND kind = ?2 AND created_at = ?3 AND body = ?4",
                    params![project_id, kind, created_at, body],
                    |r| r.get(0),
                )
                .optional()?
        } else {
            self.conn
                .query_row(
                    "SELECT id FROM events WHERE project_id = ?1 AND kind = ?2 AND body = ?3",
                    params![project_id, kind, body],
                    |r| r.get(0),
                )
                .optional()?
        };
        if seen.is_some() {
            return Ok(Change::Unchanged);
        }
        // Written under whatever session is open, which is what makes a
        // session a container rather than a pair of timestamps. Nothing has
        // to be told to do this: the assistant records as it always has, and
        // the boundary is applied by the record.
        let session = self.open_session(project_id)?.map(|s| s.id);
        self.conn.execute(
            "INSERT INTO events (project_id, session_id, kind, body, author, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project_id, session, kind, body, author, created_at],
        )?;
        Ok(Change::Added)
    }

    /// Closes an open question or wish, optionally recording the answer.
    ///
    /// The event is not deleted: the record is a history, and a question
    /// that was asked stays asked. It changes kind, so it leaves the
    /// packet's "waiting for the owner" list, and an answer becomes a
    /// decision in its own right - which is what an answer to a question
    /// about a project actually is.
    pub fn resolve_event(&self, project_id: i64, id: i64, answer: Option<&str>) -> Result<(String, String)> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT kind, body FROM events WHERE id = ?1 AND project_id = ?2",
                params![id, project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((kind, body)) = found else {
            bail!("no open question or wish [{id}] in this project; the packet lists the ids");
        };
        if !matches!(kind.as_str(), "question" | "wish") {
            bail!("[{id}] is a {kind}, not a question or a wish; only those are answered");
        }

        // `answered` and `sorted` keep the kind readable in the record: one
        // says the owner replied, the other that a wish found its place in
        // the plan.
        let resolved = if kind == "question" { "answered" } else { "sorted" };
        self.conn.execute("UPDATE events SET kind = ?1 WHERE id = ?2", params![resolved, id])?;

        if let Some(answer) = answer {
            let text = format!("{body}\n\n{answer}");
            self.record_event(project_id, "decision", &text, &now(), "owner")?;
        }
        Ok((kind, body))
    }

    /// Events matching a query, best first.
    ///
    /// Ranked by FTS5's own relevance, but with what a person wrote lifted
    /// above what a commit message said. Both are searched - "when did we
    /// fix that?" is as real a question as "where did we decide that?" - and
    /// the answer to the second must not arrive under three commits that
    /// happen to share a word.
    pub fn find_events(&self, query: &str, project: Option<&str>, kind: Option<&str>, limit: u32) -> Result<Vec<Found>> {
        // `snippet` returns the text around the match rather than the start
        // of the body. A decision here runs to fifteen hundred characters and
        // states its subject in a heading, so the first line often does not
        // contain the word that was searched for - and a result you cannot
        // see the reason for reads as a wrong result.
        let mut sql = String::from(
            "SELECT p.name, e.kind, e.created_at,
                    snippet(events_fts, 0, '', '', '…', 12), e.author, e.commit_hash
             FROM events_fts f
             JOIN events e ON e.id = f.rowid
             JOIN projects p ON p.id = e.project_id
             WHERE events_fts MATCH ?1",
        );
        if project.is_some() {
            sql.push_str(" AND p.name = ?2");
        }
        if kind.is_some() {
            sql.push_str(if project.is_some() { " AND e.kind = ?3" } else { " AND e.kind = ?2" });
        }
        // A hand-written event outranks a commit line of equal relevance:
        // the commit can be read again in git, the reasoning cannot.
        sql.push_str(" ORDER BY (e.author = 'git'), rank, e.created_at DESC LIMIT ?LIMIT");

        let limit_pos = 2 + usize::from(project.is_some()) + usize::from(kind.is_some());
        let sql = sql.replace("?LIMIT", &format!("?{limit_pos}"));

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(query.to_string())];
        if let Some(project) = project {
            params.push(Box::new(project.to_string()));
        }
        if let Some(kind) = kind {
            params.push(Box::new(kind.to_string()));
        }
        params.push(Box::new(limit));

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |r| {
            let date: String = r.get(2)?;
            let author: String = r.get(4)?;
            Ok(Found {
                project: r.get(0)?,
                kind: r.get(1)?,
                date: date.split('T').next().unwrap_or(&date).to_string(),
                body: r.get(3)?,
                from_git: author == "git",
                commit_hash: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The version by that name, with the release before it.
    ///
    /// "Before it" is by number, not by date: two versions often ship on one
    /// day, and the plan's order is what "the work that led here" means.
    pub fn version_and_predecessor(&self, project_id: i64, name: &str) -> Result<Option<(VersionFacts, Option<VersionFacts>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, title, status, shipped_at, shipped_ts FROM versions WHERE project_id = ?1")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok(VersionFacts {
                name: r.get(0)?,
                title: r.get(1)?,
                status: r.get(2)?,
                shipped_at: r.get(3)?,
                shipped_ts: r.get(4)?,
            })
        })?;
        let mut all: Vec<VersionFacts> = rows.collect::<rusqlite::Result<_>>()?;
        all.sort_by_key(|v| version_order(&v.name));

        let wanted = version_order(name);
        let at = all.iter().position(|v| version_order(&v.name) == wanted);
        let Some(at) = at else { return Ok(None) };

        // The previous *shipped* release, not merely the previous row: a
        // planned version in between never bounded any work.
        let before = all[..at].iter().rev().find(|v| v.shipped_at.is_some()).cloned();
        Ok(Some((all[at].clone(), before)))
    }

    /// Events recorded in a window of time, oldest first.
    ///
    /// The window is how a version and its events are joined: no event
    /// carries a version of its own, and the work that went into a release
    /// is what happened between the release before it and itself.
    pub fn events_between(&self, project_id: i64, after: Option<&str>, until: Option<&str>) -> Result<Vec<Found>> {
        let mut sql = String::from(
            "SELECT p.name, e.kind, e.created_at, e.body, e.author, e.commit_hash
             FROM events e JOIN projects p ON p.id = e.project_id
             WHERE e.project_id = ?1 AND e.kind <> 'next'",
        );
        // Both ends are compared as text, which sorts correctly for RFC 3339.
        // A bound may be a whole timestamp (from a tag) or a bare day (from a
        // hub, which writes dates by hand): a day is padded so that "the work
        // up to and including that day" still means what it says.
        //
        // The lower bound is exclusive and the upper inclusive, and that is
        // not symmetry for its own sake: a tag points *at* a commit, so a
        // release and its last commit share a moment. Inclusive at the top
        // keeps that commit in the release it shipped; exclusive at the
        // bottom keeps it out of the next one.
        if after.is_some() {
            sql.push_str(" AND e.created_at > ?2");
        }
        if until.is_some() {
            sql.push_str(if after.is_some() {
                " AND e.created_at <= ?3"
            } else {
                " AND e.created_at <= ?2"
            });
        }
        sql.push_str(" ORDER BY e.created_at, e.id");

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(project_id)];
        if let Some(after) = after {
            params.push(Box::new(after.to_string()));
        }
        if let Some(until) = until {
            params.push(Box::new(until.to_string()));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |r| {
            let date: String = r.get(2)?;
            let author: String = r.get(4)?;
            Ok(Found {
                project: r.get(0)?,
                kind: r.get(1)?,
                date: date.split('T').next().unwrap_or(&date).to_string(),
                body: r.get(3)?,
                from_git: author == "git",
                commit_hash: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every question waiting for an answer, across all projects.
    ///
    /// Oldest first: a question that has waited three weeks is more overdue
    /// than one asked this morning, and the order should say so.
    pub fn open_questions(&self) -> Result<Vec<Waiting>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.name, e.id, e.created_at, e.body
             FROM events e JOIN projects p ON p.id = e.project_id
             WHERE e.kind = 'question'
             ORDER BY e.created_at, p.name, e.id",
        )?;
        let rows = stmt.query_map([], |r| {
            let date: String = r.get(2)?;
            Ok(Waiting {
                project: r.get(0)?,
                id: r.get(1)?,
                date: date.split('T').next().unwrap_or(&date).to_string(),
                body: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// What happened to one project inside a window: releases, the events
    /// worth a line, and how much of the rest there was.
    pub fn digest(&self, project_id: i64, since: &str) -> Result<DigestFacts> {
        let shipped: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT name FROM versions
                 WHERE project_id = ?1 AND status = 'shipped' AND shipped_at >= ?2",
            )?;
            let rows = stmt.query_map(params![project_id, since], |r| r.get::<_, String>(0))?;
            let mut names: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
            names.sort_by_key(|n| version_order(n));
            names
        };

        // Counted by kind: a digest says how much was decided, not what.
        let mut decisions = 0u32;
        let mut findings = 0u32;
        let mut changes = 0u32;
        let mut stmt = self.conn.prepare(
            "SELECT kind, COUNT(*) FROM events
             WHERE project_id = ?1 AND created_at >= ?2 AND kind <> 'next'
             GROUP BY kind",
        )?;
        let rows = stmt.query_map(params![project_id, since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?;
        for row in rows {
            let (kind, n) = row?;
            match kind.as_str() {
                "decision" => decisions += n,
                "finding" | "pitfall" => findings += n,
                "change" => changes += n,
                _ => {}
            }
        }

        Ok(DigestFacts {
            shipped,
            decisions,
            findings,
            changes,
            waiting: self.count_open_events(project_id, "question")?,
        })
    }

    /// How many events of a kind are open for a project.
    pub fn count_open_events(&self, project_id: i64, kind: &str) -> Result<u32> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE project_id = ?1 AND kind = ?2",
            params![project_id, kind],
            |r| r.get(0),
        )?;
        Ok(n.unsigned_abs() as u32)
    }

    /// Records a change read from a commit, unless that commit is already
    /// recorded.
    ///
    /// The hash is the key, not the text: `sync` reads the same history on
    /// every run, and a message amended between runs would otherwise arrive
    /// as a second event about one change. A unique index enforces it, so
    /// two syncs racing cannot both insert.
    pub fn record_commit_event(&self, project_id: i64, hash: &str, body: &str, created_at: &str) -> Result<Change> {
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO events (project_id, kind, body, author, created_at, commit_hash)
             VALUES (?1, 'change', ?2, 'git', ?3, ?4)",
            params![project_id, body, created_at, hash],
        )?;
        if changed > 0 {
            return Ok(Change::Added);
        }

        // Already recorded - but perhaps by a rigger that kept only the day,
        // which placed every change of a day at midnight, before any tag
        // made that day. `why` bounds a version's work by tag moments, so a
        // midnight stamp files a change under the wrong release. Correcting
        // it is not a rewrite of history: it is the same commit, dated by
        // itself rather than by what the reader could store at the time.
        self.conn.execute(
            "UPDATE events SET created_at = ?1
             WHERE project_id = ?2 AND commit_hash = ?3 AND created_at <> ?1
               AND substr(created_at, 1, 10) = substr(?1, 1, 10)",
            params![created_at, project_id, hash],
        )?;
        Ok(Change::Unchanged)
    }

    /// The newest version the record says shipped, with its date.
    ///
    /// Ordered by the version number, not by the row: a hub lists its
    /// changelog newest-first, so the highest row id belongs to the *oldest*
    /// entry, and several versions often share one shipping date. Sorting by
    /// date and id answered "v0.1.0" for a project that had reached v0.10.0.
    pub fn last_shipped_version(&self, project_id: i64) -> Result<Option<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, shipped_at FROM versions WHERE project_id = ?1 AND shipped_at IS NOT NULL")?;
        let rows = stmt.query_map([project_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut shipped: Vec<(String, String)> = rows.collect::<rusqlite::Result<_>>()?;
        // The date breaks ties the other way round: a version numbered lower
        // but shipped later is genuinely the more recent release.
        shipped.sort_by(|a, b| version_order(&a.0).cmp(&version_order(&b.0)).then(a.1.cmp(&b.1)));
        Ok(shipped.pop())
    }

    /// The stage being built: the oldest planned version, with its open tasks.
    ///
    /// Oldest rather than newest, because a plan is a queue - the next stage
    /// is the one that has waited longest, not the one written last.
    pub fn current_stage(&self, project_id: i64) -> Result<Option<CurrentStage>> {
        // The lowest version number among those still planned - by number,
        // not by row: a plan lists its stages in whatever order the owner
        // wrote them, and a hub's changelog runs newest-first.
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, title FROM versions WHERE project_id = ?1 AND status = 'planned'")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
        })?;
        let mut planned: Vec<(i64, String, Option<String>)> = rows.collect::<rusqlite::Result<_>>()?;
        planned.sort_by_key(|(_, name, _)| version_order(name));
        let Some((id, name, title)) = planned.into_iter().next() else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare("SELECT id, title FROM tasks WHERE version_id = ?1 AND status = 'open' ORDER BY id")?;
        let tasks = stmt
            .query_map([id], |r| {
                Ok(Task {
                    id: r.get(0)?,
                    title: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<Task>>>()?;
        Ok(Some(CurrentStage { version: name, title, tasks }))
    }

    /// Marks a task done. The project is part of the lookup so that an id
    /// from another project's plan is refused rather than silently closing
    /// someone else's line.
    pub fn close_task(&self, project_id: i64, task_id: i64) -> Result<(String, Change)> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT title, status FROM tasks WHERE id = ?1 AND project_id = ?2",
                params![task_id, project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((title, status)) = found else {
            bail!("no task {task_id} in this project; the plan lists the ids");
        };
        if status == "done" {
            return Ok((title, Change::Unchanged));
        }
        self.conn
            .execute("UPDATE tasks SET status = 'done', closed_at = ?1 WHERE id = ?2", params![now(), task_id])?;
        Ok((title, Change::Updated))
    }

    /// Records that a version shipped, because a tag proves it.
    ///
    /// A tag outranks the plan: the status is set whatever the plan said,
    /// and the date comes from the tag's commit rather than from prose.
    /// A version the plan never mentioned is added - the release happened
    /// whether or not anyone wrote it down - and reported as `Added` so the
    /// caller can say so.
    pub fn mark_shipped(&self, project_id: i64, version: &str, date: &str, moment: &str) -> Result<Change> {
        // Matched by number, not by text: hubs spell one version several
        // ways (`v1.9` for the stage, `v1.9.0` for the release), and a tag
        // must land on the row that already exists rather than beside it.
        let existing: Option<VersionRow> = self
            .conn
            .query_row(
                "SELECT id, status, shipped_at, shipped_ts, shipped_source FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, version],
                |r| {
                    Ok(VersionRow {
                        id: r.get(0)?,
                        status: r.get(1)?,
                        shipped_at: r.get(2)?,
                        shipped_ts: r.get(3)?,
                        source: r.get(4)?,
                    })
                },
            )
            .optional()?
            .or(self.version_by_number(project_id, version)?);

        match existing {
            Some(VersionRow {
                id,
                status,
                shipped_at,
                shipped_ts,
                source,
            }) => {
                // The moment is part of "unchanged": a version recorded
                // before rigger kept the moment has the right day and no
                // way to be told apart from its same-day neighbours.
                if status == "shipped" && shipped_at.as_deref() == Some(date) && shipped_ts.as_deref() == Some(moment) && source.as_deref() == Some("tag") {
                    return Ok(Change::Unchanged);
                }
                self.conn.execute(
                    "UPDATE versions SET status = 'shipped', shipped_at = ?1, shipped_ts = ?2, shipped_source = 'tag' WHERE id = ?3",
                    params![date, moment, id],
                )?;
                Ok(Change::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO versions (project_id, name, status, shipped_at, shipped_ts, shipped_source) VALUES (?1, ?2, 'shipped', ?3, ?4, 'tag')",
                    params![project_id, version, date, moment],
                )?;
                Ok(Change::Added)
            }
        }
    }

    /// A version of this project whose number matches, however it is spelt.
    fn version_by_number(&self, project_id: i64, version: &str) -> Result<Option<VersionRow>> {
        let wanted = version_order(version);
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, status, shipped_at, shipped_ts, shipped_source FROM versions WHERE project_id = ?1")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok((
                r.get::<_, String>(1)?,
                VersionRow {
                    id: r.get(0)?,
                    status: r.get(2)?,
                    shipped_at: r.get(3)?,
                    shipped_ts: r.get(4)?,
                    source: r.get(5)?,
                },
            ))
        })?;
        for row in rows {
            let (name, version_row) = row?;
            if version_order(&name) == wanted {
                return Ok(Some(version_row));
            }
        }
        Ok(None)
    }

    /// Records where a project sits in the rotation and how fast it is
    /// meant to release. A tier without a rhythm takes the tier's own.
    pub fn set_tier(&self, project_id: i64, tier: &str, rhythm_weeks: Option<u32>) -> Result<()> {
        self.conn.execute(
            "UPDATE projects SET tier = ?2, rhythm_weeks = ?3 WHERE id = ?1",
            params![project_id, tier, rhythm_weeks],
        )?;
        Ok(())
    }

    /// Aims a version at a week, or clears the aim when given `None`.
    ///
    /// Only a version the record already holds can be planned: a typo would
    /// otherwise create a row that no plan, changelog or tag knows about,
    /// and it would sit in the calendar for ever.
    pub fn set_planned_week(&self, project_id: i64, version: &str, week: Option<&str>) -> Result<Change> {
        let existing: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT planned_week FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, version],
                |r| r.get(0),
            )
            .optional()?;
        let Some(current) = existing else {
            bail!("no version '{version}' in the record; see `rigger project show`");
        };
        if current.as_deref() == week {
            return Ok(Change::Unchanged);
        }
        self.conn.execute(
            "UPDATE versions SET planned_week = ?3 WHERE project_id = ?1 AND name = ?2",
            params![project_id, version, week],
        )?;
        Ok(Change::Updated)
    }

    /// Every version of a project that has a place in the calendar: aimed
    /// at a week, or shipped, or both. A version that is neither is in the
    /// plan and not yet on the calendar, which is a different screen.
    pub fn calendar_versions(&self, project_id: i64, project: &str) -> Result<Vec<crate::calendar::Planned>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, title, planned_week, shipped_at
             FROM versions
             WHERE project_id = ?1 AND (planned_week IS NOT NULL OR shipped_at IS NOT NULL)",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            let name: String = r.get(0)?;
            let title: Option<String> = r.get(1)?;
            let planned: Option<String> = r.get(2)?;
            let shipped_at: Option<String> = r.get(3)?;
            Ok((name, title, planned, shipped_at))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (version, title, planned, shipped_at) = row?;
            out.push(crate::calendar::Planned {
                project: project.to_string(),
                version,
                title,
                // A week that cannot be read is dropped rather than
                // refused: it came from an import or an older rigger, and
                // one bad string should not empty the calendar.
                planned: planned.as_deref().and_then(|w| crate::calendar::Week::parse(w).ok()),
                shipped: shipped_at.as_deref().and_then(crate::calendar::Week::of_recorded),
                shipped_at,
            });
        }
        out.sort_by_key(|a| version_order(&a.version));
        Ok(out)
    }

    /// Every version name the record holds for a project.
    pub fn version_names(&self, project_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM versions WHERE project_id = ?1")?;
        let rows = stmt.query_map([project_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every version the record claims shipped, newest first.
    pub fn shipped_versions(&self, project_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM versions WHERE project_id = ?1 AND status = 'shipped'")?;
        let rows = stmt.query_map([project_id], |r| r.get::<_, String>(0))?;
        let mut names: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
        names.sort_by_key(|n| std::cmp::Reverse(version_order(n)));
        Ok(names)
    }

    /// Versions the record calls shipped that no tag has confirmed.
    ///
    /// The date cannot answer this on its own: an imported hub carries dates
    /// written by hand, and they look exactly like a tag's. Only the
    /// provenance `sync` stamps separates a proven release from a claimed
    /// one. Read from the record rather than by walking history - `doctor`
    /// reports what the last `sync` found, and does not go looking itself.
    pub fn shipped_without_a_tag(&self, project_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM versions
             WHERE project_id = ?1 AND status = 'shipped' AND (shipped_source IS NULL OR shipped_source <> 'tag')",
        )?;
        let rows = stmt.query_map([project_id], |r| r.get::<_, String>(0))?;
        let mut names: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
        names.sort_by_key(|n| std::cmp::Reverse(version_order(n)));
        Ok(names)
    }

    /// What git said about the project's activity when it was last read.
    pub fn record_activity(&self, project_id: i64, commits: u32, last_commit_at: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE projects SET commits_since_tag = ?1, last_commit_at = ?2, synced_at = ?3 WHERE id = ?4",
            params![commits, last_commit_at, now(), project_id],
        )?;
        Ok(())
    }

    /// The activity recorded by the last sync, if there was one.
    pub fn activity(&self, project_id: i64) -> Result<Option<Activity>> {
        Ok(self
            .conn
            .query_row(
                "SELECT commits_since_tag, last_commit_at, synced_at FROM projects WHERE id = ?1 AND synced_at IS NOT NULL",
                [project_id],
                |r| {
                    Ok(Activity {
                        commits_since_tag: r.get(0)?,
                        last_commit_at: r.get(1)?,
                        synced_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn count_versions(&self, project_id: i64, status: &str) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM versions WHERE project_id = ?1 AND status = ?2",
            params![project_id, status],
            |r| r.get(0),
        )?;
        Ok(n.unsigned_abs())
    }

    pub fn count_open_tasks(&self, project_id: i64) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status = 'open'", [project_id], |r| {
                r.get(0)
            })?;
        Ok(n.unsigned_abs())
    }

    /// Events of a kind that are still open - questions and wishes are
    /// answered by being resolved, which a later release will do.
    pub fn open_events(&self, project_id: i64, kind: &str) -> Result<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, body FROM events WHERE project_id = ?1 AND kind = ?2 ORDER BY created_at, id")?;
        let rows = stmt.query_map(params![project_id, kind], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn latest_event_body(&self, project_id: i64, kind: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT body FROM events WHERE project_id = ?1 AND kind = ?2 ORDER BY created_at DESC, id DESC LIMIT 1",
                params![project_id, kind],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The most recent events worth reading at the start of a session:
    /// what was decided, found, tripped over or changed. Questions and
    /// wishes have their own sections, and the next step its own line.
    pub fn recent_events(&self, project_id: i64, limit: u32) -> Result<Vec<RecentEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, created_at, body, author FROM events
             WHERE project_id = ?1 AND kind NOT IN ('question', 'wish', 'next')
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![project_id, limit], |r| {
            let date: String = r.get(1)?;
            let author: String = r.get(3)?;
            Ok(RecentEvent {
                kind: r.get(0)?,
                // Timestamps are stored whole; a packet only needs the day.
                date: date.split('T').next().unwrap_or(&date).to_string(),
                body: r.get(2)?,
                // A change read from a commit can be read again in git; one
                // written by hand exists nowhere else. The packet needs to
                // tell them apart to decide what to drop first.
                from_git: author == "git",
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The session currently open on a project, if there is one.
    pub fn open_session(&self, project_id: i64) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, project_id, started_at, ended_at FROM sessions                  WHERE project_id = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
                [project_id],
                row_to_session,
            )
            .optional()?)
    }

    /// The most recently ended session, which is what "since last time"
    /// means to the packet.
    pub fn last_ended_session(&self, project_id: i64) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, project_id, started_at, ended_at FROM sessions                  WHERE project_id = ?1 AND ended_at IS NOT NULL ORDER BY ended_at DESC, id DESC LIMIT 1",
                [project_id],
                row_to_session,
            )
            .optional()?)
    }

    /// Opens a session, or returns the one already open.
    ///
    /// Starting twice is not an error: an assistant that lost its place, or
    /// a hook that fired again, should join the sitting rather than split
    /// it in two and orphan half its events.
    pub fn start_session(&self, project_id: i64, at: &str) -> Result<(Session, Change)> {
        if let Some(open) = self.open_session(project_id)? {
            return Ok((open, Change::Unchanged));
        }
        self.conn
            .execute("INSERT INTO sessions (project_id, started_at) VALUES (?1, ?2)", params![project_id, at])?;
        let id = self.conn.last_insert_rowid();
        Ok((
            Session {
                id,
                project_id,
                started_at: at.to_string(),
                ended_at: None,
            },
            Change::Added,
        ))
    }

    /// Closes the open session.
    pub fn end_session(&self, session_id: i64, at: &str) -> Result<()> {
        self.conn
            .execute("UPDATE sessions SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL", params![session_id, at])?;
        Ok(())
    }

    /// The events written while one session was open, oldest first.
    pub fn session_events(&self, session_id: i64) -> Result<Vec<RecentEvent>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, substr(created_at, 1, 10), body, commit_hash IS NOT NULL              FROM events WHERE session_id = ?1 ORDER BY id")?;
        let rows = stmt.query_map([session_id], |r| {
            Ok(RecentEvent {
                kind: r.get(0)?,
                date: r.get(1)?,
                body: r.get(2)?,
                from_git: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Versions whose tag landed between two moments.
    pub fn shipped_between(&self, project_id: i64, after: &str, until: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM versions              WHERE project_id = ?1 AND shipped_ts IS NOT NULL AND shipped_ts > ?2 AND shipped_ts <= ?3")?;
        let rows = stmt.query_map(params![project_id, after, until], |r| r.get::<_, String>(0))?;
        let mut names: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
        names.sort_by_key(|n| version_order(n));
        Ok(names)
    }

    /// Tasks closed between two moments.
    pub fn tasks_closed_between(&self, project_id: i64, after: &str, until: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT title FROM tasks              WHERE project_id = ?1 AND closed_at IS NOT NULL AND closed_at > ?2 AND closed_at <= ?3 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![project_id, after, until], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Events recorded after a moment, for the packet's "since last time".
    ///
    /// The boundary includes its own second. Timestamps are kept to the
    /// second, so an event written in the same second as the session closed
    /// is not strictly after it - and with a strict `>` the first thing
    /// recorded after a sitting could vanish from "since last time", which
    /// is precisely the thing the line exists to report. A session's own
    /// events are excluded by their `session_id`, not by the clock, so
    /// including the boundary cannot pull them back in.
    pub fn events_since(&self, project_id: i64, after: &str) -> Result<Vec<RecentEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, substr(created_at, 1, 10), body, commit_hash IS NOT NULL              FROM events WHERE project_id = ?1 AND created_at >= ?2 AND session_id IS NULL              AND kind NOT IN ('wish', 'next') ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![project_id, after], |r| {
            Ok(RecentEvent {
                kind: r.get(0)?,
                date: r.get(1)?,
                body: r.get(2)?,
                from_git: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// When anything was last recorded about a project, of any kind.
    pub fn last_event_at(&self, project_id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT MAX(created_at) FROM events WHERE project_id = ?1", [project_id], |r| r.get(0))
            .optional()?
            .flatten())
    }

    /// How many events the packet's "recent" section could draw on, so that
    /// it can say how many it left out rather than quietly ending its list.
    pub fn count_recent_events(&self, project_id: i64) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE project_id = ?1 AND kind NOT IN ('question', 'wish', 'next')",
            [project_id],
            |r| r.get(0),
        )?;
        Ok(n.unsigned_abs())
    }

    pub fn counts(&self) -> Result<Counts> {
        // SQLite counts are signed integers; a count is never negative.
        let count = |table: &str| -> Result<u64> {
            let n: i64 = self.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            Ok(n.unsigned_abs())
        };
        Ok(Counts {
            projects: count("projects")?,
            versions: count("versions")?,
            tasks: count("tasks")?,
            sessions: count("sessions")?,
            events: count("events")?,
        })
    }
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
    })
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        remote: row.get(3)?,
        created_at: row.get(4)?,
        tier: row.get(5)?,
        rhythm_weeks: row.get(6)?,
        kind: row.get::<_, String>(7)?.into(),
    })
}

/// A version number as numbers, so that v0.10.0 sorts above v0.9.0.
///
/// Hubs write versions in more shapes than semver allows: `v1.9` alongside
/// `v1.9.0`, and `v0.19.0+` for a stage that shipped as two releases. Missing
/// parts count as zero, and anything after the digits is ignored - it never
/// distinguishes two versions of one project.
pub fn version_order(name: &str) -> (u32, u32, u32) {
    let digits = name.trim_start_matches(['v', 'V']);
    let mut parts = digits
        .split('.')
        .map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// Timestamps are stored as UTC in RFC 3339, which sorts as text.
pub fn now() -> String {
    jiff::Timestamp::now()
        .round(jiff::Unit::Second)
        .map(|t| t.to_string())
        .unwrap_or_else(|_| jiff::Timestamp::now().to_string())
}

#[cfg(test)]
mod tests {
    use super::version_order;

    #[test]
    fn ten_sorts_above_nine() {
        // The defect this ordering exists for: a project that had reached
        // v0.10.0 reported v0.1.0 as its latest release.
        assert!(version_order("v0.10.0") > version_order("v0.9.0"));
        assert!(version_order("v0.20.1") > version_order("v0.19.0"));
    }

    #[test]
    fn a_two_part_number_is_a_version_too() {
        // kasl writes `v1.9 · Title — shipped, released **v1.9.0**`.
        assert_eq!(version_order("v1.9"), (1, 9, 0));
        assert!(version_order("v1.10") > version_order("v1.9"));
    }

    #[test]
    fn a_suffix_after_the_digits_is_ignored() {
        // dowel writes `v0.19.0+` for a stage that shipped as two releases.
        assert_eq!(version_order("v0.19.0+"), (0, 19, 0));
    }

    #[test]
    fn an_unparseable_name_sorts_lowest_rather_than_panicking() {
        assert_eq!(version_order("draft"), (0, 0, 0));
    }
}
