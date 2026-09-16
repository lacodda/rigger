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
    // How many blank lines a diary left after the rule between two entries.
    // Not a constant and not a habit of a hub: one diary writes one after
    // most of its rules and two after four of them, and two others write
    // none, so writing one for everybody moved every line below the first
    // entry that disagreed.
    "
    ALTER TABLE sessions ADD COLUMN gap_after INTEGER;
    ",
    // The same for a run of prose: a hub separates its blocks with a rule,
    // and the blank lines around it are the hub's own. Most write one, one
    // writes two, and two put the next heading straight under the rule.
    "
    ALTER TABLE hub_prose ADD COLUMN gap_after INTEGER;
    ",
    // And the same for a stage: the rule that separates two of them, and
    // the blank lines after it, are written by the hub and not by a rule
    // of the export's own.
    "
    ALTER TABLE versions ADD COLUMN gap_after INTEGER;
    ",
    // Where a diary entry stood among the entries of its file. A date is
    // not an order: several sittings share a day, and ordering by date
    // alone left them to whatever order the rows came out of the table.
    "
    ALTER TABLE sessions ADD COLUMN rank INTEGER;
    ",
    // Where a project's hub is. `doctor --hubs` used to guess it beside the
    // repository, and the hubs of this line live in a notes vault instead -
    // so the check found no files to compare and reported that every
    // generated file matched the record, on a hub it had never read.
    // Recorded when a hub is imported or exported, because that is when the
    // directory is actually in hand.
    "
    ALTER TABLE projects ADD COLUMN hub_path TEXT;
    ",
    // The "Состояние" block a hub keeps in its README: one dated line per
    // thing worth telling, newest first. Not a field on a version, because
    // 32 of the 310 lines across this line's hubs name no version at all -
    // a project founded, a plan reground, a stand brought up. The date
    // carries the hub's own mark beside it ("2026-09-05 (ночь)"), which is
    // how a hub tells apart three sittings in one day.
    "
    CREATE TABLE state_lines (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        position   INTEGER NOT NULL,
        stamp      TEXT,
        body       TEXT NOT NULL,
        gap_after  INTEGER NOT NULL DEFAULT 0,
        UNIQUE (project_id, position)
    );
    ",
    // v16: the day a session's diary entry is dated. A sitting read from a
    // hub started at that day's midnight, but a sitting the record opened
    // itself started when it started - and may end days later, which is
    // the day its entry belongs to. Without a day of its own, the entry a
    // session leaves could not be told apart from the same entry read
    // back from the file.
    "
    ALTER TABLE sessions ADD COLUMN day TEXT;
    UPDATE sessions SET day = substr(started_at, 1, 10) WHERE notes IS NOT NULL;
    ",
    // v17: a task's status is a word from a vocabulary rather than one of
    // two. `open` becomes `new`; `active`, `waiting-handoff` and `frozen`
    // join `done` and `dropped`. The ticket profile lives on these: a
    // ticket handed to another team is neither open nor done.
    "
    UPDATE tasks SET status = 'new' WHERE status = 'open';
    ",
    // v18: a task can be a card - the unit of work at a desk. A card has a
    // key (the ticket id, or a local one until a tracker gives it one),
    // aliases (the ids it went by before it moved), a summary, and links
    // to the repositories it is worked in, each with a branch. Events
    // already carried a task id; now they have something to point at.
    // `settings` holds what the desk needs to remember between commands:
    // which card is open.
    "
    ALTER TABLE tasks ADD COLUMN key TEXT;
    ALTER TABLE tasks ADD COLUMN aliases TEXT;
    ALTER TABLE tasks ADD COLUMN summary TEXT;
    ALTER TABLE tasks ADD COLUMN updated_at TEXT;
    CREATE INDEX tasks_by_key ON tasks (key);
    CREATE TABLE task_projects (
        task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        branch     TEXT,
        role       TEXT,
        PRIMARY KEY (task_id, project_id)
    );
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    ",
    // v19: a project's handwritten texts, which until now lived only as
    // files in a hub: the vision, the prose of the decisions journal, the
    // rituals only that project can state, and its research notes. The
    // record is the truth for everything else already; a text that lives
    // only in a file is a text the record cannot show, search or carry to
    // another machine.
    //
    // A document is addressed by project and slug, so `doc show rigger
    // vision` is stable while the title is free to change. `kind` groups
    // them - a project has one vision and many research notes - and the
    // unique index is on the slug rather than the kind for that reason.
    "
    CREATE TABLE documents (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        kind       TEXT NOT NULL,
        slug       TEXT NOT NULL,
        title      TEXT NOT NULL,
        body       TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        UNIQUE (project_id, slug)
    );
    CREATE INDEX documents_by_kind ON documents (project_id, kind);
    ",
    // v20: the file a document was read from, so an export writes it back
    // to the same place. Without it the filename was composed from the
    // title, and a research note whose title is not its filename came back
    // as a second file beside the first - found on this project's own hub,
    // where one note became two on the first `export --docs`.
    "ALTER TABLE documents ADD COLUMN source_file TEXT;",
    // v21: the command that says a project is fit to commit. Until now it
    // lived in seventeen skill files, one copy each, and drifted from the
    // workflow it was supposed to mirror. A project states it once, the
    // packet prints it, and `rigger gate` runs it - so "the gate is green"
    // stops being something an assistant reports and becomes something the
    // record witnessed.
    "ALTER TABLE projects ADD COLUMN gate TEXT;",
];

/// The place a desk keeps its cards: a project the record keeps for
/// itself, made on first use.
pub const DESK: &str = "desk";

/// The kinds a document can be.
///
/// `vision` and `rituals` are one per project and `decisions` is the prose
/// preamble of its journal - the entries themselves are events, not a
/// document. `research` is many, and `other` is the escape hatch for a text
/// that is none of these rather than a reason to invent a kind.
pub const DOC_KINDS: [&str; 5] = ["vision", "decisions", "research", "rituals", "other"];

/// The kinds a project has at most one of, so that `doc add` can say so
/// rather than quietly making a second vision nobody reads.
pub const SINGULAR_DOC_KINDS: [&str; 3] = ["vision", "decisions", "rituals"];

/// The kind of document `rules` reads.
pub const RULES_KIND: &str = "rituals";

/// The words a task's status can be. Everything before `done` is work
/// still to do; `dropped` is a line struck from a hub.
pub const TASK_STATUSES: [&str; 6] = ["new", "active", "waiting-handoff", "frozen", "done", "dropped"];

/// Whether a status counts as work still to do.
pub fn is_open_status(status: &str) -> bool {
    !matches!(status, "done" | "dropped")
}

/// Columns that migration 7 gained after a database had already recorded it
/// as applied.
///
/// An applied migration is frozen: editing one is invisible to every
/// database that has passed it, and the record then runs against a schema
/// it believes it has. This one was edited during the stage that built the
/// export, and the owner's database came out of it without `after_prose` -
/// every query naming that column failed on a live run, though a database
/// created from scratch was fine.
///
/// Applied on open rather than as a migration of its own, because the
/// databases that need it and the ones that do not are both at the same
/// schema version, and only the table can say which is which.
const REPAIRS: [(&str, &str, &str); 1] = [("versions", "after_prose", "ALTER TABLE versions ADD COLUMN after_prose INTEGER")];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub struct Db {
    conn: Connection,
    path: PathBuf,
}

/// A handwritten text of a project: its vision, its rituals, a research
/// note. What a hub kept as a file, the record keeps as a row.
#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: i64,
    pub project_id: i64,
    pub kind: String,
    /// What addresses it: `vision`, `rituals`, `2026-09-04-the-tool`.
    pub slug: String,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    /// The file in a hub this was read from, when it came from one, so an
    /// export can write it back to the same name rather than composing one.
    pub source_file: Option<String>,
}

impl Document {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project_id: row.get(1)?,
            kind: row.get(2)?,
            slug: row.get(3)?,
            title: row.get(4)?,
            body: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            source_file: row.get(8)?,
        })
    }
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
    /// Where its hub is, once a hub has been imported or exported. Not
    /// guessed: the hubs of this line live in a notes vault, nowhere near
    /// the repositories they describe.
    pub hub_path: Option<String>,
    /// The command that says this project is fit to commit, as the project
    /// states it: what CI runs, spelt for a shell.
    pub gate: Option<String>,
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
    /// One of `TASK_STATUSES`; `new` is the one a plan's box means.
    pub status: String,
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
    gap_after: Option<i64>,
    rank: Option<i64>,
}

/// A diary heading without the date it opens with, for telling one entry
/// from another when the same entry was read with and without its date.
fn heading_sans_date(heading: Option<&str>) -> String {
    let text = heading.unwrap_or("").trim();
    let date_len = text.chars().take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.').count();
    let rest = if date_len >= 8 { &text[date_len..] } else { text };
    rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '·' | '—' | '-' | ':' | '–'))
        .to_string()
}

/// What recording a stage did, and whether the stage stands shipped now -
/// the import needs the last to know whether the plan's boxes still speak
/// for its tasks.
pub struct Upserted {
    pub id: i64,
    pub change: Change,
    pub shipped: bool,
}

/// A stage as the record already holds it: its id, title, status, the
/// date it shipped, the prose written before and after its list, and the
/// shape its heading had. Named because an upsert compares all of it at
/// once.
type Recorded = (i64, Option<String>, String, Option<String>, Option<String>, Option<String>, Shape);

/// A diary entry as the record already holds it: its id, its heading, what
/// was written under it, whether a rule followed it, how many blank lines
/// came after that rule, and its rank. Named because an upsert compares
/// all of it at once.
type RecordedEntry = (i64, Option<String>, Option<String>, Option<bool>, Option<i64>, Option<i64>);

impl Shape {
    fn of(stage: &crate::hub::Stage) -> Shape {
        Shape {
            heading: (!stage.heading.trim().is_empty()).then(|| stage.heading.clone()),
            depth: Some(stage.depth as i64),
            after_prose: Some(stage.after_prose as i64),
            gap_after: Some(stage.gap_after as i64),
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
        self.repair()?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Copies the database beside itself, stamped with the moment and the
    /// schema it holds. SQLite's own backup API is used rather than a file
    /// copy: it is consistent even while something else is connected.
    pub fn backup(&self) -> Result<PathBuf> {
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "rigger".into());
        let stamp = now().replace([':', '-'], "").replace('T', "-").replace('Z', "");
        let version = self.schema_version()?;
        // The stamp is second-resolution, so two copies inside one second
        // would land on one name and the second would overwrite the first.
        // A suffix is cheaper than a finer stamp and keeps the name readable.
        let mut target = self.path.with_file_name(format!("{stem}.v{version}-{stamp}.bak"));
        for n in 2.. {
            if !target.exists() {
                break;
            }
            target = self.path.with_file_name(format!("{stem}.v{version}-{stamp}-{n}.bak"));
        }
        let mut out = Connection::open(&target).with_context(|| format!("cannot create {}", target.display()))?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut out).context("cannot start the backup")?;
        backup.step(-1).context("the backup did not finish")?;
        Ok(target)
    }

    /// Every copy taken of this database, newest first.
    ///
    /// The moment comes from the name rather than from the filesystem,
    /// which spells times differently per platform - but the name itself
    /// cannot be the sort key: the suffix that keeps two copies inside one
    /// second apart (`...-2.bak`) sorts BEFORE the unsuffixed name it was
    /// taken after. So copies sort by the moment they carry, then by that
    /// suffix as the number it is - inside one second it is the only thing
    /// that says which copy came last, and rotation deletes from the end.
    pub fn backups(&self) -> Result<Vec<PathBuf>> {
        let Some(dir) = self.path.parent() else {
            return Ok(Vec::new());
        };
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "rigger".into());
        let prefix = format!("{stem}.v");
        let mut found: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| {
                            let name = n.to_string_lossy();
                            name.starts_with(&prefix) && name.ends_with(".bak")
                        })
                        .unwrap_or(false)
                })
                .collect(),
            // A missing directory is not a failure to report here: it means
            // no copies, which is what the caller is asking about.
            Err(_) => return Ok(Vec::new()),
        };
        found.sort_by_key(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            // Reversed by negating the sequence and leaning on the reverse
            // of the moment below, so `newest first` needs no second pass.
            (stamp_of(&name), sequence_of(&name), name)
        });
        found.reverse();
        Ok(found)
    }

    /// Keeps the `keep` newest copies and deletes the rest, newest first.
    ///
    /// Returns what was deleted. `keep` of zero would mean "delete the copy
    /// just taken", which no caller wants, so it is read as one.
    pub fn prune_backups(&self, keep: usize) -> Result<Vec<PathBuf>> {
        let keep = keep.max(1);
        let all = self.backups()?;
        let mut removed = Vec::new();
        for path in all.into_iter().skip(keep) {
            // One copy that will not delete is not a reason to leave the
            // rest: rotation is housekeeping, not the point of the command.
            if std::fs::remove_file(&path).is_ok() {
                removed.push(path);
            }
        }
        Ok(removed)
    }

    /// When the newest copy was taken, as an RFC 3339 timestamp.
    ///
    /// Read from the stamp in the name rather than the file's own mtime: a
    /// copied or restored file carries a mtime that says when it was moved,
    /// and the name says when the record inside it was true.
    pub fn newest_backup_at(&self) -> Result<Option<String>> {
        let Some(newest) = self.backups()?.into_iter().next() else {
            return Ok(None);
        };
        Ok(newest.file_name().and_then(|n| stamp_of(&n.to_string_lossy())))
    }

    /// Every document of a project, newest first within each kind, in the
    /// order the kinds are declared: a vision before the research notes.
    pub fn documents(&self, project_id: i64, kind: Option<&str>) -> Result<Vec<Document>> {
        let mut sql = String::from("SELECT id, project_id, kind, slug, title, body, created_at, updated_at, source_file FROM documents WHERE project_id = ?1");
        if kind.is_some() {
            sql.push_str(" AND kind = ?2");
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = match kind {
            Some(kind) => stmt.query_map(params![project_id, kind], Document::from_row)?.collect::<Result<Vec<_>, _>>()?,
            None => stmt.query_map(params![project_id], Document::from_row)?.collect::<Result<Vec<_>, _>>()?,
        };
        let mut rows = rows;
        // Ordered in Rust rather than SQL: "the order the kinds are
        // declared" is a fact about DOC_KINDS, and a CASE in the query
        // would be a second copy of it to keep in step.
        rows.sort_by_key(|d| {
            let kind = DOC_KINDS.iter().position(|k| *k == d.kind).unwrap_or(DOC_KINDS.len());
            (kind, std::cmp::Reverse(d.updated_at.clone()))
        });
        Ok(rows)
    }

    /// One document, by the slug that addresses it.
    pub fn document(&self, project_id: i64, slug: &str) -> Result<Option<Document>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, project_id, kind, slug, title, body, created_at, updated_at, source_file
                 FROM documents WHERE project_id = ?1 AND slug = ?2",
                params![project_id, slug],
                Document::from_row,
            )
            .optional()?)
    }

    /// The one document of a kind a project has at most one of.
    ///
    /// By kind rather than by slug: the slug comes from the title, and a
    /// title with no ASCII in it - every hub of this line is in Russian -
    /// slugs to nothing and gets a made-up address instead. The kind is the
    /// stable handle, and `SINGULAR_DOC_KINDS` is what makes "the one" true.
    pub fn singular_document(&self, project_id: i64, kind: &str) -> Result<Option<Document>> {
        debug_assert!(SINGULAR_DOC_KINDS.contains(&kind), "{kind} is not a kind a project has one of");
        Ok(self.documents(project_id, Some(kind))?.into_iter().next())
    }

    /// Writes a document, making it if the slug is new and replacing its
    /// title and body if it is not. `created_at` survives a rewrite: a
    /// document keeps the day it was started.
    pub fn write_document(&self, project_id: i64, kind: &str, slug: &str, title: &str, body: &str) -> Result<Document> {
        self.write_document_from(project_id, kind, slug, title, body, None)
    }

    /// Writes a document, remembering the hub file it came from.
    ///
    /// `source` is only set when it is given: a document edited through
    /// `doc edit` must not forget the file it is exported to.
    pub fn write_document_from(&self, project_id: i64, kind: &str, slug: &str, title: &str, body: &str, source: Option<&str>) -> Result<Document> {
        if !DOC_KINDS.contains(&kind) {
            bail!("'{kind}' is not a kind of document; rigger knows {}", DOC_KINDS.join(", "));
        }
        let at = now();
        self.conn.execute(
            "INSERT INTO documents (project_id, kind, slug, title, body, created_at, updated_at, source_file)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)
             ON CONFLICT (project_id, slug) DO UPDATE SET
                 kind = excluded.kind, title = excluded.title, body = excluded.body, updated_at = excluded.updated_at,
                 source_file = COALESCE(excluded.source_file, documents.source_file)",
            params![project_id, kind, slug, title, body, at, source],
        )?;
        self.document(project_id, slug)?
            .with_context(|| format!("the document '{slug}' was written but cannot be read back"))
    }

    /// Records which hub file a document came from, without touching what
    /// it says: a document read before the record kept that column learns
    /// it on the next import.
    pub fn set_document_source(&self, id: i64, source_file: &str) -> Result<()> {
        self.conn
            .execute("UPDATE documents SET source_file = ?1 WHERE id = ?2", params![source_file, id])?;
        Ok(())
    }

    pub fn delete_document(&self, project_id: i64, slug: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM documents WHERE project_id = ?1 AND slug = ?2", params![project_id, slug])?
            > 0)
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
        // Read back rather than assembled here: a hand-built row is a second
        // place the shape of a project is spelt, and it goes stale the next
        // time a column is added.
        self.project_by_name(name)?
            .with_context(|| format!("the project '{name}' was recorded but cannot be read back"))
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {PROJECT_COLUMNS} FROM projects ORDER BY name"))?;
        let rows = stmt.query_map([], row_to_project)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn project_by_name(&self, name: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE name = ?1"), [name], row_to_project)
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
        // Every entry of that day. Matched by heading exactly first, and
        // then by the heading without its date: an earlier reader kept the
        // date apart from the heading, so a hub read by it and read again
        // now held every entry twice - once as `(ночь) · v0.2.1` and once
        // as `2026-09-03 (ночь) · v0.2.1` - and the export wrote both.
        let mut stmt = self.conn.prepare(
            "SELECT id, heading, notes, followed_by_rule, gap_after, rank FROM sessions \
             WHERE project_id = ?1 AND COALESCE(day, substr(started_at, 1, 10)) = ?2 AND notes IS NOT NULL ORDER BY id",
        )?;
        let rows: Vec<RecordedEntry> = stmt
            .query_map(params![project_id, entry.date], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let wanted = heading_sans_date(entry.heading.as_deref());
        let same = |heading: &Option<String>| heading_sans_date(heading.as_deref()) == wanted;
        let matched = rows.iter().find(|r| r.1 == entry.heading).or_else(|| rows.iter().find(|r| same(&r.1)));

        let notes = (!entry.body.trim().is_empty()).then(|| entry.body.clone());
        let Some((id, heading, was, rule, gap, rank)) = matched else {
            self.conn.execute(
                "INSERT INTO sessions (project_id, started_at, ended_at, day, heading, notes, followed_by_rule, gap_after, rank) \
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    project_id,
                    at,
                    entry.date,
                    entry.heading,
                    notes,
                    entry.followed_by_rule,
                    entry.gap_after as i64,
                    entry.rank as i64
                ],
            )?;
            return Ok(Change::Added);
        };
        // The same entry under another spelling of its heading is one
        // entry, and the record keeps one row for it.
        let mut merged = 0;
        for (other, ..) in rows.iter().filter(|r| r.0 != *id && same(&r.1)) {
            merged += self.conn.execute("DELETE FROM sessions WHERE id = ?1", [other])?;
        }
        if merged == 0
            && *heading == entry.heading
            && *was == notes
            && *rule == Some(entry.followed_by_rule)
            && *gap == Some(entry.gap_after as i64)
            && *rank == Some(entry.rank as i64)
        {
            return Ok(Change::Unchanged);
        }
        self.conn.execute(
            "UPDATE sessions SET day = ?2, heading = ?3, notes = ?4, followed_by_rule = ?5, gap_after = ?6, rank = ?7 WHERE id = ?1",
            params![
                id,
                entry.date,
                entry.heading,
                notes,
                entry.followed_by_rule,
                entry.gap_after as i64,
                entry.rank as i64
            ],
        )?;
        Ok(Change::Updated)
    }

    /// Leaves a session's diary entry in the record, where an export of the
    /// diary reads it.
    ///
    /// Until this, `end` could only append the entry to a file - and once a
    /// hub is written from the record, that file is generated, so the
    /// entry went into a file the next export would rewrite without it.
    /// The entry takes the newest rank, and the rule and spacing the diary
    /// already uses between its entries.
    pub fn write_session_diary(&self, session_id: i64, project_id: i64, day: &str, heading: Option<&str>, notes: &str) -> Result<()> {
        let (rule, gap, rank): (Option<bool>, Option<i64>, Option<i64>) = self
            .conn
            .query_row(
                "SELECT followed_by_rule, gap_after, MIN(COALESCE(rank, 0)) FROM sessions WHERE project_id = ?1 AND notes IS NOT NULL",
                [project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or((None, None, None));
        self.conn.execute(
            "UPDATE sessions SET day = ?2, heading = ?3, notes = ?4, followed_by_rule = ?5, gap_after = ?6, rank = ?7 WHERE id = ?1",
            params![
                session_id,
                day,
                heading,
                notes,
                rule.unwrap_or(false),
                gap.unwrap_or(1),
                rank.map(|r| r - 1).unwrap_or(0)
            ],
        )?;
        Ok(())
    }

    /// Puts a line at the top of a README's state block.
    ///
    /// The block is the owner's summary of where things stand, newest
    /// first, and a hub written from the record had no way to grow it: the
    /// lines came in with an import and never after. Now a sitting that
    /// shifts the state says so with one line, the way the ritual always
    /// asked, and the export writes it where the hub keeps them.
    pub fn add_state_line(&self, project_id: i64, stamp: &str, body: &str) -> Result<()> {
        let gap: i64 = self
            .conn
            .query_row("SELECT gap_after FROM state_lines WHERE project_id = ?1 AND position = 0", [project_id], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        // Shifted from the bottom up: the column is unique per position,
        // and moving the top line first would land on the one below it.
        self.conn
            .execute("UPDATE state_lines SET position = -position - 1 WHERE project_id = ?1", [project_id])?;
        self.conn
            .execute("UPDATE state_lines SET position = -position WHERE project_id = ?1", [project_id])?;
        self.conn.execute(
            "INSERT INTO state_lines (project_id, position, stamp, body, gap_after) VALUES (?1, 0, ?2, ?3, ?4)",
            params![project_id, stamp, body, gap],
        )?;
        Ok(())
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
            "SELECT COALESCE(day, substr(started_at, 1, 10)), heading, notes, followed_by_rule, gap_after, rank FROM sessions \
             WHERE project_id = ?1 AND notes IS NOT NULL \
             ORDER BY COALESCE(rank, 0) ASC, started_at DESC, id ASC",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            Ok(crate::hub::DiaryEntry {
                date: r.get(0)?,
                heading: r.get(1)?,
                body: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                followed_by_rule: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
                gap_after: r.get::<_, Option<i64>>(4)?.unwrap_or(1) as usize,
                rank: r.get::<_, Option<i64>>(5)?.unwrap_or(0) as usize,
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
                "INSERT INTO hub_prose (project_id, file, position, heading, body, gap_after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![project_id, file, run.position as i64, run.heading, run.body, run.gap_after as i64],
            )?;
        }
        Ok(if before.is_empty() { Change::Added } else { Change::Updated })
    }

    /// The state lines a project's README carries, in the order it had them.
    pub fn set_state_lines(&self, project_id: i64, lines: &[crate::hub::StateLine]) -> Result<Change> {
        let before = self.state_lines(project_id)?;
        if before == lines {
            return Ok(Change::Unchanged);
        }
        self.conn.execute("DELETE FROM state_lines WHERE project_id = ?1", [project_id])?;
        for (position, line) in lines.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO state_lines (project_id, position, stamp, body, gap_after) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![project_id, position as i64, line.stamp, line.body, line.gap_after as i64],
            )?;
        }
        Ok(match before.is_empty() {
            true => Change::Added,
            false => Change::Updated,
        })
    }

    pub fn state_lines(&self, project_id: i64) -> Result<Vec<crate::hub::StateLine>> {
        let mut stmt = self
            .conn
            .prepare("SELECT stamp, body, gap_after FROM state_lines WHERE project_id = ?1 ORDER BY position")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok(crate::hub::StateLine {
                stamp: r.get(0)?,
                body: r.get(1)?,
                gap_after: r.get::<_, i64>(2)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The prose the record holds for one file of a hub, in file order.
    pub fn hub_prose(&self, project_id: i64, file: &str) -> Result<Vec<crate::hub::Prose>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file, position, heading, body, gap_after FROM hub_prose WHERE project_id = ?1 AND file = ?2 ORDER BY position")?;
        let rows = stmt.query_map(params![project_id, file], |r| {
            Ok(crate::hub::Prose {
                file: r.get(0)?,
                position: r.get::<_, i64>(1)? as usize,
                heading: r.get(2)?,
                body: r.get(3)?,
                gap_after: r.get::<_, Option<i64>>(4)?.unwrap_or(1) as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every version of a project as a stage, for the export to write back.
    pub fn stages(&self, project_id: i64, shipped: bool) -> Result<Vec<crate::hub::Stage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, title, shipped_at, notes, heading_depth, notes_after, heading, after_prose, rank, gap_after FROM versions \
             WHERE project_id = ?1 AND status = ?2 \
             ORDER BY COALESCE(rank, 0), id",
        )?;
        let status = if shipped { "shipped" } else { "planned" };
        let rows = stmt.query_map(params![project_id, status], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(9)?,
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
                    gap_after: r.get::<_, Option<i64>>(10)?.unwrap_or(1) as usize,
                },
            ))
        })?;

        let mut stages: Vec<(i64, Option<i64>, crate::hub::Stage)> = rows.collect::<rusqlite::Result<_>>()?;
        // The order a hub had, when the record knows it. A hub does not
        // always run by version number: one changelog writes v0.17.0 above
        // v0.17.1, because the patch was written up after the release it
        // patched, and sorting by number transposed the two on every run.
        //
        // Guessing is for stages the record has never seen in a file -
        // recorded by a command rather than read from a hub. Newest first
        // for the changelog, oldest first for the plan: a hub reads its
        // history backwards and its future forwards.
        let known = stages.iter().any(|(_, rank, _)| rank.is_some());
        if known {
            // A stage the file never held is put where its number says,
            // among the stages the file did hold, in the same run of prose
            // as the neighbour it goes before. A version a tag closed after
            // the hub was last read used to land below the oldest entry,
            // under the prose that ends the file; put at the top instead,
            // a patch of an old release would have led the changelog.
            let (mut ranked, fresh): (Vec<_>, Vec<_>) = stages.into_iter().partition(|(_, rank, _)| rank.is_some());
            ranked.sort_by_key(|(_, rank, _)| rank.unwrap_or(0));
            for (id, rank, mut stage) in fresh {
                let order = version_order(&stage.version);
                let at = ranked.iter().position(|(_, _, s)| {
                    let other = version_order(&s.version);
                    if shipped { other < order } else { other > order }
                });
                let neighbour = at.map(|i| &ranked[i].2).or(ranked.last().map(|(_, _, s)| s));
                if let Some(neighbour) = neighbour {
                    stage.after_prose = neighbour.after_prose;
                }
                ranked.insert(at.unwrap_or(ranked.len()), (id, rank, stage));
            }
            stages = ranked;
        } else {
            stages.sort_by_key(|(_, _, s)| version_order(&s.version));
            if shipped {
                stages.reverse();
            }
        }
        let mut out = Vec::new();
        for (id, _, mut stage) in stages {
            stage.tasks = self.tasks_of_version(id)?;
            out.push(stage);
        }
        // A version the record knows only from a tag has nothing to write
        // up: no title, no prose, no tasks. It is a fact for the calendar
        // and for `why`, not an entry - and writing a bare heading for
        // each put forty empty releases into one changelog of this line.
        if shipped {
            out.retain(|s| {
                !s.heading.trim().is_empty() || s.title.is_some() || !s.notes.trim().is_empty() || !s.notes_after.trim().is_empty() || !s.tasks.is_empty()
            });
        }
        Ok(out)
    }

    /// The tasks of one version, in the order the plan listed them.
    fn tasks_of_version(&self, version_id: i64) -> Result<Vec<crate::hub::Task>> {
        let mut stmt = self
            .conn
            .prepare("SELECT title, status FROM tasks WHERE version_id = ?1 AND status <> 'dropped' ORDER BY COALESCE(position, id), id")?;
        let rows = stmt.query_map([version_id], |r| {
            Ok(crate::hub::Task {
                title: r.get(0)?,
                done: r.get::<_, String>(1)? == "done",
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The place the record keeps for itself: the project named after the
    /// profile, if one has been made.
    ///
    /// Named rather than "the first service project there is". A desk keeps
    /// its cards under a service project too, so `kind = 'service' LIMIT 1`
    /// was a coin toss the moment a record held both - and the loser was a
    /// retro about the whole line filed silently under the ticket desk.
    /// The profile's name is what a line-wide fact belongs to, so that is
    /// what is asked for.
    pub fn service_project(&self) -> Result<Option<Project>> {
        Ok(self
            .project_by_name(&crate::profile::Config::load()?.current_name())?
            .filter(|p| p.kind == Kind::Service))
    }

    /// The same place, made if it is not there yet.
    ///
    /// The rituals of a line are not the rituals of any one project, and a
    /// document has to hang off a project, so the profile has one of its
    /// own. Made on demand the way a desk is: nobody should have to know
    /// about it before writing down how the line works.
    pub fn profile_project(&self) -> Result<Project> {
        let name = crate::profile::Config::load()?.current_name();
        match self.project_by_name(&name)? {
            Some(existing) if existing.kind == Kind::Service => Ok(existing),
            // A repository of the same name as the profile: the line-wide
            // texts would land in a product's record, where nobody would
            // look for them. Said rather than guessed around.
            Some(other) => bail!(
                "'{name}' is the profile's name and also a repository at {};                  rename one of them, or the rituals of the line would be filed under a product",
                other.path
            ),
            None => self.add_project(&name, &format!("service:{name}"), None, Kind::Service),
        }
    }

    pub fn project_by_path(&self, path: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE path = ?1"), [path], row_to_project)
            .optional()?)
    }

    /// Puts back a column an edited migration left out.
    ///
    /// See `REPAIRS`: a database that passed migration 7 before it was
    /// edited is at the same schema version as one that passed it after,
    /// and only the table itself can tell them apart.
    fn repair(&self) -> Result<()> {
        for (table, column, sql) in REPAIRS {
            let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let mut has = false;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                if row.get::<_, String>(1)? == column {
                    has = true;
                    break;
                }
            }
            if !has {
                self.conn.execute_batch(sql)?;
            }
        }
        Ok(())
    }

    /// Remembers where a project's hub is, so a later check can find it.
    ///
    /// The hub of a project need not sit beside its repository - every hub
    /// of this line lives in a notes vault - and a check that guesses looks
    /// at nothing and says everything matches.
    pub fn set_hub_path(&self, project_id: i64, dir: &std::path::Path) -> Result<()> {
        self.conn
            .execute("UPDATE projects SET hub_path = ?2 WHERE id = ?1", params![project_id, dir.to_string_lossy()])?;
        Ok(())
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
    pub fn upsert_version(&self, project_id: i64, stage: &crate::hub::Stage, from_changelog: bool) -> Result<Upserted> {
        let mut existing: Option<Recorded> = self
            .conn
            .query_row(
                "SELECT id, title, status, shipped_at, notes, notes_after, heading, heading_depth, after_prose, rank, gap_after \
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
                            gap_after: r.get(10)?,
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
        // A version a tag proved shipped keeps the day the tag says. The
        // changelog may carry another - written up the day after, or the
        // day before the tag was pushed - and a fact from git outranks what
        // was written. Found by adopting a line twice: each import put the
        // hub's day back, and each sync then reported the version shipped
        // anew.
        let from_git: Option<String> = match &existing {
            Some((id, ..)) => self
                .conn
                .query_row("SELECT shipped_at FROM versions WHERE id = ?1 AND shipped_source = 'tag'", [id], |r| r.get(0))
                .optional()?,
            None => None,
        };
        // Nor does the plan's copy of a shipped stage un-ship it: a major
        // map lists what went out without a date, and reading it as
        // "planned" moved the whole changelog into the plan on the second
        // import of a hub.
        let already_shipped = match &existing {
            Some((_, _, was, was_at, ..)) if was == "shipped" && stage.shipped_on.is_none() => Some(was_at.clone()),
            _ => None,
        };
        let (status, shipped_on) = match (from_git, already_shipped) {
            (Some(day), _) => ("shipped", Some(day)),
            (None, Some(day)) => ("shipped", day),
            (None, None) => (if stage.shipped_on.is_some() { "shipped" } else { "planned" }, stage.shipped_on.clone()),
        };
        // A reading that does not own the shape keeps the one already
        // recorded, so the plan's copy of a shipped stage cannot move it
        // out of the changelog it was written in.
        // Which reading says how a stage is written. The changelog always
        // does. The plan does for a stage that has not shipped - it is the
        // only file that holds one - and never for a stage that has: its
        // copy of one is a line in the major map, or a stage the tag
        // closed before the plan was told, and the plan's depth and place
        // say nothing about where the entry stands in the changelog.
        let owns_shape = from_changelog || status != "shipped";
        let shape = match owns_shape {
            true => Shape::of(stage),
            false => existing.as_ref().map(|(.., recorded)| recorded.clone()).unwrap_or_else(|| Shape::of(stage)),
        };
        // An empty body is no body: a stage written without prose and one
        // whose prose was deleted are the same thing to the record, and
        // storing "" would make an export print a blank line for it.
        let notes = (!stage.notes.trim().is_empty()).then(|| stage.notes.clone());
        let notes_after = (!stage.notes_after.trim().is_empty()).then(|| stage.notes_after.clone());
        // A reading that defers on the shape defers on the prose too. The
        // plan's copy of a shipped stage carries a line of its own - "already
        // out" under the major map - and writing it over the changelog's
        // entry lost what was written about the release.
        let (notes, notes_after) = match (owns_shape, &existing) {
            (false, Some((_, _, _, _, was_notes, was_after, _))) => (was_notes.clone(), was_after.clone()),
            _ => (notes, notes_after),
        };
        match existing {
            Some((id, title, was_status, shipped_at, was_notes, was_after, was_shape)) => {
                // The prose and the shape join the comparison, or a hub whose
                // entry was rewritten would import as "nothing changed" and
                // the record would keep serving the old words to an export.
                // The heading is part of that: it is what an export writes,
                // and it cannot be composed from the other fields.
                if title.as_deref() == stage.title.as_deref()
                    && was_status == status
                    && shipped_at == shipped_on
                    && was_notes == notes
                    && was_after == notes_after
                    && was_shape == shape
                {
                    return Ok(Upserted {
                        id,
                        change: Change::Unchanged,
                        shipped: status == "shipped",
                    });
                }
                self.conn.execute(
                    "UPDATE versions SET title = ?1, status = ?2, shipped_at = ?3, notes = ?4, \
                     notes_after = ?5, heading_depth = ?6, heading = ?7, after_prose = ?8, rank = ?9, gap_after = ?10 \
                     WHERE id = ?11",
                    params![
                        stage.title,
                        status,
                        shipped_on,
                        notes,
                        notes_after,
                        shape.depth,
                        shape.heading,
                        shape.after_prose,
                        shape.rank,
                        shape.gap_after,
                        id
                    ],
                )?;
                Ok(Upserted {
                    id,
                    change: Change::Updated,
                    shipped: status == "shipped",
                })
            }
            None => {
                self.conn.execute(
                    "INSERT INTO versions (project_id, name, title, status, shipped_at, notes, notes_after, heading_depth, heading, after_prose, rank, gap_after) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        project_id,
                        stage.version,
                        stage.title,
                        status,
                        shipped_on,
                        notes,
                        notes_after,
                        shape.depth,
                        shape.heading,
                        shape.after_prose,
                        shape.rank,
                        shape.gap_after
                    ],
                )?;
                Ok(Upserted {
                    id: self.conn.last_insert_rowid(),
                    change: Change::Added,
                    shipped: status == "shipped",
                })
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
    pub fn upsert_task(&self, project_id: i64, version_id: i64, position: usize, task: &crate::hub::Task, may_reopen: bool) -> Result<(i64, Change)> {
        let status = if task.done { "done" } else { "new" };
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
                // A task the record closed is not reopened by a plan that
                // has not been told. The stage shipped, the task was closed
                // through the record, and the plan's empty box is stale
                // rather than a decision - found when adopting this
                // project's own hub reopened the three tasks of a version
                // tagged two days before.
                let status = if !may_reopen && was == "done" { "done" } else { status };
                // An empty box says "not done", not "new": a task that is
                // active or frozen in the record keeps that through a
                // reading of its hub, or every export and import would
                // reset the desk to the start.
                let status = if !task.done && is_open_status(&was) { was.as_str() } else { status };
                if was == status && title == task.title && at == Some(position) {
                    return Ok((id, Change::Unchanged));
                }
                let closed_at = (status == "done").then(now);
                self.conn.execute(
                    "UPDATE tasks SET status = ?1, \
                     closed_at = CASE WHEN ?1 = 'done' AND status = 'done' THEN closed_at ELSE ?2 END, \
                     title = ?3, position = ?4 WHERE id = ?5",
                    params![status, closed_at, task.title, position, id],
                )?;
                Ok((id, Change::Updated))
            }
            None => {
                self.conn.execute(
                    "INSERT INTO tasks (project_id, version_id, title, status, created_at, closed_at, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![project_id, version_id, task.title, status, now(), task.done.then(now), position],
                )?;
                Ok((self.conn.last_insert_rowid(), Change::Added))
            }
        }
    }

    /// Marks the open tasks of a stage that its reading no longer lists.
    ///
    /// A hub kept by hand is the owner's pen, and a line struck from it is
    /// struck: the record used to keep every task ever read as open, so a
    /// hub that had lived a month by hand exported every rewording of
    /// every task beside the current one. Done tasks stay - they are
    /// history - and a dropped task listed again comes back as open.
    pub fn drop_tasks_not_in(&self, version_id: i64, keep: &[i64]) -> Result<u32> {
        let ids = keep.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        let dropped = self.conn.execute(
            &format!("UPDATE tasks SET status = 'dropped' WHERE version_id = ?1 AND status NOT IN ('done', 'dropped') AND id NOT IN ({ids})"),
            [version_id],
        )?;
        Ok(dropped as u32)
    }

    /// Marks the planned versions a hub no longer names, in either file.
    ///
    /// A stage renumbered by hand - v1.10 becoming v1.11 when the queue
    /// moved - left its old number in the record as a stage still to come,
    /// with every task under it, and the export printed both.
    pub fn drop_versions_not_in(&self, project_id: i64, keep: &[&str]) -> Result<u32> {
        let marks = std::iter::repeat_n("?", keep.len()).collect::<Vec<_>>().join(",");
        let sql = format!("UPDATE versions SET status = 'dropped' WHERE project_id = ?1 AND status = 'planned' AND name NOT IN ({marks})");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut args: Vec<&dyn rusqlite::ToSql> = vec![&project_id];
        args.extend(keep.iter().map(|k| k as &dyn rusqlite::ToSql));
        let dropped = stmt.execute(args.as_slice())?;
        // Its open tasks go with it: they would otherwise go on being
        // counted as the project's open work under a stage nobody can see.
        self.conn.execute(
            "UPDATE tasks SET status = 'dropped' WHERE status NOT IN ('done', 'dropped') \
             AND version_id IN (SELECT id FROM versions WHERE project_id = ?1 AND status = 'dropped')",
            [project_id],
        )?;
        Ok(dropped as u32)
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
        // A gate run is a measurement, not a statement: two runs of the
        // same command with the same result are two facts, and collapsing
        // them would lose the one that says the gate is still green today.
        if kind == "gate" {
            let session = self.open_session(project_id)?.map(|s| s.id);
            self.conn.execute(
                "INSERT INTO events (project_id, session_id, kind, body, author, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![project_id, session, kind, body, author, created_at],
            )?;
            return Ok(Change::Added);
        }
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
             WHERE e.project_id = ?1 AND e.kind NOT IN ('next', 'withdrawn')",
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
             WHERE project_id = ?1 AND created_at >= ?2 AND kind NOT IN ('next', 'withdrawn')
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
            .prepare("SELECT id, title, status FROM tasks WHERE version_id = ?1 AND status NOT IN ('done', 'dropped') ORDER BY id")?;
        let tasks = stmt
            .query_map([id], |r| {
                Ok(Task {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    status: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<Task>>>()?;
        Ok(Some(CurrentStage { version: name, title, tasks }))
    }

    /// Gives a task a status from the vocabulary. The project is part of
    /// the lookup, as with `close_task`. Returns the title and the status
    /// it had.
    pub fn set_task_status(&self, project_id: i64, task_id: i64, status: &str) -> Result<(String, String)> {
        if !TASK_STATUSES.contains(&status) || status == "dropped" {
            bail!("'{status}' is not a status a task can be given; one of {}", TASK_STATUSES[..5].join(", "));
        }
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT title, status FROM tasks WHERE id = ?1 AND project_id = ?2",
                params![task_id, project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((title, was)) = found else {
            bail!("no task [{task_id}] in this project; `plan` lists the ids");
        };
        let closed_at = (status == "done").then(now);
        self.conn.execute(
            "UPDATE tasks SET status = ?1, closed_at = ?2 WHERE id = ?3",
            params![status, closed_at, task_id],
        )?;
        Ok((title, was))
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
                // A stage that just shipped leaves the plan, and the shape
                // it had there - its heading, depth and place among the
                // plan's stages - says nothing about where it stands in
                // the changelog. Cleared, it is written where the next
                // entry goes; kept, it sorted among the changelog's ranks
                // by a number from another file.
                if status != "shipped" {
                    self.conn.execute(
                        "UPDATE versions SET heading = NULL, heading_depth = NULL, rank = NULL, after_prose = NULL, gap_after = NULL WHERE id = ?1",
                        [id],
                    )?;
                }
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

    /// Sets the command that says a project is fit to commit, or clears it
    /// when given `None`.
    pub fn set_gate(&self, project_id: i64, gate: Option<&str>) -> Result<()> {
        self.conn.execute("UPDATE projects SET gate = ?2 WHERE id = ?1", params![project_id, gate])?;
        Ok(())
    }

    /// The last gate run recorded inside a sitting, if there was one.
    ///
    /// By session rather than by time. Timestamps here are stamped to the
    /// second, and a gate run and the next `session start` land in the same
    /// second often enough that "after this session began" answered yes to
    /// a run from the sitting before - which is how a reminder that should
    /// have been spent became one that fires for ever. The session id is
    /// exact where a second is not.
    ///
    /// Ordered by id as well as time for the same reason: two runs within
    /// one second would otherwise come back in whatever order the table
    /// felt like.
    pub fn last_gate_in_session(&self, session_id: i64) -> Result<Option<RecentEvent>> {
        Ok(self
            .conn
            .query_row(
                "SELECT kind, created_at, body, commit_hash IS NOT NULL FROM events
                 WHERE session_id = ?1 AND kind = 'gate' ORDER BY created_at DESC, id DESC LIMIT 1",
                params![session_id],
                |r| {
                    Ok(RecentEvent {
                        kind: r.get(0)?,
                        date: r.get(1)?,
                        body: r.get(2)?,
                        from_git: r.get(3)?,
                    })
                },
            )
            .optional()?)
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
             WHERE project_id = ?1 AND status <> 'dropped' AND (planned_week IS NOT NULL OR shipped_at IS NOT NULL)",
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

    /// The desk: where cards live, made the first time one is needed.
    pub fn desk_project(&self) -> Result<Project> {
        if let Some(desk) = self.project_by_name(DESK)? {
            return Ok(desk);
        }
        self.add_project(DESK, &format!("service:{DESK}"), None, Kind::Service)
    }

    /// Makes a card. Without a key it gets a local one, `LOCAL-<day>-<n>`,
    /// to be renamed when a tracker names the task.
    pub fn new_card(&self, key: Option<&str>, title: &str, aliases: &[String]) -> Result<crate::card::Card> {
        let desk = self.desk_project()?;
        let key = match key {
            Some(key) => key.trim().to_ascii_uppercase(),
            None => {
                let day = today().replace('-', "");
                let n: i64 = self
                    .conn
                    .query_row("SELECT COUNT(*) FROM tasks WHERE key LIKE ?1", [format!("LOCAL-{day}-%")], |r| r.get(0))?;
                format!("LOCAL-{day}-{}", n + 1)
            }
        };
        if key.is_empty() {
            bail!("a card needs a key or none at all; an empty one is neither");
        }
        if let Some(taken) = self.card_by_ref(&key)? {
            bail!("'{key}' already names a card: {}", taken.title);
        }
        let at = now();
        let aliases = serde_json::to_string(aliases)?;
        self.conn.execute(
            "INSERT INTO tasks (project_id, title, status, created_at, key, aliases, updated_at) VALUES (?1, ?2, 'new', ?3, ?4, ?5, ?3)",
            params![desk.id, title.trim(), at, key, aliases],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(self.card(id)?.expect("the card just made"))
    }

    fn row_to_card(r: &rusqlite::Row) -> rusqlite::Result<crate::card::Card> {
        let aliases: Option<String> = r.get(4)?;
        Ok(crate::card::Card {
            id: r.get(0)?,
            key: r.get(1)?,
            title: r.get(2)?,
            status: r.get(3)?,
            aliases: aliases.and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_default(),
            summary: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    }

    const CARD_COLUMNS: &'static str = "id, key, title, status, aliases, summary, created_at, updated_at";

    pub fn card(&self, id: i64) -> Result<Option<crate::card::Card>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {} FROM tasks WHERE id = ?1 AND key IS NOT NULL", Self::CARD_COLUMNS),
                [id],
                Self::row_to_card,
            )
            .optional()?)
    }

    /// A card by whatever names it: its numeric id, its key, or an alias -
    /// case does not matter.
    pub fn card_by_ref(&self, text: &str) -> Result<Option<crate::card::Card>> {
        let text = text.trim();
        if let Ok(id) = text.parse::<i64>()
            && let Some(card) = self.card(id)?
        {
            return Ok(Some(card));
        }
        let upper = text.to_ascii_uppercase();
        if let Some(card) = self
            .conn
            .query_row(
                &format!("SELECT {} FROM tasks WHERE key IS NOT NULL AND UPPER(key) = ?1", Self::CARD_COLUMNS),
                [&upper],
                Self::row_to_card,
            )
            .optional()?
        {
            return Ok(Some(card));
        }
        Ok(self.cards(None)?.into_iter().find(|c| c.aliases.iter().any(|a| a.eq_ignore_ascii_case(text))))
    }

    /// Every card, newest first; `status` narrows it - `open` is everything
    /// short of done and dropped.
    pub fn cards(&self, status: Option<&str>) -> Result<Vec<crate::card::Card>> {
        let filter = match status {
            None | Some("all") => String::new(),
            Some("open") => " AND status NOT IN ('done', 'dropped')".to_string(),
            Some(s) => format!(" AND status = '{}'", s.replace('\'', "")),
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM tasks WHERE key IS NOT NULL{filter} ORDER BY COALESCE(updated_at, created_at) DESC, id DESC",
            Self::CARD_COLUMNS
        ))?;
        let rows = stmt.query_map([], Self::row_to_card)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Gives a card a new key, keeping the old one as an alias: tasks move
    /// between trackers, and the folder of a card is not renamed because
    /// things link to it.
    pub fn rename_card(&self, id: i64, key: &str) -> Result<crate::card::Card> {
        let Some(card) = self.card(id)? else { bail!("no card [{id}]") };
        let key = key.trim().to_ascii_uppercase();
        if let Some(taken) = self.card_by_ref(&key)?
            && taken.id != id
        {
            bail!("'{key}' already names a card: {}", taken.title);
        }
        let mut aliases = card.aliases.clone();
        if !aliases.iter().any(|a| a.eq_ignore_ascii_case(&card.key)) {
            aliases.push(card.key.clone());
        }
        self.conn.execute(
            "UPDATE tasks SET key = ?2, aliases = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, key, serde_json::to_string(&aliases)?, now()],
        )?;
        Ok(self.card(id)?.expect("the card just renamed"))
    }

    pub fn add_alias(&self, id: i64, alias: &str) -> Result<crate::card::Card> {
        let Some(card) = self.card(id)? else { bail!("no card [{id}]") };
        let alias = alias.trim();
        let mut aliases = card.aliases.clone();
        if !aliases.iter().any(|a| a.eq_ignore_ascii_case(alias)) {
            aliases.push(alias.to_string());
        }
        self.conn.execute(
            "UPDATE tasks SET aliases = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, serde_json::to_string(&aliases)?, now()],
        )?;
        Ok(self.card(id)?.expect("the card just changed"))
    }

    pub fn set_card_summary(&self, id: i64, summary: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET summary = ?2, updated_at = ?3 WHERE id = ?1 AND key IS NOT NULL",
            params![id, summary.trim(), now()],
        )?;
        Ok(())
    }

    pub fn touch_card(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE tasks SET updated_at = ?2 WHERE id = ?1", params![id, now()])?;
        Ok(())
    }

    /// Links a card to a repository, with the branch it is worked on
    /// there. Linking again replaces the branch and the role.
    pub fn link_card(&self, task_id: i64, project_id: i64, branch: Option<&str>, role: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO task_projects (task_id, project_id, branch, role) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT (task_id, project_id) DO UPDATE SET branch = COALESCE(excluded.branch, branch), role = COALESCE(excluded.role, role)",
            params![task_id, project_id, branch, role],
        )?;
        self.touch_card(task_id)
    }

    pub fn card_links(&self, task_id: i64) -> Result<Vec<crate::card::Link>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.name, p.path, tp.branch, tp.role FROM task_projects tp JOIN projects p ON p.id = tp.project_id \
             WHERE tp.task_id = ?1 ORDER BY p.name",
        )?;
        let rows = stmt.query_map([task_id], |r| {
            Ok(crate::card::Link {
                project: r.get(0)?,
                path: r.get(1)?,
                branch: r.get(2)?,
                role: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The events written against a card, oldest first.
    pub fn task_events(&self, task_id: i64) -> Result<Vec<RecentEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, substr(created_at, 1, 10), body, commit_hash IS NOT NULL FROM events \
             WHERE task_id = ?1 AND kind NOT IN ('withdrawn') ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([task_id], |r| {
            Ok(RecentEvent {
                kind: r.get(0)?,
                date: r.get(1)?,
                body: r.get(2)?,
                from_git: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Cards other than the one given whose events mention a needle - an
    /// id or a case number. The weak trail: a task that moved leaves its
    /// old number in the text of the card that took it over.
    pub fn cards_mentioning(&self, needle: &str, except: &[String]) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.key, t.title FROM events e JOIN tasks t ON t.id = e.task_id \
             WHERE t.key IS NOT NULL AND instr(e.body, ?1) > 0 ORDER BY t.key",
        )?;
        let rows = stmt.query_map([needle], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|(k, _)| !except.contains(k))
            .collect())
    }

    /// Records an event against a card as well as a project.
    pub fn record_task_event(&self, project_id: i64, task_id: i64, kind: &str, body: &str, at: &str, author: &str) -> Result<Change> {
        let seen: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM events WHERE task_id = ?1 AND kind = ?2 AND body = ?3",
                params![task_id, kind, body],
                |r| r.get(0),
            )
            .optional()?;
        if seen.is_some() {
            return Ok(Change::Unchanged);
        }
        let session = self.open_session(project_id)?.map(|s| s.id);
        self.conn.execute(
            "INSERT INTO events (project_id, session_id, task_id, kind, body, author, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![project_id, session, task_id, kind, body, author, at],
        )?;
        self.touch_card(task_id)?;
        Ok(Change::Added)
    }

    /// The project a task belongs to, by id.
    pub fn task_owner(&self, task_id: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT project_id FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
            .optional()?)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(value) => {
                self.conn.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    params![key, value],
                )?;
            }
            None => {
                self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
            }
        }
        Ok(())
    }

    pub fn count_open_tasks(&self, project_id: i64) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status NOT IN ('done', 'dropped')",
            [project_id],
            |r| r.get(0),
        )?;
        Ok(n.unsigned_abs())
    }

    /// Events of a kind that are still open - questions and wishes are
    /// answered by being resolved, which a later release will do.
    /// Withdraws the owner's questions a hub no longer lists.
    ///
    /// A question the owner struck from the hub by hand was settled or let
    /// go, and the record used to keep asking it in every packet. Only the
    /// owner's own questions - read from a hub - are withdrawn this way; a
    /// question an assistant asked through `ask_owner` has never been in
    /// the file, and the file cannot strike what it never held.
    pub fn withdraw_questions_not_in(&self, project_id: i64, keep: &[String]) -> Result<u32> {
        let marks = std::iter::repeat_n("?", keep.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE events SET kind = 'withdrawn' WHERE project_id = ?1 AND kind = 'question' AND author = 'owner' \
             AND body NOT IN ({marks})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut args: Vec<&dyn rusqlite::ToSql> = vec![&project_id];
        args.extend(keep.iter().map(|k| k as &dyn rusqlite::ToSql));
        Ok(stmt.execute(args.as_slice())? as u32)
    }

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
             WHERE project_id = ?1 AND kind NOT IN ('question', 'wish', 'next', 'withdrawn')
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
            "SELECT kind, substr(created_at, 1, 10), body, commit_hash IS NOT NULL              FROM events WHERE project_id = ?1 AND created_at >= ?2 AND session_id IS NULL              AND kind NOT IN ('wish', 'next', 'withdrawn') ORDER BY created_at DESC, id DESC",
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
            "SELECT COUNT(*) FROM events WHERE project_id = ?1 AND kind NOT IN ('question', 'wish', 'next', 'withdrawn')",
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

/// The columns `row_to_project` reads, in the order it reads them.
///
/// One constant rather than the same list spelt at each query: the column
/// added in migration 21 had to reach four places, and a list spelt four
/// times is a list that reaches three.
const PROJECT_COLUMNS: &str = "id, name, path, remote, created_at, tier, rhythm_weeks, kind, hub_path, gate";

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
        hub_path: row.get(8)?,
        gate: row.get(9)?,
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

/// The moment a backup name carries, back as an RFC 3339 timestamp.
///
/// `rigger.v23-20260915-080501.bak` was taken at `2026-09-15T08:05:01Z`.
/// A name that does not hold a stamp gives nothing rather than a guess: a
/// wrong date here would be reported to the owner as the age of their
/// insurance.
pub fn stamp_of(name: &str) -> Option<String> {
    let rest = name.strip_suffix(".bak")?;
    // `<stem>.v<schema>-<date>-<time>[-<n>]`: take the two fields after the
    // schema, so the collision suffix is ignored rather than parsed.
    let after_version = rest.rsplit_once(".v")?.1;
    let mut parts = after_version.split('-');
    let _schema = parts.next()?;
    let date = parts.next()?;
    let time = parts.next()?;
    if date.len() != 8 || time.len() != 6 || !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let stamp = format!(
        "{}-{}-{}T{}:{}:{}Z",
        &date[0..4],
        &date[4..6],
        &date[6..8],
        &time[0..2],
        &time[2..4],
        &time[4..6]
    );
    // Parsed, not just shaped: 20261345 has the right length and is not a day.
    stamp.parse::<jiff::Timestamp>().ok().map(|_| stamp)
}

/// The collision suffix a backup name carries, as the number it is.
///
/// `rigger.v23-20260915-080501-2.bak` is the second copy taken in that
/// second; the unsuffixed name is the first. Within one second this is the
/// only thing that says which copy is newer, and `2` must not sort before
/// `10` the way its text does.
fn sequence_of(name: &str) -> u32 {
    let Some(rest) = name.strip_suffix(".bak") else {
        return 0;
    };
    let Some(after_version) = rest.rsplit_once(".v").map(|(_, tail)| tail) else {
        return 0;
    };
    // `<schema>-<date>-<time>[-<n>]`: a fourth field, if there is one.
    match after_version.split('-').nth(3) {
        Some(n) => n.parse().unwrap_or(0),
        None => 1,
    }
}

/// What addresses a document on a command line, from its title.
///
/// ASCII letters and digits survive, everything else becomes a hyphen, and
/// runs of hyphens collapse. A title with no ASCII in it at all - the
/// owner's hub is in Russian - would slug to nothing, and an empty address
/// is no address: those keep the kind and a number instead, chosen by the
/// caller, so this returns empty rather than inventing one.
pub fn slugify(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Timestamps are stored as UTC in RFC 3339, which sorts as text.
pub fn now() -> String {
    jiff::Timestamp::now()
        .round(jiff::Unit::Second)
        .map(|t| t.to_string())
        .unwrap_or_else(|_| jiff::Timestamp::now().to_string())
}

/// Today, as a hub dates its lines: `2026-09-08`.
pub fn today() -> String {
    now().split('T').next().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::{slugify, stamp_of, version_order};

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

    #[test]
    fn a_title_becomes_an_address_that_can_be_typed() {
        assert_eq!(slugify("Vision"), "vision");
        assert_eq!(slugify("2026-09-04 - The tool for projects"), "2026-09-04-the-tool-for-projects");
        // Runs of punctuation collapse rather than leaving a row of hyphens.
        assert_eq!(slugify("What   now?!"), "what-now");
        // A title with no ASCII gives nothing rather than a row of hyphens
        // pretending to be an address.
        assert_eq!(slugify("Видение"), "");
        assert_eq!(slugify("2026-09-04 — Инструмент"), "2026-09-04");
    }

    #[test]
    fn a_backup_name_gives_back_the_moment_it_was_taken() {
        assert_eq!(stamp_of("rigger.v23-20260915-080501.bak").as_deref(), Some("2026-09-15T08:05:01Z"));
        // The suffix that keeps two copies inside one second apart is not
        // part of the moment.
        assert_eq!(stamp_of("rigger.v23-20260915-080501-2.bak").as_deref(), Some("2026-09-15T08:05:01Z"));
        // A stem with dots of its own still parses: the schema is found
        // from the right.
        assert_eq!(stamp_of("my.db.v1-20260101-000000.bak").as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    /// A wrong date here would be reported to the owner as the age of their
    /// insurance, so a name that does not hold a real moment gives nothing.
    #[test]
    fn a_name_that_is_not_a_moment_gives_no_moment() {
        // Right shape, impossible month: shape alone must not be enough.
        assert_eq!(stamp_of("rigger.v1-20261345-080501.bak"), None);
        // Impossible hour, and a day that does not exist in that month.
        assert_eq!(stamp_of("rigger.v1-20260915-250000.bak"), None);
        assert_eq!(stamp_of("rigger.v1-20260231-080501.bak"), None);
        // Not stamped at all.
        assert_eq!(stamp_of("rigger.bak"), None);
        assert_eq!(stamp_of("rigger.v1-notadate-080501.bak"), None);
        // Not a copy.
        assert_eq!(stamp_of("rigger.v1-20260915-080501.db"), None);
    }
}
