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
    pub tasks: Vec<String>,
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
        let existing: Option<(i64, Option<String>, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT id, title, status, shipped_at FROM versions WHERE project_id = ?1 AND name = ?2",
                params![project_id, stage.version],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
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

    /// Records a task of a version. The pair (version, title) identifies it:
    /// the hub has no ids, and the text of a line is what the owner edits
    /// least once a stage is written.
    pub fn upsert_task(&self, project_id: i64, version_id: i64, task: &crate::hub::Task) -> Result<Change> {
        let status = if task.done { "done" } else { "open" };
        let existing: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT id, status FROM tasks WHERE version_id = ?1 AND title = ?2",
                params![version_id, task.title],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match existing {
            Some((_, was)) if was == status => Ok(Change::Unchanged),
            Some((id, _)) => {
                let closed_at = task.done.then(now);
                self.conn
                    .execute("UPDATE tasks SET status = ?1, closed_at = ?2 WHERE id = ?3", params![status, closed_at, id])?;
                Ok(Change::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO tasks (project_id, version_id, title, status, created_at, closed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![project_id, version_id, task.title, status, now(), task.done.then(now)],
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

    /// The newest version the record says shipped, with its date.
    pub fn last_shipped_version(&self, project_id: i64) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, shipped_at FROM versions WHERE project_id = ?1 AND shipped_at IS NOT NULL ORDER BY shipped_at DESC, id DESC LIMIT 1",
                [project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// The stage being built: the oldest planned version, with its open tasks.
    ///
    /// Oldest rather than newest, because a plan is a queue - the next stage
    /// is the one that has waited longest, not the one written last.
    pub fn current_stage(&self, project_id: i64) -> Result<Option<CurrentStage>> {
        let stage: Option<(i64, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT id, name, title FROM versions WHERE project_id = ?1 AND status = 'planned' ORDER BY id LIMIT 1",
                [project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((id, name, title)) = stage else { return Ok(None) };
        let mut stmt = self
            .conn
            .prepare("SELECT title FROM tasks WHERE version_id = ?1 AND status = 'open' ORDER BY id")?;
        let tasks = stmt.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(Some(CurrentStage { version: name, title, tasks }))
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
    pub fn recent_events(&self, project_id: i64, limit: u32) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, created_at, body FROM events
             WHERE project_id = ?1 AND kind NOT IN ('question', 'wish', 'next')
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![project_id, limit], |r| {
            let date: String = r.get(1)?;
            // Timestamps are stored whole; a packet only needs the day.
            Ok((r.get(0)?, date.split('T').next().unwrap_or(&date).to_string(), r.get(2)?))
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

/// Timestamps are stored as UTC in RFC 3339, which sorts as text.
pub fn now() -> String {
    jiff::Timestamp::now()
        .round(jiff::Unit::Second)
        .map(|t| t.to_string())
        .unwrap_or_else(|_| jiff::Timestamp::now().to_string())
}
