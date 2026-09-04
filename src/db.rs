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

/// An event as the packet reads it back.
pub struct RecentEvent {
    pub kind: String,
    pub date: String,
    pub body: String,
    /// Read from a commit message rather than written by a person.
    pub from_git: bool,
}

/// A version row as `mark_shipped` needs to see it: what the record
/// currently holds, before a tag overrules it.
struct VersionRow {
    id: i64,
    status: String,
    shipped_at: Option<String>,
    source: Option<String>,
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

    pub fn add_project(&self, name: &str, path: &str, remote: Option<&str>) -> Result<Project> {
        if let Some(existing) = self.project_by_path(path)? {
            bail!("{} is already recorded as project '{}'", path, existing.name);
        }
        if self.project_by_name(name)?.is_some() {
            bail!("a project named '{name}' already exists; pick another with --name");
        }
        let created_at = now();
        self.conn.execute(
            "INSERT INTO projects (name, path, remote, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, path, remote, created_at],
        )?;
        Ok(Project {
            id: self.conn.last_insert_rowid(),
            name: name.to_string(),
            path: path.to_string(),
            remote: remote.map(str::to_string),
            created_at,
        })
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare("SELECT id, name, path, remote, created_at FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], row_to_project)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn project_by_name(&self, name: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, path, remote, created_at FROM projects WHERE name = ?1",
                [name],
                row_to_project,
            )
            .optional()?)
    }

    pub fn project_by_path(&self, path: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, path, remote, created_at FROM projects WHERE path = ?1",
                [path],
                row_to_project,
            )
            .optional()?)
    }

    /// Records a stage, or updates the one already recorded under that
    /// version. Returns its id and whether anything changed - the import
    /// report counts on the difference, and a second import of an unchanged
    /// hub must report nothing.
    pub fn upsert_version(&self, project_id: i64, stage: &crate::hub::Stage) -> Result<(i64, Change)> {
        let mut existing: Option<(i64, Option<String>, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT id, title, status, shipped_at FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, stage.version],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
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
                existing = Some((twin_id, stage.title.clone(), String::new(), None));
            }
        }
        let status = if stage.shipped_on.is_some() { "shipped" } else { "planned" };
        match existing {
            Some((id, title, was_status, shipped_at)) => {
                if title.as_deref() == stage.title.as_deref() && was_status == status && shipped_at == stage.shipped_on {
                    return Ok((id, Change::Unchanged));
                }
                self.conn.execute(
                    "UPDATE versions SET title = ?1, status = ?2, shipped_at = ?3 WHERE id = ?4",
                    params![stage.title, status, stage.shipped_on, id],
                )?;
                Ok((id, Change::Updated))
            }
            None => {
                self.conn.execute(
                    "INSERT INTO versions (project_id, name, title, status, shipped_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![project_id, stage.version, stage.title, status, stage.shipped_on],
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
        self.conn.execute(
            "INSERT INTO events (project_id, kind, body, author, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_id, kind, body, author, created_at],
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
        Ok(if changed > 0 { Change::Added } else { Change::Unchanged })
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
    pub fn mark_shipped(&self, project_id: i64, version: &str, date: &str) -> Result<Change> {
        // Matched by number, not by text: hubs spell one version several
        // ways (`v1.9` for the stage, `v1.9.0` for the release), and a tag
        // must land on the row that already exists rather than beside it.
        let existing: Option<VersionRow> = self
            .conn
            .query_row(
                "SELECT id, status, shipped_at, shipped_source FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, version],
                |r| {
                    Ok(VersionRow {
                        id: r.get(0)?,
                        status: r.get(1)?,
                        shipped_at: r.get(2)?,
                        source: r.get(3)?,
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
                source,
            }) => {
                if status == "shipped" && shipped_at.as_deref() == Some(date) && source.as_deref() == Some("tag") {
                    return Ok(Change::Unchanged);
                }
                self.conn.execute(
                    "UPDATE versions SET status = 'shipped', shipped_at = ?1, shipped_source = 'tag' WHERE id = ?2",
                    params![date, id],
                )?;
                Ok(Change::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO versions (project_id, name, status, shipped_at, shipped_source) VALUES (?1, ?2, 'shipped', ?3, 'tag')",
                    params![project_id, version, date],
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
            .prepare("SELECT id, name, status, shipped_at, shipped_source FROM versions WHERE project_id = ?1")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok((
                r.get::<_, String>(1)?,
                VersionRow {
                    id: r.get(0)?,
                    status: r.get(2)?,
                    shipped_at: r.get(3)?,
                    source: r.get(4)?,
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

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        remote: row.get(3)?,
        created_at: row.get(4)?,
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
