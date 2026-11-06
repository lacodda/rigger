//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod activity;
mod adopt;
mod answers;
mod card;
mod commit;
mod context;
mod db;
mod delivery;
mod deps;
#[allow(dead_code)]
mod doc;
mod export;
mod gate;
mod git;
mod go;
mod handoff;
mod hub;
mod hubfile;
mod import;
mod kasl;
mod line;
mod link;
mod mcp;
mod open;
mod owner;
mod paint;
mod paths;
mod profile;
mod queue;
mod repo;
mod search;
mod session;
mod show;
mod skill;
mod sync;
mod tray;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::db::Db;

#[derive(Parser)]
#[command(name = "rigger", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Print what the command shows or wrote as JSON, one document on stdout
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Create the database and the default profile
    Init,
    /// Add, list and show projects
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Read a notes hub into versions, tasks and events
    Import {
        /// Project name
        project: String,
        /// Directory of the hub to read; the one the record knows when omitted
        #[arg(long, conflicts_with = "answers")]
        hub: Option<PathBuf>,
        /// A questionnaire the owner answered, as JSON: its choices become decisions and wishes
        #[arg(long, value_name = "FILE")]
        answers: Option<PathBuf>,
        /// Say what would be recorded, and write nothing
        #[arg(long)]
        check: bool,
    },
    /// Switch, list and add profiles: one record per way of working
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Task cards: make, find and show them, take them from kasl's inbox, give a task a status
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// A card's tray: the material that arrives for a task, and where it goes once sorted
    Tray {
        #[command(subcommand)]
        command: TrayCommand,
    },
    /// Record every repository under a directory, with its hub and its tags
    Adopt {
        /// Directory whose children are repositories; the profile's roots when omitted
        root: Option<PathBuf>,
        /// Directory whose children are hubs, one per project name; the profile's when omitted
        #[arg(long)]
        hubs: Option<PathBuf>,
        /// Say what would be recorded, and write nothing
        #[arg(long)]
        check: bool,
    },
    /// Write a thin project skill from a template and the record
    Skill {
        /// Project name
        #[arg(required_unless_present_any = ["print_template", "line"])]
        project: Option<String>,
        /// Write it into the assistant's skills directory instead of printing it
        #[arg(long)]
        install: bool,
        /// Write it under this directory instead; implies --install
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
        /// Overwrite a skill file that was written by hand
        #[arg(long)]
        replace: bool,
        /// Read the template from this file instead of the data directory
        #[arg(long, value_name = "FILE")]
        template: Option<PathBuf>,
        /// Print the built-in template, to start one of your own from; for a named project, the template of its kind
        #[arg(long)]
        print_template: bool,
        /// Write one skill for the whole line instead of one per project
        #[arg(long, conflicts_with = "project")]
        line: bool,
    },
    /// The project's screen: what it is, where it stands, what it has written
    Show {
        /// Project name
        project: String,
    },
    /// Print what an assistant needs to start a session on a project
    Context {
        /// Project name
        project: String,
        /// Show what each section of the packet costs
        #[arg(long)]
        explain: bool,
        /// Token budget for the packet
        #[arg(long, default_value_t = context::DEFAULT_BUDGET)]
        budget: usize,
    },
    /// Record an event: a decision, a finding, a pitfall, a change, a next step
    Note {
        /// Project name
        project: String,
        /// What happened; a release may leave it out and be described by its flags
        text: Option<String>,
        /// Kind of event
        #[arg(long, value_name = "KIND", default_value = "finding")]
        kind: NoteKind,
        /// The principle this decision stands on
        #[arg(long, value_name = "NAME")]
        principle: Option<String>,
        /// The day a question needs its answer by: 2026-09-25, tomorrow, +3d, friday
        #[arg(long, value_name = "DAY")]
        due: Option<String>,
        /// The tag a release went out under
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
        /// The release was published with its archives
        #[arg(long)]
        release: bool,
        /// A registry the release reached; repeat for each
        #[arg(long = "registry", value_name = "NAME")]
        registries: Vec<String>,
    },
    /// Work on what comes next: the project, its stage, your assistant started on it
    Go {
        /// Work on this project instead of the one that comes next
        project: Option<String>,
        /// Go past the projects left unfinished, to the next one with nothing left over
        #[arg(long)]
        force: bool,
    },
    /// Read tags and commits into facts: what shipped, and what has happened since
    Sync {
        /// Project name; every project when omitted
        project: Option<String>,
    },
    /// Questions waiting for your answer, across every project
    Inbox {
        /// Only this project
        #[arg(long)]
        project: Option<String>,
    },
    /// What moved lately, five lines per project
    Digest {
        /// Project name; every project that moved when omitted
        project: Option<String>,
        /// How far back to look, as days: 7d, 30d
        #[arg(long, default_value = "7d")]
        since: String,
        /// Write the digest into this markdown note - a daily note, say - instead of printing it
        #[arg(long, value_name = "FILE")]
        md: Option<PathBuf>,
    },
    /// Search the record: where was this decided, when was that fixed
    Find {
        /// What to look for; FTS5 syntax, so `budget AND packet` works
        query: String,
        /// Only this project
        #[arg(long)]
        project: Option<String>,
        /// Only this kind of event
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
        /// How many results to show
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// The events that led to a version: what was decided, found and hit
    Why {
        /// Project name; omitted when reading a principle across the line
        project: Option<String>,
        /// Version, as the record spells it
        version: Option<String>,
        /// Read one principle across every project instead of one version
        #[arg(long, value_name = "NAME")]
        principle: Option<String>,
        /// List the principles the record has used
        #[arg(long)]
        principles: bool,
    },
    /// One version whole, and adding stages to the plan
    Version {
        #[command(subcommand)]
        command: VersionCommand,
    },
    /// What comes next: every project in the order it gets a turn, and why
    Queue {
        #[command(subcommand)]
        command: Option<QueueCommand>,
    },
    /// How often each project comes round: `rigger priority sample up`, `rigger priority sample 6`
    Priority {
        /// Project name; every project's weight when omitted
        project: Option<String>,
        /// `up`, `down`, or a weight: 3 comes round three times as often as 1, 0 leaves the queue
        change: Option<String>,
    },
    /// Open and close a sitting, so its events belong together
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Write a hub back out of the record
    Export {
        /// Project name; omitted with --line or --all
        #[arg(required_unless_present_any = ["line", "all"], conflicts_with_all = ["line", "all"])]
        project: Option<String>,
        /// Directory of the hub to write; the one the record knows when omitted
        #[arg(long, conflicts_with_all = ["line", "all"])]
        hub: Option<PathBuf>,
        /// Write every hub the record knows, each where it lies, making the ones that are gone
        #[arg(long, conflicts_with = "line")]
        all: bool,
        /// Write the line's public registry instead of a hub
        #[arg(long)]
        line: bool,
        /// Where the registry goes; standard output when omitted
        #[arg(long, value_name = "FILE", requires = "line")]
        to: Option<PathBuf>,
        /// Say what would change without writing anything
        #[arg(long)]
        check: bool,
        /// Take over files written by hand, so the record owns them from now on
        #[arg(long)]
        adopt: bool,
        /// Also write the handwritten texts and the kept files back out: vision, rituals, research, attachments
        #[arg(long)]
        docs: bool,
    },
    /// Serve the record over MCP, on stdin and stdout
    Mcp,
    /// Read one recorded event whole: a wish, a question, a decision the packet shortened
    Event {
        /// Project name
        project: String,
        /// Id of the event, as the packet lists it in brackets
        id: i64,
    },
    /// Answer a question or sort a wish, so it leaves the packet
    Resolve {
        /// Project name
        project: String,
        /// Id of the question or wish, as the packet lists it
        id: i64,
        /// The answer; a question answered this way becomes a decision
        answer: Option<String>,
        /// A neighbour's order is built, not only sorted into the plan: the neighbour moves up
        #[arg(long, conflicts_with = "answer")]
        built: bool,
    },
    /// Record a wish: something to sort into the plan later
    Wish {
        /// Project name
        project: String,
        /// What you want
        text: String,
        /// The neighbour asking for it, when a wish comes from one
        #[arg(long = "from", value_name = "PROJECT")]
        from_project: Option<String>,
        /// The neighbour's stage is waiting for this: the project asked moves up until it is resolved
        #[arg(long, requires = "from_project")]
        waits: bool,
    },
    /// Tie two projects together: a pair, what one draws on, what draws on it
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
    /// The handwritten texts of a project: vision, rituals, research
    Doc {
        #[command(subcommand)]
        command: DocCommand,
    },
    /// Run the command that says a project is fit to commit, and record how it went
    Gate {
        /// Project name; the one the working directory sits in when omitted
        project: Option<String>,
        /// Print what would be run, and run nothing
        #[arg(long)]
        check: bool,
    },
    /// How the work is done: the rituals of the line, and of one project
    Rules {
        /// Project name; the line's own rituals alone when omitted
        project: Option<String>,
    },
    /// Copy the database aside, stamped with the moment and its schema
    Backup {
        /// How many copies to keep; older ones are deleted
        #[arg(long, default_value_t = KEEP_BACKUPS, value_name = "N")]
        keep: usize,
        /// List the copies instead of taking one
        #[arg(long)]
        list: bool,
    },
    /// Show the database path, schema version and record counts
    Doctor {
        /// Also check the hubs the record generates against what is on disk
        #[arg(long)]
        hubs: bool,
    },
}

/// The kinds a `note` can record.
///
/// A question was once left out, on the grounds that it is addressed to the
/// owner and arrives through the assistant's `ask_owner`. That made MCP the
/// only door to the inbox: a script, an assistant without the server, or the
/// owner at a terminal had no way to put a question there - while `inbox`,
/// which reads them, was a command all along.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum NoteKind {
    /// A decision and its reason
    Decision,
    /// Something learnt about the code or the domain
    Finding,
    /// A trap worth remembering
    Pitfall,
    /// Something that changed in the product
    Change,
    /// The one line the next session starts from
    Next,
    /// A line for the hub's state block: where things stand, in one sentence
    State,
    /// A step of the plan of edits, for a card
    Plan,
    /// A question only the owner can settle; it waits in the inbox
    Question,
    /// What stops the tag and is out of reach: the queue goes on without the project until it is cleared
    Blocker,
    /// A release that went out, as the release engine saw it
    Shipped,
}

impl NoteKind {
    fn as_str(self) -> &'static str {
        match self {
            NoteKind::Decision => "decision",
            NoteKind::Finding => "finding",
            NoteKind::Pitfall => "pitfall",
            NoteKind::Change => "change",
            NoteKind::Next => "next",
            NoteKind::State => "state",
            NoteKind::Plan => "plan",
            NoteKind::Question => "question",
            NoteKind::Blocker => "blocker",
            NoteKind::Shipped => "shipped",
        }
    }
}

/// What a link between two projects says.
///
/// Mirrors `link::Kind` rather than being it: `clap` wants an enum it can
/// derive a value parser on, and the domain type should not have to know
/// what a command line is.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum LinkKind {
    /// Two halves of one capability; neither ships alone
    Pair,
    /// This project draws on the other
    Consumer,
    /// The other project draws on this one
    Donor,
}

impl LinkKind {
    fn to_domain(self) -> link::Kind {
        match self {
            LinkKind::Pair => link::Kind::Pair,
            LinkKind::Consumer => link::Kind::Consumer,
            LinkKind::Donor => link::Kind::Donor,
        }
    }
}

#[derive(Subcommand)]
enum LinkCommand {
    /// Record a tie between two projects, or two of their versions
    ///
    /// Each side is a project, or a project and a version written
    /// `kasl@v1.13.0`. The version is part of one argument rather than a
    /// positional of its own: with two optional versions between two
    /// projects, `rigger link kasl v1.13.0 kasl-server` has two readings
    /// and a command line that guesses would anchor the wrong half.
    Add {
        /// Project on this side, optionally `project@version`
        from: String,
        /// Project on the other side, optionally `project@version`
        to: String,
        /// What the tie is
        #[arg(long, value_name = "KIND", default_value = "pair")]
        kind: LinkKind,
        /// A sentence saying what the two share
        #[arg(long)]
        note: Option<String>,
    },
    /// The ties one project has, read from its side
    List {
        /// Project name; every link of the record when omitted
        project: Option<String>,
    },
    /// Forget a tie, by the id `link list` prints
    Remove {
        /// Id of the link
        id: i64,
    },
    /// The pairs whose halves have parted company
    Drift {},
    /// What the projects build against, read from their manifests
    Uses {
        /// Only what touches this project; every project when omitted
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum DocCommand {
    /// List the documents of a project
    List {
        /// Project name
        project: String,
        /// Only this kind: vision, decisions, research, rituals, other
        #[arg(long)]
        kind: Option<String>,
    },
    /// Print a document
    Show {
        /// Project name
        project: String,
        /// The document's address, as `list` prints it
        slug: String,
    },
    /// Write a new document, in $EDITOR unless a body is given
    Add {
        /// Project name
        project: String,
        /// What it is called
        title: String,
        /// Kind: vision, decisions, research, rituals, other
        #[arg(long, default_value = "other")]
        kind: String,
        /// The address to give it; made from the title when omitted
        #[arg(long)]
        slug: Option<String>,
        /// The body, instead of opening an editor; `-` reads standard input
        #[arg(long)]
        body: Option<String>,
    },
    /// Edit a document in $EDITOR
    Edit {
        /// Project name
        project: String,
        /// The document's address, as `list` prints it
        slug: String,
        /// A new title for it
        #[arg(long)]
        title: Option<String>,
        /// The body, instead of opening an editor; `-` reads standard input
        #[arg(long)]
        body: Option<String>,
    },
    /// Remove a document, or a kept file, from the record
    Remove {
        /// Project name
        project: String,
        /// The document's address, as `list` prints it, or a file's path, as `files` prints it
        slug: String,
    },
    /// List the files the record keeps for a project's hub that are not texts
    Files {
        /// Project name
        project: String,
    },
    /// The skeleton a new document of a kind starts from
    Template {
        /// Kind: vision, decisions, research, rituals, other
        kind: String,
        /// Write it to the profile's directory, to edit into your own
        #[arg(long)]
        write: bool,
    },
}

#[derive(Subcommand)]
enum ProfileCommand {
    /// List the profiles, marking the one in use
    List {},
    /// Show one profile; the one in use when no name is given
    Show {
        /// Profile name
        name: Option<String>,
    },
    /// Make a profile the one every command uses
    Use {
        /// Profile name
        name: String,
    },
    /// Add a profile, with a database of its own
    Add {
        /// Profile name
        name: String,
        /// What the unit of work is
        #[arg(long, value_enum, default_value_t = profile::Kind::Line)]
        kind: profile::Kind,
        /// A directory whose children are repositories; may be given more than once
        #[arg(long = "root", value_name = "DIR")]
        roots: Vec<PathBuf>,
        /// The directory whose children are hubs, one per project name
        #[arg(long, value_name = "DIR")]
        hubs: Option<PathBuf>,
        /// How a ticket id is spelt, as a regular expression
        #[arg(long, value_name = "REGEX")]
        id_pattern: Option<String>,
        /// Where the trays of the cards are kept; `trays/` in the profile's directory when omitted
        #[arg(long, value_name = "DIR")]
        trays: Option<PathBuf>,
        /// Switch to it right away
        #[arg(long)]
        r#use: bool,
    },
    /// Change what a profile says about itself; a field given replaces what it had
    Set {
        /// Profile name; the one in use when omitted
        name: Option<String>,
        /// A directory whose children are repositories; may be given more than once, replaces the roots
        #[arg(long = "root", value_name = "DIR")]
        roots: Vec<PathBuf>,
        /// The directory whose children are hubs, one per project name
        #[arg(long, value_name = "DIR")]
        hubs: Option<PathBuf>,
        /// How a ticket id is spelt, as a regular expression
        #[arg(long, value_name = "REGEX")]
        id_pattern: Option<String>,
        /// Where the trays of the cards are kept; `trays/` in the profile's directory when omitted
        #[arg(long, value_name = "DIR")]
        trays: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Make a card for a task: a ticket id, or a local key until one is given
    New {
        /// The task's title
        title: String,
        /// Ticket id, such as ACME-7310; a local key is made when omitted
        #[arg(long = "id", value_name = "KEY")]
        key: Option<String>,
        /// Another name the task goes by; may be given more than once
        #[arg(long = "alias", value_name = "TEXT")]
        aliases: Vec<String>,
        /// A project the task is worked in; may be given more than once
        #[arg(long = "project", value_name = "NAME")]
        projects: Vec<String>,
        /// The branch it is worked on, in every project given
        #[arg(long)]
        branch: Option<String>,
        /// What the task is, in one paragraph
        #[arg(long)]
        summary: Option<String>,
    },
    /// Find the card a line of text means: by id, by title, by case number
    Find {
        /// The task as it was handed over: id, title, numbers, any of them
        query: String,
    },
    /// Make a card the open one, so that the record knows what is in hand
    Open {
        /// Card key, alias or id
        task: String,
    },
    /// The card in hand, if one is open
    Active {},
    /// Close a card: done by default, or the status given
    Close {
        /// Card key, alias or id; the open card when omitted
        task: Option<String>,
        /// The status to leave it in
        #[arg(long, default_value = "done")]
        status: String,
    },
    /// Show a card: what it is, where it is worked, what was written
    Show {
        /// Card key, alias or id
        task: String,
    },
    /// The packet an assistant starts a task from
    Context {
        /// Card key, alias or id
        task: String,
        /// Token budget for the packet
        #[arg(long, default_value_t = context::DEFAULT_BUDGET)]
        budget: usize,
    },
    /// Link a card to a project, with the branch it is worked on there
    Link {
        /// Card key, alias or id
        task: String,
        /// Project name
        project: String,
        /// The branch
        #[arg(long)]
        branch: Option<String>,
        /// What the project is to the task: where it is fixed, or only read
        #[arg(long)]
        role: Option<String>,
    },
    /// List cards
    List {
        /// open (the default), all, or one status
        #[arg(long, default_value = "open")]
        status: String,
        /// Include the cards that are asleep, each with the day it is due back
        #[arg(long)]
        snoozed: bool,
    },
    /// Give a card a new key; the old one stays as an alias
    Rename {
        /// Card key, alias or id
        task: String,
        /// The new key
        key: String,
    },
    /// Add a name a card goes by
    Alias {
        /// Card key, alias or id
        task: String,
        /// The alias
        alias: String,
    },
    /// Say what a card is, in one paragraph
    Summary {
        /// Card key, alias or id
        task: String,
        /// The paragraph
        text: String,
    },
    /// Record an event against a card: a decision, a finding, a pitfall, a plan step, a change, the next step
    Note {
        /// Card key, alias or id
        task: String,
        /// What happened
        text: String,
        /// Kind of event
        #[arg(long, value_name = "KIND", default_value = "finding")]
        kind: NoteKind,
    },
    /// Give a task a status: new, active, waiting-handoff, frozen or done
    Status {
        /// Card key, alias, or the id the packet or `plan` lists
        task: String,
        /// The status
        status: String,
    },
    /// Put a task out of sight until a day: out of the packet, the list and the hand
    Snooze {
        /// Card key, alias, or the id the packet or `plan` lists
        task: String,
        /// The day it comes back: 2026-10-05, monday, +3d, 3d, 2w
        #[arg(long, value_name = "DAY")]
        until: String,
    },
    /// Wake a sleeping task now
    Unsnooze {
        /// Card key, alias, or the id the packet or `plan` lists
        task: String,
    },
    /// Set a task aside on purpose, saying why; `task status <task> active` takes it up again
    Freeze {
        /// Card key, alias, or the id the packet or `plan` lists
        task: String,
        /// Why it is set aside; recorded as a decision against it
        #[arg(long, value_name = "TEXT")]
        why: Option<String>,
    },
    /// Write a text for the team that owns the fix, and hand the card over to it
    Handoff {
        /// Card key, alias or id; the open card when omitted
        task: Option<String>,
        /// Who it goes to: a product or a team, as one word
        #[arg(long, value_name = "WHO")]
        to: String,
    },
    /// The tickets in kasl's inbox that have no card, and the cards whose ticket has gone
    Incoming {
        /// Every ticket without a card, not only the first ones
        #[arg(long)]
        all: bool,
    },
    /// Make a card from a ticket in kasl's inbox: its key and title come from there
    Take {
        /// The ticket's key, such as ACME-7310
        key: String,
        /// A project the task is worked in; may be given more than once
        #[arg(long = "project", value_name = "NAME")]
        projects: Vec<String>,
        /// The branch it is worked on, in every project given
        #[arg(long)]
        branch: Option<String>,
    },
}

#[derive(Subcommand)]
enum TrayCommand {
    /// The tray of a card: its form and its files; made, with a blank form, the first time
    Show {
        /// Card key, alias or id; the card in hand when omitted
        task: Option<String>,
    },
    /// Open the tray's form in $EDITOR
    Edit {
        /// Card key, alias or id; the card in hand when omitted
        task: Option<String>,
    },
    /// Copy a folder's files - a tester's, a share's - into the tray
    Fetch {
        /// Card key, alias or id; the card in hand when omitted
        task: Option<String>,
        /// The folder or file to copy; may be given more than once. The folders the form lists under "Links to files" when omitted
        #[arg(long = "from", value_name = "PATH")]
        from: Vec<PathBuf>,
        /// Leave behind files larger than this, in megabytes
        #[arg(long, value_name = "MB", default_value_t = tray::MAX_MB)]
        max_mb: u64,
        /// Take recordings too; they are listed and left where they are otherwise
        #[arg(long)]
        video: bool,
    },
    /// Take the screenshots, downloads and desktop files written lately
    Intake {
        /// Card key, alias or id; the card in hand when omitted
        task: Option<String>,
        /// How far back: 4h, 90m, 1d, or a number of hours
        #[arg(long, default_value = "4h")]
        since: String,
        /// Leave behind files larger than this, in megabytes
        #[arg(long, value_name = "MB", default_value_t = tray::MAX_MB)]
        max_mb: u64,
        /// Take recordings too; they are listed and left where they are otherwise
        #[arg(long)]
        video: bool,
    },
    /// Sorting is over: the material goes to the card's archive, and the form back to blank
    Done {
        /// Card key, alias or id; the card in hand when omitted
        task: Option<String>,
    },
    /// Every tray with something waiting in it
    List {},
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// Record a repository as a project, or a folder of work with no repository
    Add {
        /// Path to the repository root, or to the folder the work lives in
        path: PathBuf,
        /// Project name; defaults to the name the repository declares
        #[arg(long)]
        name: Option<String>,
    },
    /// Record a place the record keeps for itself, with no repository
    Service {
        /// Project name
        name: String,
    },
    /// List recorded projects
    List {},
    /// Show one project
    Show {
        /// Project name
        name: String,
    },
    /// Set what the record keeps about a project
    Set {
        /// Project name
        name: String,
        /// The command that says the project is fit to commit, as a shell would run it
        #[arg(long, value_name = "COMMAND")]
        gate: Option<String>,
        /// Forget the gate
        #[arg(long, conflicts_with = "gate")]
        no_gate: bool,
        /// The command to run when a sitting on this project closes
        #[arg(long, value_name = "COMMAND")]
        on_session_end: Option<String>,
        /// Forget the end-of-session command
        #[arg(long, conflicts_with = "on_session_end")]
        no_on_session_end: bool,
        /// One line about the project, for one whose manifest says nothing
        #[arg(long, value_name = "TEXT")]
        about: Option<String>,
        /// Forget the line about the project
        #[arg(long, conflicts_with = "about")]
        no_about: bool,
    },
    /// Record how a product looks from outside: its mark, colours, form and docs
    Mark {
        /// Project name
        name: String,
        /// The two-letter code of its mark
        #[arg(long)]
        code: Option<String>,
        /// The colour it owns, as #RRGGBB
        #[arg(long)]
        accent: Option<String>,
        /// The second colour, for a mark drawn as a pair
        #[arg(long)]
        accent2: Option<String>,
        /// What shape of thing it is: cli, desktop, web, library, service
        #[arg(long)]
        form: Option<String>,
        /// Where its documentation lives
        #[arg(long, value_name = "URL")]
        docs: Option<String>,
        /// Forget the mark entirely
        #[arg(long, conflicts_with_all = ["code", "accent", "accent2", "form", "docs"])]
        clear: bool,
    },
    /// Take a project out of the work for good, keeping its history readable
    Retire {
        /// Project name
        name: String,
        /// Why it leaves the work; recorded as a decision against it
        #[arg(long, value_name = "TEXT")]
        why: String,
    },
    /// Bring a retired project back into the work
    Unretire {
        /// Project name
        name: String,
        /// Why it comes back; recorded as a decision against it
        #[arg(long, value_name = "TEXT")]
        why: Option<String>,
    },
}

#[derive(Subcommand)]
enum SessionCommand {
    /// Open a sitting; everything recorded until `end` belongs to it
    Start {
        /// Project name; the project of the working directory when omitted
        project: Option<String>,
    },
    /// Close the sitting and say what it held
    End {
        /// Project name; the project of the working directory when omitted
        project: Option<String>,
        /// A title for the diary entry, if one is being written
        #[arg(long, value_name = "TEXT")]
        heading: Option<String>,
        /// Put the entry into this diary file: one kept by hand gets it at the top, a generated one is written from the record
        #[arg(long, value_name = "FILE")]
        diary: Option<PathBuf>,
        /// Use this entry instead of the composed one - an edited `session draft`; `-` reads standard input
        #[arg(long, value_name = "FILE")]
        entry: Option<PathBuf>,
        /// Say nothing unless something is worth saying, for a hook
        #[arg(long)]
        remind: bool,
    },
    /// Show the diary entry the open sitting would leave, to edit before `end`
    Draft {
        /// Project name; the project of the working directory when omitted
        project: Option<String>,
        /// A title for the entry
        #[arg(long, value_name = "TEXT")]
        heading: Option<String>,
    },
}

#[derive(Subcommand)]
enum VersionCommand {
    /// One version: its number, the week it is aimed at, how far it got, its tasks
    Show {
        /// Project name
        project: String,
        /// Version, as the record spells it; the stage being built when omitted
        version: Option<String>,
    },
    /// Add a stage to the plan, with its tasks; or tasks to a stage already there
    Add {
        /// Project name
        project: String,
        /// Version, such as v0.3.0
        version: String,
        /// What the stage is about, in a few words
        #[arg(long)]
        title: Option<String>,
        /// A task of the stage; may be given more than once
        #[arg(long = "task", value_name = "TEXT")]
        tasks: Vec<String>,
    },
}

#[derive(Subcommand)]
enum QueueCommand {
    /// Put a project first, until its next release
    First {
        /// Project name
        project: String,
        /// Why it cannot wait for its turn
        #[arg(long)]
        why: Option<String>,
    },
    /// Put a project back in its turn: what moved it up no longer holds
    Back {
        /// Project name
        project: String,
    },
}

/// The stack the work runs on.
///
/// Windows gives the main thread 1 MB, and clap's derived parser walks the
/// command tree with one frame per level. Unoptimised frames are several
/// times fatter than optimised ones, so a tree this size overflowed that
/// megabyte in debug builds while release was fine - `rigger --version`
/// died before reaching any code of ours. Tests run debug binaries, so
/// this was every test, not a corner.
///
/// Asking for the stack rather than flattening the commands: the tree is
/// the product's surface, and it should be free to grow.
const STACK: usize = 16 * 1024 * 1024;

fn main() -> ExitCode {
    quiet_on_a_closed_pipe();
    // The default thread stack is what `main` gets; a spawned one takes
    // the size it is given, on every platform rigger ships to.
    match std::thread::Builder::new().stack_size(STACK).spawn(work).map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        // A panic has already printed itself; exiting with the code a
        // panicking process uses keeps that unchanged.
        Ok(Err(_)) => ExitCode::from(101),
        Err(e) => {
            eprintln!("error: cannot start the working thread: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A reader that stops reading is not a failure of the writer.
///
/// `rigger doctor | head -3` closes the pipe after three lines, and the next
/// `println!` panics on it - "failed printing to stdout ... os error 232" on
/// Windows, a broken pipe elsewhere - so a command that did all its work
/// ended in a panic and code 101. git and coreutils go quiet and exit 0
/// there, and so does rigger: once, for the whole binary, in the panic
/// hook, rather than in each of the hundreds of places that print.
fn quiet_on_a_closed_pipe() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or_default();
        if is_a_closed_pipe(message) {
            std::process::exit(0);
        }
        report(info);
    }));
}

/// Whether a panic is the standard library failing to print into a pipe
/// whose reader has gone. Told by the error's number, never its words: the
/// words are the system's and come in its language ("Канал был закрыт" on a
/// Russian Windows). EPIPE is 32 on Unix; on Windows a closed pipe is 109,
/// one being closed 232, and one with nobody at the other end 233.
fn is_a_closed_pipe(message: &str) -> bool {
    message.starts_with("failed printing to stdout")
        && ["(os error 32)", "(os error 109)", "(os error 232)", "(os error 233)"]
            .iter()
            .any(|code| message.ends_with(code))
}

/// The commands a person types. The rest are how an assistant reads and writes
/// the record: they work the same, and `--help --all` lists them.
const OWNER_COMMANDS: [&str; 8] = ["go", "queue", "priority", "inbox", "init", "project", "doctor", "help"];

/// The command line, with the help showing what a person needs of it.
fn command_line() -> clap::Command {
    Cli::command()
        .mut_subcommands(|command| {
            let hide = !OWNER_COMMANDS.contains(&command.get_name());
            command.hide(hide)
        })
        .after_help("`rigger --help --all` lists the commands an assistant uses.")
}

fn work() -> ExitCode {
    // Only as `rigger --help --all`: `--all` means other things to the commands
    // that take it.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args.iter().any(|a| a == "--all") && args.iter().any(|a| a == "--help" || a == "-h") {
        return match Cli::command().print_help() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let cli = match command_line().try_get_matches().and_then(|matches| Cli::from_arg_matches(&matches)) {
        Ok(cli) => cli,
        Err(err) => return usage_error(err),
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Prints what clap wants to say, and chooses the exit code.
///
/// `--help` and `--version` are successes; a usage error is a failure. What
/// matters is *which* failure: clap's own code is 2, and 2 is the code an
/// assistant's Stop hook uses to refuse the stop and hold the turn open. A
/// hook is a command line written once in a settings file and never seen
/// again - a typo in it, or an older rigger on the PATH without the
/// subcommand, would wedge every session it fired in. Found by installing
/// the hook and running it: the rigger on PATH was a release behind, and
/// `rigger session end --remind` exited 2.
///
/// So rigger never exits 2. A usage error is exit 1 like every other
/// failure, and a hook that cannot be understood is simply ignored.
fn usage_error(err: clap::Error) -> ExitCode {
    let _ = err.print();
    match err.use_stderr() {
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    }
}

fn run(cli: Cli) -> Result<()> {
    let json = cli.json;
    let Some(command) = cli.command else {
        return bare(json);
    };
    match command {
        Command::Init => init(json),
        Command::Project { command } => match command {
            ProjectCommand::Add { path, name } => project_add(path, name, json),
            ProjectCommand::Service { name } => project_service(&name, json),
            ProjectCommand::List {} => project_list(json),
            ProjectCommand::Show { name } => project_show(&name, json),
            ProjectCommand::Set {
                name,
                gate,
                no_gate,
                on_session_end,
                no_on_session_end,
                about,
                no_about,
            } => project_set(
                &name,
                &Settings {
                    gate: gate.as_deref(),
                    no_gate,
                    on_session_end: on_session_end.as_deref(),
                    no_on_session_end,
                    about: about.as_deref(),
                    no_about,
                },
                json,
            ),
            ProjectCommand::Mark {
                name,
                code,
                accent,
                accent2,
                form,
                docs,
                clear,
            } => project_mark(
                &name,
                db::Mark {
                    code,
                    accent,
                    accent2,
                    form,
                    docs_url: docs,
                },
                clear,
                json,
            ),
            ProjectCommand::Retire { name, why } => project_retire(&name, &why, json),
            ProjectCommand::Unretire { name, why } => project_unretire(&name, why.as_deref(), json),
        },
        Command::Import { project, hub, answers, check } => match answers {
            Some(answers) => import_answers(&project, &answers, check, json),
            None => import_hub(&project, hub.as_deref(), check, json),
        },
        Command::Profile { command } => match command {
            ProfileCommand::List {} => profile_list(json),
            ProfileCommand::Show { name } => profile_show(name.as_deref(), json),
            ProfileCommand::Use { name } => profile_use(&name, json),
            ProfileCommand::Add {
                name,
                kind,
                roots,
                hubs,
                id_pattern,
                trays,
                r#use,
            } => profile_add(
                &name,
                profile::Profile {
                    kind,
                    roots,
                    hubs,
                    id_pattern,
                    trays,
                },
                r#use,
                json,
            ),
            ProfileCommand::Set {
                name,
                roots,
                hubs,
                id_pattern,
                trays,
            } => profile_set(name.as_deref(), roots, hubs, id_pattern, trays, json),
        },
        Command::Task { command } => match command {
            TaskCommand::New {
                title,
                key,
                aliases,
                projects,
                branch,
                summary,
            } => task_new(&title, key.as_deref(), &aliases, &projects, branch.as_deref(), summary.as_deref(), json),
            TaskCommand::Find { query } => task_find(&query, json),
            TaskCommand::Open { task } => task_open(&task, json),
            TaskCommand::Active {} => task_active(json),
            TaskCommand::Close { task, status } => task_close(task.as_deref(), &status, json),
            TaskCommand::Show { task } => task_show(&task, json),
            TaskCommand::Context { task, budget } => task_context(&task, budget, json),
            TaskCommand::Link { task, project, branch, role } => task_link(&task, &project, branch.as_deref(), role.as_deref(), json),
            TaskCommand::List { status, snoozed } => task_list(&status, snoozed, json),
            TaskCommand::Rename { task, key } => task_rename(&task, &key, json),
            TaskCommand::Alias { task, alias } => task_alias(&task, &alias, json),
            TaskCommand::Summary { task, text } => task_summary(&task, &text, json),
            TaskCommand::Note { task, text, kind } => note_on_card(&task, kind.as_str(), &text, json),
            TaskCommand::Status { task, status } => task_status(&task, &status, json),
            TaskCommand::Snooze { task, until } => task_snooze(&task, Some(&until), json),
            TaskCommand::Unsnooze { task } => task_snooze(&task, None, json),
            TaskCommand::Freeze { task, why } => task_freeze(&task, why.as_deref(), json),
            TaskCommand::Handoff { task, to } => task_handoff(task.as_deref(), &to, json),
            TaskCommand::Incoming { all } => task_incoming(all, json),
            TaskCommand::Take { key, projects, branch } => task_take(&key, &projects, branch.as_deref(), json),
        },
        Command::Tray { command } => match command {
            TrayCommand::Show { task } => tray_show(task.as_deref(), json),
            TrayCommand::Edit { task } => tray_edit(task.as_deref(), json),
            TrayCommand::Fetch { task, from, max_mb, video } => tray_fetch(task.as_deref(), &from, tray_rules(max_mb, video), json),
            TrayCommand::Intake { task, since, max_mb, video } => tray_intake(task.as_deref(), &since, tray_rules(max_mb, video), json),
            TrayCommand::Done { task } => tray_done(task.as_deref(), json),
            TrayCommand::List {} => tray_list(json),
        },
        Command::Adopt { root, hubs, check } => adopt_root(root.as_deref(), hubs.as_deref(), check, json),
        Command::Skill {
            project,
            install,
            dir,
            replace,
            template,
            print_template,
            line,
        } => {
            let install = install || dir.is_some();
            if line {
                write_line_skill(install, dir.as_deref(), replace, template.as_deref(), print_template, json)
            } else {
                write_skill(project.as_deref(), install, dir.as_deref(), replace, template.as_deref(), print_template, json)
            }
        }
        Command::Show { project } => show_project(&project, json),
        Command::Context { project, explain, budget } => show_context(&project, json, explain, budget),
        Command::Go { project, force } => go_on(project.as_deref(), force, json),
        Command::Note {
            project,
            text,
            kind,
            principle,
            due,
            tag,
            release,
            registries,
        } => {
            // The flags that belong to one kind are refused on the others,
            // so that a `--due` on a finding is a mistake said out loud
            // rather than a deadline silently dropped.
            if due.is_some() && kind != NoteKind::Question {
                bail!("--due belongs to a question; this is a {}", kind.as_str());
            }
            if kind != NoteKind::Shipped && (tag.is_some() || release || !registries.is_empty()) {
                bail!("--tag, --release and --registry describe a release; write --kind shipped");
            }
            match kind {
                NoteKind::Shipped => {
                    let tag = tag.context("a release is recorded by its tag: --tag v0.23.0")?;
                    note_shipped(&project, &tag, release, &registries, text.as_deref(), json)
                }
                _ => {
                    let text = text.with_context(|| format!("say what the {} is", kind.as_str()))?;
                    note(&project, kind.as_str(), &text, principle.as_deref(), None, due.as_deref(), json)
                }
            }
        }
        Command::Sync { project } => sync_projects(project.as_deref(), json),
        Command::Inbox { project } => inbox(project.as_deref(), json),
        Command::Digest { project, since, md } => digest(project.as_deref(), &since, md.as_deref(), json),
        Command::Find { query, project, kind, limit } => find(&query, project.as_deref(), kind.as_deref(), limit, json),
        Command::Why {
            project,
            version,
            principle,
            principles,
        } => why(project.as_deref(), version.as_deref(), principle.as_deref(), principles, json),
        Command::Version { command } => match command {
            VersionCommand::Show { project, version } => version_show(&project, version.as_deref(), json),
            VersionCommand::Add {
                project,
                version,
                title,
                tasks,
            } => version_add(&project, &version, title.as_deref(), &tasks, json),
        },
        Command::Queue { command } => match command {
            None => show_queue(json),
            Some(QueueCommand::First { project, why }) => queue_first(&project, why.as_deref(), json),
            Some(QueueCommand::Back { project }) => queue_back(&project, json),
        },
        Command::Priority { project, change } => priority(project.as_deref(), change.as_deref(), json),
        Command::Session { command } => match command {
            SessionCommand::Start { project } => session_start(project.as_deref(), json),
            SessionCommand::End {
                project,
                heading,
                diary,
                entry,
                remind,
            } => session_end(project.as_deref(), heading.as_deref(), diary.as_deref(), entry.as_deref(), remind, json),
            SessionCommand::Draft { project, heading } => session_draft(project.as_deref(), heading.as_deref(), json),
        },
        Command::Export {
            project,
            hub,
            all,
            line,
            to,
            check,
            adopt,
            docs,
        } => match (line, all) {
            (true, _) => export_line(to.as_deref(), check, json),
            (false, true) => export_all(check, adopt, docs, json),
            (false, false) => export_hub(
                &project.expect("clap requires a project without --line or --all"),
                hub.as_deref(),
                check,
                adopt,
                docs,
                json,
            ),
        },
        Command::Mcp => mcp::serve(),
        Command::Event { project, id } => show_event(&project, id, json),
        Command::Resolve { project, id, answer, built } => resolve(&project, id, answer.as_deref(), built, json),
        Command::Wish {
            project,
            text,
            from_project,
            waits,
        } => wish(&project, &text, from_project.as_deref(), waits, json),
        Command::Link { command } => match command {
            LinkCommand::Add { from, to, kind, note } => link_add(&from, &to, kind, note.as_deref(), json),
            LinkCommand::List { project } => link_list(project.as_deref(), json),
            LinkCommand::Remove { id } => link_remove(id, json),
            LinkCommand::Drift {} => link_drift(json),
            LinkCommand::Uses { project } => link_uses(project.as_deref(), json),
        },
        Command::Doc { command } => match command {
            DocCommand::List { project, kind } => doc_list(&project, kind.as_deref(), json),
            DocCommand::Show { project, slug } => doc_show(&project, &slug, json),
            DocCommand::Add {
                project,
                title,
                kind,
                slug,
                body,
            } => doc_add(&project, &title, &kind, slug.as_deref(), body.as_deref(), json),
            DocCommand::Edit { project, slug, title, body } => doc_edit(&project, &slug, title.as_deref(), body.as_deref(), json),
            DocCommand::Remove { project, slug } => doc_remove(&project, &slug, json),
            DocCommand::Files { project } => doc_files(&project, json),
            DocCommand::Template { kind, write } => doc_template(&kind, write, json),
        },
        Command::Gate { project, check } => gate(project.as_deref(), check, json),
        Command::Rules { project } => rules(project.as_deref(), json),
        Command::Backup { keep, list } => backup(keep, list, json),
        Command::Doctor { hubs } => doctor(hubs, json),
    }
}

/// Reads a questionnaire the owner answered into the record.
///
/// The page that produced it is where these answers otherwise stay, and a
/// plan made from them says what was decided without saying what it was
/// decided against - which is the half that answers "why this and not that"
/// a year later.
fn import_answers(project: &str, path: &Path, check: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    let answers = answers::read(path)?;
    let entries = answers.entries();
    let at = answers.at().unwrap_or_else(db::now);

    if check {
        if json {
            let entries: Vec<serde_json::Value> = entries.iter().map(|e| serde_json::json!({ "kind": e.kind, "body": e.body })).collect();
            return print_json(&serde_json::json!({ "project": project.name, "would_take": entries }));
        }
        println!(
            "{} would take {} from {}:",
            project.name,
            plural(entries.len(), "event", "events"),
            path.display()
        );
        for entry in &entries {
            println!("  {:<9} {}", entry.kind, first_line(&entry.body));
        }
        return Ok(());
    }

    let mut added = 0usize;
    let mut already = 0usize;
    for entry in &entries {
        match db.record_event(project.id, entry.kind, &entry.body, &at, "owner")? {
            db::Change::Unchanged => already += 1,
            _ => added += 1,
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": project.name,
                "added": added,
                "already_recorded": already,
            }))?
        );
        return Ok(());
    }
    println!("{}:", project.name);
    println!("  {:<10} {added} added", "answers");
    if already > 0 {
        // Reading the same page twice is ordinary - it is how a correction
        // to it is applied - and saying so is how that stays ordinary.
        println!("  {:<10} {already} already recorded", "");
    }
    Ok(())
}

/// The hub a command reads or writes when none is named: the one the record
/// knows, or - for the line's own place - the directory the profile keeps
/// every hub in, which is where the line's own files lie.
fn recorded_hub(project: &db::Project) -> Result<PathBuf> {
    if let Some(dir) = &project.hub_path {
        return Ok(PathBuf::from(dir));
    }
    if !hubfile::has_record_files(project) {
        let config = profile::Config::load()?;
        if project.name == config.current_name()
            && let Some(hubs) = config.current()?.1.hubs.clone()
        {
            return Ok(hubs);
        }
    }
    bail!("the record knows no hub for {}; name one with --hub", project.name)
}

fn import_hub(project: &str, hub_dir: Option<&Path>, check: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let hub_dir = match hub_dir {
        Some(dir) => dir.to_path_buf(),
        None => recorded_hub(&project)?,
    };
    // A retired project is read for its texts and files alone: its history
    // is closed, and the schema refuses new stages, sittings and events for
    // it - but what it wrote is not new history, it is the history kept.
    // The line's own place, the same: it has no plan to read.
    let reading = hubfile::reading_for(&db, &project, &hub_dir)?;
    let hub = hub::read_with(&hub_dir, &reading)?;
    // Where the hub is, so a later check can find it. Not guessed from the
    // repository path: the hubs of this line live in a notes vault. Spelt
    // the way the platform spells it, not the way the shell happened to.
    let hub_dir = &dunce::canonicalize(&hub_dir).unwrap_or(hub_dir);
    let run = |db: &Db| {
        // Only when the record knows no hub yet. Reading a directory is not
        // moving the hub there: an import from a scratch copy, or from a
        // folder holding one file to take in, made that folder the
        // project's hub until the next export, and every later check read
        // the wrong place.
        if project.hub_path.is_none() {
            db.set_hub_path(project.id, hub_dir)?;
        }
        import::import(db, project.id, &hub)
    };
    let mut report = match check {
        true => db.rehearse(run)?,
        false => run(&db)?,
    };
    report.texts_only = reading.texts_only;
    // A hub inside this one that no project names is left out of it, and
    // said: it is somebody's hub, not this place's texts.
    if !hubfile::has_record_files(&project) {
        for stray in hubfile::unrecorded_hubs(hub_dir, &hubfile::recorded_inside(&db, &project, hub_dir)?) {
            report.warnings.push(format!(
                "{} holds a plan or a changelog and no project names it as its hub; left out - `rigger import <project> --hub {}` reads it",
                stray.display(),
                stray.display()
            ));
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if reading.texts_only {
        match &project.retired_at {
            Some(at) => println!(
                "note: {} is retired since {}: only its texts and files were read",
                project.name,
                at.split('T').next().unwrap_or(at)
            ),
            None => println!("note: {} is the line's own place: only its texts and files were read", project.name),
        }
    }
    for warning in &report.warnings {
        println!("note: {warning}");
    }
    if !report.untouched.is_empty() {
        println!("note: not read back, still what the record wrote: {}", report.untouched.join(", "));
    }
    if !report.changed() {
        println!("{}: nothing changed", project.name);
        return Ok(());
    }
    match check {
        true => println!("{} - what an import would change; nothing was written:", project.name),
        false => println!("{}:", project.name),
    }
    let line = |label: &str, added: u32, updated: u32| {
        if added + updated > 0 {
            println!("  {label:<10} {added} added, {updated} updated");
        }
    };
    line("versions", report.versions_added, report.versions_updated);
    line("tasks", report.tasks_added, report.tasks_updated);
    if report.versions_dropped + report.tasks_dropped > 0 {
        println!(
            "  {:<10} {} and {} struck from the hub",
            "dropped",
            plural(report.versions_dropped as usize, "version", "versions"),
            plural(report.tasks_dropped as usize, "task", "tasks")
        );
    }
    if report.questions_withdrawn > 0 {
        println!(
            "  {:<10} {} struck from the hub",
            "withdrawn",
            plural(report.questions_withdrawn as usize, "question", "questions")
        );
    }
    if report.decisions_added > 0 {
        println!("  {:<10} {} added", "decisions", report.decisions_added);
    }
    if report.questions_added > 0 {
        println!("  {:<10} {} added", "questions", report.questions_added);
    }
    if report.wishes_added > 0 {
        println!("  {:<10} {} taken out of {}", "wishes", report.wishes_added, hub::WISHES_FILE);
    }
    line("diary", report.diary_added, report.diary_updated);
    line("documents", report.documents_added, report.documents_updated);
    line("files", report.files_added, report.files_updated);
    Ok(())
}

/// Records every checkout under a directory, and reads each one's hub and
/// tags - the three commands a project used to take, once for the line.
///
/// Told nothing, it walks the roots the profile names, with the profile's
/// hubs: a line that has said once where it keeps things need not say so
/// again every time it has grown.
fn adopt_root(root: Option<&Path>, hubs: Option<&Path>, check: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let config = profile::Config::load()?;
    let (name, current) = config.current()?;
    let roots: Vec<PathBuf> = match root {
        Some(root) => vec![root.to_path_buf()],
        None if current.roots.is_empty() => {
            bail!("profile '{name}' names no roots; give a directory, or set one with `rigger profile set {name} --root <dir>`")
        }
        None => current.roots.clone(),
    };
    let hubs = hubs.or(current.hubs.as_deref());
    let mut adopted = Vec::new();
    for root in &roots {
        adopted.extend(adopt::adopt(&db, root, hubs, check)?);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&adopted)?);
        return Ok(());
    }
    let width = adopted.iter().map(|a| a.name.len()).max().unwrap_or(0);
    let mut recorded = 0;
    let mut known = 0;
    let mut without = 0;
    let mut skipped = 0;
    let mut hubs_read = 0;
    for a in &adopted {
        let status = match (&a.status, check) {
            (adopt::Status::Recorded, true) => "would record",
            (adopt::Status::Recorded, false) => "recorded",
            (adopt::Status::Known, _) => "known",
            (adopt::Status::NoHub, _) => "no hub",
            (adopt::Status::Skipped(_), _) => "skipped",
        };
        match &a.status {
            adopt::Status::Recorded => recorded += 1,
            adopt::Status::Known => known += 1,
            adopt::Status::NoHub => without += 1,
            adopt::Status::Skipped(_) => skipped += 1,
        }
        let hub = match (&a.hub, &a.status, check) {
            (_, adopt::Status::NoHub, _) => String::new(),
            (None, _, _) => "hub: none".to_string(),
            (Some(_), adopt::Status::Skipped(_), _) | (Some(_), _, true) => "hub: found".to_string(),
            (Some(_), _, false) => {
                hubs_read += 1;
                a.hub_summary()
            }
        };
        let git = match (&a.status, check) {
            (adopt::Status::Skipped(_) | adopt::Status::NoHub, _) | (_, true) => String::new(),
            _ if a.shipped + a.changes_read == 0 => "   git: nothing new".to_string(),
            _ => format!(
                "   git: {} shipped, {} read",
                plural(a.shipped as usize, "version", "versions"),
                plural(a.changes_read as usize, "change", "changes")
            ),
        };
        println!("{:width$}  {status:<12} {hub}{git}", a.name);
        if let adopt::Status::Skipped(reason) = &a.status {
            println!("{:width$}  {reason}", "");
        }
        for warning in &a.warnings {
            println!("{:width$}  note: {warning}", "");
        }
    }
    let total = adopted.len();
    let verb = if check { "would be recorded" } else { "recorded" };
    let without = match without {
        0 => String::new(),
        n => format!(", {n} without a hub"),
    };
    println!(
        "\n{}: {recorded} {verb}, {known} known{without}, {skipped} skipped; {} read.",
        plural(total, "repository", "repositories"),
        plural(hubs_read, "hub", "hubs")
    );
    if check {
        println!("Nothing was written. Run again without --check to record them.");
    }
    Ok(())
}

/// Writes a project's skill from the template and the record.
///
/// Printed unless asked to install, so the first run shows what a skill
/// will say before anything is overwritten. A file somebody wrote by hand
/// is not replaced without `--replace`: what it holds may belong in the hub
/// first, and the mark is how the next run knows the file is rigger's.
/// What a skill command did, as `--json` prints it.
fn print_skill_json(path: Option<&Path>, action: &str, text: &str) -> Result<()> {
    print_json(&serde_json::json!({ "path": path, "action": action, "text": text }))
}

fn write_skill(
    project: Option<&str>,
    install: bool,
    dir: Option<&Path>,
    replace: bool,
    template: Option<&Path>,
    print_template: bool,
    json: bool,
) -> Result<()> {
    if print_template {
        // The template of the kind the named project is; a repository's when
        // none is named, since that is what most of a line is.
        let built_in = match project {
            Some(name) => skill::built_in(open_project(&Db::open(&paths::db_path()?)?, name)?.kind)?,
            None => skill::DEFAULT_TEMPLATE,
        };
        if json {
            return print_skill_json(None, "template", built_in);
        }
        print!("{built_in}");
        return Ok(());
    }
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project.unwrap_or_default())?;
    let (template, source) = skill::load_template(template, project.kind)?;
    let about = repo::about(&project);
    let fields = skill::Fields {
        name: &project.name,
        path: &project.path,
        remote: project.remote.as_deref(),
        hub: project.hub_path.as_deref().map(Path::new),
        about: about.as_deref(),
    };
    let rendered = skill::render(&template, &fields)?;
    for note in &rendered.notes {
        eprintln!("note: {note}");
    }
    if !install {
        if json {
            return print_skill_json(None, "printed", &rendered.text);
        }
        print!("{}", rendered.text);
        return Ok(());
    }

    let dir = match dir {
        Some(dir) => dir.to_path_buf(),
        None => skill::skills_dir()?,
    }
    .join(&project.name);
    let path = dir.join("SKILL.md");
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    if !before.is_empty() && !skill::is_generated(&before) && !replace {
        bail!(
            "{} was written by hand and rigger has not written it before.
Move what it says that only this project can say into the hub, then run again with `--replace`.",
            path.display()
        );
    }
    if before == rendered.text {
        if json {
            return print_skill_json(Some(&path), "unchanged", &rendered.text);
        }
        println!("{} is already what the template says.", path.display());
        return Ok(());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::write(&path, &rendered.text).with_context(|| format!("cannot write {}", path.display()))?;
    if json {
        return print_skill_json(Some(&path), if before.is_empty() { "wrote" } else { "rewrote" }, &rendered.text);
    }
    let what = if before.is_empty() { "Wrote" } else { "Rewrote" };
    println!("{what} {} from {source}.", path.display());
    Ok(())
}

/// One skill for the whole line, instead of one per project.
///
/// Seventeen near-identical skills were seventeen copies of the same
/// instructions, and a change to the ritual was seventeen rewrites - or,
/// as it usually went, one rewrite and sixteen files quietly stale. What
/// differs between them is the project's name, and a name is an argument,
/// not a file.
///
/// The name of the skill is the profile's: a line of products and a ticket
/// desk are different ways of working and should not answer to one skill.
fn write_line_skill(install: bool, dir: Option<&Path>, replace: bool, template: Option<&Path>, print_template: bool, json: bool) -> Result<()> {
    if print_template {
        if json {
            return print_skill_json(None, "template", skill::DEFAULT_LINE_TEMPLATE);
        }
        print!("{}", skill::DEFAULT_LINE_TEMPLATE);
        return Ok(());
    }
    let db = Db::open(&paths::db_path()?)?;
    let line = profile::Config::load()?.current_name().to_string();
    let (template, source) = skill::load_line_template(template)?;
    let listed = listed_projects(&db)?;
    let description = skill::line_description(&line, &listed);
    let rendered = skill::render_line(&template, &line, &description, &listed)?;
    if !install {
        if json {
            return print_skill_json(None, "printed", &rendered.text);
        }
        print!("{}", rendered.text);
        return Ok(());
    }

    let dir = match dir {
        Some(dir) => dir.to_path_buf(),
        None => skill::skills_dir()?,
    }
    .join(&line);
    let path = dir.join("SKILL.md");
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    if !before.is_empty() && !skill::is_generated(&before) && !replace {
        bail!(
            "{} was written by hand and rigger has not written it before.
Move what it says into the record, then run again with `--replace`.",
            path.display()
        );
    }
    if before == rendered.text {
        if json {
            return print_skill_json(Some(&path), "unchanged", &rendered.text);
        }
        println!("{} is already what the template says.", path.display());
        return Ok(());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::write(&path, &rendered.text).with_context(|| format!("cannot write {}", path.display()))?;
    if json {
        return print_skill_json(Some(&path), if before.is_empty() { "wrote" } else { "rewrote" }, &rendered.text);
    }
    let what = if before.is_empty() { "Wrote" } else { "Rewrote" };
    println!(
        "{what} {} from {source}: {} projects, description {} of {} characters.",
        path.display(),
        listed.len(),
        description.chars().count(),
        skill::DESCRIPTION_LIMIT
    );
    Ok(())
}

/// Every project the line's skill lists, in the order it lists them.
///
/// A place the record keeps for itself is left out: it has no directory to
/// work in and no stage to continue, so naming it among the projects would
/// be offering an assistant somewhere it cannot go. A folder with no
/// repository is listed - it is worked on like any product.
fn listed_projects(db: &Db) -> Result<Vec<skill::Listed>> {
    let mut listed: Vec<skill::Listed> = db
        .projects()?
        .into_iter()
        .filter(|p| p.kind.has_a_place())
        .map(|p| skill::Listed {
            about: repo::about(&p),
            name: p.name,
            path: p.path,
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(listed)
}

fn open_project(db: &Db, name: &str) -> Result<db::Project> {
    if let Some(project) = db.project_by_name(name)? {
        return Ok(project);
    }
    // The profile's own name is a place, not a typo: it is where facts
    // about every project go - the rituals of the line, a retro across all
    // of them. Made on demand rather than by a setup step nobody would know
    // to run, the way a desk makes itself the first time a card needs one.
    if name == profile::Config::load()?.current_name() {
        return db.profile_project();
    }
    bail!("no project named '{name}'; see `rigger project list`")
}

/// A project in the work, for the commands that add to its history.
///
/// A retired project refuses new history at the schema, but a trigger
/// cannot name the project it refused; these commands refuse first, and
/// say which project and since when.
fn working_project(db: &Db, name: &str) -> Result<db::Project> {
    let project = open_project(db, name)?;
    project.ensure_working()?;
    Ok(project)
}

/// A project named outright, or the one the working directory sits in.
///
/// A hook has no project name to pass: the Stop hook of an assistant is
/// handed a working directory and nothing else. But it runs *in* the
/// project, and the record already knows every project by its path - so the
/// directory is the name, and the hook needs to be told nothing.
///
/// Walks upwards, because a session ends wherever the last command left the
/// shell, which may be a subdirectory of the checkout.
fn project_here(db: &Db, name: Option<&str>) -> Result<db::Project> {
    if let Some(name) = name {
        return open_project(db, name);
    }
    let here = std::env::current_dir().context("cannot read the working directory")?;
    let here = dunce::canonicalize(&here).unwrap_or(here);
    for dir in here.ancestors() {
        if let Some(project) = db.project_by_path(&dir.to_string_lossy())? {
            return Ok(project);
        }
    }
    bail!(
        "no project recorded at {} or above it; name one, or add this directory with `rigger project add`",
        here.display()
    )
}

fn show_project(project: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let about = repo::about(&project);
    let screen = show::build(&db, &project, about)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&screen)?);
        return Ok(());
    }
    print!("{}", show::render(&screen));
    Ok(())
}

fn show_context(project: &str, json: bool, explain: bool, budget: usize) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let packet = context::build(&db, &project, budget)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&packet)?);
        return Ok(());
    }
    let text = context::render(&packet);
    print!("{text}");
    if explain {
        println!("\n## Cost");
        for cost in context::costs(&packet) {
            println!("{:<14} {:>5} tokens", cost.section, cost.tokens);
        }
        println!("{:<14} {:>5} tokens of {budget}", "total", context::estimate_tokens(&text));
        // What the budget refused, by name. The packet says how many
        // events it dropped; this says which, so that "there is something
        // you have not seen" can be acted on.
        print!("{}", context::render_dropped(&packet));
    }
    Ok(())
}

/// Starts the assistant on what comes next, and says how the turn went.
///
/// One turn, and then it stops: the next project is another `go`, so the
/// owner sees each one end before the next begins.
fn go_on(named: Option<&str>, force: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    // A name that is no project is a typo, and must not read as "left out".
    if let Some(name) = named {
        working_project(&db, name)?;
    }
    // Usually a second or two, but a machine that has just woken has been seen
    // to take over a minute, and a silent command looks hung.
    if !json && std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        eprintln!("Reading git across the line...");
    }
    let chosen = queue::now(&db)?;
    let choice = go::choose(&chosen, named, force);

    // What would be started, and with what - the assistant is not started:
    // a program reading JSON is not a person about to work in a terminal.
    if json {
        let (program, _) = open::assistant();
        let start = match &choice {
            go::Choice::Start(target) => Some(target),
            _ => None,
        };
        return print_json(&serde_json::json!({
            "project": start.map(|t| &t.project),
            "version": start.and_then(|t| t.version.as_ref()),
            "title": start.and_then(|t| t.title.as_ref()),
            "reason": start.and_then(|t| t.reason),
            "unfinished": start.and_then(|t| t.unfinished.as_ref()),
            "dir": start.map(|t| working_project(&db, &t.project).map(|p| p.path)).transpose()?,
            "program": program,
            "message": start.map(go::first_message),
            "passed_over": start.filter(|_| force && named.is_none()).map(|t| go::passed_over(&chosen, t)).unwrap_or_default(),
            "busy": match &choice {
                go::Choice::Busy { project, since } => Some(serde_json::json!({ "project": project, "since": since })),
                _ => None,
            },
            "nothing": match &choice {
                go::Choice::Nothing(said) => Some(said),
                _ => None,
            },
        }));
    }

    let target = match choice {
        go::Choice::Start(target) => target,
        go::Choice::Busy { project, since } => bail!("{}", go::busy(&project, &since)),
        go::Choice::Nothing(said) => {
            println!("{said}");
            return Ok(());
        }
    };
    if !chosen.unread.is_empty() {
        eprintln!(
            "Git did not read for {}: what this knows of them is as of the last time it did.",
            chosen.unread.join(", ")
        );
    }

    let project = working_project(&db, &target.project)?;
    if !project.kind.has_a_place() {
        bail!("{} is a place the record keeps for itself: there is nothing to work in", project.name);
    }
    let dir = Path::new(&project.path);
    open::check_dir(dir)?;
    let (program, _) = open::assistant();
    let message = go::first_message(&target);
    if force && named.is_none() {
        let past = go::passed_over(&chosen, &target);
        if !past.is_empty() {
            println!("Going past {}.", past.join(", "));
        }
    }
    print!("{}", go::before(&target, &program, &project.path));

    let shipped_before = db.latest_delivery(project.id)?.map(|d| d.version);
    // The sitting is opened here rather than left to the assistant, so that a
    // second `go` in another terminal sees it from the first moment. One this
    // run joined belongs to whoever opened it, and is theirs to close.
    let joined = db.open_session(project.id)?.is_some();
    open_sitting(&db, &project)?;
    let code = open::run(dir, &message);
    if !joined {
        // Whoever was at work has gone, or never came. An assistant's own hook
        // has usually closed it already, and then this says nothing.
        session_end(Some(&project.name), None, None, None, true, false)?;
    }
    let code = code?;

    let after = queue::now(&db)?;
    let shipped_now = db.latest_delivery(project.id)?.map(|d| d.version);
    let shipped = shipped_now.as_deref().filter(|_| shipped_now != shipped_before);
    print!("{}", go::after(&target, shipped, &after));
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Reads git for one project, or for every recorded project.
fn sync_projects(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects = match project {
        Some(name) => vec![working_project(&db, name)?],
        None => db.projects()?,
    };
    let mut reports = Vec::new();
    // Why GitHub stopped being asked, once it has. Offline, every project
    // would fail the same way, and eighteen lines saying so hide the one
    // project that changed; the first failure ends the asking for the run.
    let mut unreachable: Option<String> = None;
    for project in &projects {
        // A place the record keeps for itself has no repository, and asking
        // git about it would warn on every run about a project working
        // exactly as intended. Named on its own it says so once, rather
        // than failing at something it was never meant to do.
        if !project.kind.reads_git() {
            if projects.len() == 1 {
                match project.kind {
                    db::Kind::Folder => println!("{} is a folder with no repository; there is no git to read", project.name),
                    _ => println!("{} is a place the record keeps for itself; there is no repository to read", project.name),
                }
            }
            continue;
        }
        let mut report = sync::sync(&db, project)?;
        if unreachable.is_none() {
            let tags: Vec<&str> = report.shipped.iter().map(|s| s.version.as_str()).collect();
            match sync::read_delivery(&db, project, &tags)? {
                Ok(note) => report.delivery = note,
                // No `gh` is a way of working, not a failure: the tag stays
                // the fact it always was, and nothing is said.
                Err(delivery::Unread::NoGh) => unreachable = Some(String::new()),
                Err(delivery::Unread::Failed(why)) => unreachable = Some(why),
            }
        }
        reports.push(report);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
        return Ok(());
    }
    for report in &reports {
        print_sync(report, projects.len() > 1);
    }
    if let Some(why) = unreachable.filter(|w| !w.is_empty()) {
        println!("GitHub was not read, so releases and publish runs were not checked: {}", first_line(&why));
    }
    Ok(())
}

/// One project's sync, as a line or as a paragraph.
///
/// A quiet project prints nothing when several are synced at once: a run
/// across the whole line is read for what changed, and seventeen "nothing
/// changed" lines hide the two that did.
fn print_sync(report: &sync::Report, many: bool) {
    let quiet = !report.changed() && report.untagged.is_empty() && report.warnings.is_empty() && report.short_delivery().is_none();
    if many && quiet {
        return;
    }
    println!("{}:", report.project);
    for warning in &report.warnings {
        println!("  note: {warning}");
    }
    let newly: Vec<&sync::Shipped> = report.shipped.iter().filter(|s| s.newly).collect();
    for shipped in &newly {
        let unplanned = report.unplanned.contains(&shipped.version);
        let note = if unplanned { "  (not in the plan)" } else { "" };
        println!("  shipped    {} on {}{note}", shipped.version, shipped.date);
    }
    if report.changes_recorded > 0 {
        let n = report.changes_recorded;
        let plural = if n == 1 { "change" } else { "changes" };
        println!("  read       {n} {plural} from commit messages");
    }
    for version in &report.untagged {
        println!("  no tag     {version} is closed in the plan");
    }
    // A short delivery is said on every run until it is fixed: it is not
    // news but a standing fault, and the calendar counts the version as
    // shipped all the while. A full one is said once, when it is learnt.
    if let Some(note) = &report.delivery
        && (note.state.is_short() || note.newly)
    {
        println!("  delivery   {}: {}", note.version, note.state.describe());
    }
    // Activity is state, not news: it says the same thing on every run until
    // someone commits. Printed when there is something else to say, so a run
    // that changed nothing does not end with a line that looks like it did.
    if report.commits_since_tag > 0 && !quiet {
        let since = match report.shipped.iter().max_by_key(|s| db::version_order(&s.version)) {
            Some(newest) => format!(" since {}", newest.version),
            None => String::new(),
        };
        let when = report.last_commit_at.as_deref().unwrap_or("unknown");
        let commits = report.commits_since_tag;
        let plural = if commits == 1 { "commit" } else { "commits" };
        println!("  activity   {commits} {plural}{since}, last on {when}");
    }
    // A card is news when this read tied it to the repository or found new
    // commits for it; a branch already known is state, like activity.
    for card in report.cards.iter().filter(|c| c.linked_now || c.new_commits > 0) {
        let mut said = Vec::new();
        if card.new_commits > 0 {
            let plural = if card.new_commits == 1 { "commit" } else { "commits" };
            said.push(format!("{} new {plural}", card.new_commits));
        }
        if card.branches > 0 {
            let plural = if card.branches == 1 { "branch" } else { "branches" };
            said.push(format!("{} {plural}", card.branches));
        }
        if card.linked_now {
            said.push("linked now".to_string());
        }
        println!("  card       {}: {}", card.key, said.join(", "));
    }
    if quiet {
        println!("  nothing changed");
    }
}

/// An event written against a card, under the desk.
fn note_on_card(task: &str, kind: &str, text: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    match kind {
        "state" => bail!("a state line belongs to a project's README, not to a card"),
        "question" => bail!("a question waits in a project's inbox, not on a card; write `rigger note <project> --kind question`"),
        "shipped" => bail!("a release belongs to a project's versions, not to a card; write `rigger note <project> --kind shipped`"),
        _ => {}
    }
    let card = find_card(&db, task)?;
    let desk = db.desk_project()?;
    let change = db.record_task_event(desk.id, card.id, kind, text, &db::now(), "assistant")?;
    if json {
        let id = db.event_id_by_body(desk.id, Some(card.id), kind, text)?;
        return print_event_json(&db, id, change != db::Change::Unchanged);
    }
    match kind {
        "next" => println!("Set the next step for {}", card.key),
        "plan" => println!("Recorded a step of the plan on {}", card.key),
        _ => println!("Recorded a {kind} on {}", card.key),
    }
    Ok(())
}

/// Records a release as the release engine reports it.
fn note_shipped(project: &str, tag: &str, release: bool, registries: &[String], text: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    let recorded = delivery::record(&db, &project, tag, release, registries, text)?;
    if json {
        return print_json(&serde_json::json!({ "project": project.name, "shipped": recorded }));
    }
    match recorded.closed {
        true => println!("{} {} shipped - {}", project.name, recorded.version, recorded.delivery.describe()),
        false => println!(
            "{} {} had shipped already; now {}",
            project.name,
            recorded.version,
            recorded.delivery.describe()
        ),
    }
    if !recorded.registries.is_empty() {
        println!("  reached {}", recorded.registries.join(", "));
    }
    Ok(())
}

fn note(project: &str, kind: &str, text: &str, principle: Option<&str>, asked_by: Option<&str>, due: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    // Read before anything is written, so that a day that is not a day
    // refuses the question rather than leaving it recorded without one.
    let due = due.map(|d| db::parse_day(d, &db::today())).transpose()?;
    // A state line is not an event: it is the top of the README's state
    // block, which an export writes from the record.
    if kind == "state" {
        db.add_state_line(project.id, &db::today(), text)?;
        if json {
            return print_json(&serde_json::json!({ "state": { "project": project.name, "day": db::today(), "body": text } }));
        }
        println!("Added a state line for {}; `rigger export` writes it into the README", project.name);
        return Ok(());
    }
    // A principle names what a decision stands on, and only a decision
    // stands on one: a change or a gate run is something that happened, not
    // something believed. Said rather than ignored, because a principle
    // silently dropped is a principle the owner thinks is recorded.
    if principle.is_some() && kind != "decision" {
        bail!("a principle belongs to a decision; this is a {kind}");
    }
    // The neighbour has to exist before the wish is written, so that a
    // typo in the name is a refusal rather than a wish nobody asked for.
    let asker = match asked_by {
        Some(name) => Some(working_project(&db, name)?),
        None => None,
    };
    if let Some(asker) = &asker
        && asker.id == project.id
    {
        bail!("{} cannot be the neighbour asking {} for something", asker.name, project.name);
    }
    let change = db.record_event(project.id, kind, text, &db::now(), "assistant")?;
    if let Some(id) = db.latest_event_id(project.id, kind)? {
        if let Some(principle) = principle {
            db.name_principle(id, principle)?;
        }
        if let Some(asker) = &asker {
            db.name_asker(id, asker.id)?;
        }
    }
    // Found by its text rather than as the newest question: asking the same
    // question again is how a day is given to one already waiting, and the
    // newest question may be another one entirely.
    if let Some(due) = &due
        && let Some(id) = db.question_by_text(project.id, text)?
    {
        db.set_due(id, Some(due))?;
    }
    if json {
        let id = db.event_id_by_body(project.id, None, kind, text)?;
        return print_event_json(&db, id, change != db::Change::Unchanged);
    }
    match (kind, &asker) {
        ("wish", Some(asker)) => println!("Recorded a wish for {} from {}", project.name, asker.name),
        ("question", _) => println!("Asked the owner about {}; it waits in the inbox until answered", project.name),
        ("next", _) => println!("Set the next step for {}; the next session starts from it", project.name),
        _ => println!("Recorded a {kind} for {}", project.name),
    }
    if let Some(due) = &due {
        println!("  due {due}");
    }
    if let Some(principle) = principle {
        println!("  on the principle {principle:?}");
    }
    if change == db::Change::Unchanged {
        println!("  the record already held it");
    }
    Ok(())
}

/// One event whole, by the id the packet printed. Under `--json` it is the
/// event in the form a write prints it, so `note` and this are a pair.
fn show_event(project: &str, id: i64, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let event = context::read(&db, &project, id)?;
    if json {
        return print_json(&serde_json::json!({ "event": event }));
    }
    print!("{}", context::render_whole(&event));
    Ok(())
}

/// Records a wish, and says when the neighbour's stage is waiting for it.
fn wish(project: &str, text: &str, from: Option<&str>, waits: bool, json: bool) -> Result<()> {
    note(project, "wish", text, None, from, None, json)?;
    if waits {
        let db = Db::open(&paths::db_path()?)?;
        let project = working_project(&db, project)?;
        if let Some(id) = db.latest_event_id(project.id, "wish")? {
            db.mark_waiting(id)?;
        }
    }
    Ok(())
}

fn resolve(project: &str, id: i64, answer: Option<&str>, built: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    // A retired project's wishes and questions can still be put away - that
    // is tidying what it holds - but an answer is a new decision.
    if answer.is_some() {
        project.ensure_working()?;
    }
    let (kind, body) = db.resolve_event(project.id, id, answer, built)?;
    if json {
        return print_json(&serde_json::json!({
            "resolved": { "id": id, "project": project.name, "was": kind, "body": body, "answer": answer }
        }));
    }
    let first_line = body.lines().next().unwrap_or(&body);
    match kind.as_str() {
        "question" => println!("Answered [{id}]: {first_line}"),
        "blocker" => println!("Cleared [{id}]: {first_line}"),
        "wish" if built => println!("Built [{id}]: {first_line}"),
        _ => println!("Sorted [{id}]: {first_line}"),
    }
    if answer.is_some() {
        println!("  the answer is recorded as a decision");
    }
    Ok(())
}

/// Searches every project's events at once.
fn find(query: &str, project: Option<&str>, kind: Option<&str>, limit: u32, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    // A project that does not exist is a typo, not an empty result: saying
    // "nothing found" would send someone looking for the wrong thing.
    if let Some(name) = project {
        open_project(&db, name)?;
    }
    use search::Provider as _;
    let provider = search::Fts(&db);
    let hits = search::look(&provider, query, project, kind, limit).with_context(|| format!("{query:?} is not a search {} understands", provider.name()))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
        return Ok(());
    }
    if let Some(phrase) = &hits.read_as {
        println!("(read as the phrase {phrase}: FTS5 could not parse the query as it was written)");
    }
    if hits.is_empty() {
        println!("{}", search::nothing_found(query, project, kind));
        return Ok(());
    }
    // The project column is dead weight when the search was for one project.
    let show_project = project.is_none();
    for event in &hits.events {
        print!("{}", search::render_event(event, show_project));
    }
    // Documents under the events, and named: a vision and a decision are
    // different kinds of answer, and one that has to be opened with a
    // second command should say so rather than look like a line of the
    // list above it.
    if !hits.documents.is_empty() {
        if !hits.events.is_empty() {
            println!();
        }
        println!("Documents");
        for doc in &hits.documents {
            print!("{}", search::render_document(doc, show_project));
            println!("             {}", search::open_command(doc));
        }
    }
    if hits.events.len() as u32 == limit {
        println!("({limit} shown; --limit for more)");
    }

    Ok(())
}

/// The work that went into one version.
fn why(project: Option<&str>, version: Option<&str>, principle: Option<&str>, principles: bool, json: bool) -> Result<()> {
    if principles {
        return list_principles(json);
    }
    if let Some(principle) = principle {
        return why_principle(principle, project, json);
    }
    let (Some(project), Some(version)) = (project, version) else {
        bail!("`why` wants a project and a version, or `--principle <name>`; `rigger why --principles` lists the names")
    };
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let why = search::why(&db, &project, version)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&why)?);
        return Ok(());
    }

    let mut heading = why.version.name.clone();
    if let Some(title) = &why.version.title {
        heading.push_str(&format!(" · {title}"));
    }
    match &why.version.shipped_at {
        Some(on) => println!("{heading} — shipped {on}"),
        None => println!("{heading} — being built"),
    }
    match &why.after {
        Some(before) => println!("the work after {} ({})", before.name, before.shipped_at.as_deref().unwrap_or("undated")),
        None => println!("the work from the start of the record"),
    }
    println!();

    if why.events.is_empty() {
        println!("Nothing was recorded in that window.");
        // Two releases can share a moment - a tag points at a commit, and
        // this line sometimes tags two of them in the same second. Saying so
        // is better than an empty answer that looks like a missing record.
        if let Some(before) = &why.after
            && before.shipped_ts.is_some()
            && before.shipped_ts == why.version.shipped_ts
        {
            println!(
                "{} and {} were tagged in the same second, so no work falls between them.",
                before.name, why.version.name
            );
        }
        return Ok(());
    }
    for event in &why.events {
        print!("{}", search::render_event(event, false));
    }
    Ok(())
}

/// The names the record has used for principles, and how often.
///
/// A vocabulary, not a list to maintain: what makes a name a principle of
/// this profile is that a decision was recorded on it. A list kept beside
/// the decisions would be a second truth, and the one that went stale.
fn list_principles(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let principles = db.principles()?;
    if json {
        let rows: Vec<serde_json::Value> = principles
            .iter()
            .map(|(name, count)| serde_json::json!({ "principle": name, "decisions": count }))
            .collect();
        return print_json(&rows);
    }
    if principles.is_empty() {
        println!("No decision names a principle yet.");
        println!("`rigger note <project> <text> --kind decision --principle <name>` starts one.");
        return Ok(());
    }
    for (name, count) in &principles {
        println!("{name}  —  {}", plural(*count as usize, "decision", "decisions"));
    }
    Ok(())
}

/// One principle read across the whole line.
///
/// A principle is believed because of what happened, and what happened is
/// spread over eighteen projects: "no users, no compatibility" was arrived
/// at in one product and applied in six. Read one project at a time it
/// looks like six opinions; read as one thread it is something the line
/// learnt, and the thread is what makes it arguable.
fn why_principle(principle: &str, project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if let Some(name) = project {
        open_project(&db, name)?;
    }
    let mut events = db.on_principle(principle)?;
    if let Some(name) = project {
        events.retain(|e| e.project == name);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
        return Ok(());
    }
    if events.is_empty() {
        println!("Nothing stands on {principle:?} in the record.");
        // A principle nobody has used is far likelier a misspelling than a
        // new one: the names are written by hand, twice, months apart.
        let known = db.principles()?;
        if !known.is_empty() {
            println!();
            println!("The record knows these:");
            for (name, count) in known.iter().take(12) {
                println!("  {name} ({count})");
            }
        }
        return Ok(());
    }
    let projects: std::collections::BTreeSet<&str> = events.iter().map(|e| e.project.as_str()).collect();
    println!("{principle}");
    println!(
        "{} across {}",
        plural(events.len(), "decision", "decisions"),
        plural(projects.len(), "project", "projects")
    );
    println!();
    for event in &events {
        let day = event.at.split('T').next().unwrap_or(&event.at);
        let version = event.version.as_deref().map(|v| format!(" · {v}")).unwrap_or_default();
        println!("{day} · {}{version}", event.project);
        for line in event.body.lines() {
            println!("  {line}");
        }
        println!();
    }
    Ok(())
}

/// One side of a link as it is written on a command line: `kasl@v1.13.0`,
/// or just `kasl`.
fn link_side(text: &str) -> (&str, Option<&str>) {
    match text.split_once('@') {
        Some((project, version)) if !version.is_empty() => (project, Some(version)),
        _ => (text, None),
    }
}

fn link_add(from: &str, to: &str, kind: LinkKind, note: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (from_name, from_version) = link_side(from);
    let (to_name, to_version) = link_side(to);
    let from_project = working_project(&db, from_name)?;
    let to_project = working_project(&db, to_name)?;
    if from_project.id == to_project.id {
        bail!("a link joins two projects; {} is one", from_project.name);
    }
    let kind = kind.to_domain();
    let (id, change, anchored) = db.add_link(&db::NewLink {
        kind,
        from_project: from_project.id,
        from_version,
        to_project: to_project.id,
        to_version,
        note,
    })?;
    if json {
        let link = db.links_of(from_project.id)?.into_iter().find(|l| l.id == id);
        return print_json(&serde_json::json!({ "link": link, "changed": change == db::Change::Added }));
    }
    let side = |name: &str, version: Option<&String>| match version {
        Some(v) => format!("{name} {v}"),
        None => name.to_string(),
    };
    let arrow = if kind.symmetric() { "<->" } else { "->" };
    let both = format!(
        "{} {arrow} {} ({kind})",
        side(&from_project.name, anchored.from.as_ref()),
        side(&to_project.name, anchored.to.as_ref())
    );
    match change {
        db::Change::Added => println!("[{id}] {both}"),
        _ => println!("[{id}] already recorded: {both}"),
    }
    Ok(())
}

fn link_list(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let links = match project {
        Some(name) => {
            let project = open_project(&db, name)?;
            db.links_of(project.id)?
        }
        None => {
            // The line as it is worked: a tie with a retired end is part of
            // that project's history, read by naming it.
            let working = db.projects()?;
            let mut all: Vec<link::Link> = Vec::new();
            for project in &working {
                for found in db.links_of(project.id)? {
                    if !working.iter().any(|p| p.name == found.far.project) {
                        continue;
                    }
                    // Each link is found from both of its ends; keep the
                    // first sighting so the list is of links, not of ends.
                    if !all.iter().any(|seen| seen.id == found.id) {
                        all.push(found);
                    }
                }
            }
            all.sort_by(|a, b| a.near.project.cmp(&b.near.project).then_with(|| a.id.cmp(&b.id)));
            all
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&links)?);
        return Ok(());
    }
    if links.is_empty() {
        match project {
            Some(name) => println!("{name} is tied to nothing in the record."),
            None => println!("The record holds no links yet."),
        }
        println!("`rigger link add <project>[@version] <project>[@version]` records one.");
        return Ok(());
    }
    for found in &links {
        println!("{}", render_link(found));
    }
    Ok(())
}

/// One link on one line, read from the near end.
fn render_link(found: &link::Link) -> String {
    let side = |end: &link::End| match &end.version {
        Some(version) => format!("{} {version}", end.project),
        None => end.project.clone(),
    };
    let arrow = if found.kind.symmetric() { "<->" } else { "->" };
    let note = found.note.as_deref().map(|n| format!("  {n}")).unwrap_or_default();
    format!("[{}] {:<9} {} {arrow} {}{note}", found.id, found.kind, side(&found.near), side(&found.far))
}

fn link_remove(id: i64, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    match db.remove_link(id)? {
        true if json => print_json(&serde_json::json!({ "removed": id }))?,
        true => println!("Forgot link [{id}]"),
        false => bail!("the record has no link [{id}]; `rigger link list` prints the ids"),
    }
    Ok(())
}

/// What the projects build against, from their manifests and the record's ties.
fn link_uses(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if let Some(name) = project {
        open_project(&db, name)?;
    }
    let found: Vec<queue::Use> = queue::uses(&db)?
        .into_iter()
        .filter(|u| project.is_none_or(|name| u.consumer == name || u.supplier == name))
        .collect();
    if json {
        return print_json(&serde_json::json!({ "uses": found }));
    }
    if found.is_empty() {
        println!("Nothing is drawn on: no manifest names a neighbour's package, and no tie says so.");
        return Ok(());
    }
    for item in &found {
        println!("{}", queue::use_line(item));
    }
    Ok(())
}

fn link_drift(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let parted = link::parted(&db.pairs()?);
    if json {
        println!("{}", serde_json::to_string_pretty(&parted)?);
        return Ok(());
    }
    if parted.is_empty() {
        println!("Every pair in the record has both halves in step.");
        return Ok(());
    }
    for item in &parted {
        println!("{}", drift_line(item));
    }
    Ok(())
}

/// What a parted pair says, in one line.
pub(crate) fn drift_line(item: &link::Parted) -> String {
    fn version(end: &link::End) -> &str {
        end.version.as_deref().unwrap_or("its half")
    }
    let note = item.note.as_deref().map(|n| format!(" — {n}")).unwrap_or_default();
    match item.drift {
        link::Drift::ShippedAlone => format!(
            "{} {} shipped without {} {}{note}",
            item.ahead.project,
            version(&item.ahead),
            item.behind.project,
            version(&item.behind),
        ),
        link::Drift::RunAhead => format!(
            "{} is {} ahead of {}, which is still at {}{note}",
            item.ahead.project,
            plural(item.versions.unwrap_or(0) as usize, "version", "versions"),
            item.behind.project,
            version(&item.behind),
        ),
    }
}

/// The questions waiting for the owner, gathered from every project.
fn inbox(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if let Some(name) = project {
        open_project(&db, name)?;
    }
    let mut waiting = db.open_questions()?;
    if let Some(name) = project {
        waiting.retain(|q| q.project == name);
    }
    // A neighbour's order is not a question for the owner - nobody is being
    // asked to decide anything - but it is the other thing that sits in a
    // project waiting on someone else, and the inbox is where the owner
    // looks for what is waiting. Kept as its own group rather than mixed in
    // with the questions, because answering one and doing the other are not
    // the same job.
    let mut asked = db.all_asked_wishes()?;
    if let Some(name) = project {
        asked.retain(|a| a.project == name);
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "waiting": waiting,
                "shared": owner::shared_subjects(&waiting),
                "asked_by_neighbours": asked,
            }))?
        );
        return Ok(());
    }

    if waiting.is_empty() && asked.is_empty() {
        match project {
            Some(name) => println!("{name} is waiting on nothing."),
            None => println!("Nothing is waiting on you."),
        }
        return Ok(());
    }

    let projects: std::collections::BTreeSet<&str> = waiting.iter().map(|q| q.project.as_str()).collect();
    if waiting.is_empty() {
        print_asked(&asked, project.is_some());
        return Ok(());
    }
    let late = waiting.iter().filter(|q| q.overdue).count();
    let late = match late {
        0 => String::new(),
        n => format!(", {}", paint::red(&format!("{n} overdue"))),
    };
    match project {
        Some(_) => println!(
            "{}{late}
",
            plural(waiting.len(), "question", "questions")
        ),
        None => println!(
            "{} in {}{late}
",
            plural(waiting.len(), "question", "questions"),
            plural(projects.len(), "project", "projects")
        ),
    }

    // Grouped by project, because answering is done a project at a time -
    // and within one, oldest first, since that is what has waited longest.
    let mut last: Option<&str> = None;
    for question in &waiting {
        let name = if last == Some(question.project.as_str()) {
            String::new()
        } else {
            question.project.clone()
        };
        last = Some(&question.project);
        println!(
            "{name:<12} [{:>3}] {}  {}{}",
            question.id,
            question.date,
            owner::subject(&question.body),
            due_mark(question)
        );
    }

    // One answer that settles three projects is the most valuable thing on
    // this screen, and without saying so it looks like three separate jobs.
    let shared = owner::shared_subjects(&waiting);
    if !shared.is_empty() {
        println!(
            "
Asked by several projects - one answer settles each group:"
        );
        for group in &shared {
            println!("  {} — {}", group.subject, group.projects.join(", "));
        }
    }
    print_asked(&asked, project.is_some());
    println!(
        "
Answer one with: rigger resolve <project> <id> \"<answer>\""
    );
    Ok(())
}

/// What a question's day says, as the tail of its line.
///
/// A question past its day is written out as overdue and, for a person at a
/// terminal, in red - the words carry it for a pipe, the colour for an eye
/// scanning a list of forty lines that otherwise all look the same.
fn due_mark(question: &db::Waiting) -> String {
    match (&question.due, question.overdue) {
        (Some(due), true) => format!("  {}", paint::red(&format!("overdue since {due}"))),
        (Some(due), false) => format!("  due {due}"),
        (None, _) => String::new(),
    }
}

/// The orders neighbours have placed, as their own group.
///
/// These are not questions and the owner is not being asked to decide
/// anything: they are work one product is waiting on another to do. They
/// belong on this screen because it is where the owner looks for what is
/// waiting - and apart from the questions because the two are answered in
/// entirely different ways.
fn print_asked(asked: &[db::AskedOf], one_project: bool) {
    if asked.is_empty() {
        return;
    }
    println!();
    println!("Neighbours are asking for:");
    let mut last: Option<&str> = None;
    for wish in asked {
        // Grouped by the project being asked, like the questions above, so
        // that one product's orders read as one list.
        let name = match (one_project, last == Some(wish.project.as_str())) {
            (true, _) | (_, true) => String::new(),
            _ => wish.project.clone(),
        };
        last = Some(&wish.project);
        let first = wish.body.lines().next().unwrap_or("").trim();
        println!("{name:<12} [{:>3}] {}  {} — {}", wish.id, wish.date, owner::subject(first), wish.asked_by);
    }
    println!();
    println!("Sort one with: rigger resolve <project> <id>");
}

/// What moved lately, per project.
fn digest(project: Option<&str>, since: &str, md: Option<&Path>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let days = parse_days(since)?;
    let from = day_before(days);

    let projects = match project {
        Some(name) => vec![open_project(&db, name)?],
        None => db.projects()?,
    };

    let mut reports = Vec::new();
    for project in &projects {
        let facts = db.digest(project.id, &from)?;
        let stage = db.current_stage(project.id)?;
        let next = stage.map(|s| match s.title {
            Some(title) => format!("{} · {title}", s.version),
            None => s.version,
        });
        let quiet = db.last_event_at(project.id)?.as_deref().and_then(days_since_utc);
        let lines = owner::digest_lines(&facts, next.as_deref(), quiet);
        reports.push((project.name.clone(), facts, next, lines));
    }

    let mut written = None;
    if let Some(path) = md {
        let listed: Vec<(String, Vec<String>)> = reports
            .iter()
            .filter(|(_, facts, _, _)| project.is_some() || !facts.shipped.is_empty() || facts.decisions + facts.findings + facts.changes > 0)
            .map(|(name, _, _, lines)| (name.clone(), lines.clone()))
            .collect();
        let quiet: Vec<String> = match project {
            Some(_) => Vec::new(),
            None => reports
                .iter()
                .filter(|(name, _, _, _)| !listed.iter().any(|(listed, _)| listed == name))
                .map(|(name, _, _, _)| name.clone())
                .collect(),
        };
        let block = owner::digest_markdown(&from, &listed, &quiet);
        let change = owner::write_block(path, &block)?;
        if !json {
            match change {
                db::Change::Added => println!("Added the digest since {from} to {}", path.display()),
                db::Change::Updated => println!("Replaced the digest in {} with the one since {from}", path.display()),
                db::Change::Unchanged => println!("{} already holds this digest", path.display()),
            }
            return Ok(());
        }
        written = Some(serde_json::json!({ "path": path, "changed": change != db::Change::Unchanged }));
    }

    if json {
        let payload: Vec<_> = reports
            .iter()
            .map(|(name, facts, next, lines)| serde_json::json!({ "project": name, "facts": facts, "next": next, "lines": lines }))
            .collect();
        let mut answer = serde_json::json!({ "since": from, "projects": payload });
        // The note written, when the digest went into one.
        if let Some(note) = written {
            answer["note"] = note;
        }
        println!("{}", serde_json::to_string_pretty(&answer)?);
        return Ok(());
    }

    println!(
        "Since {from}
"
    );

    // A project with nothing but its next stage to report has not moved:
    // naming it in one line beats five lines that say nothing happened.
    //
    let (moved, still): (Vec<_>, Vec<_>) = reports
        .iter()
        .partition(|(_, facts, _, _)| !facts.shipped.is_empty() || facts.decisions + facts.findings + facts.changes > 0);

    let listed = if project.is_some() {
        reports.iter().collect::<Vec<_>>()
    } else {
        moved.clone()
    };
    for (name, _, _, lines) in &listed {
        println!("{name}");
        for line in lines.iter() {
            println!("  {line}");
        }
    }
    if listed.is_empty() {
        println!("Nothing moved.");
    }
    if project.is_none() && !still.is_empty() {
        let names: Vec<&str> = still.iter().map(|(name, _, _, _)| name.as_str()).collect();
        println!(
            "
Quiet: {}",
            names.join(", ")
        );
    }
    Ok(())
}

/// `7d`, `30d`, or a bare number of days.
fn parse_days(since: &str) -> Result<i64> {
    let digits = since.trim().trim_end_matches(['d', 'D']);
    digits
        .parse::<i64>()
        .ok()
        .filter(|d| *d >= 0)
        .with_context(|| format!("{since:?} is not a number of days; write it as `7d` or `30`"))
}

/// The day `days` before today, in UTC.
fn day_before(days: i64) -> String {
    let seconds = jiff::Timestamp::now().as_second() - days * 86_400;
    jiff::Timestamp::from_second(seconds)
        .map(|t| t.to_string().split('T').next().unwrap_or_default().to_string())
        .unwrap_or_default()
}

/// Whole days between a recorded timestamp and now.
fn days_since_utc(timestamp: &str) -> Option<i64> {
    let then: jiff::Timestamp = timestamp.parse().ok()?;
    Some(((jiff::Timestamp::now().as_second() - then.as_second()) / 86_400).max(0))
}

/// A count and its noun, agreed: `1 task`, `2 tasks`. One for the whole
/// binary - three copies of it had drifted, and the project's screen said
/// "1 versions planned" while the packet said "1 version planned".
pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A body given on the command line, or read from standard input when it
/// is `-`: a document is prose, and prose arrives from a pipe as often as
/// from a keyboard.
fn body_argument(body: &str) -> Result<String> {
    if body != "-" {
        return Ok(body.to_string());
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).context("cannot read the body from standard input")?;
    Ok(text)
}

/// Opens `seed` in the editor and gives back what was saved.
///
/// The scratch file is removed afterwards whatever happened: it holds the
/// owner's prose, and leaving copies of that in the temporary directory is
/// not something a record tool should do.
fn body_from_editor(project: &str, slug: &str, seed: &str) -> Result<String> {
    let path = doc::scratch_path(project, slug);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    }
    std::fs::write(&path, seed).with_context(|| format!("cannot write {}", path.display()))?;
    let edited = doc::edit_file(&path).and_then(|()| std::fs::read_to_string(&path).with_context(|| format!("cannot read back {}", path.display())));
    let _ = std::fs::remove_file(&path);
    // The directory is this process's, so it goes with the file it held.
    let _ = std::fs::remove_dir(doc::scratch_dir());
    edited
}

/// The body for a new or edited document: what was passed, or what the
/// editor was left with.
fn body_for(project: &str, slug: &str, body: Option<&str>, seed: &str) -> Result<String> {
    match body {
        Some(body) => body_argument(body),
        None => body_from_editor(project, slug, seed),
    }
}

fn doc_list(project: &str, kind: Option<&str>, json: bool) -> Result<()> {
    if let Some(kind) = kind {
        doc::check_kind(kind)?;
    }
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let docs = db.documents(project.id, kind)?;
    if json {
        // Without the bodies: a listing is for finding a document, and the
        // vision of a mature project is longer than the rest of the screen.
        println!(
            "{}",
            serde_json::to_string_pretty(
                &docs
                    .iter()
                    .map(|d| serde_json::json!({
                        "slug": d.slug,
                        "kind": d.kind,
                        "title": d.title,
                        "updated_at": d.updated_at,
                        "bytes": d.body.len(),
                        "file": d.source_file,
                    }))
                    .collect::<Vec<_>>()
            )?
        );
        return Ok(());
    }
    if docs.is_empty() {
        println!("{} has no documents yet.", project.name);
        println!("Write one with: rigger doc add {} \"Vision\" --kind vision", project.name);
        return Ok(());
    }
    println!("{}:", plural(docs.len(), "document", "documents"));
    // The addresses set the column, so one long slug pushes the rest along
    // rather than stepping out of a fixed width and bending the whole table.
    let width = docs.iter().map(|d| d.slug.chars().count()).max().unwrap_or(0).max(12);
    for d in &docs {
        let when = days_since_utc(&d.updated_at)
            .map(|days| match days {
                0 => "today".to_string(),
                1 => "yesterday".to_string(),
                d => format!("{d} days ago"),
            })
            .unwrap_or_else(|| "unknown".to_string());
        println!("  {:<width$} {:<10} {:<12} {}", d.slug, d.kind, when, first_line(&d.title));
    }
    Ok(())
}

fn doc_show(project: &str, slug: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let doc = open_document(&db, &project, slug)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }
    // The body alone, so that `rigger doc show x vision > vision.md` gives
    // back a file rather than a screen with a header glued to the top.
    println!("{}", doc.body.trim_end());
    Ok(())
}

fn doc_add(project: &str, title: &str, kind: &str, slug: Option<&str>, body: Option<&str>, json: bool) -> Result<()> {
    doc::check_kind(kind)?;
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;

    // A vision written twice is a vision nobody reads, so the second one is
    // refused by name rather than made: the fix is to edit the first.
    if doc::is_singular(kind)
        && let Some(existing) = db.documents(project.id, Some(kind))?.first()
    {
        bail!(
            "{} already has a {kind}: '{}'; edit it with `rigger doc edit {} {}`",
            project.name,
            existing.slug,
            project.name,
            existing.slug
        );
    }

    let slug = match slug {
        Some(slug) => slug.to_string(),
        // A title with no ASCII in it - the owner's hub is in Russian -
        // slugs to nothing, and an empty address is no address: the kind
        // plus a number is one that can at least be typed.
        None => match db::slugify(title) {
            slug if !slug.is_empty() => slug,
            _ => next_slug(&db, project.id, kind)?,
        },
    };
    if db.document(project.id, &slug)?.is_some() {
        bail!(
            "{} already has a document at '{slug}'; give another with --slug, or edit that one",
            project.name
        );
    }

    let text = body_for(&project.name, &slug, body, &doc::template(kind, title))?;
    if text.trim().is_empty() {
        if json {
            return print_json(&serde_json::json!({ "document": null, "changed": false }));
        }
        println!("Nothing was written; no document was made.");
        return Ok(());
    }
    let written = db.write_document(project.id, kind, &slug, title, &text)?;
    if json {
        return print_json(&serde_json::json!({ "document": written, "changed": true }));
    }
    println!(
        "Wrote {} ({kind}, {}) to {}",
        written.slug,
        plural(written.body.len(), "byte", "bytes"),
        project.name
    );
    Ok(())
}

fn doc_edit(project: &str, slug: &str, title: Option<&str>, body: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let doc = open_document(&db, &project, slug)?;

    // A title change alone must not open an editor: it was asked for on the
    // command line and is answered there.
    let text = match (title, body) {
        (Some(_), None) => doc.body.clone(),
        _ => body_for(&project.name, &doc.slug, body, &doc.body)?,
    };
    let title = title.unwrap_or(&doc.title);
    if text == doc.body && title == doc.title {
        if json {
            return print_json(&serde_json::json!({ "document": doc, "changed": false }));
        }
        println!("{} is unchanged.", doc.slug);
        return Ok(());
    }
    if text.trim().is_empty() {
        bail!(
            "the document was left empty; nothing was written. Remove it with `rigger doc remove {} {}`",
            project.name,
            doc.slug
        );
    }
    let written = db.write_document(project.id, &doc.kind, &doc.slug, title, &text)?;
    if json {
        return print_json(&serde_json::json!({ "document": written, "changed": true }));
    }
    println!("Wrote {} ({}, {})", written.slug, written.kind, plural(written.body.len(), "byte", "bytes"));
    Ok(())
}

fn doc_remove(project: &str, slug: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    // A kept file is named by its path, which no address can be: an
    // address has no `/` and no extension of a file that is not a text.
    // Tried when no document answers, so a document always wins its name.
    if db.document(project.id, slug)?.is_none() && db.delete_attachment(project.id, slug)? {
        if json {
            return print_json(&serde_json::json!({ "removed": { "project": project.name, "file": slug } }));
        }
        println!(
            "Removed the file {slug} from {}; while the hub still has it, the next import takes it in again",
            project.name
        );
        return Ok(());
    }
    let doc = open_document(&db, &project, slug)?;
    db.delete_document(project.id, &doc.slug)?;
    if json {
        return print_json(&serde_json::json!({ "removed": { "project": project.name, "slug": doc.slug, "kind": doc.kind } }));
    }
    println!("Removed {} ({}) from {}", doc.slug, doc.kind, project.name);
    Ok(())
}

/// The files a hub keeps that are not texts, as the record holds them.
fn doc_files(project: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let files = db.attachments(project.id)?;
    if json {
        return print_json(&serde_json::json!({ "project": project.name, "files": files }));
    }
    if files.is_empty() {
        println!("{} keeps no files besides its texts.", project.name);
        return Ok(());
    }
    let total: u64 = files.iter().map(|f| f.bytes).sum();
    println!("{}, {}:", plural(files.len(), "file", "files"), size_of(total));
    let width = files.iter().map(|f| f.path.chars().count()).max().unwrap_or(0);
    for f in &files {
        println!(
            "  {:<width$}  {:>9}  {}",
            f.path,
            size_of(f.bytes),
            f.updated_at.split('T').next().unwrap_or(&f.updated_at)
        );
    }
    println!("`rigger export {} --docs` writes them back into the hub", project.name);
    Ok(())
}

/// A size for a person: bytes, then kilobytes, then megabytes.
fn size_of(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => plural(usize::try_from(b).unwrap_or(usize::MAX), "byte", "bytes"),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

/// Shows the skeleton a kind starts from, or writes it out to be edited.
///
/// Without this the override is a filename in a doc page: the way to change
/// what a vision asks you should be to run the command that shows it.
fn doc_template(kind: &str, write: bool, json: bool) -> Result<()> {
    doc::check_kind(kind)?;
    if !write {
        let paths = doc::template_paths(kind)?;
        let from = paths.iter().find(|p| p.is_file());
        if json {
            return print_json(&serde_json::json!({
                "kind": kind,
                "template": doc::template(kind, doc::TITLE_PLACEHOLDER),
                "from": from,
                "written": null,
            }));
        }
        println!("{}", doc::template(kind, doc::TITLE_PLACEHOLDER).trim_end());
        println!();
        match from {
            Some(path) => println!("(from {})", path.display()),
            None => println!("(the built-in skeleton; `rigger doc template {kind} --write` to make it yours)"),
        }
        return Ok(());
    }
    let path = doc::template_paths(kind)?
        .into_iter()
        .next()
        .context("the profile has no directory to write a skeleton into")?;
    if path.exists() {
        bail!("{} already exists; edit it, or delete it to go back to the built-in one", path.display());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    }
    // Written with the placeholder in it, so the first edit does not have to
    // discover that the title is substituted.
    std::fs::write(&path, doc::template(kind, doc::TITLE_PLACEHOLDER)).with_context(|| format!("cannot write {}", path.display()))?;
    if json {
        return print_json(&serde_json::json!({
            "kind": kind,
            "template": doc::template(kind, doc::TITLE_PLACEHOLDER),
            "from": null,
            "written": path,
        }));
    }
    println!("Wrote {}", path.display());
    println!("Edit it; `{}` in it becomes the document's title.", doc::TITLE_PLACEHOLDER);
    Ok(())
}

/// Prints how the work is done: the rituals of the line, then those of one
/// project.
///
/// Both are documents, and the split is the point. What every project does
/// the same way (a stage ends in a tag, English is what ships, the gate is
/// green before a commit) is written once, against the profile; what only
/// this project can say about itself is written against the project.
/// Before this, the two were glued together into every skill file, so a
/// change to the line's rituals was seventeen edits, and the copies
/// drifted.
///
/// Read by an assistant at the start of a sitting, which is why it prints
/// a document and nothing around it: no counts, no timestamps, no advice.
fn rules(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let line = db.service_project()?;
    let line_rules = match &line {
        Some(p) => db.singular_document(p.id, db::RULES_KIND)?,
        None => None,
    };
    let project = project.map(|name| open_project(&db, name)).transpose()?;
    let project_rules = match &project {
        Some(p) => db.singular_document(p.id, db::RULES_KIND)?,
        None => None,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "line": line_rules.as_ref().map(|d| &d.body),
                "project": project.as_ref().map(|p| &p.name),
                "project_rules": project_rules.as_ref().map(|d| &d.body),
            }))?
        );
        return Ok(());
    }

    let mut printed = false;
    if let Some(doc) = &line_rules {
        println!("{}", doc.body.trim_end());
        printed = true;
    }
    if let Some(doc) = &project_rules {
        if printed {
            println!();
        }
        println!("{}", doc.body.trim_end());
        printed = true;
    }
    if printed {
        return Ok(());
    }

    // Nothing written yet is a state to get out of, so the way out is the
    // whole message.
    let profile = profile::Config::load()?.current_name();
    match &project {
        Some(p) => println!("Neither the line nor {} has written down how the work is done.", p.name),
        None => println!("The line has not written down how the work is done."),
    }
    println!("Write the line's with:    rigger doc add {profile} \"Rituals\" --kind rituals");
    if let Some(p) = &project {
        println!("And what only {} says:   rigger doc add {} \"Rituals\" --kind rituals", p.name, p.name);
    }
    Ok(())
}

/// A document by its address, with the list of what there is when it is
/// not one: a wrong address is nearly always a typo for a right one.
fn open_document(db: &Db, project: &db::Project, slug: &str) -> Result<db::Document> {
    if let Some(doc) = db.document(project.id, slug)? {
        return Ok(doc);
    }
    let known = db.documents(project.id, None)?;
    if known.is_empty() {
        bail!("{} has no document at '{slug}', and none at all yet", project.name);
    }
    bail!(
        "{} has no document at '{slug}'; it has {}",
        project.name,
        known.iter().map(|d| d.slug.as_str()).collect::<Vec<_>>().join(", ")
    );
}

/// An address for a document whose title gives none: the kind, then the
/// first free number after it.
fn next_slug(db: &Db, project_id: i64, kind: &str) -> Result<String> {
    if db.document(project_id, kind)?.is_none() {
        return Ok(kind.to_string());
    }
    for n in 2.. {
        let candidate = format!("{kind}-{n}");
        if db.document(project_id, &candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    unreachable!("the loop returns on the first free number")
}

/// How many copies `backup` keeps when nothing else is asked.
///
/// Enough that a fault noticed a few sittings late still has a copy from
/// before it, few enough that a database of a few megabytes does not turn
/// its own directory into a disk problem.
const KEEP_BACKUPS: usize = 10;

/// When `doctor` starts saying the insurance is old, and when it says it is
/// a problem. A day is one sitting's worth of work at risk; a week is the
/// point where the copy no longer resembles the record.
const BACKUP_STALE_DAYS: i64 = 1;
const BACKUP_OLD_DAYS: i64 = 7;

/// How `doctor` judges the age of the newest copy: fresh, stale, old, or
/// none at all. One function so the JSON and the printed line cannot drift.
fn backup_state(age_days: Option<i64>) -> &'static str {
    match age_days {
        None => "none",
        Some(d) if d >= BACKUP_OLD_DAYS => "old",
        Some(d) if d >= BACKUP_STALE_DAYS => "stale",
        Some(_) => "fresh",
    }
}

fn backup(keep: usize, list: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if list {
        let copies = db.backups()?;
        if json {
            let rows: Vec<serde_json::Value> = copies
                .iter()
                .map(|path| {
                    let taken_at = path.file_name().and_then(|n| db::stamp_of(&n.to_string_lossy()));
                    serde_json::json!({ "path": path, "taken_at": taken_at })
                })
                .collect();
            return print_json(&serde_json::json!({ "backups": rows }));
        }
        if copies.is_empty() {
            println!("No copies yet. Take one with: rigger backup");
            return Ok(());
        }
        println!("{} newest first:", plural(copies.len(), "copy", "copies"));
        for path in &copies {
            let age = path
                .file_name()
                .and_then(|n| db::stamp_of(&n.to_string_lossy()))
                .and_then(|at| days_since_utc(&at))
                .map(|d| match d {
                    0 => "today".to_string(),
                    1 => "yesterday".to_string(),
                    d => format!("{d} days ago"),
                })
                .unwrap_or_else(|| "unknown".to_string());
            println!("  {:<40} {age}", path.file_name().unwrap_or_default().to_string_lossy());
        }
        return Ok(());
    }
    let target = db.backup()?;
    let removed = db.prune_backups(keep)?;
    if json {
        return print_json(&serde_json::json!({ "backup": target, "removed": removed }));
    }
    println!("Copied to {}", target.display());
    if !removed.is_empty() {
        println!("Kept the {keep} newest, deleted {}.", plural(removed.len(), "older copy", "older copies"));
    }
    Ok(())
}

fn init(json: bool) -> Result<()> {
    // The config first, so that a fresh install has a profile to speak of
    // and the file a person can edit is where `doctor` says it is.
    let config_path = profile::Config::path()?;
    let config_created = !config_path.exists();
    if config_created {
        profile::Config::default().save()?;
        if !json {
            println!("Created {} with the '{}' profile", config_path.display(), profile::DEFAULT);
        }
    }
    let path = paths::db_path()?;
    let answer = |db: &Db, created: bool| -> Result<()> {
        print_json(&serde_json::json!({
            "config": config_path,
            "config_created": config_created,
            "profile": profile::Config::load()?.current_name(),
            "database": db.path(),
            "created": created,
            "schema": db.schema_version()?,
        }))
    };
    if path.exists() {
        let db = Db::open(&path)?;
        if json {
            return answer(&db, false);
        }
        println!("Already initialised: {}", path.display());
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let db = Db::create(&path)?;
    if json {
        return answer(&db, true);
    }
    println!("Created {} (schema version {})", db.path().display(), db.schema_version()?);
    println!("Next: rigger project add <path>");
    Ok(())
}

fn profile_list(json: bool) -> Result<()> {
    let config = profile::Config::load()?;
    let current = config.current_name();
    if json {
        let rows: Vec<serde_json::Value> = config
            .profiles
            .iter()
            .map(|(name, p)| serde_json::json!({ "name": name, "current": *name == current, "profile": p }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    let width = config.profiles.keys().map(String::len).max().unwrap_or(0);
    for (name, p) in &config.profiles {
        let mark = if *name == current { "*" } else { " " };
        println!("{mark} {name:width$}  {}  {}", p.kind.as_str(), profile::db_path_for(name)?.display());
    }
    Ok(())
}

fn profile_show(name: Option<&str>, json: bool) -> Result<()> {
    let config = profile::Config::load()?;
    let name = name.map(str::to_string).unwrap_or_else(|| config.current_name());
    let Some(p) = config.profiles.get(&name) else {
        bail!("no profile named '{name}'; see `rigger profile list`");
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "name": name, "profile": p }))?);
        return Ok(());
    }
    println!("{name}");
    println!("  kind:      {}", p.kind.as_str());
    println!("  database:  {}", profile::db_path_for(&name)?.display());
    if !p.roots.is_empty() {
        println!(
            "  roots:     {}",
            p.roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    if let Some(hubs) = &p.hubs {
        println!("  hubs:      {}", hubs.display());
    }
    if let Some(pattern) = &p.id_pattern {
        println!("  ids:       {pattern}");
    }
    match &p.trays {
        Some(trays) => println!("  trays:     {}", trays.display()),
        None => println!("  trays:     {}", profile::dir(&name)?.join("trays").display()),
    }
    println!("  config:    {}", profile::Config::path()?.display());
    Ok(())
}

fn profile_use(name: &str, json: bool) -> Result<()> {
    let mut config = profile::Config::load()?;
    if !config.profiles.contains_key(name) {
        bail!("no profile named '{name}'; see `rigger profile list`");
    }
    config.current = name.to_string();
    config.save()?;
    if json {
        return profile_show(Some(name), true);
    }
    println!("Every command now uses the '{name}' profile");
    if std::env::var_os(profile::PROFILE_ENV).is_some() {
        println!("  note: {} is set and overrides this while it is", profile::PROFILE_ENV);
    }
    Ok(())
}

/// Adds a profile and its database. The database is created here rather
/// than on first use, so that `profile list` can point at a file that is
/// there.
fn profile_add(name: &str, p: profile::Profile, use_it: bool, json: bool) -> Result<()> {
    if name.trim().is_empty() || name.contains(['/', '\\', ' ']) {
        bail!("a profile name is one word, without slashes: '{name}' is not");
    }
    let mut config = profile::Config::load()?;
    if config.profiles.contains_key(name) {
        bail!("a profile named '{name}' already exists; see `rigger profile show {name}`");
    }
    let path = profile::db_path_for(name)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    if !path.exists() {
        Db::create(&path)?;
    }
    config.profiles.insert(name.to_string(), p.clone());
    if use_it {
        config.current = name.to_string();
    }
    config.save()?;
    if json {
        return profile_show(Some(name), true);
    }
    println!("Added the '{name}' profile ({}) with its database at {}", p.kind.as_str(), path.display());
    match use_it {
        true => println!("Every command now uses it"),
        false => println!("Switch to it with: rigger profile use {name}"),
    }
    Ok(())
}

/// Changes what a profile says about itself. Only the fields given change;
/// the roots given replace the roots it had, because a list appended to
/// can never be shortened.
fn profile_set(name: Option<&str>, roots: Vec<PathBuf>, hubs: Option<PathBuf>, id_pattern: Option<String>, trays: Option<PathBuf>, json: bool) -> Result<()> {
    let mut config = profile::Config::load()?;
    let name = name.map(str::to_string).unwrap_or_else(|| config.current_name());
    let Some(p) = config.profiles.get_mut(&name) else {
        bail!("no profile named '{name}'; see `rigger profile list`");
    };
    if roots.is_empty() && hubs.is_none() && id_pattern.is_none() && trays.is_none() {
        bail!("nothing to set; give --root, --hubs, --id-pattern or --trays");
    }
    if !roots.is_empty() {
        p.roots = roots;
    }
    if let Some(hubs) = hubs {
        p.hubs = Some(hubs);
    }
    if let Some(pattern) = id_pattern {
        p.id_pattern = Some(pattern);
    }
    if let Some(trays) = trays {
        p.trays = Some(trays);
    }
    config.save()?;
    if json {
        return profile_show(Some(&name), true);
    }
    println!("Profile '{name}' updated");
    profile_show(Some(&name), false)
}

/// The setting that names the card in hand, and when it was taken up.
const ACTIVE_CARD: &str = "active_card";
const ACTIVE_SINCE: &str = "active_since";
/// A card left open this long is not in hand any more: a desk that forgot
/// to close it yesterday must not be told today that it is.
const STALE_HOURS: i64 = 8;

/// Prints one JSON document on stdout: what `--json` promises of every
/// command.
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// A card after a write, as `--json` prints it: the card as it now stands,
/// and the repositories it is worked in.
fn print_card_json(db: &Db, card_id: i64) -> Result<()> {
    let card = db.card(card_id)?.context("the card just written is not in the record")?;
    print_json(&serde_json::json!({ "card": card, "links": db.card_links(card_id)? }))
}

/// A task after a write, as `--json` prints it: a card or a task of a plan.
fn print_task_json(db: &Db, task_id: i64) -> Result<()> {
    let task = db.task_written(task_id)?.context("the task just written is not in the record")?;
    print_json(&serde_json::json!({ "task": task }))
}

/// An event after a write, as `--json` prints it, with whether this write
/// is what put it there or the record already held it.
fn print_event_json(db: &Db, id: Option<i64>, changed: bool) -> Result<()> {
    let event = match id {
        Some(id) => db.event_written(id)?,
        None => None,
    }
    .context("the event just written is not in the record")?;
    print_json(&serde_json::json!({ "event": event, "changed": changed }))
}

/// A project as `project show --json` prints it, read again after a write.
fn print_project_json(db: &Db, name: &str) -> Result<()> {
    let project = db.project_by_name(name)?.context("the project just written is not in the record")?;
    print_json(&project)
}

/// A card by whatever names it, or a clear refusal.
fn find_card(db: &Db, text: &str) -> Result<card::Card> {
    match db.card_by_ref(text)? {
        Some(card) => Ok(card),
        None => bail!("no card named '{text}'; `rigger task find` looks one up, `rigger task new` makes one"),
    }
}

/// The task a reference means - a card by key or alias, or a plain task
/// of a project by id - with the project it belongs to.
fn find_task(db: &Db, text: &str) -> Result<(i64, i64)> {
    if let Some(card) = db.card_by_ref(text)? {
        return Ok((db.desk_project()?.id, card.id));
    }
    if let Ok(id) = text.trim().parse::<i64>()
        && let Some(project_id) = db.task_owner(id)?
    {
        return Ok((project_id, id));
    }
    bail!("no task named '{text}'; a card's key, or the id the packet lists")
}

fn task_status(task: &str, status: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (project_id, id) = find_task(&db, task)?;
    let (title, was) = db.set_task_status(project_id, id, status)?;
    if json {
        return print_task_json(&db, id);
    }
    match was == status {
        true => println!("Task {task} was already {status}: {title}"),
        false => println!("Task {task} is now {status} (was {was}): {title}"),
    }
    Ok(())
}

/// Lets go of the card in hand if it is this task: work put down is not
/// work in hand.
fn let_go_of(db: &Db, task_id: i64) -> Result<()> {
    if db.setting(ACTIVE_CARD)?.as_deref() == Some(&task_id.to_string()) {
        db.set_setting(ACTIVE_CARD, None)?;
        db.set_setting(ACTIVE_SINCE, None)?;
    }
    Ok(())
}

/// Puts a task to sleep until a day, or wakes it when `until` is `None`.
fn task_snooze(task: &str, until: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (project_id, id) = find_task(&db, task)?;
    let today = db::today();
    let day = match until {
        Some(text) => {
            let day = db::parse_day(text, &today)?;
            if day <= today {
                bail!("{day} is not after today; a task sleeps until a day still to come");
            }
            Some(day)
        }
        None => None,
    };
    let (title, _, was) = db.set_snooze(project_id, id, day.as_deref())?;
    if day.is_some() {
        let_go_of(&db, id)?;
    }
    if json {
        return print_task_json(&db, id);
    }
    match (&day, was) {
        (Some(day), _) => {
            println!("Task {task} sleeps until {day}: {title}");
            println!("  out of the packet, the list and the hand until then; `rigger task unsnooze {task}` wakes it sooner");
        }
        (None, Some(was)) => println!("Task {task} is awake (it slept until {was}): {title}"),
        (None, None) => println!("Task {task} was not asleep: {title}"),
    }
    Ok(())
}

/// Sets a task aside on purpose. The reason is the point: a frozen task
/// that does not say why is a question for whoever finds it.
fn task_freeze(task: &str, why: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (project_id, id) = find_task(&db, task)?;
    let (title, was) = db.set_task_status(project_id, id, "frozen")?;
    if let Some(why) = why.map(str::trim).filter(|w| !w.is_empty()) {
        db.record_task_event(project_id, id, "decision", &format!("Set aside: {why}"), &db::now(), "assistant")?;
    }
    let_go_of(&db, id)?;
    if json {
        return print_task_json(&db, id);
    }
    match was.as_str() {
        "frozen" => println!("Task {task} was already frozen: {title}"),
        _ => println!("Task {task} is frozen (was {was}): {title}"),
    }
    println!("  `rigger task status {task} active` takes it up again");
    Ok(())
}

/// Hands a card over to the team that owns the fix. The first run writes
/// the form into the card's tray; the run after it is filled makes both
/// texts, marks the card as waiting on that team, and says so on the card.
fn task_handoff(task: Option<&str>, to: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = card_or_in_hand(&db, task, "task handoff")?;
    let to = handoff::slug(to);
    if to.is_empty() {
        bail!("say who it goes to with --to: a product or a team, such as alerts-api");
    }
    let tray = tray::locate(&tray::root()?, &card)?;
    let files = handoff::files_for(&tray, &to, &db::today())?;
    if !files.form.is_file() {
        let material: Vec<String> = card
            .summary
            .iter()
            .map(|s| format!("summary: {s}"))
            .chain(
                db.task_events(card.id)?
                    .iter()
                    .filter(|e| matches!(e.kind.as_str(), "finding" | "decision" | "pitfall"))
                    .map(|e| format!("{} · {}: {}", e.date, e.kind, e.body.trim())),
            )
            .collect();
        handoff::ensure_form(&files, &handoff::blank_form(&card, &to, &material))?;
        if json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "state": "form", "files": files }))?);
            return Ok(());
        }
        println!("Wrote the form for a handoff of {} to {to}:", card.key);
        println!("  {}", files.form.display());
        println!("Fill it in, then run `rigger task handoff {} --to {to}` again to make the texts.", card.key);
        return Ok(());
    }
    let (parts, text) = handoff::send(&files)?;
    let desk = db.desk_project()?;
    let (_, was) = db.set_task_status(desk.id, card.id, "waiting-handoff")?;
    let rel = files
        .markdown
        .strip_prefix(&tray)
        .unwrap_or(&files.markdown)
        .display()
        .to_string()
        .replace('\\', "/");
    db.record_task_event(
        desk.id,
        card.id,
        "change",
        &format!("Handed over to {to}: {} ({rel})", parts.title().unwrap_or_default()),
        &db::now(),
        "assistant",
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "state": "sent", "files": files, "status": "waiting-handoff", "text": text }))?
        );
        return Ok(());
    }
    print!("{text}");
    println!();
    println!("Made the texts for {to}:");
    println!("  {}", files.markdown.display());
    println!("  {}", files.jira.display());
    match was.as_str() {
        "waiting-handoff" => println!("{} was already waiting on a handoff.", card.key),
        _ => println!("{} is now waiting-handoff (was {was}); nothing was sent - that is yours to do.", card.key),
    }
    Ok(())
}

fn task_new(title: &str, key: Option<&str>, aliases: &[String], projects: &[String], branch: Option<&str>, summary: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if title.trim().is_empty() {
        bail!("a card needs a title");
    }
    let card = db.new_card(key, title, aliases)?;
    if let Some(summary) = summary {
        db.set_card_summary(card.id, summary)?;
    }
    link_and_hold(&db, &card, projects, branch)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("Made card {} and opened it: {}", card.key, card.title);
    if key.is_none() {
        println!("  a local key; `rigger task rename {} <ID>` when the tracker names it", card.key);
    }
    print_links(&db, card.id)?;
    Ok(())
}

/// Links a card to the projects given and puts it in hand: nobody makes a
/// card for a task they are not about to work on.
fn link_and_hold(db: &Db, card: &card::Card, projects: &[String], branch: Option<&str>) -> Result<()> {
    for name in projects {
        let project = working_project(db, name)?;
        db.link_card(card.id, project.id, branch, None)?;
    }
    db.set_setting(ACTIVE_CARD, Some(&card.id.to_string()))?;
    db.set_setting(ACTIVE_SINCE, Some(&db::now()))?;
    Ok(())
}

fn print_links(db: &Db, task_id: i64) -> Result<()> {
    for link in db.card_links(task_id)? {
        let branch = link.branch.as_deref().map(|b| format!(" on {b}")).unwrap_or_default();
        let role = link.role.as_deref().map(|r| format!(" - {r}")).unwrap_or_default();
        println!("  {} ({}){branch}{role}", link.project, link.path);
    }
    Ok(())
}

fn task_find(query: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let parsed = card::parse(query);
    let cards = db.cards(None)?;
    let (hits, verdict) = card::find(&parsed, &cards, 8);
    // The weak trail: an id or a case number mentioned in the text of
    // other cards - a task that moved leaves its old number behind.
    let shown: Vec<String> = hits.iter().map(|h| h.key.clone()).collect();
    let mut mentions = Vec::new();
    for needle in parsed.ids.iter().chain(parsed.numbers.iter()) {
        for (key, title) in db.cards_mentioning(needle, &shown)? {
            mentions.push((needle.clone(), key, title));
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "verdict": verdict,
                "hits": hits,
                "mentions": mentions.iter().map(|(n, k, t)| serde_json::json!({ "needle": n, "key": k, "title": t })).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    if cards.is_empty() {
        println!("No cards yet. Make one with: rigger task new \"<title>\" [--id <KEY>]");
        return Ok(());
    }
    if hits.is_empty() {
        println!("No match among {} cards.", cards.len());
    } else {
        let width = hits.iter().map(|h| h.key.len()).max().unwrap_or(0);
        for h in &hits {
            println!("{:>3}  {:width$}  {:<16} {}  ({})", h.score, h.key, h.status, h.title, h.why);
        }
    }
    for (needle, key, title) in &mentions {
        println!("mentioned {needle}: {key} - {title}");
    }
    println!(
        "\n{}",
        match verdict {
            card::Verdict::Take => format!("take {}: it is the one.", hits[0].key),
            card::Verdict::Ask => "ask: show these and let the owner pick, or say it is new.".to_string(),
            card::Verdict::New => "new: nothing is close; make a card.".to_string(),
        }
    );
    Ok(())
}

/// The card in hand, unless it was left open longer than a working day.
fn active_card(db: &Db) -> Result<Option<card::Card>> {
    let Some(id) = db.setting(ACTIVE_CARD)? else { return Ok(None) };
    if let Some(since) = db.setting(ACTIVE_SINCE)?
        && let Ok(then) = since.parse::<jiff::Timestamp>()
        && (jiff::Timestamp::now().as_second() - then.as_second()) > STALE_HOURS * 3600
    {
        return Ok(None);
    }
    // A card asleep is not in hand, even if it was put to sleep by
    // another tool than `task snooze`.
    Ok(db.card(id.parse().unwrap_or_default())?.filter(|c| c.snoozed_until.is_none()))
}

fn task_open(task: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    db.set_setting(ACTIVE_CARD, Some(&card.id.to_string()))?;
    db.set_setting(ACTIVE_SINCE, Some(&db::now()))?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("Card {} is in hand: {}", card.key, card.title);
    Ok(())
}

fn task_active(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = active_card(&db)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&card)?);
        return Ok(());
    }
    match card {
        Some(card) => println!("{}  {:<16} {}", card.key, card.status, card.title),
        None => println!("No card is in hand. Open one with: rigger task open <KEY>"),
    }
    Ok(())
}

/// The card named, or the one in hand when none is; `command` is what to
/// name one with when neither is there.
fn card_or_in_hand(db: &Db, task: Option<&str>, command: &str) -> Result<card::Card> {
    match task {
        Some(text) => find_card(db, text),
        None => match active_card(db)? {
            Some(card) => Ok(card),
            None => bail!("no card is in hand; name one: rigger {command} <KEY>"),
        },
    }
}

fn task_close(task: Option<&str>, status: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = card_or_in_hand(&db, task, "task close")?;
    let desk = db.desk_project()?;
    let (title, was) = db.set_task_status(desk.id, card.id, status)?;
    let_go_of(&db, card.id)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("Closed {} as {status} (was {was}): {title}", card.key);
    Ok(())
}

fn task_show(task: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let links = db.card_links(card.id)?;
    let events = db.task_events(card.id)?;
    let activity = db.card_activity(card.id)?;
    let idle = idle_days(&activity);
    let ticket = kasl::ticket_of(&card, &kasl::inbox());
    let tray_dir = tray::locate(&tray::root()?, &card)?;
    let tray = tray::read(&tray_dir)?;
    let handoffs = handoff::list(&tray_dir)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "card": card,
                "links": links,
                "activity": activity,
                "idle_days": idle,
                "events": events.len(),
                "ticket": ticket,
                "tray": tray,
                "handoffs": handoffs,
            }))?
        );
        return Ok(());
    }
    println!("{} · {}", card.key, card.title);
    println!("  status:   {}", card.status);
    if let Some(day) = &card.snoozed_until {
        println!("  asleep:   until {day}");
    }
    if !card.aliases.is_empty() {
        println!("  aliases:  {}", card.aliases.join(", "));
    }
    println!("  since:    {}", card.created_at);
    if let Some(summary) = &card.summary {
        println!("  summary:  {summary}");
    }
    if let Some(ticket) = &ticket {
        println!("  ticket:   {} (kasl)", ticket.facts());
        if let Some(url) = &ticket.url {
            println!("            {url}");
        }
    }
    if !links.is_empty() {
        println!("  worked in:");
        print_links(&db, card.id)?;
    }
    let mut counts = std::collections::BTreeMap::new();
    for e in &events {
        *counts.entry(e.kind.as_str()).or_insert(0) += 1;
    }
    if !counts.is_empty() {
        let parts: Vec<String> = counts.iter().map(|(k, n)| format!("{n} {k}")).collect();
        println!("  recorded: {}", parts.join(", "));
    }
    if let Some(next) = events.iter().rev().find(|e| e.kind == "next") {
        println!("  next:     {}", first_line(&next.body));
    }
    if !activity.is_empty() {
        println!("  in git:");
        for line in activity_lines(&activity) {
            println!("    {line}");
        }
        if let Some(days) = idle {
            println!("  idle:     {}", describe_idle(days));
        }
    } else if !links.is_empty() {
        println!("  in git:   nothing yet - `rigger sync` reads branches and commits that carry the key");
    }
    if let Some(tray) = &tray {
        println!("  tray:     {} - {}", tray.path, tray_state(tray));
    }
    for line in handoff_lines(&handoffs) {
        println!("  handoff:  {line}");
    }
    Ok(())
}

/// A tray in half a line: the form, what waits, what was sorted.
fn tray_state(tray: &tray::Tray) -> String {
    let mut parts = vec![if tray.form.filled { "the form filled" } else { "the form blank" }.to_string()];
    parts.push(match tray.files.len() {
        0 => "nothing waiting".to_string(),
        n => format!("{} waiting", plural(n, "file", "files")),
    });
    if let Some(last) = tray.sorted.last() {
        parts.push(format!("sorted {}", last.day));
    }
    parts.join(", ")
}

/// Whole days since a card last moved in any repository: the newest of its
/// commits and branch tips. `None` when git has said nothing about it.
fn idle_days(activity: &[card::Activity]) -> Option<i64> {
    activity
        .iter()
        .filter_map(|a| a.last_commit.as_ref())
        .map(|c| c.at.as_str())
        .max()
        .and_then(activity::days_since)
}

fn describe_idle(days: i64) -> String {
    match days {
        0 => "moved today".to_string(),
        1 => "1 day without movement".to_string(),
        n => format!("{n} days without movement"),
    }
}

/// A card's activity, a line per repository and one per branch.
fn activity_lines(activity: &[card::Activity]) -> Vec<String> {
    let mut out = Vec::new();
    for a in activity {
        let mut line = a.project.clone();
        if a.commits > 0 {
            let plural = if a.commits == 1 { "commit names" } else { "commits name" };
            line.push_str(&format!(" - {} {plural} it", a.commits));
        }
        if let Some(last) = &a.last_commit {
            let short: String = last.hash.chars().take(8).collect();
            line.push_str(&format!("; last {} {short} {}", day_of(&last.at), last.subject));
        }
        out.push(line);
        for b in &a.branches {
            let place = b.remote.as_deref().map(|r| format!(" (only on {r})")).unwrap_or_default();
            out.push(format!("  branch {}{place}, tip {}", b.name, day_of(&b.tip_at)));
        }
    }
    out
}

fn day_of(moment: &str) -> &str {
    moment.split('T').next().unwrap_or(moment)
}

/// The packet a task starts from: what it is, where it is worked, and
/// everything written against it, by kind, newest first.
fn render_card(db: &Db, card: &card::Card, budget: usize) -> Result<String> {
    let links = db.card_links(card.id)?;
    let events = db.task_events(card.id)?;
    let mut out = format!("# {} · {}\n\n", card.key, card.title);
    out.push_str(&format!("Status: {}", card.status));
    if let Some(day) = &card.snoozed_until {
        out.push_str(&format!(" · asleep until {day}"));
    }
    if !card.aliases.is_empty() {
        out.push_str(&format!(" · also {}", card.aliases.join(", ")));
    }
    out.push('\n');
    if let Some(summary) = &card.summary {
        out.push_str(&format!("\n{summary}\n"));
    }
    if let Some(ticket) = kasl::ticket_of(card, &kasl::inbox()) {
        out.push_str("\n## Ticket\n");
        let facts = ticket.facts();
        if !facts.is_empty() {
            out.push_str(&format!("{facts}\n"));
        }
        if let Some(url) = &ticket.url {
            out.push_str(&format!("{url}\n"));
        }
    }
    if !links.is_empty() {
        out.push_str("\n## Worked in\n");
        for link in &links {
            let branch = link.branch.as_deref().map(|b| format!(" on `{b}`")).unwrap_or_default();
            let role = link.role.as_deref().map(|r| format!(" - {r}")).unwrap_or_default();
            out.push_str(&format!("- {} ({}){branch}{role}\n", link.project, link.path));
        }
    }
    let activity = db.card_activity(card.id)?;
    if !activity.is_empty() {
        out.push_str("\n## In git\n");
        for line in activity_lines(&activity) {
            out.push_str(&format!("- {line}\n"));
        }
        if let Some(days) = idle_days(&activity) {
            out.push_str(&format!("- {}\n", describe_idle(days)));
        }
    }
    let tray_dir = tray::locate(&tray::root()?, card)?;
    if let Some(tray) = tray::read(&tray_dir)? {
        out.push_str(&tray_section(&tray));
    }
    let handoffs = handoff_lines(&handoff::list(&tray_dir)?);
    if !handoffs.is_empty() {
        out.push_str("\n## Handed over\n");
        for line in handoffs {
            out.push_str(&format!("- {line}\n"));
        }
    }
    if let Some(next) = events.iter().rev().find(|e| e.kind == "next") {
        out.push_str(&format!("\n## Next step\n{}\n", next.body.trim()));
    }
    // Newest first within a kind, and the kinds in the order a session
    // reads them: what was decided, what was found, what bit, what is
    // planned, what changed.
    let sections = [
        ("decision", "Decisions"),
        ("finding", "Findings"),
        ("pitfall", "Pitfalls"),
        ("plan", "Plan"),
        ("change", "Changes"),
        ("question", "Waiting for the owner"),
    ];
    let mut left_out = 0usize;
    for (kind, heading) in sections {
        let mut items: Vec<&db::RecentEvent> = events.iter().filter(|e| e.kind == kind).collect();
        items.reverse();
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {heading}\n"));
        for item in items {
            let line = format!("- {} · {}\n", item.date, item.body.trim());
            if context::estimate_tokens(&out) + context::estimate_tokens(&line) > budget {
                left_out += 1;
                continue;
            }
            out.push_str(&line);
        }
    }
    if left_out > 0 {
        out.push_str(&format!("\n({left_out} older events left out by the budget)\n"));
    }
    Ok(out)
}

/// A card's handoffs, a line each: to whom, when, and whether the texts
/// were made or the form is still being written.
fn handoff_lines(handoffs: &[handoff::Handed]) -> Vec<String> {
    handoffs
        .iter()
        .map(|h| match (&h.title, h.sent) {
            (Some(title), true) => format!("to {} on {}: {title} ({})", h.to, h.day, h.path),
            _ => format!("to {}: the form begun {}, not made into texts yet ({})", h.to, h.day, h.path),
        })
        .collect()
}

/// How many of a tray's files the packet names; the rest are counted.
const TRAY_FILES_NAMED: usize = 10;

/// The tray in a card's packet: where it is, whether the form says
/// anything, and what waits - the material a sitting starts by reading.
fn tray_section(tray: &tray::Tray) -> String {
    let mut out = format!("\n## Tray\n{}\n", tray.path);
    match tray.form.filled {
        true => out.push_str(&format!("- the form is filled; read it first: {}\n", tray.form.path)),
        false => out.push_str("- the form is blank\n"),
    }
    if !tray.files.is_empty() {
        let mut named: Vec<String> = tray
            .files
            .iter()
            .take(TRAY_FILES_NAMED)
            .map(|f| format!("{} ({})", f.path, tray::size(f.bytes)))
            .collect();
        if tray.files.len() > TRAY_FILES_NAMED {
            named.push(format!("and {} more", tray.files.len() - TRAY_FILES_NAMED));
        }
        out.push_str(&format!("- {} waiting: {}\n", plural(tray.files.len(), "file", "files"), named.join(", ")));
    }
    if !tray.sorted.is_empty() {
        let rounds: Vec<String> = tray
            .sorted
            .iter()
            .map(|r| format!("{} ({})", r.day, plural(r.files, "file", "files")))
            .collect();
        out.push_str(&format!("- sorted before: {}\n", rounds.join(", ")));
    }
    out
}

fn task_context(task: &str, budget: usize, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let packet = render_card(&db, &card, budget)?;
    if json {
        return print_json(&serde_json::json!({ "card": card, "packet": packet }));
    }
    print!("{packet}");
    Ok(())
}

fn task_link(task: &str, project: &str, branch: Option<&str>, role: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let project = working_project(&db, project)?;
    db.link_card(card.id, project.id, branch, role)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("{} is worked in:", card.key);
    print_links(&db, card.id)
}

fn task_list(status: &str, snoozed: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (cards, asleep): (Vec<card::Card>, Vec<card::Card>) = db.cards(Some(status))?.into_iter().partition(|c| snoozed || c.snoozed_until.is_none());
    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }
    // Every card asleep is not "no cards": the line below says what there is.
    if cards.is_empty() && asleep.is_empty() {
        println!(
            "No cards{}.",
            if status == "all" { "" } else { " that are " }.to_string() + if status == "all" { "" } else { status }
        );
    }
    let width = cards.iter().map(|c| c.key.len()).max().unwrap_or(0);
    for c in &cards {
        let zzz = c.snoozed_until.as_deref().map(|d| format!("  (asleep until {d})")).unwrap_or_default();
        println!("{:width$}  {:<16} {}{zzz}", c.key, c.status, c.title);
    }
    if !asleep.is_empty() {
        println!(
            "{} asleep; `rigger task list --snoozed` shows {}",
            plural(asleep.len(), "card", "cards"),
            if asleep.len() == 1 { "it" } else { "them" }
        );
    }
    Ok(())
}

fn task_rename(task: &str, key: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let renamed = db.rename_card(card.id, key)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("{} is now {}; {} stays as an alias", card.key, renamed.key, card.key);
    Ok(())
}

fn task_alias(task: &str, alias: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let card = db.add_alias(card.id, alias)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("{} also goes by: {}", card.key, card.aliases.join(", "));
    Ok(())
}

fn task_summary(task: &str, text: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    db.set_card_summary(card.id, text)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("{} summarised", card.key);
    Ok(())
}

/// kasl's inbox, or why it cannot be read: the commands that ask for it
/// by name are told, where a card's screen stays silent. `otherwise` is
/// what the person can do without it.
fn read_kasl_inbox(otherwise: Option<&str>) -> Result<Vec<kasl::Issue>> {
    let otherwise = otherwise.map(|o| format!("\n  {o}")).unwrap_or_default();
    match kasl::inbox() {
        kasl::Inbox::Read(issues) => Ok(issues),
        kasl::Inbox::Absent => bail!("no kasl here to read tickets from; rigger reads them from kasl's inbox (`kasl inbox list --json`){otherwise}"),
        kasl::Inbox::Refused(why) => bail!(
            "kasl did not list its inbox ({why})\n  rigger reads `kasl inbox list --all --snoozed --json`; a kasl without `--json` on its inbox cannot answer it yet{otherwise}"
        ),
    }
}

/// The card a ticket is, by its key or an alias.
fn card_of<'a>(cards: &'a [card::Card], issue: &kasl::Issue) -> Option<&'a card::Card> {
    cards
        .iter()
        .find(|c| c.key.eq_ignore_ascii_case(&issue.key) || c.aliases.iter().any(|a| a.eq_ignore_ascii_case(&issue.key)))
}

/// How many tickets without a card are named before the rest are counted.
const INCOMING_NAMED: usize = 10;

/// The tickets kasl holds that the desk has not taken, and the cards whose
/// ticket has gone - the two things the inbox says that a card does not.
fn task_incoming(all: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let issues = read_kasl_inbox(None)?;
    let cards = db.cards(None)?;
    if json {
        let rows: Vec<serde_json::Value> = issues
            .iter()
            .map(|issue| {
                let mut row = serde_json::to_value(issue).unwrap_or_default();
                row["card"] = match card_of(&cards, issue) {
                    Some(c) => serde_json::json!({ "key": c.key, "title": c.title, "status": c.status }),
                    None => serde_json::Value::Null,
                };
                row
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "source": "kasl", "issues": rows }))?);
        return Ok(());
    }

    // Taken in kasl first: the person has already said those are theirs.
    let mut without: Vec<&kasl::Issue> = issues
        .iter()
        .filter(|i| !i.is_gone() && !i.is_asleep() && card_of(&cards, i).is_none())
        .collect();
    without.sort_by_key(|i| i.taken_at.is_none());
    let gone: Vec<(&kasl::Issue, &card::Card)> = issues
        .iter()
        .filter(|i| i.is_gone())
        .filter_map(|i| card_of(&cards, i).map(|c| (i, c)))
        .filter(|(_, c)| db::is_open_status(&c.status))
        .collect();

    let present = issues.iter().filter(|i| !i.is_gone()).count();
    println!(
        "kasl's inbox: {} - {} without a card, {} gone with the card still open",
        plural(present, "ticket", "tickets"),
        without.len(),
        gone.len()
    );
    if without.is_empty() && gone.is_empty() {
        println!("Every ticket in it has a card, and no open card's ticket has gone.");
        return Ok(());
    }
    if !without.is_empty() {
        println!("\nWithout a card:");
        let shown = if all { without.len() } else { without.len().min(INCOMING_NAMED) };
        let width = without.iter().map(|i| i.key.len()).max().unwrap_or(0);
        for issue in &without[..shown] {
            let taken = if issue.taken_at.is_some() { "taken" } else { "" };
            let status = issue.status.as_deref().unwrap_or("");
            println!("  {:width$}  {taken:<5}  {status:<14} {}", issue.key, issue.summary);
        }
        if shown < without.len() {
            println!("  and {} more: rigger task incoming --all", without.len() - shown);
        }
        println!("Take one with: rigger task take <KEY>");
    }
    if !gone.is_empty() {
        println!("\nThe ticket has gone - closed or reassigned - and the card is open:");
        let width = gone.iter().map(|(_, c)| c.key.len()).max().unwrap_or(0);
        for (issue, card) in &gone {
            let when = issue.gone_at.as_deref().map(kasl::day).unwrap_or("");
            println!("  {:width$}  {:<16} gone {when}  {}", card.key, card.status, card.title);
        }
        println!("Close one with: rigger task close <KEY>");
    }
    Ok(())
}

/// Makes a card from a ticket in kasl's inbox: the key and the title are
/// the tracker's, so nobody types them twice. A ticket that already has a
/// card is put in hand instead - taking is what the person meant.
fn task_take(key: &str, projects: &[String], branch: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if let Some(card) = db.card_by_ref(key)? {
        link_and_hold(&db, &card, projects, branch)?;
        if json {
            return print_card_json(&db, card.id);
        }
        println!("{} already has a card; it is in hand: {}", card.key, card.title);
        print_links(&db, card.id)?;
        return Ok(());
    }
    let by_hand = format!("`rigger task new \"<title>\" --id {key}` makes the card by hand");
    let issues = read_kasl_inbox(Some(&by_hand))?;
    let Some(issue) = issues.iter().find(|i| i.key.eq_ignore_ascii_case(key.trim())) else {
        bail!("kasl's inbox does not know {key}; `rigger task new \"<title>\" --id {key}` makes the card by hand");
    };
    let card = db.new_card(Some(&issue.key), &issue.summary, &[])?;
    link_and_hold(&db, &card, projects, branch)?;
    if json {
        return print_card_json(&db, card.id);
    }
    println!("Made card {} from kasl's inbox and opened it: {}", card.key, card.title);
    let facts = issue.facts();
    if !facts.is_empty() {
        println!("  ticket: {facts}");
    }
    if let Some(url) = &issue.url {
        println!("          {url}");
    }
    print_links(&db, card.id)?;
    Ok(())
}

fn tray_rules(max_mb: u64, video: bool) -> tray::Rules {
    tray::Rules {
        max_bytes: max_mb.saturating_mul(1024 * 1024),
        video,
    }
}

/// The card, and its tray made if it is not there yet.
fn open_tray(task: Option<&str>, command: &str) -> Result<(card::Card, PathBuf, bool)> {
    let db = Db::open(&paths::db_path()?)?;
    let card = card_or_in_hand(&db, task, command)?;
    let dir = tray::locate(&tray::root()?, &card)?;
    let made = tray::ensure(&dir, &card)?;
    Ok((card, dir, made))
}

fn tray_show(task: Option<&str>, json: bool) -> Result<()> {
    let (card, dir, made) = open_tray(task, "tray show")?;
    let tray = tray::read(&dir)?.context("the tray just made is not there")?;
    if json {
        let mut value = serde_json::to_value(&tray)?;
        value["card"] = serde_json::json!({ "key": card.key, "title": card.title });
        value["made"] = serde_json::Value::Bool(made);
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    println!("{} · {}", card.key, card.title);
    if made {
        println!("  made the tray, with a blank form to fill in");
    }
    println!("  tray:  {}", tray.path);
    println!("  form:  {} ({})", tray.form.path, if tray.form.filled { "filled" } else { "blank" });
    if tray.files.is_empty() {
        println!("  nothing waits in it");
    } else {
        println!("  {} waiting:", plural(tray.files.len(), "file", "files"));
        let width = tray.files.iter().map(|f| f.path.chars().count()).max().unwrap_or(0);
        for f in &tray.files {
            let when = f.modified.as_deref().map(tray::local_minute).unwrap_or_default();
            println!("    {:width$}  {:>9}  {when}", f.path, tray::size(f.bytes));
        }
    }
    if !tray.sorted.is_empty() {
        let rounds: Vec<String> = tray
            .sorted
            .iter()
            .map(|r| format!("{} ({})", r.day, plural(r.files, "file", "files")))
            .collect();
        println!("  sorted before: {}", rounds.join(", "));
    }
    Ok(())
}

fn tray_edit(task: Option<&str>, json: bool) -> Result<()> {
    let (card, dir, _) = open_tray(task, "tray edit")?;
    let form = dir.join(tray::FORM);
    doc::edit_file(&form)?;
    if json {
        return tray_show(Some(&card.key), true);
    }
    let filled = std::fs::read_to_string(&form).map(|t| tray::is_filled(&t)).unwrap_or(false);
    println!("The form of {} is {}: {}", card.key, if filled { "filled" } else { "blank" }, form.display());
    Ok(())
}

fn tray_fetch(task: Option<&str>, from: &[PathBuf], rules: tray::Rules, json: bool) -> Result<()> {
    let (card, dir, _) = open_tray(task, "tray fetch")?;
    let sources = match from.is_empty() {
        false => from.to_vec(),
        true => {
            let form = dir.join(tray::FORM);
            let linked = tray::linked_sources(&std::fs::read_to_string(&form).unwrap_or_default());
            if linked.is_empty() {
                bail!(
                    "nothing to fetch from: give --from <PATH>, or list the folders under \"Links to files\" in the form\n  {}",
                    form.display()
                );
            }
            linked
        }
    };
    if !json {
        println!(
            "Into the tray of {} from {}:",
            card.key,
            sources.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    let taken = tray::fetch(&dir, &sources, rules, &mut |item| print_taken(item, json))?;
    report_taken(&card, &dir, &taken, json)
}

fn tray_intake(task: Option<&str>, since: &str, rules: tray::Rules, json: bool) -> Result<()> {
    let seconds = tray::parse_window(since)?;
    let (card, dir, _) = open_tray(task, "tray intake")?;
    let folders = tray::intake_dirs();
    if folders.is_empty() {
        bail!("no folders to take from: the platform names none, and {} is not set", tray::INTAKE_ENV);
    }
    if !json {
        println!(
            "Into the tray of {}, what was written in the last {since} in {}:",
            card.key,
            folders.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    let taken = tray::intake(&dir, seconds, rules, &mut |item| print_taken(item, json))?;
    report_taken(&card, &dir, &taken, json)
}

/// A file as it lands, so a slow copy from a share is seen to be moving.
fn print_taken(item: &tray::Item, json: bool) {
    if !json {
        println!("  took {}  {}", item.path, tray::size(item.bytes));
    }
}

fn report_taken(card: &card::Card, dir: &Path, taken: &tray::Taken, json: bool) -> Result<()> {
    if json {
        let mut value = serde_json::to_value(taken)?;
        value["card"] = serde_json::Value::String(card.key.clone());
        value["path"] = serde_json::Value::String(dir.display().to_string());
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    // What was already there is the quiet case - a second fetch of the same
    // folder - and is counted rather than listed.
    let present = taken.skipped.iter().filter(|s| s.why == tray::Why::Present).count();
    for s in taken.skipped.iter().filter(|s| s.why != tray::Why::Present) {
        println!("  left {} ({}): {}", s.path, tray::size(s.bytes), s.why.describe());
    }
    let bytes: u64 = taken.taken.iter().map(|i| i.bytes).sum();
    let mut line = match taken.taken.len() {
        0 => "Took nothing".to_string(),
        n => format!("Took {} ({})", plural(n, "file", "files"), tray::size(bytes)),
    };
    if present > 0 {
        line.push_str(&format!("; {} already in the tray", plural(present, "was", "were")));
    }
    let left = taken.skipped.len() - present;
    if left > 0 {
        line.push_str(&format!(
            "; left {} where {}",
            plural(left, "file", "files"),
            if left == 1 { "it is" } else { "they are" }
        ));
    }
    println!("{line}\n  tray: {}", dir.display());
    Ok(())
}

fn tray_done(task: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = card_or_in_hand(&db, task, "tray done")?;
    let dir = tray::locate(&tray::root()?, &card)?;
    let Some(before) = tray::read(&dir)? else {
        bail!("{} has no tray; `rigger tray show {}` makes one", card.key, card.key);
    };
    if !before.holds_anything() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "card": card.key, "moved": 0, "archive": null, "form": false }))?
            );
        } else {
            println!("Nothing waits in the tray of {}; nothing to sort", card.key);
        }
        return Ok(());
    }
    let done = tray::done(&dir, &card, &jiff::Zoned::now().date().to_string())?;
    if json {
        let mut value = serde_json::to_value(&done)?;
        value["card"] = serde_json::Value::String(card.key.clone());
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    let form = if done.form {
        "; the filled form went with them, and the tray's is blank again"
    } else {
        ""
    };
    println!(
        "Sorted the tray of {}: {} moved to {}{form}",
        card.key,
        plural(done.moved, "entry", "entries"),
        done.archive
    );
    Ok(())
}

fn tray_list(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let root = tray::root()?;
    let mut rows = Vec::new();
    for (name, tray) in tray::all(&root)? {
        if !tray.holds_anything() {
            continue;
        }
        let card = db.card_by_ref(&name)?;
        rows.push((name, card, tray));
    }
    if json {
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|(name, card, tray)| {
                serde_json::json!({
                    "key": name,
                    "title": card.as_ref().map(|c| &c.title),
                    "status": card.as_ref().map(|c| &c.status),
                    "path": tray.path,
                    "files": tray.files.len(),
                    "form_filled": tray.form.filled,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if rows.is_empty() {
        println!("Nothing waits in any tray ({})", root.display());
        return Ok(());
    }
    let width = rows.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    for (name, card, tray) in &rows {
        let mut what = Vec::new();
        if !tray.files.is_empty() {
            what.push(plural(tray.files.len(), "file", "files"));
        }
        if tray.form.filled {
            what.push("the form filled".to_string());
        }
        let title = card.as_ref().map(|c| c.title.as_str()).unwrap_or("(no card of that name)");
        println!("{name:width$}  {:<24} {title}", what.join(", "));
    }
    Ok(())
}

fn project_add(path: PathBuf, name: Option<String>, json: bool) -> Result<()> {
    let root = dunce::canonicalize(&path).with_context(|| format!("{} is not a directory rigger can read", path.display()))?;
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let db = Db::open(&paths::db_path()?)?;
    let kind = db::Kind::of_directory(&root);
    let remote = repo::detect_remote(&root);
    // A folder recorded before git arrived in it. The path is the project,
    // so adding it again is how the record learns what the disk now says -
    // rather than a flag that could set a kind the directory does not have.
    if let Some(existing) = db.project_by_path(&root.to_string_lossy())?
        && existing.kind == db::Kind::Folder
        && kind == db::Kind::Repo
        && name.as_deref().is_none_or(|n| n == existing.name)
    {
        existing.ensure_working()?;
        db.set_kind(existing.id, kind, remote.as_deref())?;
        if json {
            refresh_line_skill(&db, false)?;
            return print_project_json(&db, &existing.name);
        }
        println!("'{}' has a repository now: recorded as one, and sync reads it from here on", existing.name);
        if let Some(url) = &remote {
            println!("  remote: {url}");
        }
        refresh_line_skill(&db, true)?;
        return Ok(());
    }
    let name = name.unwrap_or_else(|| repo::detect_name(&root));
    let project = db.add_project(&name, &root.to_string_lossy(), remote.as_deref(), kind)?;
    if json {
        refresh_line_skill(&db, false)?;
        return print_project_json(&db, &project.name);
    }
    println!("Recorded '{}' at {}", project.name, project.path);
    match (project.kind, &project.remote) {
        (db::Kind::Folder, _) => {
            println!("  no repository: a folder of work, and sync will not ask git about it");
            if repo::about(&project).is_none() {
                println!(
                    "  nothing says what it is; give it a line with `rigger project set {} --about \"<text>\"`",
                    project.name
                );
            }
        }
        (_, Some(url)) => println!("  remote: {url}"),
        (_, None) => println!("  remote: none (no origin in .git/config)"),
    }
    refresh_line_skill(&db, true)?;
    Ok(())
}

/// Rewrites the line's one skill, when one has been installed.
///
/// The body lists the projects, so a project added after it was written
/// leaves it describing a line that is one short - and an assistant asked
/// to work on the new project reads a skill that has never heard of it.
/// Only when the file is already there: installing a skill is something
/// the owner asks for once, not something `project add` decides.
/// `say` is false under `--json`, where stdout holds one document and the
/// line about the skill would break it.
fn refresh_line_skill(db: &Db, say: bool) -> Result<()> {
    // Only inside the directory this run was pointed at. A run with a
    // record of its own has no business rewriting the skill in the owner's
    // home - and did, until a test's fixture turned up there.
    if !skill::may_refresh_installed() {
        return Ok(());
    }
    let line = profile::Config::load()?.current_name().to_string();
    let path = skill::skills_dir()?.join(&line).join("SKILL.md");
    let before = match std::fs::read_to_string(&path) {
        Ok(text) if skill::is_generated(&text) => text,
        _ => return Ok(()),
    };
    let (template, _) = skill::load_line_template(None)?;
    let listed = listed_projects(db)?;
    let description = skill::line_description(&line, &listed);
    let rendered = skill::render_line(&template, &line, &description, &listed)?;
    if rendered.text == before {
        return Ok(());
    }
    std::fs::write(&path, &rendered.text).with_context(|| format!("cannot write {}", path.display()))?;
    if say {
        println!("  {} now lists {}", path.display(), plural(listed.len(), "project", "projects"));
    }
    Ok(())
}

/// Records a place the record keeps for itself.
///
/// A retro looks across every project and has to leave its summary
/// somewhere that is not one of them. That place has no repository and
/// never will, so it is recorded as what it is: `sync` does not ask git
/// about it and `doctor` does not list it as waiting to be synced.
fn project_service(name: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    // The path is a name, not a location: the column is unique and every
    // other project fills it with a directory, so a marker keeps the two
    // apart without pretending there is a directory to look in.
    let path = format!("service:{name}");
    let project = db.add_project(name, &path, None, db::Kind::Service)?;
    if json {
        return print_project_json(&db, &project.name);
    }
    println!("Recorded '{}' as a place the record keeps for itself", project.name);
    println!("  no repository: sync will not ask git about it");
    Ok(())
}

/// Every project recorded, the retired among them: this is the index of
/// the record, and a project taken out of the work is still in it.
fn project_list(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects = db.all_projects()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&projects)?);
        return Ok(());
    }
    if projects.is_empty() {
        println!("No projects yet. Add one with: rigger project add <path>");
        return Ok(());
    }
    let width = projects.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for p in &projects {
        match p.retired_at.as_deref().and_then(|at| at.split('T').next()) {
            Some(day) => println!("{:width$}  {}  (retired {day})", p.name, where_it_lives(p)),
            None => println!("{:width$}  {}", p.name, where_it_lives(p)),
        }
    }
    Ok(())
}

/// What to show where a project's location goes.
///
/// A place the record keeps for itself has no location, and the marker its
/// path column holds is bookkeeping - showing it reads as a broken path.
fn where_it_lives(project: &db::Project) -> String {
    match project.kind {
        db::Kind::Repo => project.path.clone(),
        db::Kind::Folder => format!("{} (a folder, no repository)", project.path),
        db::Kind::Service => "(no repository - a place the record keeps for itself)".to_string(),
    }
}

fn project_show(name: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let Some(project) = db.project_by_name(name)? else {
        bail!("no project named '{name}'; see `rigger project list`");
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&project)?);
        return Ok(());
    }
    println!("{}", project.name);
    match project.kind {
        db::Kind::Repo => {
            println!("  path:    {}", project.path);
            println!("  remote:  {}", project.remote.as_deref().unwrap_or("none"));
        }
        db::Kind::Folder => {
            println!("  path:    {}", project.path);
            println!("  kind:    a folder with no repository; git is never asked about it");
        }
        db::Kind::Service => println!("  kind:    a place the record keeps for itself; no repository"),
    }
    if let Some(about) = &project.about {
        println!("  about:   {about}");
    }
    println!("  since:   {}", project.created_at);
    if let Some(at) = &project.retired_at {
        match project.retired_by {
            Some(id) => println!("  retired: {at} - why in [{id}], `rigger event {} {id}`", project.name),
            None => println!("  retired: {at}"),
        }
    }
    if let Some(gate) = &project.gate {
        println!("  gate:    {gate}");
    }
    Ok(())
}

/// Sets what the record keeps about a project, beyond what git can tell it.
///
/// The gate is the first of these: the command CI runs, so that "green
/// before a commit" has one spelling per project instead of a copy in the
/// skill file, the README and whatever an assistant remembers.
fn project_set(name: &str, set: &Settings, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, name)?;
    if json {
        // Checked and written exactly as below, then answered as the project
        // now stands rather than line by line.
        project_set_quietly(&db, &project, set)?;
        return print_project_json(&db, &project.name);
    }
    let Settings {
        gate,
        no_gate,
        on_session_end,
        no_on_session_end,
        about,
        no_about,
    } = *set;

    if !no_gate && gate.is_none() && !no_on_session_end && on_session_end.is_none() && !no_about && about.is_none() {
        // Nothing asked for is a question, not a no-op: a command that
        // silently did nothing would look like it had worked.
        match &project.gate {
            Some(gate) => println!("{}: gate is `{gate}`", project.name),
            None => println!("{} has no gate; set one with --gate \"<command>\"", project.name),
        }
        match &project.on_session_end {
            Some(hook) => println!("{}: a closing session runs `{hook}`", project.name),
            None => println!(
                "{} runs nothing when a session closes; set it with --on-session-end \"<command>\"",
                project.name
            ),
        }
        match (&project.about, repo::about(&project)) {
            (Some(about), _) => println!("{}: the record says it is \"{about}\"", project.name),
            (None, Some(about)) => println!("{}: its manifest says it is \"{about}\"", project.name),
            (None, None) => println!("{} says nothing about itself; set a line with --about \"<text>\"", project.name),
        }
        return Ok(());
    }

    if no_gate {
        db.set_gate(project.id, None)?;
        println!("{} has no gate now.", project.name);
    } else if let Some(gate) = gate {
        let gate = gate.trim();
        if gate.is_empty() {
            bail!("an empty gate is not a gate; use --no-gate to take it off");
        }
        db.set_gate(project.id, Some(gate))?;
        println!("{}: gate is `{gate}`", project.name);
        println!("Run it with: rigger gate {}", project.name);
    }

    if no_on_session_end {
        db.set_on_session_end(project.id, None)?;
        println!("{} runs nothing when a session closes now.", project.name);
    } else if let Some(hook) = on_session_end {
        let hook = hook.trim();
        if hook.is_empty() {
            bail!("an empty command is not a command; use --no-on-session-end to take it off");
        }
        db.set_on_session_end(project.id, Some(hook))?;
        println!("{}: a closing session runs `{hook}`", project.name);
        println!("It runs after the sitting is written down, and its outcome is recorded.");
    }

    if no_about {
        db.set_about(project.id, None)?;
        println!("{} has no line about it in the record now.", project.name);
    } else if let Some(about) = about {
        let about = checked_about(&project, about)?;
        db.set_about(project.id, Some(&about))?;
        println!("{}: \"{about}\"", project.name);
        refresh_line_skill(&db, true)?;
    }
    Ok(())
}

/// What `project set` was asked to change.
#[derive(Clone, Copy)]
struct Settings<'a> {
    gate: Option<&'a str>,
    no_gate: bool,
    on_session_end: Option<&'a str>,
    no_on_session_end: bool,
    about: Option<&'a str>,
    no_about: bool,
}

/// A line about a project, as the record may hold it.
///
/// Refused where the manifest already says one: the manifest is what the
/// registry publishes, and a second line in the record would be a second
/// truth that the first edit to either makes disagree. And refused where
/// it would break what reads it - a skill's description is one line in a
/// catalogue an assistant parses, and angle brackets break that catalogue
/// for every skill around it.
fn checked_about(project: &db::Project, about: &str) -> Result<String> {
    let about = about.trim();
    if about.is_empty() {
        bail!("an empty line says nothing; use --no-about to take it off");
    }
    if about.contains('\n') {
        bail!("the line about a project is one line: it is what a skill's description and the line's table say");
    }
    if let Some(c) = about.chars().find(|c| skill::FORBIDDEN_IN_DESCRIPTION.contains(c)) {
        bail!("the line contains `{c}`, which breaks the catalogue of skills an assistant parses");
    }
    if project.kind.has_a_place()
        && let Some(manifest) = repo::detect_about(Path::new(&project.path))
    {
        bail!(
            "{} is described by its manifest: \"{manifest}\"\nChange the description there; a second line in the record would be a second truth.",
            project.name
        );
    }
    Ok(about.to_string())
}

/// Runs a project's gate and records how it went.
///
/// Exits non-zero when the gate is red, so that it composes: a hook, a
/// script, or a shell can act on it without reading the text. The output of
/// the gate goes straight to the terminal, because a gate is watched while
/// it runs; what the record keeps is the verdict and how long it took.
fn gate(project: Option<&str>, check: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = project_here(&db, project)?;
    // Refused before the command runs rather than after: a gate is a run
    // and a record of it, and the record would be refused anyway.
    project.ensure_working()?;
    let Some(command) = project.gate.clone() else {
        bail!(
            "{} has no gate. Set it to what CI runs:
  rigger project set {} --gate \"cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test\"",
            project.name,
            project.name
        );
    };

    let dir = Path::new(&project.path);
    if !dir.is_dir() {
        bail!("{} is recorded at {}, which is not there", project.name, project.path);
    }

    if check {
        if json {
            return print_json(&serde_json::json!({ "project": project.name, "command": command, "dir": project.path }));
        }
        println!("{} would run, in {}:", project.name, project.path);
        println!("  {command}");
        return Ok(());
    }

    let run = gate::run(&command, dir, json).with_context(|| format!("cannot run the gate of {}", project.name))?;
    let body = run.event_body(&command);
    db.record_event(project.id, "gate", &body, &db::now(), "assistant")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": project.name,
                "command": command,
                "passed": run.passed(),
                "exit_code": run.code,
                "seconds": run.seconds,
            }))?
        );
    } else {
        println!();
        println!("{}: {body}", project.name);
    }

    if !run.passed() {
        // Said by the exit code rather than by an error, because the gate has
        // already printed why it is red and an anyhow message on top would
        // bury it under a second account of the same failure.
        std::process::exit(1);
    }
    Ok(())
}

/// Records how a product looks from outside.
/// `given` holds what the command line named; a field left `None` keeps
/// what the project had.
fn project_mark(name: &str, given: db::Mark, clear: bool, json: bool) -> Result<()> {
    let (code, accent, accent2, form, docs) = (
        given.code.as_deref(),
        given.accent.as_deref(),
        given.accent2.as_deref(),
        given.form.as_deref(),
        given.docs_url.as_deref(),
    );
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, name)?;

    if clear {
        db.set_mark(project.id, &db::Mark::default())?;
        if json {
            return print_project_json(&db, &project.name);
        }
        println!("{} has no mark recorded now.", project.name);
        return Ok(());
    }

    if code.is_none() && accent.is_none() && accent2.is_none() && form.is_none() && docs.is_none() {
        if json {
            return print_project_json(&db, &project.name);
        }
        // Nothing asked for is a question, not a no-op.
        match (&project.mark_code, &project.accent) {
            (None, None) => println!("{} has no mark recorded; `--code` and `--accent` start one", project.name),
            _ => {
                println!("{}:", project.name);
                for (label, value) in [
                    ("code", &project.mark_code),
                    ("accent", &project.accent),
                    ("accent 2", &project.accent2),
                    ("form", &project.form),
                    ("docs", &project.docs_url),
                ] {
                    if let Some(value) = value {
                        println!("  {label:<8} {value}");
                    }
                }
            }
        }
        return Ok(());
    }

    // What was given is checked before anything is written, so a command
    // that refuses one field does not leave the other four changed.
    if let Some(code) = code {
        line::check_code(code)?;
    }
    for accent in [accent, accent2].into_iter().flatten() {
        line::check_accent(accent)?;
    }
    if let Some(form) = form {
        line::check_form(form)?;
    }

    // What is not named keeps what it had: `--form cli` states the form,
    // not the whole mark. `--clear` is how a mark is taken off.
    let mark = db::Mark {
        code: code.map(str::to_string).or(project.mark_code),
        accent: accent.map(str::to_string).or(project.accent),
        accent2: accent2.map(str::to_string).or(project.accent2),
        form: form.map(str::to_string).or(project.form),
        docs_url: docs.map(str::to_string).or(project.docs_url),
    };
    db.set_mark(project.id, &mark)
        .with_context(|| format!("cannot record the mark of {}", project.name))?;
    if json {
        return print_project_json(&db, &project.name);
    }
    println!("{}: mark recorded.", project.name);
    for (label, value) in [
        ("code", &mark.code),
        ("accent", &mark.accent),
        ("accent 2", &mark.accent2),
        ("form", &mark.form),
        ("docs", &mark.docs_url),
    ] {
        if let Some(value) = value {
            println!("  {label:<8} {value}");
        }
    }
    Ok(())
}

/// Writes the line's public registry.
fn export_line(to: Option<&Path>, check: bool, json_out: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let registry = line::build(&db, repo::about)?;
    // Before the file is written, every time. The one mistake this command
    // can make is publishing something private, and that one cannot be
    // taken back.
    line::check_public(&registry)?;
    let json = format!(
        "{}
",
        serde_json::to_string_pretty(&registry)?
    );

    let Some(path) = to else {
        print!("{json}");
        return Ok(());
    };
    let before = std::fs::read_to_string(path).unwrap_or_default();
    let answer = |changed: bool, written: bool| {
        print_json(&serde_json::json!({
            "path": path,
            "product_count": registry.products.len(),
            "changed": changed,
            "written": written,
        }))
    };
    if before == json {
        if json_out {
            return answer(false, false);
        }
        println!("{} is already what the record says.", path.display());
        return Ok(());
    }
    if check {
        if json_out {
            return answer(true, false);
        }
        println!("{} would change: {} products", path.display(), registry.products.len());
        return Ok(());
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::write(path, &json).with_context(|| format!("cannot write {}", path.display()))?;
    if json_out {
        return answer(true, true);
    }
    let what = if before.is_empty() { "Wrote" } else { "Rewrote" };
    println!("{what} {} - {} products", path.display(), registry.products.len());
    Ok(())
}

/// The writes of `project set`, without a word: what `--json` runs before
/// it prints the project.
fn project_set_quietly(db: &Db, project: &db::Project, set: &Settings) -> Result<()> {
    let Settings {
        gate,
        no_gate,
        on_session_end,
        no_on_session_end,
        about,
        no_about,
    } = *set;
    if no_gate {
        db.set_gate(project.id, None)?;
    } else if let Some(gate) = gate {
        let gate = gate.trim();
        if gate.is_empty() {
            bail!("an empty gate is not a gate; use --no-gate to take it off");
        }
        db.set_gate(project.id, Some(gate))?;
    }
    if no_on_session_end {
        db.set_on_session_end(project.id, None)?;
    } else if let Some(hook) = on_session_end {
        let hook = hook.trim();
        if hook.is_empty() {
            bail!("an empty command is not a command; use --no-on-session-end to take it off");
        }
        db.set_on_session_end(project.id, Some(hook))?;
    }
    if no_about {
        db.set_about(project.id, None)?;
    } else if let Some(about) = about {
        db.set_about(project.id, Some(&checked_about(project, about)?))?;
        refresh_line_skill(db, false)?;
    }
    Ok(())
}

/// Takes a project out of the work for good, keeping its history.
///
/// Weight 0 is the nearest thing and is not this: it is "worked
/// on when asked", so the line's skill goes on offering the project. A retired
/// project leaves every list that looks ahead - the line's skill, the registry,
/// the queue, the inbox, `sync` - and stays readable by name in every one that
/// looks back.
///
/// What it leaves open is left as it was and counted, not closed on its
/// behalf: a wish nobody sorted is part of the history too.
fn project_retire(name: &str, why: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, name)?;
    let why = why.trim();
    if why.is_empty() {
        bail!("say why with --why: a retirement is a decision, and the record keeps why it was taken");
    }
    if project.kind == db::Kind::Service {
        bail!(
            "{} is a place the record keeps for itself, not a project in the work; there is nothing to retire it from",
            project.name
        );
    }
    // The close of a sitting may record what it ran, and a retired project
    // takes nothing new; so the sitting is closed first, by the person who
    // opened it, rather than left open with no way to end it cleanly.
    if let Some(open) = db.open_session(project.id)? {
        bail!(
            "a sitting on {} has been open since {}; close it first with `rigger session end {}`",
            project.name,
            open.started_at,
            project.name
        );
    }
    let change = db.retire(project.id, why)?;
    if json {
        refresh_line_skill(&db, false)?;
        return print_project_json(&db, &project.name);
    }
    let project = db.project_by_name(&project.name)?.context("the project just retired is not in the record")?;
    let day = project.retired_at.as_deref().and_then(|at| at.split('T').next()).unwrap_or_default();
    if change == db::Change::Unchanged {
        println!("{} was retired already, on {day}", project.name);
        println!("  `rigger project unretire {}` brings it back", project.name);
        return Ok(());
    }
    println!("{} is retired, as of {day}: {why}", project.name);
    let questions = db.count_open_events(project.id, "question")?;
    let wishes = db.count_open_events(project.id, "wish")?;
    let tasks = db.count_open_tasks(project.id)?;
    if questions + wishes > 0 || tasks > 0 {
        println!(
            "  left as they were: {}, {}, {} - kept in its history, gone from the inbox and the plan",
            plural(questions as usize, "question", "questions"),
            plural(wishes as usize, "wish", "wishes"),
            plural(tasks as usize, "open task", "open tasks")
        );
    }
    println!("  gone from the line's skill, the registry, the calendar, the rotation, the inbox and sync");
    println!("  `rigger show {}` still reads it; nothing new is recorded against it", project.name);
    println!("  `rigger project unretire {}` brings it back", project.name);
    refresh_line_skill(&db, true)?;
    Ok(())
}

/// Brings a retired project back into the work: the lists that left it
/// out take it in again, with the weight it had.
fn project_unretire(name: &str, why: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, name)?;
    let why = why.map(str::trim).filter(|w| !w.is_empty());
    let change = db.unretire(project.id, why)?;
    if json {
        refresh_line_skill(&db, false)?;
        return print_project_json(&db, &project.name);
    }
    if change == db::Change::Unchanged {
        println!("{} is not retired; it is in the work already", project.name);
        return Ok(());
    }
    println!("{} is back in the work", project.name);
    println!("  its weight is {}, as it was; `rigger priority` changes it", project.weight);
    refresh_line_skill(&db, true)?;
    Ok(())
}

/// One version, whole.
///
/// The JSON is a contract, not a convenience: a release engine reads it to
/// learn which number goes out on Friday and under what title, so every
/// field is named on the reference page and a test holds the two together.
/// Adds a stage to the plan, or tasks to a stage already there.
///
/// The plan was something only a hub could hold: a stage came from a
/// heading in a markdown file and nowhere else, so a person with no hub -
/// anyone but the line it was built in - had no way to say what comes next.
fn version_add(project: &str, version: &str, title: Option<&str>, tasks: &[String], json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    if !names_a_version(version) {
        bail!("'{version}' is not a version; write it as a tag would be, such as v0.3.0");
    }
    let title = title.map(str::trim).filter(|t| !t.is_empty());
    let (version_id, name, change) = db.add_version(project.id, version, title)?;
    let mut added = 0;
    for task in tasks.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if db.add_plan_task(project.id, version_id, task)? == db::Change::Added {
            added += 1;
        }
    }
    if json {
        return version_show(&project.name, Some(&name), true);
    }
    let tasks = plural(added, "task", "tasks");
    match change {
        db::Change::Added => println!("Added {name} to {}, with {tasks}", project.name),
        db::Change::Updated => println!("{name} is now called {}; {tasks} added", title.unwrap_or_default()),
        db::Change::Unchanged => println!("{name} was in the plan already; {tasks} added"),
    }
    Ok(())
}

/// What comes next, across the line.
fn show_queue(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let found = queue::now(&db)?;
    if json {
        return print_json(&found);
    }
    print!("{}", queue::render(&found));
    Ok(())
}

/// What `rigger` alone does: the queue, the one thing worth seeing without
/// asking. Before there is a record there is no queue, and the help says how
/// to start one.
fn bare(json: bool) -> Result<()> {
    if paths::db_path()?.is_file() {
        return show_queue(json);
    }
    command_line().print_help()?;
    Ok(())
}

/// Puts a project first until its next release. An event rather than a field:
/// it stays in the record with its reason, and being settled by a release
/// needs nobody to remember it.
fn queue_first(project: &str, why: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    if !project.kind.has_a_place() {
        bail!("{} is a place the record keeps for itself: it has no turn to move", project.name);
    }
    let why = why.map(str::trim).filter(|w| !w.is_empty()).unwrap_or("the owner put it first");
    db.record_event(project.id, "moved", why, &db::now(), "owner")?;
    if json {
        return show_queue(true);
    }
    println!("{} comes first until its next release: {why}", project.name);
    Ok(())
}

/// Puts a project back in its turn.
fn queue_back(project: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = working_project(&db, project)?;
    let last = queue::last_turn(&db, &project)?;
    let open = db.moves(project.id, last.as_deref())?;
    for moved in &open {
        db.resolve_event(project.id, moved.id, None, false)?;
    }
    if json {
        return show_queue(true);
    }
    let pulled = queue::gather(&db)?
        .into_iter()
        .find(|s| s.project == project.name)
        .map_or(Vec::new(), |s| s.pulled);
    match (open.is_empty(), pulled.first()) {
        (_, Some(pull)) => println!("{} stays moved up: {}. Its next release settles that.", project.name, pull.head),
        (true, None) => println!("{} was not moved up: it is in its turn already.", project.name),
        (false, None) => println!("{} is back in its turn.", project.name),
    }
    Ok(())
}

/// Shows the weights, or changes one.
fn priority(name: Option<&str>, change: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let Some(name) = name else {
        return show_weights(&db, json);
    };
    let mut project = working_project(&db, name)?;
    if !project.kind.has_a_place() {
        bail!("{} is a place the record keeps for itself: it has no turn", project.name);
    }
    if let Some(change) = change {
        project.weight = queue::changed_weight(project.weight, change)?;
        db.set_weight(project.id, project.weight)?;
    }
    if json {
        return print_json(&serde_json::json!({ "project": project.name, "weight": project.weight }));
    }
    match project.weight {
        0 => println!("{} has weight 0: out of the queue, worked on when asked", project.name),
        weight => println!("{} has weight {weight}", project.name),
    }
    Ok(())
}

/// Every project's weight, heaviest first, with the share of the turns it means.
fn show_weights(db: &Db, json: bool) -> Result<()> {
    let mut projects: Vec<db::Project> = db.projects()?.into_iter().filter(|p| p.kind.has_a_place()).collect();
    projects.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.name.cmp(&b.name)));
    if json {
        let weights: Vec<serde_json::Value> = projects.iter().map(|p| serde_json::json!({ "project": p.name, "weight": p.weight })).collect();
        return print_json(&serde_json::json!({ "weights": weights }));
    }
    let total: u32 = projects.iter().map(|p| p.weight).sum();
    let name = projects.iter().map(|p| p.name.chars().count()).max().unwrap_or(0);
    for project in &projects {
        let turns = match (project.weight, total) {
            (0, _) | (_, 0) => "out of the queue".to_string(),
            (weight, _) => format!("{weight} turn{} in every {total}", if weight == 1 { "" } else { "s" }),
        };
        println!("{:<name$}  {:>2}  {turns}", project.name, project.weight);
    }
    Ok(())
}

/// Whether text names a version the way a tag does: `v` and then a digit.
fn names_a_version(text: &str) -> bool {
    text.strip_prefix('v').is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

fn version_show(project: &str, version: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let Some(card) = db.version_card(project.id, version)? else {
        // Only reachable without a version named: a named one that is not
        // in the record is refused by the lookup itself.
        match json {
            true => println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "project": project.name, "version": null }))?
            ),
            false => println!("{} has no stage being built; every version in the record has shipped.", project.name),
        }
        return Ok(());
    };
    let open: Vec<&db::Task> = card.tasks.iter().filter(|t| db::is_open_status(&t.status)).collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": project.name,
                "version": card.version,
                "title": card.title,
                "status": card.status,
                "shipped_at": card.shipped_at,
                "delivery": card.delivery,
                "registries": card.registries,
                "tasks": card.tasks,
                "open_tasks": open.len(),
            }))?
        );
        return Ok(());
    }

    match &card.title {
        Some(title) => println!("{} {} · {title}", project.name, card.version),
        None => println!("{} {}", project.name, card.version),
    }
    println!();
    if card.status == "shipped" {
        let on = card.shipped_at.as_deref().unwrap_or("an unknown day");
        match card.delivery {
            Some(delivery) => println!("shipped    on {on} - {}", delivery.describe()),
            None => println!("shipped    on {on}"),
        }
        if !card.registries.is_empty() {
            println!("reached    {}", card.registries.join(", "));
        }
    }
    println!("tasks      {} open of {}", open.len(), card.tasks.len());
    for task in &card.tasks {
        println!("  [{}] {:<7} {}", task.id, task.status, task.title);
    }
    Ok(())
}

/// The reminder a red gate leaves, if the last run of one was red.
///
/// Read from the last gate event rather than from a flag on the project: a
/// flag would have to be cleared by whoever fixed the gate, and the thing
/// nobody does is clear a flag. The record already holds every run, so the
/// last one is the answer.
///
/// Only this sitting's own runs count, and "this sitting" is the session
/// id, not a moment in time. A gate that went red a week ago and was never
/// run again says nothing about this sitting, and a reminder that fires for
/// ever is a reminder that gets ignored.
/// What a project's closing command came to.
#[derive(Debug, serde::Serialize)]
struct HookRun {
    command: String,
    verdict: String,
    passed: bool,
}

/// Runs the command a project says a closing sitting should run.
///
/// A red result does not fail the close. The session is already over and
/// already written down; refusing to end it would leave the record with a
/// sitting open for ever because a publish step could not reach the
/// network. So the outcome is recorded and said out loud, and the session
/// closes either way - visible, not fatal.
///
/// Recorded as a change, because that is what it is: something happened
/// outside the record because a session ended.
fn run_session_hook(db: &Db, project: &db::Project, json: bool) -> Result<Option<HookRun>> {
    let Some(command) = project.on_session_end.clone() else {
        return Ok(None);
    };
    let dir = Path::new(&project.path);
    if !dir.is_dir() {
        // A project whose directory has moved is not a reason to fail a
        // close; saying so beats a shell error nobody can place.
        eprintln!("{} is not a directory, so `{command}` was not run", project.path);
        return Ok(None);
    }
    eprintln!("Running the end-of-session command: {command}");
    let run = match gate::run(&command, dir, json) {
        Ok(run) => run,
        Err(e) => {
            eprintln!("cannot run `{command}`: {e}");
            return Ok(None);
        }
    };
    let verdict = run.event_body(&command);
    // Stamped at the moment it finished, not with the day: two runs on one
    // day that came back the same would otherwise be one row, and the
    // second - the one that says the publish worked on the retry - would
    // vanish into the first.
    db.record_event(project.id, "change", &format!("end of session: {verdict}"), &db::now(), "rigger")?;
    Ok(Some(HookRun {
        command,
        passed: run.passed(),
        verdict,
    }))
}

fn red_gate_reminder(db: &Db, project: &db::Project, session: i64) -> Result<Option<String>> {
    let Some(last) = db.last_gate_in_session(session)? else {
        return Ok(None);
    };
    if gate::body_is_green(&last.body) {
        return Ok(None);
    }
    Ok(Some(format!(
        "the gate was last {} - `rigger gate {}` to run it again",
        last.body, project.name
    )))
}

/// Opens a sitting. Everything recorded until `end` belongs to it.
/// Opens a sitting, or joins the one open, and tells kasl of a new one.
fn open_sitting(db: &Db, project: &db::Project) -> Result<(db::Session, db::Change, Option<kasl::Told>)> {
    let (session, change) = db.start_session(project.id, &db::now())?;
    // Told only when the sitting is new: joining one already open is not a
    // second interval, and telling kasl twice would split the time it
    // measures into two pieces of the same work.
    let kasl = match change {
        db::Change::Unchanged => None,
        _ => {
            let stage = db.current_stage(project.id)?;
            let name = kasl::task_name(&project.name, stage.as_ref().map(|s| (s.version.as_str(), s.title.as_deref())));
            Some(kasl::start(&name))
        }
    };
    Ok((session, change, kasl))
}

fn session_start(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = project_here(&db, project)?;
    project.ensure_working()?;
    let (session, change, kasl) = open_sitting(&db, &project)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "session": session,
                "already_open": change == db::Change::Unchanged,
                "kasl": kasl,
            }))?
        );
        return Ok(());
    }
    match change {
        // Joining rather than splitting: an assistant that lost its place,
        // or a hook that fired twice, should not orphan half a sitting.
        db::Change::Unchanged => println!("A session on {} is already open, since {}.", project.name, session.started_at),
        _ => println!("Session open on {}. Everything recorded now belongs to it.", project.name),
    }
    Ok(())
}

/// Closes the sitting and says what it held.
///
/// This is the end-of-session ritual, which has always been a list in a
/// skill file that the assistant had to remember at exactly the moment it
/// was running out of context. A ritual that depends on remembering is a
/// ritual that stops happening.
fn session_end(project: Option<&str>, heading: Option<&str>, diary: Option<&Path>, entry: Option<&Path>, remind: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    // A hook has no name to pass, so a failure to find one is not a failure
    // worth reporting: it fires in every directory, most of which are not
    // projects. Ending silently is the only behaviour that does not turn
    // every unrelated session into an error message.
    let project = match (project_here(&db, project), remind) {
        (Ok(project), _) => project,
        (Err(_), true) => return Ok(()),
        (Err(e), false) => return Err(e),
    };

    let Some(open) = db.open_session(project.id)? else {
        // A hook fires whether or not a session was opened, so having none
        // is ordinary and not a failure.
        if remind {
            return Ok(());
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "session": serde_json::Value::Null }))?);
            return Ok(());
        }
        println!("No session is open on {}.", project.name);
        println!("Open one with: rigger session start {}", project.name);
        return Ok(());
    };

    let at = db::now();
    let summary = session_summary(&db, &project, &open, &at)?;
    // The owner's words, when the draft was edited: they replace what was
    // composed, in the record and in the file alike. Read before anything
    // is closed, so an entry that cannot be read leaves the sitting open to
    // try again rather than closed without its diary.
    let entry = match entry {
        Some(path) => Some(read_entry(path)?),
        None => None,
    };

    db.end_session(open.id, &at)?;
    // Closed right after the record closes the sitting, so the interval in
    // kasl ends where the session does rather than after a backup, a diary
    // and a hook have had their turn.
    let kasl = kasl::stop();

    // A sitting's worth of work has just been written down, and the record
    // is one file. If the newest copy predates today, take one now: the
    // ritual's step that is most often skipped is the one that costs most
    // when it is, and a session always ends, hook or not.
    let insured = match db.newest_backup_at()? {
        Some(at) if days_since_utc(&at).is_some_and(|d| d < BACKUP_STALE_DAYS) => None,
        // A copy that will not be written must not fail the close: the
        // session is already ended and reporting it as an error would
        // invite a second `end` on a record that has none open.
        _ => match db.backup() {
            Ok(target) => {
                db.prune_backups(KEEP_BACKUPS)?;
                Some(target)
            }
            Err(e) => {
                eprintln!("The session closed, but the database could not be copied: {e:#}");
                None
            }
        },
    };

    // The entry goes into the record whatever else happens to it: a hub
    // written from the record reads its diary from there, and a sitting
    // that only wrote to a file was lost the moment the file was generated.
    // `--diary` still appends it to a file, for a hub kept by hand.
    let day = at.split('T').next().unwrap_or_default().to_string();
    let edited = entry.is_some();
    let text = match entry {
        // The owner's words keep their own heading; words without one get
        // the heading a composed entry would have had.
        Some(text) if text.starts_with("## ") => text,
        Some(text) => format!("{}\n\n{text}\n", session::entry_heading(&day, heading)),
        None => session::diary_entry(&summary, &day, heading),
    };
    // One heading for the record and the file: the one the entry's text
    // carries. The record took it from `--heading` and the file from the
    // text, so an edited entry with a title of its own lost it in the
    // record, a `--heading` lost it in the file, and the hub's diary read
    // differently from the record it was generated from after either.
    let (title, body) = session::split_entry(&text);
    let entry = hub::DiaryEntry {
        heading: (!title.is_empty() && title != day).then_some(title),
        date: day.clone(),
        body: body.to_string(),
        followed_by_rule: false,
        gap_after: 1,
        rank: 0,
    };
    if !summary.empty() || edited {
        db.write_session_diary(open.id, project.id, &day, entry.heading.as_deref(), &entry.body)?;
    }
    let written = match diary {
        Some(path) => Some(write_diary(&db, &project, path, &entry)?),
        None => None,
    };
    // The hub, brought up to date wherever that loses nothing: a file the
    // record last wrote is written again, one somebody wrote in is named.
    // A hub that will not be written must not fail the close any more than
    // a copy that will not be taken: the session is already ended.
    let hub = match hubfile::refresh(&db, &project) {
        Ok(refreshed) => refreshed,
        Err(e) => {
            eprintln!("The session closed, but its hub was not brought up to date: {e:#}");
            hubfile::Refreshed::default()
        }
    };

    // The project's own closing command, after the record is written and
    // before anything is reported. After, because what it does - rhapsod
    // publishes its novellas - belongs to the sitting that has just been
    // written down, and a command that ran first would publish a session
    // the record had not yet closed.
    let hook = run_session_hook(&db, &project, json)?;

    let mut missing = summary.missing();
    if let Some(red) = red_gate_reminder(&db, &project, open.id)? {
        missing.push(red);
    }
    if !hub.edited.is_empty() {
        missing.push(format!(
            "the hub holds edits the record has not seen ({}): `rigger import {} --hub <dir>` takes them in, `rigger export` discards them",
            hub.edited.join(", "),
            project.name
        ));
    }
    if let Some(run) = &hook
        && !run.passed
    {
        missing.push(format!("`{}` came back {}", run.command, run.verdict));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "session": summary,
                "missing": missing,
                "diary": written,
                "hub": hub,
                "backup": insured,
                "on_session_end": hook,
                "kasl": kasl,
            }))?
        );
        return Ok(());
    }

    // A hook speaks only when there is something to say. A reminder that
    // fires on every stop is a reminder nobody reads.
    if remind {
        if missing.is_empty() {
            return Ok(());
        }
        println!("Session on {} closed - {}:", project.name, plural(summary.recorded(), "event", "events"));
        for item in &missing {
            println!("  {item}");
        }
        return Ok(());
    }

    println!("Session on {} closed, open since {}.", project.name, open.started_at);
    println!();
    if summary.empty() {
        println!("Nothing was recorded in it.");
    } else {
        if !summary.shipped.is_empty() {
            println!("shipped {}", summary.shipped.join(", "));
        }
        let counted = [
            ("decision", "decisions", summary.decisions.len()),
            ("finding", "findings", summary.findings.len()),
            ("pitfall", "pitfalls", summary.pitfalls.len()),
            ("change", "changes", summary.changes.len()),
            ("question", "questions", summary.questions.len()),
        ];
        let recorded: Vec<String> = counted.iter().filter(|(_, _, n)| *n > 0).map(|(one, many, n)| plural(*n, one, many)).collect();
        if !recorded.is_empty() {
            println!("recorded {}", recorded.join(", "));
        }
        if !summary.tasks_closed.is_empty() {
            println!("closed {}", plural(summary.tasks_closed.len(), "task", "tasks"));
        }
    }
    if let Some(next) = &summary.next_step {
        println!("next: {}", first_line(next));
    }

    if let Some(target) = &insured {
        println!("copied the database to {}", target.display());
    }
    if !hub.written.is_empty() {
        println!("brought the hub up to date: {}", hub.written.join(", "));
    }
    if let Some(run) = &hook {
        println!("{}: {}", run.command, run.verdict);
    }

    if !missing.is_empty() {
        println!();
        println!("The ritual asks for:");
        for item in &missing {
            println!("  {item}");
        }
    }

    // The diary of a hub the record writes takes the entry from the record,
    // so only a project with no such hub is told how to keep one by hand.
    match written {
        Some(path) => println!(
            "
Diary entry written to {path}"
        ),
        None if hubfile::dir_of(&project).is_none() => println!(
            "
Write it into a diary with: rigger session end {} --diary <file>",
            project.name
        ),
        None => {}
    }
    Ok(())
}

/// What a session holds up to a moment: the summary `end` writes and
/// `draft` shows.
///
/// The project's own repository is read first. A commit made in the sitting
/// is part of what it did, and until `sync` has read it the record does not
/// know the commit exists - so a session that forgot to sync would leave a
/// diary with the day's work missing from it.
fn session_summary(db: &Db, project: &db::Project, open: &db::Session, at: &str) -> Result<session::Summary> {
    if project.kind.reads_git()
        && std::path::Path::new(&project.path).is_dir()
        && let Err(e) = sync::sync(db, project)
    {
        eprintln!("The repository was not read, so this session's commits may be missing: {e:#}");
    }
    let events = db.session_events(open.id)?;
    let shipped = db.shipped_between(project.id, &open.started_at, at)?;
    let closed = db.tasks_closed_between(project.id, &open.started_at, at)?;
    let commits = db.commits_between(project.id, &open.started_at, at)?;
    let next_step = db.latest_event_body(project.id, "next")?;
    let ended = db::Session {
        ended_at: Some(at.to_string()),
        ..open.clone()
    };
    Ok(session::summarise(&project.name, &ended, &events, shipped, closed, commits, next_step))
}

/// Reads an edited diary entry from a file, or from standard input as `-`.
///
/// The entry keeps the shape a composed one has: a `## ` heading line, then
/// the body. One that arrives without a heading is given one by `end`,
/// which knows the day and the `--heading`, so that an edit of the body
/// alone is still an entry the hub can place.
fn read_entry(path: &Path) -> Result<String> {
    let text = match path.as_os_str() == "-" {
        true => body_argument("-")?,
        false => std::fs::read_to_string(path).with_context(|| format!("cannot read the entry from {}", path.display()))?,
    };
    let text = text.trim();
    if text.is_empty() {
        bail!("the entry is empty; leave --entry out to write the composed one");
    }
    Ok(format!("{}\n", db::lf(text)))
}

/// Shows the diary entry the open session would leave, without closing it.
///
/// The entry is composed from what the sitting recorded, and the assistant
/// is the one who knows what the day meant. So it reads the draft, edits
/// it, and hands it back to `session end --entry` - editing a list of what
/// happened rather than remembering it from a blank page.
fn session_draft(project: Option<&str>, heading: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = project_here(&db, project)?;
    let Some(open) = db.open_session(project.id)? else {
        bail!("no session is open on {}; a draft is of the sitting in progress", project.name);
    };
    let at = db::now();
    let summary = session_summary(&db, &project, &open, &at)?;
    let entry = session::diary_entry(&summary, &db::today(), heading);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "session": summary, "done": summary.done(), "entry": entry }))?
        );
        return Ok(());
    }
    print!("{entry}");
    Ok(())
}

/// Puts a sitting's entry into a diary file.
///
/// A diary the record generates is written from the record, by the
/// function the export uses: appending into it gave the file an entry the
/// record spelt differently, and the hub's diary drifted from the record on
/// every sitting. The hub's own diary is left to the refresh that follows,
/// which knows whether somebody has written in it; any other generated
/// diary says at its top that edits there are overwritten, and is
/// rewritten. A diary kept by hand gets the entry at the top, under its
/// preamble and above what came before - newest first, as the hubs write
/// it - in the shape the export would give it.
fn write_diary(db: &Db, project: &db::Project, path: &Path, entry: &hub::DiaryEntry) -> Result<String> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    if export::is_generated(&existing) {
        let canonical = |p: &Path| dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let hub_diary = hubfile::dir_of(project).map(|dir| canonical(&dir.join(export::GENERATED[2])));
        if hub_diary.as_deref() != Some(canonical(path).as_path()) {
            let text = hubfile::generate(db, project, export::GENERATED[2])?;
            let text = export::with_line_ending(&text, export::line_ending(&existing));
            std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))?;
        }
        return Ok(path.display().to_string());
    }

    // The preamble is whatever the file says before its first entry: a
    // title, a note about the format, a rule. A new entry goes after it and
    // above the entries, because that is where the newest one belongs.
    let ending = export::line_ending(&existing);
    let existing = db::lf(&existing);
    let (preamble, entries) = match existing.find("\n## ") {
        Some(at) => existing.split_at(at + 1),
        None => (existing.as_ref(), ""),
    };
    let mut out = String::new();
    if !preamble.trim().is_empty() {
        out.push_str(preamble.trim_end());
        out.push_str("\n\n");
    }
    out.push_str(&export::entry_heading(entry));
    out.push_str("\n\n");
    if !entry.body.trim().is_empty() {
        out.push_str(entry.body.trim());
        out.push_str("\n\n");
    }
    if !entries.trim().is_empty() {
        out.push_str(entries.trim_start());
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let out = export::with_line_ending(&out, ending);
    std::fs::write(path, out).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path.display().to_string())
}

/// The first line of a body, for a screen with room for one.
fn first_line(text: &str) -> &str {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim()
}

/// Writes a hub back out of the record.
///
/// The point at which the hub stops being where work is written down and
/// becomes a view of what was written down somewhere else. The generated
/// files are always written; the handwritten texts only with `--docs`.
fn export_hub(project: &str, hub_dir: Option<&Path>, check: bool, adopt: bool, docs: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    // A directory named on the command line has to be there: a mistyped one
    // would become the project's hub. The one the record knows is made when
    // it is gone - that is the hub being written back, which is the point.
    let hub_dir = match hub_dir {
        Some(dir) if !dir.is_dir() => bail!("{} is not a directory", dir.display()),
        Some(dir) => dir.to_path_buf(),
        None => recorded_hub(&project)?,
    };
    if !adopt && !check {
        let by_hand = hubfile::by_hand(&db, &project, &hub_dir)?;
        if let Some(first) = by_hand.first() {
            bail!(
                "{} was written by hand and the record does not own it yet.
Check what would change with `--check`, then hand it over with `--adopt`.",
                hub_dir.join(first).display()
            );
        }
    }
    let HubWritten { dir: hub_dir, written, diffs } = write_hub(&db, &project, &hub_dir, check, docs)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "project": project.name, "files": written }))?
        );
        return Ok(());
    }

    let changed = written.iter().filter(|w| !w.unchanged).count();
    for file in &written {
        let state = match (file.unchanged, check) {
            (true, _) => "unchanged",
            (false, true) => "would change",
            (false, false) => "written",
        };
        println!("  {:<14} {state:<12} {} bytes", file.file, file.bytes);
    }
    for (file, lines) in &diffs {
        if lines.is_empty() {
            continue;
        }
        println!("\n{file}:");
        for line in lines {
            println!("  {line}");
        }
    }
    match (changed, check) {
        (0, _) => println!("\n{} is already what the record says.", hub_dir.display()),
        (n, true) => println!("\n{} of {} files differ from the record.", n, written.len()),
        (n, false) => println!("\n{} wrote {} of {} files.", project.name, n, written.len()),
    }
    Ok(())
}

/// What writing one hub did.
struct HubWritten {
    /// Where it was written, spelt the way the platform spells it.
    dir: PathBuf,
    written: Vec<export::Written>,
    /// For `--check`, the lines each text would change.
    diffs: Vec<(String, Vec<String>)>,
}

/// Writes one hub out of the record: the generated files always, the
/// handwritten texts and the kept files with `docs`. Makes the directory
/// when it is gone - unless only asked what would change.
///
/// A file kept by hand is the caller's to refuse before this runs: `--all`
/// refuses for every hub before it writes into any.
fn write_hub(db: &Db, project: &db::Project, hub_dir: &Path, check: bool, docs: bool) -> Result<HubWritten> {
    if !check && !hub_dir.is_dir() {
        std::fs::create_dir_all(hub_dir).with_context(|| format!("cannot make {}", hub_dir.display()))?;
    }
    // Spelt the way the platform spells it, the way import records it.
    let hub_dir = dunce::canonicalize(hub_dir).unwrap_or_else(|_| hub_dir.to_path_buf());
    let present = hub_dir.is_dir();

    // The handwritten texts are planned always and written only when asked
    // for. Off by default because they are a cache and nothing reads them
    // back: the record is where they live now, and writing them every time
    // would put more files in every diff of every hub for no one's benefit.
    // Planned all the same, so that one already saying what the record says
    // is noted as such - an edit to it later is then told apart from a file
    // of unknown history.
    let planned = hubfile::planned(db, project, true, present.then_some(hub_dir.as_path()))?;
    if !check {
        db.set_hub_path(project.id, &hub_dir)?;
    }

    let mut written = Vec::new();
    let mut diffs: Vec<(String, Vec<String>)> = Vec::new();
    for file in &planned {
        let path = hub_dir.join(&file.name);
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        if !file.generated && !docs {
            if !check && !before.is_empty() && db::lf(&before) == file.text.as_str() {
                hubfile::account(db, project.id, file, &before)?;
            }
            continue;
        }
        // Written in the ending the file already used. Every hub of this
        // line is CRLF, and a generated file in LF would differ from its
        // source on every line - which is not a diff anybody reads.
        let text = export::with_line_ending(&file.text, export::line_ending(&before));
        let unchanged = before == text;
        if !check {
            hubfile::write(db, project.id, &hub_dir, file)?;
        }
        // What would change, not just which file: the question `--check`
        // answers is whether a hand-written line is about to be lost.
        if check && !unchanged {
            diffs.push((file.name.clone(), export::diff(&before, &text, 2)));
        }
        written.push(export::Written {
            file: file.name.clone(),
            bytes: text.len(),
            unchanged,
        });
    }
    // The files the record keeps whole, byte for byte, with the texts: a
    // cache like them, and asked for the same way.
    if docs {
        for attachment in db.attachments(project.id)? {
            let unchanged = hubfile::attachment_state(&attachment, &hub_dir) == hubfile::State::Current;
            if !check && !unchanged {
                hubfile::write_attachment(db, project.id, &hub_dir, &attachment)?;
            }
            written.push(export::Written {
                file: attachment.path,
                bytes: usize::try_from(attachment.bytes).unwrap_or(usize::MAX),
                unchanged,
            });
        }
    }
    Ok(HubWritten { dir: hub_dir, written, diffs })
}

/// Writes every hub the record knows, each where the record says it lies.
///
/// What makes the hubs a cache rather than a second copy: deleted, every
/// one comes back from the record - the retired projects' and the line's
/// own place among them - and the directory of one that is gone is made.
fn export_all(check: bool, adopt: bool, docs: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects: Vec<db::Project> = db.all_projects()?.into_iter().filter(|p| p.hub_path.is_some()).collect();
    // Refused for every hub before any is written: half a line written and
    // half refused is a state nobody asked for.
    if !adopt && !check {
        let mut by_hand = Vec::new();
        for project in &projects {
            let dir = recorded_hub(project)?;
            for file in hubfile::by_hand(&db, project, &dir)? {
                by_hand.push(format!("{}: {file}", project.name));
            }
        }
        if !by_hand.is_empty() {
            bail!(
                "these files were written by hand and the record does not own them yet: {}.
Check what would change with `--check`, then hand them over with `--adopt`.",
                by_hand.join("; ")
            );
        }
    }
    // Which were gone, asked before any is written: one hub lies inside
    // another - every project's inside the line's own place - and making
    // the first would make the second look as if it had been there.
    let mut planned = Vec::new();
    for project in &projects {
        let dir = recorded_hub(project)?;
        let gone = !dir.is_dir();
        planned.push((project, dir, gone));
    }
    let mut hubs = Vec::new();
    for (project, dir, gone) in planned {
        let out = write_hub(&db, project, &dir, check, docs)?;
        hubs.push((project.name.clone(), out, gone));
    }

    if json {
        let hubs: Vec<serde_json::Value> = hubs
            .iter()
            .map(|(name, out, gone)| {
                serde_json::json!({
                    "project": name,
                    "hub": out.dir,
                    "made": *gone && !check,
                    "files": out.written,
                })
            })
            .collect();
        return print_json(&serde_json::json!({ "hubs": hubs }));
    }
    let width = hubs.iter().map(|(name, _, _)| name.len()).max().unwrap_or(0).max(8);
    let mut changed_total = 0;
    for (name, out, gone) in &hubs {
        let changed = out.written.iter().filter(|w| !w.unchanged).count();
        changed_total += changed;
        let what = match (changed, check) {
            (0, _) => "already what the record says".to_string(),
            (n, true) => format!("{} of {} would change", n, plural(out.written.len(), "file", "files")),
            (n, false) => format!("wrote {} of {}", n, plural(out.written.len(), "file", "files")),
        };
        let made = match (gone, check) {
            (true, true) => " - the directory is gone and would be made",
            (true, false) => " - the directory was gone and is made again",
            _ => "",
        };
        println!("  {name:<width$} {what}{made}");
    }
    match (changed_total, check) {
        (0, _) => println!("\nEvery hub is already what the record says."),
        (n, true) => println!("\n{} would change across {}.", plural(n, "file", "files"), plural(hubs.len(), "hub", "hubs")),
        (n, false) => println!("\nWrote {} across {}.", plural(n, "file", "files"), plural(hubs.len(), "hub", "hubs")),
    }
    Ok(())
}

/// One file of a hub that `doctor --hubs` has something to say about.
struct Drift {
    project: String,
    file: String,
    state: &'static str,
    why: &'static str,
}

/// How the hubs stand against the record.
struct HubDrift {
    /// The files it cannot vouch for.
    drift: Vec<Drift>,
    /// The files that are only behind it, by project and name.
    behind: Vec<(String, String)>,
    /// The directory the profile keeps its hubs in, when it names one.
    root: Option<PathBuf>,
}

impl HubDrift {
    /// The files deleting the hubs would lose: everything named, except a
    /// project with no hub to lose or whose hub is gone already.
    fn at_stake(&self) -> usize {
        self.drift.iter().filter(|d| !matches!(d.state, "no-hub" | "hub-missing")).count()
    }
}

/// How the hubs stand against the record: the files it cannot vouch for,
/// and the files that are only behind it.
///
/// Two lists, because they ask opposite things of the owner. A file that
/// holds an edit is a decision to make - take the edit in, or let the
/// export discard it. A file the record wrote and has since moved past is
/// no decision at all: writing it again loses nothing, and the end of the
/// project's next session does exactly that. Reported in one list, the
/// second kind buried the first: 64 of the 66 files `doctor` once named on
/// the line's hubs were only behind, and the two edits were lost in them.
fn hub_drift(db: &Db) -> Result<HubDrift> {
    let mut drift = Vec::new();
    let mut behind = Vec::new();
    // Every project with a hub, the retired among them: a retired project's
    // hub is as much in the vault as a working one's, and it was the one
    // nobody checked.
    for project in db.all_projects()? {
        // A place the record keeps for itself has no hub to vouch for -
        // until it has one: the line's own place keeps its files in the
        // directory the hubs lie in. A folder is worked on like any product.
        if !project.kind.has_a_place() && project.hub_path.is_none() {
            continue;
        }
        let mut say = |file: String, state: &'static str, why: &'static str| {
            drift.push(Drift {
                project: project.name.clone(),
                file,
                state,
                why,
            })
        };
        // Where the record says the hub is. It used to be guessed beside
        // the repository, and every hub of this line lives in a notes vault
        // instead - so the check read no files at all and reported that
        // every generated file matched, on hubs it had never opened.
        let Some(dir) = project.hub_path.as_deref().map(PathBuf::from) else {
            say("-".to_string(), "no-hub", "no hub recorded; import or export one");
            continue;
        };
        if !dir.is_dir() {
            say("-".to_string(), "hub-missing", "the hub is not where the record says");
            continue;
        }
        let mut by_hand = Vec::new();
        for file in hubfile::planned(db, &project, true, Some(&dir))? {
            let on_disk = std::fs::read_to_string(dir.join(&file.name)).ok();
            match hubfile::state(db, project.id, &file, on_disk.as_deref())? {
                hubfile::State::Missing | hubfile::State::Current => {}
                hubfile::State::Behind => behind.push((project.name.clone(), file.name)),
                hubfile::State::Edited => say(file.name, "edited", "edited since the record wrote it; `rigger import` takes the edit in"),
                hubfile::State::Unaccounted => say(
                    file.name,
                    "unaccounted",
                    "differs, and the record has no note of writing it; `rigger export --check` shows how",
                ),
                hubfile::State::ByHand => by_hand.push(file.name),
            }
        }
        // A hub still kept by hand is one the record cannot vouch for
        // either - and the one thing "every project through rigger" has
        // left to do. Named as such, so the list says how far along the
        // line is rather than staying silent about the files it skipped.
        if !by_hand.is_empty() {
            say(by_hand.join(", "), "by-hand", "kept by hand; `rigger export --adopt` hands it over");
        }
        // A file the record keeps whole that differs from the one there:
        // one of the two is newer, and only a person knows which.
        for attachment in db.attachments(project.id)? {
            if hubfile::attachment_state(&attachment, &dir) == hubfile::State::Edited {
                say(
                    attachment.path,
                    "edited",
                    "differs from the file the record keeps; `rigger import` takes it in, `rigger export --docs` puts the record's back",
                );
            }
        }
        // And what the record does not hold at all: deleting the hub would
        // lose exactly these.
        for missing in hubfile::not_held(db, &project, &dir)? {
            say(missing.file, missing.state, missing.why);
        }
    }

    // The directory the hubs lie in holds files of the line's own - its
    // marks, its research, its calendar - and until the line's place is
    // given that directory as its hub, those are in nobody's record.
    let root = profile::Config::load()?
        .current()?
        .1
        .hubs
        .clone()
        .filter(|dir| dir.is_dir())
        .map(|dir| hub::canonical(&dir));
    if let Some(root) = &root {
        let projects = db.all_projects()?;
        let line_has_it = projects
            .iter()
            .any(|p| p.hub_path.as_deref().is_some_and(|h| hub::canonical(Path::new(h)) == *root));
        if !line_has_it {
            let line = profile::Config::load()?.current_name();
            let recorded: Vec<PathBuf> = projects
                .iter()
                .filter_map(|p| p.hub_path.as_deref())
                .map(|h| hub::canonical(Path::new(h)))
                .filter(|h| h != root && h.starts_with(root))
                .collect();
            let strays = hubfile::unrecorded_hubs(root, &recorded);
            let skip: Vec<PathBuf> = recorded.iter().chain(&strays).cloned().collect();
            for rel in hub::files(root, &skip) {
                drift.push(Drift {
                    project: line.clone(),
                    file: rel,
                    state: "not-held",
                    why: "the line's own, in no project's hub; `rigger import <profile>` takes it in",
                });
            }
            for stray in strays {
                drift.push(Drift {
                    project: line.clone(),
                    file: stray.strip_prefix(root).unwrap_or(&stray).to_string_lossy().replace('\\', "/"),
                    state: "unrecorded-hub",
                    why: "a hub no project of the record names; `rigger import <project> --hub <dir>` reads it",
                });
            }
        }
    }
    Ok(HubDrift { drift, behind, root })
}

/// Hubs whose wishes file still holds something.
///
/// The file stopped being read at every session in v0.20.0: a wish now
/// arrives through `rigger wish` or the assistant's tool, and lives in the
/// record like everything else. Which means a wish written into the file
/// after that is a wish nobody will ever read - so the one thing the record
/// owes the file is to say when it is not empty.
///
/// Cheap enough to run unasked: one small file per project with a hub.
fn hubs_with_wishes(db: &Db) -> Result<Vec<(String, usize)>> {
    let mut out = Vec::new();
    for project in db.all_projects()? {
        // The line's own place has no wishes file: a file of that name in
        // the directory every hub lies in is a text of its own.
        if !hubfile::has_record_files(&project) {
            continue;
        }
        let Some(hub) = &project.hub_path else { continue };
        let path = Path::new(hub).join(hub::WISHES_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        // Only what the record does not hold yet: right after an import
        // took them in, the file still has them, and telling the owner to
        // import what was just imported sent them round in a circle.
        let mut left = 0;
        for wish in hub::parse_wishes(&text) {
            if !db.holds_wish(project.id, &wish)? {
                left += 1;
            }
        }
        if left > 0 {
            out.push((project.name, left));
        }
    }
    Ok(out)
}

fn doctor(hubs: bool, json: bool) -> Result<()> {
    let config = profile::Config::load()?;
    let (profile_name, _) = config.current()?;
    let path = paths::db_path()?;
    if !path.exists() {
        if json {
            println!("{}", serde_json::json!({ "profile": profile_name, "database": path, "initialised": false }));
        } else {
            println!("profile:   {profile_name}");
            println!("database:  {} (missing - run `rigger init`)", path.display());
        }
        return Ok(());
    }
    if !json {
        println!("profile:   {profile_name} ({})", profile::Config::path()?.display());
    }
    let db = Db::open(&path)?;
    let schema = db.schema_version()?;
    let counts = db.counts()?;
    // When the record is one file, its age is the one number that says how
    // much work a corrupt file would cost. It is read before the hub checks
    // so that it is printed whether or not those are asked for.
    let newest_backup = db.newest_backup_at()?;
    let backup_age = newest_backup.as_deref().and_then(days_since_utc);

    // Whether the other door opens. An assistant talks to the record over
    // MCP, and a server that does not answer looks from the outside like a
    // record with nothing in it - the failure reads as "this project has no
    // history", which is the most misleading thing rigger could say.
    let server = mcp::self_check(&db);

    // Where the plan and git disagree. Reported, never corrected: the record
    // cannot prove a tag's absence - it may simply not have been fetched -
    // and a silent correction would erase what the owner wrote (ADR 0005).
    let mut mismatches = Vec::new();
    let mut apart = Vec::new();
    let mut unsynced = Vec::new();
    for project in db.projects()? {
        // Never synced is a thing to fix only for a project git can answer
        // for; a service project would sit in that list for ever, being
        // advised a command that cannot help it.
        if !project.kind.reads_git() {
            continue;
        }
        if db.activity(project.id)?.is_none() {
            unsynced.push(project.name.clone());
            continue;
        }
        for version in db.shipped_without_a_tag(project.id)? {
            mismatches.push((project.name.clone(), version));
        }
        for (closed, tagged) in db.one_step_apart(project.id)? {
            apart.push((project.name.clone(), closed, tagged));
        }
    }

    // Read once, for whichever of the two shapes is printed. Off by
    // default because it reads every hub from disk.
    let drift = match hubs {
        true => Some(hub_drift(&db)?),
        false => None,
    };
    let waiting = hubs_with_wishes(&db)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "profile": profile_name,
                "database": path,
                "initialised": true,
                "schema_version": schema,
                "counts": counts,
                "mcp": match &server {
                    Ok(check) => serde_json::to_value(check)?,
                    Err(e) => serde_json::json!({ "error": format!("{e:#}") }),
                },
                "backup": {
                    "newest_at": newest_backup,
                    "age_days": backup_age,
                    "copies": db.backups()?.len(),
                    "state": backup_state(backup_age),
                },
                "hubs": match &drift {
                    Some(HubDrift { drift, .. }) => serde_json::to_value(
                        drift
                            .iter()
                            .map(|d| serde_json::json!({ "project": d.project, "file": d.file, "state": d.state, "why": d.why }))
                            .collect::<Vec<_>>(),
                    )?,
                    None => serde_json::Value::Null,
                },
                "hubs_behind": match &drift {
                    Some(HubDrift { behind, .. }) => serde_json::to_value(
                        behind
                            .iter()
                            .map(|(project, file)| serde_json::json!({ "project": project, "file": file }))
                            .collect::<Vec<_>>(),
                    )?,
                    None => serde_json::Value::Null,
                },
                "hubs_verdict": match &drift {
                    Some(d) => serde_json::json!({
                        "root": d.root,
                        "loses_nothing": d.at_stake() == 0 && waiting.is_empty(),
                    }),
                    None => serde_json::Value::Null,
                },
                "closed_without_a_tag": mismatches
                    .iter()
                    .map(|(project, version)| serde_json::json!({ "project": project, "version": version }))
                    .collect::<Vec<_>>(),
                "one_step_apart": apart
                    .iter()
                    .map(|(project, closed, tagged)| serde_json::json!({ "project": project, "closed": closed, "tagged": tagged }))
                    .collect::<Vec<_>>(),
                "never_synced": unsynced,
                "wishes_left_in_the_hub": waiting
                    .iter()
                    .map(|(project, count)| serde_json::json!({ "project": project, "wishes": count }))
                    .collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }
    println!("database:  {}", path.display());
    println!("schema:    version {schema}");
    match counts.retired {
        0 => println!("projects:  {}", counts.projects),
        retired => println!("projects:  {} ({retired} retired)", counts.projects),
    }
    println!("versions:  {}", counts.versions);
    println!("tasks:     {}", counts.tasks);
    println!("sessions:  {}", counts.sessions);
    println!("events:    {}", counts.events);
    match &server {
        Ok(check) => println!(
            "mcp:       answers - protocol {}, {}, {}",
            check.protocol,
            plural(check.tools, "tool", "tools"),
            plural(check.prompts, "prompt", "prompts")
        ),
        Err(e) => println!("mcp:       does not answer: {e:#}"),
    }
    match backup_age {
        None => println!("backup:    none - one file, no copy of it; run `rigger backup`"),
        Some(days) => {
            let copies = db.backups()?.len();
            let when = match days {
                0 => "today".to_string(),
                1 => "yesterday".to_string(),
                d => format!("{d} days ago"),
            };
            let verdict = match backup_state(Some(days)) {
                "old" => " - older than a week; run `rigger backup`",
                "stale" => " - older than a day; run `rigger backup`",
                _ => "",
            };
            println!("backup:    {when}, {} kept{verdict}", plural(copies, "copy", "copies"));
        }
    }

    if !unsynced.is_empty() {
        println!(
            "
never synced ({}): {}",
            unsynced.len(),
            unsynced.join(", ")
        );
        println!("  run `rigger sync` to read what git says about them");
    }
    if !mismatches.is_empty() {
        println!(
            "
closed in the plan, no tag in git ({}):",
            mismatches.len()
        );
        for (project, version) in &mismatches {
            println!("  {project:<12} {version}");
        }
        println!("  a tag would settle it; rigger does not change what you wrote");
    }
    if !apart.is_empty() {
        println!(
            "
the plan and the tags look one step apart ({}):",
            apart.len()
        );
        for (project, closed, tagged) in &apart {
            println!("  {project:<12} {closed} closed without a tag, beside {tagged} tagged without a stage");
        }
        println!("  the tags may name the stage before the one the plan gave them; renumber the plan, or tag the stage");
    }

    if !waiting.is_empty() {
        println!();
        println!("wishes left in {} ({}):", hub::WISHES_FILE, waiting.len());
        for (project, count) in &waiting {
            println!("  {project:<12} {}", plural(*count, "wish", "wishes"));
        }
        println!("  the file is no longer read at every session: `rigger import <project> --hub <dir>` takes them in");
    }

    // A file somebody wrote in has stopped being a view of the record, and
    // the next export would overwrite the edit without saying so.
    if let Some(found) = &drift {
        let HubDrift { drift, behind, root } = found;
        // What lives only in a hub is named by project, a line each: before
        // the line's own place was read, its research alone was 191 files,
        // and a line per file buried every edit beside them.
        let (only_here, vouch): (Vec<&Drift>, Vec<&Drift>) = drift.iter().partition(|d| d.state == "not-held");
        println!();
        // "Every file says what the record says" only when nothing at all is
        // named below it: a file only the hub has does not say it.
        match (vouch.is_empty(), behind.is_empty() && only_here.is_empty()) {
            (true, true) => println!("hubs: every file says what the record says"),
            (true, false) => println!("hubs: nothing written in by hand"),
            _ => {
                println!("hubs the record cannot vouch for ({}):", vouch.len());
                for d in &vouch {
                    println!("  {:<12} {:<14} {}", d.project, d.file, d.why);
                }
                // The advice only fits an edit; a hub the record has never
                // seen needs the other sentence.
                if vouch.iter().any(|d| d.state == "edited") {
                    println!("  edited: `rigger import` takes the edit into the record; `rigger export` discards it");
                }
            }
        }
        if !only_here.is_empty() {
            let mut projects: Vec<&str> = only_here.iter().map(|d| d.project.as_str()).collect();
            projects.dedup();
            println!(
                "only in the hubs, not in the record ({} in {}):",
                plural(only_here.len(), "file", "files"),
                plural(projects.len(), "hub", "hubs")
            );
            for project in &projects {
                let files: Vec<&str> = only_here.iter().filter(|d| d.project == *project).map(|d| d.file.as_str()).collect();
                let shown: Vec<&str> = files.iter().take(3).copied().collect();
                let more = match files.len().saturating_sub(shown.len()) {
                    0 => String::new(),
                    n => format!(" and {n} more"),
                };
                println!("  {project:<12} {}{more}", shown.join(", "));
            }
            println!("  `rigger import <project>` takes them in; `rigger doctor --hubs --json` names every one");
        }
        // Behind is not a fault: the record wrote these and has moved on.
        // Named by project, in one line each, because the list is long on
        // a busy day and says the same thing about every file in it.
        if !behind.is_empty() {
            let mut projects: Vec<&str> = behind.iter().map(|(project, _)| project.as_str()).collect();
            projects.dedup();
            println!(
                "behind the record ({} in {}): {}",
                plural(behind.len(), "file", "files"),
                plural(projects.len(), "hub", "hubs"),
                projects.join(", ")
            );
            println!("  nothing in them is lost: the next session end, or `rigger export`, writes them again");
        }
        // The question the whole check exists to answer, in one line: can
        // the hubs go. Behind loses nothing; what is named above would.
        let place = match root {
            Some(root) => root.display().to_string(),
            None => "the hubs".to_string(),
        };
        match found.at_stake() + waiting.len() {
            0 => println!("{place}: the record holds every file - deleting it loses nothing; `rigger export --all --docs` writes it back"),
            _ => println!("{place}: deleting it would lose what is named above"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_closed_pipe_is_told_by_its_number_in_any_language() {
        assert!(super::is_a_closed_pipe("failed printing to stdout: Канал был закрыт. (os error 109)"));
        assert!(super::is_a_closed_pipe("failed printing to stdout: Broken pipe (os error 32)"));
        assert!(super::is_a_closed_pipe("failed printing to stdout: The pipe is being closed. (os error 232)"));
        // Another failure to print is still a failure, and so is a closed
        // pipe somewhere other than stdout.
        assert!(!super::is_a_closed_pipe("failed printing to stdout: Access is denied. (os error 5)"));
        assert!(!super::is_a_closed_pipe("failed printing to stderr: Broken pipe (os error 32)"));
        assert!(!super::is_a_closed_pipe("attempt to divide by zero"));
    }
}
