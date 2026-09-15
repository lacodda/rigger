//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod adopt;
mod calendar;
mod card;
mod commit;
mod context;
mod db;
#[allow(dead_code)]
mod doc;
mod export;
mod hub;
mod import;
mod mcp;
mod open;
mod owner;
mod paths;
mod profile;
mod repo;
mod retro;
mod search;
mod session;
mod skill;
mod sync;
mod week;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::db::Db;

#[derive(Parser)]
#[command(name = "rigger", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
        /// Directory of the hub to read
        #[arg(long)]
        hub: PathBuf,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Switch, list and add profiles: one record per way of working
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Give a task a status
    Task {
        #[command(subcommand)]
        command: TaskCommand,
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
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Write a thin project skill from a template and the record
    Skill {
        /// Project name
        #[arg(required_unless_present = "print_template")]
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
        /// Print the built-in template, to start one of your own from
        #[arg(long)]
        print_template: bool,
    },
    /// Print what an assistant needs to start a session on a project
    Context {
        /// Project name
        project: String,
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
        /// What happened
        text: String,
        /// Kind of event
        #[arg(long, value_name = "KIND", default_value = "finding")]
        kind: NoteKind,
    },
    /// Start an assistant session in the project, with the packet in hand
    Open {
        /// Project name
        project: String,
        /// Print the first message instead of starting a session
        #[arg(long)]
        print: bool,
        /// Token budget for the packet
        #[arg(long, default_value_t = context::DEFAULT_BUDGET)]
        budget: usize,
    },
    /// Read tags and commits into facts: what shipped, and what has happened since
    Sync {
        /// Project name; every project when omitted
        project: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Questions waiting for your answer, across every project
    Inbox {
        /// Only this project
        #[arg(long)]
        project: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// What moved lately, five lines per project
    Digest {
        /// Project name; every project that moved when omitted
        project: Option<String>,
        /// How far back to look, as days: 7d, 30d
        #[arg(long, default_value = "7d")]
        since: String,
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// The events that led to a version: what was decided, found and hit
    Why {
        /// Project name
        project: String,
        /// Version, as the record spells it
        version: String,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Plan a version: aim it at a week of the calendar
    Version {
        #[command(subcommand)]
        command: VersionCommand,
    },
    /// Weeks by projects: what is planned, what shipped, what slipped
    Calendar {
        /// How many weeks to show, starting this week
        #[arg(long, default_value_t = 6)]
        weeks: u32,
        /// Start from this week instead of the current one
        #[arg(long, value_name = "WEEK")]
        from: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// This week's focus: what is aimed at it, and what is already late
    Next {
        /// Read a week other than the current one
        #[arg(long, value_name = "WEEK")]
        week: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// The Monday brief: the focus, what ships on Friday, what waits on you
    Week {
        /// Read a week other than the current one
        #[arg(long, value_name = "WEEK")]
        week: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// The shopfront queue: what has gone out this week, and what waits for Friday
    ReleaseDay {
        /// Read a week other than the current one
        #[arg(long, value_name = "WEEK")]
        week: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Look back: what the plan said, what the tags say, where they parted
    Retro {
        /// Look back over a whole cycle of the calendar instead of the default weeks
        #[arg(long)]
        cycle: bool,
        /// How many weeks to look back over, ending with this week
        #[arg(long, value_name = "N", conflicts_with = "cycle")]
        weeks: Option<u32>,
        /// End the window at this week instead of the current one
        #[arg(long, value_name = "WEEK")]
        to: Option<String>,
        /// Write the summary into the record as an event
        #[arg(long)]
        record: bool,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Open and close a sitting, so its events belong together
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Write a hub back out of the record
    Export {
        /// Project name
        project: String,
        /// Directory of the hub to write
        #[arg(long)]
        hub: PathBuf,
        /// Say what would change without writing anything
        #[arg(long)]
        check: bool,
        /// Take over files written by hand, so the record owns them from now on
        #[arg(long)]
        adopt: bool,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Serve the record over MCP, on stdin and stdout
    Mcp,
    /// Answer a question or sort a wish, so it leaves the packet
    Resolve {
        /// Project name
        project: String,
        /// Id of the question or wish, as the packet lists it
        id: i64,
        /// The answer; a question answered this way becomes a decision
        answer: Option<String>,
    },
    /// Record a wish: something to sort into the plan later
    Wish {
        /// Project name
        project: String,
        /// What you want
        text: String,
    },
    /// The handwritten texts of a project: vision, rituals, research
    Doc {
        #[command(subcommand)]
        command: DocCommand,
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
}

/// The kinds a `note` can record. A question is not among them: it is
/// addressed to the owner and arrives from the hub or, later, from the
/// assistant's `ask_owner` tool.
#[derive(Clone, Copy, clap::ValueEnum)]
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
        }
    }
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print a document
    Show {
        /// Project name
        project: String,
        /// The document's address, as `list` prints it
        slug: String,
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
    /// Remove a document from the record
    Remove {
        /// Project name
        project: String,
        /// The document's address, as `list` prints it
        slug: String,
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
    List {
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one profile; the one in use when no name is given
    Show {
        /// Profile name
        name: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
        /// Where incoming material lands
        #[arg(long, value_name = "DIR")]
        inbox: Option<PathBuf>,
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
        /// Where incoming material lands
        #[arg(long, value_name = "DIR")]
        inbox: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Make a card for a task: a ticket id, or a local key until one is given
    New {
        /// The task's title
        title: String,
        /// Ticket id, such as WA-4130; a local key is made when omitted
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Make a card the open one, so that the record knows what is in hand
    Open {
        /// Card key, alias or id
        task: String,
    },
    /// The card in hand, if one is open
    Active {
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// Record a repository as a project
    Add {
        /// Path to the repository root
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
    List {
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one project
    Show {
        /// Project name
        name: String,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Set the tier a project sits in, and how often it should release
    Tier {
        /// Project name
        name: String,
        /// A, B, C, or out for a project outside the rotation
        tier: String,
        /// Weeks between releases; the tier's own rhythm when omitted
        #[arg(long, value_name = "WEEKS")]
        rhythm: Option<u32>,
    },
}

#[derive(Subcommand)]
enum SessionCommand {
    /// Open a sitting; everything recorded until `end` belongs to it
    Start {
        /// Project name; the project of the working directory when omitted
        project: Option<String>,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Close the sitting and say what it held
    End {
        /// Project name; the project of the working directory when omitted
        project: Option<String>,
        /// A title for the diary entry, if one is being written
        #[arg(long, value_name = "TEXT")]
        heading: Option<String>,
        /// Append the entry to this diary file
        #[arg(long, value_name = "FILE")]
        diary: Option<PathBuf>,
        /// Say nothing unless something is worth saying, for a hook
        #[arg(long)]
        remind: bool,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum VersionCommand {
    /// Aim a version at a week of the calendar
    Plan {
        /// Project name
        project: String,
        /// Version, as the record spells it
        version: String,
        /// The week it is aimed at, as `2026-W37`
        #[arg(long, value_name = "WEEK")]
        week: Option<String>,
        /// Take the version off the calendar
        #[arg(long, conflicts_with = "week")]
        clear: bool,
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

fn work() -> ExitCode {
    let cli = match Cli::try_parse() {
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
    match cli.command {
        Command::Init => init(),
        Command::Project { command } => match command {
            ProjectCommand::Add { path, name } => project_add(path, name),
            ProjectCommand::Service { name } => project_service(&name),
            ProjectCommand::List { json } => project_list(json),
            ProjectCommand::Show { name, json } => project_show(&name, json),
            ProjectCommand::Tier { name, tier, rhythm } => project_tier(&name, &tier, rhythm),
        },
        Command::Import { project, hub, json } => import_hub(&project, &hub, json),
        Command::Profile { command } => match command {
            ProfileCommand::List { json } => profile_list(json),
            ProfileCommand::Show { name, json } => profile_show(name.as_deref(), json),
            ProfileCommand::Use { name } => profile_use(&name),
            ProfileCommand::Add {
                name,
                kind,
                roots,
                hubs,
                id_pattern,
                inbox,
                r#use,
            } => profile_add(
                &name,
                profile::Profile {
                    kind,
                    roots,
                    hubs,
                    id_pattern,
                    inbox,
                },
                r#use,
            ),
            ProfileCommand::Set {
                name,
                roots,
                hubs,
                id_pattern,
                inbox,
            } => profile_set(name.as_deref(), roots, hubs, id_pattern, inbox),
        },
        Command::Task { command } => match command {
            TaskCommand::New {
                title,
                key,
                aliases,
                projects,
                branch,
                summary,
            } => task_new(&title, key.as_deref(), &aliases, &projects, branch.as_deref(), summary.as_deref()),
            TaskCommand::Find { query, json } => task_find(&query, json),
            TaskCommand::Open { task } => task_open(&task),
            TaskCommand::Active { json } => task_active(json),
            TaskCommand::Close { task, status } => task_close(task.as_deref(), &status),
            TaskCommand::Show { task, json } => task_show(&task, json),
            TaskCommand::Context { task, budget } => task_context(&task, budget),
            TaskCommand::Link { task, project, branch, role } => task_link(&task, &project, branch.as_deref(), role.as_deref()),
            TaskCommand::List { status, json } => task_list(&status, json),
            TaskCommand::Rename { task, key } => task_rename(&task, &key),
            TaskCommand::Alias { task, alias } => task_alias(&task, &alias),
            TaskCommand::Summary { task, text } => task_summary(&task, &text),
            TaskCommand::Note { task, text, kind } => note_on_card(&task, kind.as_str(), &text),
            TaskCommand::Status { task, status } => task_status(&task, &status),
        },
        Command::Adopt { root, hubs, check, json } => adopt_root(root.as_deref(), hubs.as_deref(), check, json),
        Command::Skill {
            project,
            install,
            dir,
            replace,
            template,
            print_template,
        } => write_skill(
            project.as_deref(),
            install || dir.is_some(),
            dir.as_deref(),
            replace,
            template.as_deref(),
            print_template,
        ),
        Command::Context {
            project,
            json,
            explain,
            budget,
        } => show_context(&project, json, explain, budget),
        Command::Open { project, print, budget } => open_session(&project, print, budget),
        Command::Note { project, text, kind } => note(&project, kind.as_str(), &text),
        Command::Sync { project, json } => sync_projects(project.as_deref(), json),
        Command::Inbox { project, json } => inbox(project.as_deref(), json),
        Command::Digest { project, since, json } => digest(project.as_deref(), &since, json),
        Command::Find {
            query,
            project,
            kind,
            limit,
            json,
        } => find(&query, project.as_deref(), kind.as_deref(), limit, json),
        Command::Why { project, version, json } => why(&project, &version, json),
        Command::Version { command } => match command {
            VersionCommand::Plan { project, version, week, clear } => version_plan(&project, &version, week.as_deref(), clear),
        },
        Command::Calendar { weeks, from, json } => show_calendar(weeks, from.as_deref(), json),
        Command::Next { week, json } => show_next(week.as_deref(), json),
        Command::Week { week, json } => show_week(week.as_deref(), json),
        Command::ReleaseDay { week, json } => show_release_day(week.as_deref(), json),
        Command::Retro {
            cycle,
            weeks,
            to,
            record,
            json,
        } => show_retro(cycle, weeks, to.as_deref(), record, json),
        Command::Session { command } => match command {
            SessionCommand::Start { project, json } => session_start(project.as_deref(), json),
            SessionCommand::End {
                project,
                heading,
                diary,
                remind,
                json,
            } => session_end(project.as_deref(), heading.as_deref(), diary.as_deref(), remind, json),
        },
        Command::Export {
            project,
            hub,
            check,
            adopt,
            json,
        } => export_hub(&project, &hub, check, adopt, json),
        Command::Mcp => mcp::serve(),
        Command::Resolve { project, id, answer } => resolve(&project, id, answer.as_deref()),
        Command::Wish { project, text } => note(&project, "wish", &text),
        Command::Doc { command } => match command {
            DocCommand::List { project, kind, json } => doc_list(&project, kind.as_deref(), json),
            DocCommand::Show { project, slug, json } => doc_show(&project, &slug, json),
            DocCommand::Add {
                project,
                title,
                kind,
                slug,
                body,
            } => doc_add(&project, &title, &kind, slug.as_deref(), body.as_deref()),
            DocCommand::Edit { project, slug, title, body } => doc_edit(&project, &slug, title.as_deref(), body.as_deref()),
            DocCommand::Remove { project, slug } => doc_remove(&project, &slug),
            DocCommand::Template { kind, write } => doc_template(&kind, write),
        },
        Command::Backup { keep, list } => backup(keep, list),
        Command::Doctor { hubs, json } => doctor(hubs, json),
    }
}

fn import_hub(project: &str, hub_dir: &Path, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let Some(project) = db.project_by_name(project)? else {
        bail!("no project named '{project}'; see `rigger project list`");
    };
    let hub = hub::read(hub_dir)?;
    // Where the hub is, so a later check can find it. Not guessed from the
    // repository path: the hubs of this line live in a notes vault. Spelt
    // the way the platform spells it, not the way the shell happened to.
    let hub_dir = &dunce::canonicalize(hub_dir).unwrap_or_else(|_| hub_dir.to_path_buf());
    db.set_hub_path(project.id, hub_dir)?;
    let report = import::import(&db, project.id, &hub)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    for warning in &report.warnings {
        println!("note: {warning}");
    }
    if !report.changed() {
        println!("{}: nothing changed", project.name);
        return Ok(());
    }
    println!("{}:", project.name);
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
    line("documents", report.documents_added, report.documents_updated);
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
fn write_skill(project: Option<&str>, install: bool, dir: Option<&Path>, replace: bool, template: Option<&Path>, print_template: bool) -> Result<()> {
    if print_template {
        print!("{}", skill::DEFAULT_TEMPLATE);
        return Ok(());
    }
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project.unwrap_or_default())?;
    let (template, source) = skill::load_template(template)?;
    let about = match project.kind {
        db::Kind::Repo => repo::detect_about(Path::new(&project.path)),
        db::Kind::Service => None,
    };
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
        println!("{} is already what the template says.", path.display());
        return Ok(());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::write(&path, &rendered.text).with_context(|| format!("cannot write {}", path.display()))?;
    let what = if before.is_empty() { "Wrote" } else { "Rewrote" };
    println!("{what} {} from {source}.", path.display());
    Ok(())
}

fn open_project(db: &Db, name: &str) -> Result<db::Project> {
    match db.project_by_name(name)? {
        Some(project) => Ok(project),
        None => bail!("no project named '{name}'; see `rigger project list`"),
    }
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
    }
    Ok(())
}

fn open_session(project: &str, print: bool, budget: usize) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let packet = context::build(&db, &project, budget)?;
    let message = open::first_message(&context::render(&packet));

    if print {
        print!("{message}");
        return Ok(());
    }
    let dir = Path::new(&project.path);
    open::check_dir(dir)?;
    let (program, _) = open::assistant();
    eprintln!("Starting {program} in {} with the packet for {}", project.path, project.name);
    let code = open::run(dir, &message)?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Reads git for one project, or for every recorded project.
fn sync_projects(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects = match project {
        Some(name) => vec![open_project(&db, name)?],
        None => db.projects()?,
    };
    let mut reports = Vec::new();
    for project in &projects {
        // A place the record keeps for itself has no repository, and asking
        // git about it would warn on every run about a project working
        // exactly as intended. Named on its own it says so once, rather
        // than failing at something it was never meant to do.
        if !project.kind.reads_git() {
            if projects.len() == 1 {
                println!("{} is a place the record keeps for itself; there is no repository to read", project.name);
            }
            continue;
        }
        reports.push(sync::sync(&db, project)?);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
        return Ok(());
    }
    for report in &reports {
        print_sync(report, projects.len() > 1);
    }
    Ok(())
}

/// One project's sync, as a line or as a paragraph.
///
/// A quiet project prints nothing when several are synced at once: a run
/// across the whole line is read for what changed, and seventeen "nothing
/// changed" lines hide the two that did.
fn print_sync(report: &sync::Report, many: bool) {
    let quiet = !report.changed() && report.untagged.is_empty() && report.warnings.is_empty();
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
    if quiet {
        println!("  nothing changed");
    }
}

/// An event written against a card, under the desk.
fn note_on_card(task: &str, kind: &str, text: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if kind == "state" {
        bail!("a state line belongs to a project's README, not to a card");
    }
    let card = find_card(&db, task)?;
    let desk = db.desk_project()?;
    db.record_task_event(desk.id, card.id, kind, text, &db::now(), "assistant")?;
    println!("Recorded a {kind} on {}", card.key);
    Ok(())
}

fn note(project: &str, kind: &str, text: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    // A state line is not an event: it is the top of the README's state
    // block, which an export writes from the record.
    if kind == "state" {
        db.add_state_line(project.id, &db::today(), text)?;
        println!("Added a state line for {}; `rigger export` writes it into the README", project.name);
        return Ok(());
    }
    db.record_event(project.id, kind, text, &db::now(), "assistant")?;
    println!("Recorded a {kind} for {}", project.name);
    Ok(())
}

fn resolve(project: &str, id: i64, answer: Option<&str>) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let (kind, body) = db.resolve_event(project.id, id, answer)?;
    let first_line = body.lines().next().unwrap_or(&body);
    match kind.as_str() {
        "question" => println!("Answered [{id}]: {first_line}"),
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
    let found = db
        .find_events(&search::as_fts_query(query), project, kind, limit)
        .with_context(|| format!("{query:?} is not a search FTS5 understands"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }
    if found.is_empty() {
        println!("{}", search::nothing_found(query, project, kind));
        return Ok(());
    }
    // The project column is dead weight when the search was for one project.
    let show_project = project.is_none();
    for event in &found {
        print!("{}", search::render_event(event, show_project));
    }
    if found.len() as u32 == limit {
        println!("({limit} shown; --limit for more)");
    }
    Ok(())
}

/// The work that went into one version.
fn why(project: &str, version: &str, json: bool) -> Result<()> {
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

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "waiting": waiting,
                "shared": owner::shared_subjects(&waiting),
            }))?
        );
        return Ok(());
    }

    if waiting.is_empty() {
        match project {
            Some(name) => println!("{name} is waiting on nothing."),
            None => println!("Nothing is waiting on you."),
        }
        return Ok(());
    }

    let projects: std::collections::BTreeSet<&str> = waiting.iter().map(|q| q.project.as_str()).collect();
    match project {
        Some(_) => println!(
            "{}
",
            plural(waiting.len(), "question", "questions")
        ),
        None => println!(
            "{} in {}
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
        println!("{name:<12} [{:>3}] {}  {}", question.id, question.date, owner::subject(&question.body));
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
    println!(
        "
Answer one with: rigger resolve <project> <id> \"<answer>\""
    );
    Ok(())
}

/// What moved lately, per project.
fn digest(project: Option<&str>, since: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let days = parse_days(since)?;
    let from = day_before(days);

    let projects = match project {
        Some(name) => vec![open_project(&db, name)?],
        None => db.projects()?,
    };

    // The tier signals are read for the current week, whatever window the
    // digest itself covers: a promise broken is broken now, and a shorter
    // `--since` should not hide it.
    let signals = week_facts(&db, calendar::Week::current())?.signals;

    let mut reports = Vec::new();
    for project in &projects {
        let facts = db.digest(project.id, &from)?;
        let stage = db.current_stage(project.id)?;
        let next = stage.map(|s| match s.title {
            Some(title) => format!("{} · {title}", s.version),
            None => s.version,
        });
        let quiet = db.last_event_at(project.id)?.as_deref().and_then(days_since_utc);
        let signal = signals.iter().find(|s| s.project == project.name).map(signal_line);
        let lines = owner::digest_lines(&facts, next.as_deref(), quiet, signal.as_deref());
        reports.push((project.name.clone(), facts, next, lines, signal));
    }

    if json {
        let payload: Vec<_> = reports
            .iter()
            .map(|(name, facts, next, lines, signal)| serde_json::json!({ "project": name, "facts": facts, "next": next, "lines": lines, "signal": signal }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "since": from, "projects": payload }))?);
        return Ok(());
    }

    println!(
        "Since {from}
"
    );

    // A project with nothing but its next stage to report has not moved:
    // naming it in one line beats five lines that say nothing happened.
    //
    // A project raising a signal is the exception, and the important one: a
    // carrying product that has stopped releasing is quiet by definition,
    // and folding it into the quiet line is exactly how it stays unnoticed.
    let (moved, still): (Vec<_>, Vec<_>) = reports
        .iter()
        .partition(|(_, facts, _, _, signal)| signal.is_some() || !facts.shipped.is_empty() || facts.decisions + facts.findings + facts.changes > 0);

    let listed = if project.is_some() {
        reports.iter().collect::<Vec<_>>()
    } else {
        moved.clone()
    };
    for (name, _, _, lines, _) in &listed {
        println!("{name}");
        for line in lines.iter() {
            println!("  {line}");
        }
    }
    if listed.is_empty() {
        println!("Nothing moved.");
    }
    if project.is_none() && !still.is_empty() {
        let names: Vec<&str> = still.iter().map(|(name, _, _, _, _)| name.as_str()).collect();
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

fn plural(n: usize, one: &str, many: &str) -> String {
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

fn doc_add(project: &str, title: &str, kind: &str, slug: Option<&str>, body: Option<&str>) -> Result<()> {
    doc::check_kind(kind)?;
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;

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
        println!("Nothing was written; no document was made.");
        return Ok(());
    }
    let written = db.write_document(project.id, kind, &slug, title, &text)?;
    println!(
        "Wrote {} ({kind}, {}) to {}",
        written.slug,
        plural(written.body.len(), "byte", "bytes"),
        project.name
    );
    Ok(())
}

fn doc_edit(project: &str, slug: &str, title: Option<&str>, body: Option<&str>) -> Result<()> {
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
    println!("Wrote {} ({}, {})", written.slug, written.kind, plural(written.body.len(), "byte", "bytes"));
    Ok(())
}

fn doc_remove(project: &str, slug: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    let doc = open_document(&db, &project, slug)?;
    db.delete_document(project.id, &doc.slug)?;
    println!("Removed {} ({}) from {}", doc.slug, doc.kind, project.name);
    Ok(())
}

/// Shows the skeleton a kind starts from, or writes it out to be edited.
///
/// Without this the override is a filename in a doc page: the way to change
/// what a vision asks you should be to run the command that shows it.
fn doc_template(kind: &str, write: bool) -> Result<()> {
    doc::check_kind(kind)?;
    if !write {
        let paths = doc::template_paths(kind)?;
        let from = paths.iter().find(|p| p.is_file());
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
    println!("Wrote {}", path.display());
    println!("Edit it; `{}` in it becomes the document's title.", doc::TITLE_PLACEHOLDER);
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

fn backup(keep: usize, list: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if list {
        let copies = db.backups()?;
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
    println!("Copied to {}", target.display());
    let removed = db.prune_backups(keep)?;
    if !removed.is_empty() {
        println!("Kept the {keep} newest, deleted {}.", plural(removed.len(), "older copy", "older copies"));
    }
    Ok(())
}

fn init() -> Result<()> {
    // The config first, so that a fresh install has a profile to speak of
    // and the file a person can edit is where `doctor` says it is.
    let config_path = profile::Config::path()?;
    if !config_path.exists() {
        profile::Config::default().save()?;
        println!("Created {} with the '{}' profile", config_path.display(), profile::DEFAULT);
    }
    let path = paths::db_path()?;
    if path.exists() {
        Db::open(&path)?;
        println!("Already initialised: {}", path.display());
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let db = Db::create(&path)?;
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
    if let Some(inbox) = &p.inbox {
        println!("  inbox:     {}", inbox.display());
    }
    println!("  config:    {}", profile::Config::path()?.display());
    Ok(())
}

fn profile_use(name: &str) -> Result<()> {
    let mut config = profile::Config::load()?;
    if !config.profiles.contains_key(name) {
        bail!("no profile named '{name}'; see `rigger profile list`");
    }
    config.current = name.to_string();
    config.save()?;
    println!("Every command now uses the '{name}' profile");
    if std::env::var_os(profile::PROFILE_ENV).is_some() {
        println!("  note: {} is set and overrides this while it is", profile::PROFILE_ENV);
    }
    Ok(())
}

/// Adds a profile and its database. The database is created here rather
/// than on first use, so that `profile list` can point at a file that is
/// there.
fn profile_add(name: &str, p: profile::Profile, use_it: bool) -> Result<()> {
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
fn profile_set(name: Option<&str>, roots: Vec<PathBuf>, hubs: Option<PathBuf>, id_pattern: Option<String>, inbox: Option<PathBuf>) -> Result<()> {
    let mut config = profile::Config::load()?;
    let name = name.map(str::to_string).unwrap_or_else(|| config.current_name());
    let Some(p) = config.profiles.get_mut(&name) else {
        bail!("no profile named '{name}'; see `rigger profile list`");
    };
    if roots.is_empty() && hubs.is_none() && id_pattern.is_none() && inbox.is_none() {
        bail!("nothing to set; give --root, --hubs, --id-pattern or --inbox");
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
    if let Some(inbox) = inbox {
        p.inbox = Some(inbox);
    }
    config.save()?;
    println!("Profile '{name}' updated");
    profile_show(Some(&name), false)
}

/// The setting that names the card in hand, and when it was taken up.
const ACTIVE_CARD: &str = "active_card";
const ACTIVE_SINCE: &str = "active_since";
/// A card left open this long is not in hand any more: a desk that forgot
/// to close it yesterday must not be told today that it is.
const STALE_HOURS: i64 = 8;

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

fn task_status(task: &str, status: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let (project_id, id) = find_task(&db, task)?;
    let (title, was) = db.set_task_status(project_id, id, status)?;
    match was == status {
        true => println!("Task {task} was already {status}: {title}"),
        false => println!("Task {task} is now {status} (was {was}): {title}"),
    }
    Ok(())
}

fn task_new(title: &str, key: Option<&str>, aliases: &[String], projects: &[String], branch: Option<&str>, summary: Option<&str>) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    if title.trim().is_empty() {
        bail!("a card needs a title");
    }
    let card = db.new_card(key, title, aliases)?;
    if let Some(summary) = summary {
        db.set_card_summary(card.id, summary)?;
    }
    for name in projects {
        let project = open_project(&db, name)?;
        db.link_card(card.id, project.id, branch, None)?;
    }
    // The card just made is the one in hand: nobody makes a card for a
    // task they are not about to work on.
    db.set_setting(ACTIVE_CARD, Some(&card.id.to_string()))?;
    db.set_setting(ACTIVE_SINCE, Some(&db::now()))?;
    println!("Made card {} and opened it: {}", card.key, card.title);
    if key.is_none() {
        println!("  a local key; `rigger task rename {} <ID>` when the tracker names it", card.key);
    }
    print_links(&db, card.id)?;
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
    db.card(id.parse().unwrap_or_default())
}

fn task_open(task: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    db.set_setting(ACTIVE_CARD, Some(&card.id.to_string()))?;
    db.set_setting(ACTIVE_SINCE, Some(&db::now()))?;
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

fn task_close(task: Option<&str>, status: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = match task {
        Some(text) => find_card(&db, text)?,
        None => match active_card(&db)? {
            Some(card) => card,
            None => bail!("no card is in hand; name one: rigger task close <KEY>"),
        },
    };
    let desk = db.desk_project()?;
    let (title, was) = db.set_task_status(desk.id, card.id, status)?;
    if db.setting(ACTIVE_CARD)?.as_deref() == Some(&card.id.to_string()) {
        db.set_setting(ACTIVE_CARD, None)?;
        db.set_setting(ACTIVE_SINCE, None)?;
    }
    println!("Closed {} as {status} (was {was}): {title}", card.key);
    Ok(())
}

fn task_show(task: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let links = db.card_links(card.id)?;
    let events = db.task_events(card.id)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "card": card, "links": links, "events": events.len() }))?
        );
        return Ok(());
    }
    println!("{} · {}", card.key, card.title);
    println!("  status:   {}", card.status);
    if !card.aliases.is_empty() {
        println!("  aliases:  {}", card.aliases.join(", "));
    }
    println!("  since:    {}", card.created_at);
    if let Some(summary) = &card.summary {
        println!("  summary:  {summary}");
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
    Ok(())
}

/// The packet a task starts from: what it is, where it is worked, and
/// everything written against it, by kind, newest first.
fn render_card(db: &Db, card: &card::Card, budget: usize) -> Result<String> {
    let links = db.card_links(card.id)?;
    let events = db.task_events(card.id)?;
    let mut out = format!("# {} · {}\n\n", card.key, card.title);
    out.push_str(&format!("Status: {}", card.status));
    if !card.aliases.is_empty() {
        out.push_str(&format!(" · also {}", card.aliases.join(", ")));
    }
    out.push('\n');
    if let Some(summary) = &card.summary {
        out.push_str(&format!("\n{summary}\n"));
    }
    if !links.is_empty() {
        out.push_str("\n## Worked in\n");
        for link in &links {
            let branch = link.branch.as_deref().map(|b| format!(" on `{b}`")).unwrap_or_default();
            let role = link.role.as_deref().map(|r| format!(" - {r}")).unwrap_or_default();
            out.push_str(&format!("- {} ({}){branch}{role}\n", link.project, link.path));
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

fn task_context(task: &str, budget: usize) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    print!("{}", render_card(&db, &card, budget)?);
    Ok(())
}

fn task_link(task: &str, project: &str, branch: Option<&str>, role: Option<&str>) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let project = open_project(&db, project)?;
    db.link_card(card.id, project.id, branch, role)?;
    println!("{} is worked in:", card.key);
    print_links(&db, card.id)
}

fn task_list(status: &str, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let cards = db.cards(Some(status))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }
    if cards.is_empty() {
        println!(
            "No cards{}.",
            if status == "all" { "" } else { " that are " }.to_string() + if status == "all" { "" } else { status }
        );
        return Ok(());
    }
    let width = cards.iter().map(|c| c.key.len()).max().unwrap_or(0);
    for c in &cards {
        println!("{:width$}  {:<16} {}", c.key, c.status, c.title);
    }
    Ok(())
}

fn task_rename(task: &str, key: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let renamed = db.rename_card(card.id, key)?;
    println!("{} is now {}; {} stays as an alias", card.key, renamed.key, card.key);
    Ok(())
}

fn task_alias(task: &str, alias: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    let card = db.add_alias(card.id, alias)?;
    println!("{} also goes by: {}", card.key, card.aliases.join(", "));
    Ok(())
}

fn task_summary(task: &str, text: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let card = find_card(&db, task)?;
    db.set_card_summary(card.id, text)?;
    println!("{} summarised", card.key);
    Ok(())
}

fn project_add(path: PathBuf, name: Option<String>) -> Result<()> {
    let root = dunce::canonicalize(&path).with_context(|| format!("{} is not a directory rigger can read", path.display()))?;
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let db = Db::open(&paths::db_path()?)?;
    let name = name.unwrap_or_else(|| repo::detect_name(&root));
    let remote = repo::detect_remote(&root);
    let project = db.add_project(&name, &root.to_string_lossy(), remote.as_deref(), db::Kind::Repo)?;
    println!("Recorded '{}' at {}", project.name, project.path);
    match &project.remote {
        Some(url) => println!("  remote: {url}"),
        None => println!("  remote: none (no origin in .git/config)"),
    }
    Ok(())
}

/// Records a place the record keeps for itself.
///
/// A retro looks across every project and has to leave its summary
/// somewhere that is not one of them. That place has no repository and
/// never will, so it is recorded as what it is: `sync` does not ask git
/// about it and `doctor` does not list it as waiting to be synced.
fn project_service(name: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    // The path is a name, not a location: the column is unique and every
    // other project fills it with a directory, so a marker keeps the two
    // apart without pretending there is a directory to look in.
    let path = format!("service:{name}");
    let project = db.add_project(name, &path, None, db::Kind::Service)?;
    println!("Recorded '{}' as a place the record keeps for itself", project.name);
    println!("  no repository: sync will not ask git about it");
    Ok(())
}

fn project_list(json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects = db.projects()?;
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
        println!("{:width$}  {}", p.name, where_it_lives(p));
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
        db::Kind::Service => println!("  kind:    a place the record keeps for itself; no repository"),
    }
    println!("  since:   {}", project.created_at);
    Ok(())
}

fn project_tier(name: &str, tier: &str, rhythm: Option<u32>) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, name)?;
    let tier = calendar::Tier::parse(tier)?;
    // A tier carries a rhythm of its own, so setting one is a single word
    // in the common case; `--rhythm` is for the project that keeps its
    // tier's company but not its pace.
    let rhythm = match rhythm {
        Some(0) => bail!("a rhythm of 0 weeks is not a rhythm; leave it out to use the tier's"),
        Some(weeks) => Some(weeks),
        None => tier.default_rhythm(),
    };
    db.set_tier(project.id, tier.as_str(), rhythm)?;

    println!("{} is tier {tier} - {}", project.name, tier.describe());
    match rhythm {
        Some(weeks) => println!("  a release every {}", plural(weeks as usize, "week", "weeks")),
        None => println!("  no rhythm to keep"),
    }
    Ok(())
}

fn version_plan(project: &str, version: &str, week: Option<&str>, clear: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    if week.is_none() && !clear {
        bail!("say which week with --week 2026-W37, or --clear to take it off the calendar");
    }
    let week = week.map(calendar::Week::parse).transpose()?;
    let stored = week.map(|w| w.to_string());
    let change = db.set_planned_week(project.id, version, stored.as_deref())?;

    match (week, change) {
        (_, db::Change::Unchanged) => println!("{version} was already there; nothing changed"),
        (Some(week), _) => println!("{version} is aimed at {week} - the week of {}", week.friday()),
        (None, _) => println!("{version} is off the calendar"),
    }
    Ok(())
}

/// The grid: weeks across, projects down.
fn show_calendar(weeks: u32, from: Option<&str>, json: bool) -> Result<()> {
    if weeks == 0 {
        bail!("a calendar of 0 weeks shows nothing; ask for at least one");
    }
    let db = Db::open(&paths::db_path()?)?;
    let now = calendar::Week::current();
    let from = match from {
        Some(text) => calendar::Week::parse(text)?,
        None => now,
    };

    let mut rows = Vec::new();
    let mut all = Vec::new();
    for project in db.projects()? {
        let versions = db.calendar_versions(project.id, &project.name)?;
        let tier = project.tier.as_deref().and_then(|t| calendar::Tier::parse(t).ok());
        let row = calendar::row(&project.name, tier, project.rhythm_weeks, &versions, from, weeks, now);
        if !row.cells.is_empty() {
            rows.push(row);
        }
        all.push((project.name.clone(), versions));
    }

    let span: Vec<calendar::Week> = (0..weeks).map(|n| from.plus(i64::from(n))).collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "now": now,
                "weeks": span,
                "projects": rows,
            }))?
        );
        return Ok(());
    }

    if rows.is_empty() {
        println!("Nothing is on the calendar for these {}.", plural(weeks as usize, "week", "weeks"));
        println!("Aim a version at a week with: rigger version plan <project> <version> --week 2026-W37");
        return Ok(());
    }

    // Each column is as wide as the widest thing in it, so a week holding
    // two releases does not push the rest of the grid out of line.
    let name_width = rows.iter().map(|r| r.project.chars().count()).max().unwrap_or(0).max(7);
    let widths: Vec<usize> = span
        .iter()
        .map(|week| {
            rows.iter()
                .map(|row| cell_text(row, *week).chars().count())
                .max()
                .unwrap_or(0)
                // The heading needs room too, and this week's carries a mark.
                .max(week.to_string().chars().count() + usize::from(*week == now))
        })
        .collect();

    print!("{:name_width$}", "");
    for (week, width) in span.iter().zip(&widths) {
        // This week is marked in the heading, because a grid read on a
        // Wednesday is read from where the reader stands.
        let heading = if *week == now { format!("{week}*") } else { week.to_string() };
        print!("  {heading:width$}");
    }
    println!();

    for row in &rows {
        print!("{:name_width$}", row.project);
        for (week, width) in span.iter().zip(&widths) {
            print!("  {:width$}", cell_text(row, *week));
        }
        if let Some(tier) = row.tier {
            print!("   {tier}");
        }
        println!();
    }

    println!();
    println!(
        "{} shipped as planned   {} slipped   {} overdue   {} unplanned   {} planned",
        calendar::Standing::Shipped.mark(),
        calendar::Standing::Slipped.mark(),
        calendar::Standing::Overdue.mark(),
        calendar::Standing::Unplanned.mark(),
        calendar::Standing::Planned.mark(),
    );

    // Slippage, spelt out. The grid shows that a release moved; only a
    // number says how far, and that is what a retrospective needs.
    let mut late: Vec<String> = Vec::new();
    for row in &rows {
        let Some((_, versions)) = all.iter().find(|(name, _)| *name == row.project) else {
            continue;
        };
        for cell in &row.cells {
            if !matches!(cell.standing, calendar::Standing::Slipped | calendar::Standing::Overdue) {
                continue;
            }
            let Some(version) = versions.iter().find(|v| v.version == cell.version) else {
                continue;
            };
            let Some(weeks) = version.slip().or_else(|| version.overdue(now)) else {
                continue;
            };
            let aimed = version.planned.map(|w| w.to_string()).unwrap_or_default();
            late.push(format!(
                "{:name_width$}  {} — aimed at {aimed}, {}",
                row.project,
                cell.version,
                weeks_late(weeks)
            ));
        }
    }
    if !late.is_empty() {
        println!();
        for line in &late {
            println!("{line}");
        }
    }
    Ok(())
}

/// What one cell of the grid says.
///
/// Two releases in a week are named; more than two are counted. The real
/// record made this necessary rather than tidy: one week of one project
/// holds forty-six releases, and naming them all stretched the column past
/// three hundred characters, wrapped every row and pushed the heading out
/// of line - a grid that could not be read at all. The count keeps the
/// shape, and `why` is where the names belong anyway.
fn cell_text(row: &calendar::Row, week: calendar::Week) -> String {
    let cells: Vec<&calendar::Cell> = row.cells.iter().filter(|cell| cell.week == week).collect();
    let named = |cell: &calendar::Cell| format!("{}{}", cell.standing.mark(), cell.version);
    match cells.len() {
        0 => String::new(),
        1..=2 => cells.iter().map(|c| named(c)).collect::<Vec<_>>().join(" "),
        n => {
            // The first and last say what the run spans; the mark is the
            // worst standing in it, so a slipped release inside a busy week
            // is not hidden by the ones around it.
            let worst = cells
                .iter()
                .map(|c| c.standing)
                .max_by_key(|s| severity(*s))
                .unwrap_or(calendar::Standing::Shipped);
            format!(
                "{}{}..{} ({n})",
                worst.mark(),
                cells.first().map(|c| c.version.as_str()).unwrap_or(""),
                cells.last().map(|c| c.version.as_str()).unwrap_or("")
            )
        }
    }
}

/// How much a standing wants to be seen when a cell can only show one.
fn severity(standing: calendar::Standing) -> u8 {
    match standing {
        calendar::Standing::Overdue => 4,
        calendar::Standing::Slipped => 3,
        calendar::Standing::Planned => 2,
        calendar::Standing::Unplanned => 1,
        calendar::Standing::Shipped => 0,
    }
}

fn weeks_late(weeks: i64) -> String {
    match weeks {
        1 => "a week late".to_string(),
        n if n < 0 => format!("{} early", plural(n.unsigned_abs() as usize, "week", "weeks")),
        n => format!("{} late", plural(n as usize, "week", "weeks")),
    }
}

/// Everything the week screens read, gathered once.
///
/// `next`, `week` and `release-day` are three views of one week, and the
/// awkward part is not any of the three but keeping them agreed: a version
/// counted as the focus by one and as shipped by another would make the
/// screens argue with each other in front of the owner.
struct WeekFacts {
    focus: Vec<calendar::Focus>,
    overdue: Vec<calendar::Focus>,
    lapsed: Vec<calendar::Overdue>,
    signals: Vec<week::Raised>,
    release_day: week::ReleaseDay,
}

fn week_facts(db: &Db, now: calendar::Week) -> Result<WeekFacts> {
    let mut focus = Vec::new();
    let mut overdue = Vec::new();
    let mut rhythms = Vec::new();
    let mut standings = Vec::new();
    let mut all_versions = Vec::new();

    for project in db.projects()? {
        let versions = db.calendar_versions(project.id, &project.name)?;
        let tier = project.tier.as_deref().and_then(|t| calendar::Tier::parse(t).ok());

        for version in &versions {
            if version.planned == Some(now) && version.shipped.is_none() {
                focus.push(calendar::Focus {
                    project: project.name.clone(),
                    tier,
                    version: version.version.clone(),
                    title: version.title.clone(),
                    planned: now,
                    overdue_weeks: None,
                });
            } else if let Some(weeks) = version.overdue(now) {
                overdue.push(calendar::Focus {
                    project: project.name.clone(),
                    tier,
                    version: version.version.clone(),
                    title: version.title.clone(),
                    planned: version.planned.unwrap_or(now),
                    overdue_weeks: Some(weeks),
                });
            }
        }

        let last_shipped = versions
            .iter()
            .filter_map(|v| v.shipped.map(|week| (db::version_order(&v.version), week)))
            .max()
            .map(|(_, week)| week);

        // The rhythm check needs a tier and a number to check against; a
        // project with neither is out of the rotation by omission.
        if let (Some(tier), Some(rhythm)) = (tier, project.rhythm_weeks)
            && tier != calendar::Tier::Out
        {
            rhythms.push((project.name.clone(), tier, rhythm, last_shipped));
        }

        if let Some(tier) = tier {
            // A turn in the focus leaves a mark whether or not it ends in a
            // tag: the last commit and the last note both count, because a
            // week spent on a product that shipped nothing was still spent.
            let touched = [db.last_event_at(project.id)?, db.activity(project.id)?.and_then(|a| a.last_commit_at)]
                .into_iter()
                .flatten()
                .filter_map(|stamp| calendar::Week::of_recorded(&stamp))
                .max();
            standings.push(week::Standing {
                project: project.name.clone(),
                tier,
                rhythm_weeks: project.rhythm_weeks,
                last_shipped,
                last_touched: touched,
                has_first_release: last_shipped.is_some(),
            });
        }

        all_versions.extend(versions);
    }

    focus.sort_by(|a, b| a.tier.cmp(&b.tier).then_with(|| a.project.cmp(&b.project)));
    overdue.sort_by(|a, b| b.overdue_weeks.cmp(&a.overdue_weeks).then_with(|| a.project.cmp(&b.project)));

    Ok(WeekFacts {
        focus,
        overdue,
        lapsed: calendar::lapsed(&rhythms, now),
        signals: week::signals(&standings, now),
        release_day: week::release_day(now, &all_versions),
    })
}

/// Reads a week from the flag, or takes the current one.
fn week_or_now(week: Option<&str>) -> Result<calendar::Week> {
    match week {
        Some(text) => calendar::Week::parse(text),
        None => Ok(calendar::Week::current()),
    }
}

/// The focus of a week: what is aimed at it, and what should have shipped
/// before it.
fn show_next(week_arg: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let now = week_or_now(week_arg)?;
    let WeekFacts {
        focus,
        overdue,
        lapsed,
        signals,
        ..
    } = week_facts(&db, now)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "week": now,
                "friday": now.friday().to_string(),
                "focus": focus,
                "overdue": overdue,
                "lapsed": lapsed,
                "signals": signals,
            }))?
        );
        return Ok(());
    }

    println!("{now} — releases on {}", now.friday());
    println!();

    if focus.is_empty() {
        println!("Nothing is aimed at this week.");
    } else {
        for item in &focus {
            let tier = item.tier.map(|t| format!(" [{t}]")).unwrap_or_default();
            let title = item.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            println!("{}{tier}  {}{title}", item.project, item.version);
        }
    }

    if !overdue.is_empty() {
        println!();
        println!("Past their week:");
        for item in &overdue {
            let weeks = item.overdue_weeks.unwrap_or_default();
            let ago = if weeks == 1 {
                "a week ago".to_string()
            } else {
                format!("{} ago", plural(weeks.max(0) as usize, "week", "weeks"))
            };
            println!("{}  {} — was due {} ({ago})", item.project, item.version, item.planned);
        }
    }

    // A project that has kept no rhythm is not late for a week, it is late
    // for its tier - the failure the written calendar could never see,
    // because nothing ever compared the rotation to the tags.
    if !lapsed.is_empty() {
        println!();
        println!("Behind their rhythm:");
        for item in &lapsed {
            let since = match item.since {
                Some(week) => format!("last shipped {week}"),
                None => "never shipped".to_string(),
            };
            println!(
                "{} [{}]  {since}, {} without a release, rhythm is {}",
                item.project,
                item.tier,
                plural(item.weeks.max(0) as usize, "week", "weeks"),
                plural(item.rhythm_weeks as usize, "week", "weeks")
            );
        }
    }

    print_signals(&signals);
    Ok(())
}

/// The minimums each tier promised, and which of them are being broken.
///
/// Separate from the rhythm lapse above on purpose: a rhythm is a pace and
/// this is a floor. A carrying product is allowed to miss one cycle, so the
/// lapse fires first and the signal only when the allowance is spent.
/// One signal as a line of prose, for a screen that has room for one.
fn signal_line(item: &week::Raised) -> String {
    let weeks = item.weeks.map(|w| plural(w.max(0) as usize, "week", "weeks")).unwrap_or_default();
    match item.signal {
        week::Signal::MissedCycle => format!("tier {} asks for more: more than one cycle missed - {weeks} without a release", item.tier),
        week::Signal::WithoutFocus => format!("tier {} asks for more: no turn in the focus for {weeks}", item.tier),
        week::Signal::SecondStart => match item.alongside.as_deref() {
            Some(first) => format!("tier {} asks for more: started before {first} shipped anything", item.tier),
            None => format!("tier {} asks for more: started out of turn", item.tier),
        },
    }
}

fn print_signals(signals: &[week::Raised]) {
    if signals.is_empty() {
        return;
    }
    println!();
    println!("Their tier asks for more:");
    for item in signals {
        // Worded once, in `signal_line`, and read here with the heading's
        // own phrase removed. The calendar legend taught this at v0.10.0:
        // two places spelling one fact drift, and the test that compared
        // them is what found it.
        let said = signal_line(item).replacen(&format!("tier {} asks for more: ", item.tier), "", 1);
        println!("{} [{}]  {said}", item.project, item.tier);
    }
}

/// The Monday brief: one screen the week opens on.
///
/// The three things it answers are the three the owner otherwise asks by
/// hand on a Monday morning, from three different places: what am I meant
/// to be working on, what goes out on Friday, and what is waiting on me.
/// None of them is new - the brief is that they arrive together, before the
/// week is spent rather than after.
fn show_week(week_arg: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let now = week_or_now(week_arg)?;
    let facts = week_facts(&db, now)?;
    let waiting = db.open_questions()?;
    let shared = owner::shared_subjects(&waiting);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "week": now,
                "monday": now.monday().to_string(),
                "friday": now.friday().to_string(),
                "focus": facts.focus,
                "overdue": facts.overdue,
                "shipping": facts.release_day.queued,
                "shipped": facts.release_day.shipped,
                "waiting": waiting,
                "shared": shared,
                "lapsed": facts.lapsed,
                "signals": facts.signals,
            }))?
        );
        return Ok(());
    }

    println!("{now} — {} to {}", now.monday(), now.friday());
    println!();

    println!("Focus");
    if facts.focus.is_empty() {
        println!("  nothing is aimed at this week");
    } else {
        for item in &facts.focus {
            let tier = item.tier.map(|t| format!(" [{t}]")).unwrap_or_default();
            let title = item.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            println!("  {}{tier}  {}{title}", item.project, item.version);
        }
    }

    println!();
    println!("Ships on {}", now.friday());
    if facts.release_day.queued.is_empty() && facts.release_day.shipped.is_empty() {
        println!("  nothing is queued");
    } else {
        for item in &facts.release_day.queued {
            let title = item.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            println!("  {}  {}{title}", item.project, item.version);
        }
        // What has already gone out is part of the same answer: the week has
        // one slot on the shopfront, and a week that has spent it has
        // nothing left to ship however full the queue behind it looks.
        let out = facts.release_day.shipped.len();
        if out > 0 {
            let over = facts.release_day.over_the_slot();
            let spent = if over > 0 {
                format!("  {} already out — {} past this week's one slot", plural(out, "release", "releases"), over)
            } else {
                format!("  {} already out — this week's slot is spent", plural(out, "release", "releases"))
            };
            println!("{spent}");
            println!("  see the queue with: rigger release-day");
        }
    }

    println!();
    println!("Waiting on you");
    if waiting.is_empty() {
        println!("  nothing");
    } else {
        let projects: std::collections::BTreeSet<&str> = waiting.iter().map(|q| q.project.as_str()).collect();
        println!(
            "  {} in {}",
            plural(waiting.len(), "question", "questions"),
            plural(projects.len(), "project", "projects")
        );
        // The groups are what makes the queue smaller than it looks, so they
        // are the part worth naming on a screen that is meant to be short.
        for group in shared.iter().take(3) {
            println!("  {} — {}", group.subject, group.projects.join(", "));
        }
        println!("  see them with: rigger inbox");
    }

    if !facts.overdue.is_empty() {
        println!();
        println!("Past their week:");
        for item in &facts.overdue {
            println!("  {}  {} — was due {}", item.project, item.version, item.planned);
        }
    }

    print_signals(&facts.signals);
    Ok(())
}

/// The shopfront queue: what a week has already put out, and what is due.
///
/// The rule this reads against is the one the written calendar set for the
/// outside view: one release a week, on a Friday, and a version ready on a
/// Tuesday waits rather than going out on top of the last one. The reason
/// is not tidiness - two releases in a day read as one burst to anyone
/// watching, and two in different weeks read as a rhythm. The trace is what
/// is meant to be even, not the work.
fn show_release_day(week_arg: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let now = week_or_now(week_arg)?;
    let day = week_facts(&db, now)?.release_day;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "week": day.week,
                "friday": day.friday,
                "shipped": day.shipped,
                "queued": day.queued,
                "early": day.early(),
                "over_the_slot": day.over_the_slot(),
            }))?
        );
        return Ok(());
    }

    println!("{now} — releases on {}", day.friday);
    println!();

    if day.queued.is_empty() {
        println!("Nothing is waiting for Friday.");
    } else {
        println!("Waiting for Friday:");
        for item in &day.queued {
            let title = item.title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default();
            println!("  {}  {}{title}", item.project, item.version);
        }
    }

    // Folded by day, because the day is what the rule is about and because
    // a real week of this line holds ninety-four releases: a line each puts
    // the two numbers that answer the question below the fold, where the
    // calendar grid learnt the same lesson at v0.10.0.
    let days = day.days();
    if !days.is_empty() {
        println!();
        println!("Already out this week:");
        for entry in &days {
            let mark = if entry.on_release_day { "Friday" } else { "early" };
            let named: Vec<String> = entry.projects.iter().map(|p| format!("{} {}", p.project, p.summary())).collect();
            println!("  {}  {:<6}  {:>2}  {}", entry.day, mark, entry.releases, named.join(", "));
        }
    }

    // The two numbers say which half of the rule is being broken: going out
    // before Friday, and going out more than once in a week. They are said
    // as counts rather than as complaints - the record reports, and what to
    // do about it is the owner's.
    let early = day.early();
    let over = day.over_the_slot();
    if early > 0 || over > 0 {
        println!();
        if over > 0 {
            println!("{} past the one release this week has room for", plural(over, "release", "releases"));
        }
        if early > 0 {
            println!("{} went out before Friday", plural(early, "release", "releases"));
        }
    }
    Ok(())
}

/// The look back: what the plan said, what the tags say, and where the two
/// parted company.
///
/// The written calendar asked for this every seven weeks and had no way to
/// do it, because nothing there ever read a tag - so the check was a thing
/// to remember, and a thing to remember is a thing that stops happening.
fn show_retro(cycle: bool, weeks: Option<u32>, to: Option<&str>, record: bool, json: bool) -> Result<()> {
    let span = match (cycle, weeks) {
        (true, _) => retro::CYCLE_WEEKS,
        (_, Some(0)) => bail!("a retro of 0 weeks looks back at nothing; ask for at least one"),
        (_, Some(n)) => n,
        // Four weeks by default: long enough to hold more than one release
        // of a tier A product, short enough that a Monday can read it.
        (false, None) => 4,
    };
    let db = Db::open(&paths::db_path()?)?;
    let to = week_or_now(to)?;
    let from = to.plus(-i64::from(span - 1));

    let mut versions = Vec::new();
    let mut projects = Vec::new();
    for project in db.projects()? {
        versions.extend(db.calendar_versions(project.id, &project.name)?);
        let tier = project.tier.as_deref().and_then(|t| calendar::Tier::parse(t).ok());
        projects.push((project.name.clone(), tier, project.rhythm_weeks));
    }
    let looked = retro::look_back(from, to, &versions, &projects);
    let summary = retro::summary(&looked);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "from": looked.from,
                "to": looked.to,
                "weeks": looked.weeks(),
                "shipped": looked.shipped,
                "missed": looked.missed,
                "standings": looked.standings,
                "on_time": looked.on_time(),
                "slipped": looked.slipped(),
                "unplanned": looked.unplanned(),
                "planned_share": looked.planned_share(),
                "summary": summary,
            }))?
        );
        return Ok(());
    }

    println!("{} to {} — {}", looked.from, looked.to, plural(looked.weeks().max(0) as usize, "week", "weeks"));
    println!();

    if looked.shipped.is_empty() && looked.missed.is_empty() {
        println!("Nothing shipped and nothing was aimed at these weeks.");
        // A window where nothing happened is a real answer, but it is not
        // one worth filing: a retro is kept so a later one can find what
        // was concluded, and "nothing" concludes nothing.
        if record {
            println!();
            println!("Nothing to keep.");
        }
        return Ok(());
    }

    // The three numbers the check is read for, and the share underneath
    // them: how much of what shipped was ever planned. A line where nothing
    // was planned has a calendar in name only, and that is worth saying.
    println!(
        "{} shipped — {} on time, {} slipped, {} unplanned",
        looked.shipped.len(),
        looked.on_time(),
        looked.slipped(),
        looked.unplanned()
    );
    if let Some(share) = looked.planned_share() {
        println!("{share}% of what shipped had been planned");
    }

    if !looked.missed.is_empty() {
        println!();
        println!("Planned and not shipped:");
        for item in &looked.missed {
            println!(
                "  {}  {} — was due {} ({} by the end of the window)",
                item.project,
                item.version,
                item.planned,
                weeks_late(item.weeks)
            );
        }
    }

    // Slippage spelt out, worst first: the grid shows that a release moved,
    // only a number says how far, and "what turned out dearer" was one of
    // the three questions the written calendar asked.
    let mut slipped: Vec<&retro::Shipped> = looked.shipped.iter().filter(|s| s.slip.is_some_and(|n| n != 0)).collect();
    slipped.sort_by_key(|s| std::cmp::Reverse(s.slip));
    if !slipped.is_empty() {
        println!();
        println!("Shipped, but not when it was aimed:");
        for item in slipped.iter().take(10) {
            let aimed = item.planned.map(|w| w.to_string()).unwrap_or_default();
            println!(
                "  {}  {} — aimed at {aimed}, out in {} ({})",
                item.project,
                item.version,
                item.week,
                weeks_late(item.slip.unwrap_or(0))
            );
        }
        if slipped.len() > 10 {
            println!("  ... and {} more", slipped.len() - 10);
        }
    }

    if !looked.standings.is_empty() {
        println!();
        println!("Per project:");
        let width = looked.standings.iter().map(|s| s.project.chars().count()).max().unwrap_or(0);
        for item in &looked.standings {
            let tier = item.tier.map(|t| format!("[{t}]")).unwrap_or_else(|| "   ".to_string());
            let asked = match item.expected {
                Some(n) => format!("{n} asked"),
                None => "none asked".to_string(),
            };
            let missed = if item.missed > 0 {
                format!(", {} missed", item.missed)
            } else {
                String::new()
            };
            println!(
                "  {:width$} {tier}  {} shipped ({} planned), {asked}{missed}",
                item.project, item.shipped, item.planned_and_shipped
            );
        }
    }

    // "Do the tiers need moving" was the third question the calendar asked.
    // The two directions are shown apart because they are different
    // problems: a product shipping twenty times its tier has outgrown it,
    // one shipping nothing is stalled, and a single list of "misfits" loses
    // exactly the distinction worth acting on.
    let stalled = looked.misfits(retro::Misfit::Stalled);
    let outgrown = looked.misfits(retro::Misfit::Outgrown);
    if !stalled.is_empty() {
        println!();
        println!("Nothing shipped, and their tier asked for something:");
        for item in &stalled {
            let tier = item.tier.map(|t| t.to_string()).unwrap_or_default();
            println!("  {} [{tier}]  0 against {} asked for", item.project, item.expected.unwrap_or(0));
        }
    }
    if !outgrown.is_empty() {
        println!();
        println!("Shipping past their tier — it may be describing the wrong thing now:");
        for item in outgrown.iter().take(5) {
            let tier = item.tier.map(|t| t.to_string()).unwrap_or_default();
            let over = item.times_over().unwrap_or(0);
            println!(
                "  {} [{tier}]  {} shipped against {} asked for ({over}x)",
                item.project,
                item.shipped,
                item.expected.unwrap_or(0)
            );
        }
        if outgrown.len() > 5 {
            println!("  ... and {} more", outgrown.len() - 5);
        }
    }
    if !stalled.is_empty() || !outgrown.is_empty() {
        println!("  move one with: rigger project tier <project> <A|B|C|out>");
    }

    println!();
    if record {
        record_retro(&db, &looked, &summary)?;
    } else {
        println!("Keep this in the record with: rigger retro --record");
    }
    Ok(())
}

/// Writes the retro's summary into the record.
///
/// It goes to the project the record keeps for itself rather than to any of
/// the projects looked at: the summary is about all of them, and filing it
/// under one would make it findable from the wrong place and invisible from
/// the rest. A retro that is only ever printed leaves the same hole the
/// written calendar had, where the check happened and nothing afterwards
/// could tell that it did.
fn record_retro(db: &Db, looked: &retro::Retro, summary: &str) -> Result<()> {
    let Some(project) = db.service_project()? else {
        bail!(
            "no place to keep it: a retro is about every project, so its summary belongs to none of them.
Make one with: rigger project service line"
        );
    };
    // Dated by the window it looked at, not by the moment it was run. The
    // same retro of the same weeks is the same fact however often it is
    // asked for, and stamping it with "now" filed a fresh copy every time -
    // which is how a record fills with restatements of one conclusion.
    let at = format!("{}T00:00:00Z", looked.to.friday());
    let change = db.record_event(project.id, "change", summary, &at, "assistant")?;
    match change {
        db::Change::Unchanged => println!("That retro is already in the record, under '{}'.", project.name),
        _ => println!("Kept in the record under '{}'.", project.name),
    }
    Ok(())
}

/// Opens a sitting. Everything recorded until `end` belongs to it.
fn session_start(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = project_here(&db, project)?;
    let (session, change) = db.start_session(project.id, &db::now())?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "session": session, "already_open": change == db::Change::Unchanged }))?
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
fn session_end(project: Option<&str>, heading: Option<&str>, diary: Option<&Path>, remind: bool, json: bool) -> Result<()> {
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
    let events = db.session_events(open.id)?;
    let shipped = db.shipped_between(project.id, &open.started_at, &at)?;
    let closed = db.tasks_closed_between(project.id, &open.started_at, &at)?;
    let next_step = db.latest_event_body(project.id, "next")?;
    let ended = db::Session {
        ended_at: Some(at.clone()),
        ..open.clone()
    };
    let summary = session::summarise(&project.name, &ended, &events, shipped, closed, next_step);

    db.end_session(open.id, &at)?;

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
    if !summary.empty() {
        let entry = session::diary_entry(&summary, &day, heading);
        let body = entry.split_once('\n').map(|(_, rest)| rest.trim()).unwrap_or_default();
        let title = heading.map(str::trim).filter(|h| !h.is_empty()).map(|h| format!("{day} · {h}"));
        db.write_session_diary(open.id, project.id, &day, title.as_deref(), body)?;
    }
    let written = match diary {
        Some(path) => Some(write_diary(path, &summary, heading)?),
        None => None,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "session": summary,
                "missing": summary.missing(),
                "diary": written,
                "backup": insured,
            }))?
        );
        return Ok(());
    }

    // A hook speaks only when there is something to say. A reminder that
    // fires on every stop is a reminder nobody reads.
    if remind {
        let missing = summary.missing();
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

    let missing = summary.missing();
    if !missing.is_empty() {
        println!();
        println!("The ritual asks for:");
        for item in &missing {
            println!("  {item}");
        }
    }

    match written {
        Some(path) => println!(
            "
Diary entry appended to {path}"
        ),
        None => println!(
            "
Write it into a diary with: rigger session end {} --diary <file>",
            project.name
        ),
    }
    Ok(())
}

/// Appends the entry to a diary file, newest first.
///
/// Newest-first is how the hub's diary is written, so a new entry goes
/// under the heading and above what came before rather than at the end.
fn write_diary(path: &Path, summary: &session::Summary, heading: Option<&str>) -> Result<String> {
    let day = summary.ended_at.split('T').next().unwrap_or_default().to_string();
    let entry = session::diary_entry(summary, &day, heading);

    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };

    // The preamble is whatever the file says before its first entry: a
    // title, a note about the format, a rule. A new entry goes after it and
    // above the entries, because that is where the newest one belongs.
    let (preamble, entries) = match existing.find(
        "
## ",
    ) {
        Some(at) => existing.split_at(at + 1),
        None => (existing.as_str(), ""),
    };
    let mut out = String::new();
    if !preamble.trim().is_empty() {
        out.push_str(preamble.trim_end());
        out.push_str(
            "

",
        );
    }
    out.push_str(entry.trim_end());
    out.push_str(
        "

",
    );
    if !entries.trim().is_empty() {
        out.push_str(entries.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
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
/// becomes a view of what was written down somewhere else. Only the three
/// files the record can rebuild are touched: Vision, the decision log's
/// prose and the research notes are argument rather than record, and the
/// record has no way to hold an argument that would survive being rebuilt.
fn export_hub(project: &str, hub_dir: &Path, check: bool, adopt: bool, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    if !hub_dir.is_dir() {
        bail!("{} is not a directory", hub_dir.display());
    }
    // Spelt the way the platform spells it, the way import records it.
    let hub_dir = &dunce::canonicalize(hub_dir).unwrap_or_else(|_| hub_dir.to_path_buf());

    let mut files = Vec::new();
    for name in export::GENERATED {
        files.push((name, generate(&db, &project, name)?));
    }
    if !check {
        db.set_hub_path(project.id, hub_dir)?;
    }

    let mut written = Vec::new();
    for (name, text) in &files {
        let path = hub_dir.join(name);
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        // Written in the ending the file already used. Every hub of this
        // line is CRLF, and a generated file in LF would differ from its
        // source on every line - which is not a diff anybody reads.
        let text = &export::with_line_ending(text, export::line_ending(&before));
        let unchanged = before == *text;

        // A file a person has been writing in is not overwritten without
        // being asked. The mark is what says the record owns it, and it is
        // put there by an export - so the first one has to be deliberate.
        // A file a person has been writing in is not overwritten without
        // being asked. The mark is what says the record owns it, and only
        // an explicit `--adopt` puts the mark there the first time.
        if !unchanged && !before.is_empty() && !export::is_generated(&before) && !adopt && !check {
            bail!(
                "{} was written by hand and the record does not own it yet.
Check what would change with `--check`, then hand it over with `--adopt`.",
                path.display()
            );
        }
        if !check && !unchanged {
            std::fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
        }
        written.push(export::Written {
            file: name.to_string(),
            bytes: text.len(),
            unchanged,
        });
    }

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
    match (changed, check) {
        (0, _) => println!("\n{} is already what the record says.", hub_dir.display()),
        (n, true) => println!("\n{} of {} files differ from the record.", n, written.len()),
        (n, false) => println!("\n{} wrote {} of {} files.", project.name, n, written.len()),
    }
    Ok(())
}

/// One generated file of a hub, from the record.
///
/// The single place a hub file is produced, so that `export` and
/// `doctor --hubs` cannot disagree about what the record says a file should
/// contain - a check that generated the file a second way would eventually
/// pass while the export wrote something else.
fn generate(db: &Db, project: &db::Project, name: &str) -> Result<String> {
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

/// Where a generated hub file no longer matches the record.
///
/// A file the record owns is a view of the record; edited by hand it stops
/// being one, and the next export would overwrite the edit without saying
/// so. This is what tells the owner before that happens.
fn hub_drift(db: &Db) -> Result<Vec<(String, String, &'static str)>> {
    let mut out = Vec::new();
    for project in db.projects()? {
        // A place the record keeps for itself has no hub to vouch for.
        if !project.kind.reads_git() {
            continue;
        }
        // Where the record says the hub is. It used to be guessed beside
        // the repository, and every hub of this line lives in a notes vault
        // instead - so the check read no files at all and reported that
        // every generated file matched, on hubs it had never opened.
        let Some(dir) = project.hub_path.as_deref().map(std::path::PathBuf::from) else {
            out.push((project.name.clone(), String::from("-"), "no hub recorded; import or export one"));
            continue;
        };
        if !dir.is_dir() {
            out.push((project.name.clone(), String::from("-"), "the hub is not where the record says"));
            continue;
        }
        let mut by_hand = Vec::new();
        for name in export::GENERATED {
            let path = dir.join(name);
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if !export::is_generated(&text) {
                by_hand.push(name);
                continue;
            }
            let want = generate(db, &project, name)?;
            let want = export::with_line_ending(&want, export::line_ending(&text));
            if want != text {
                out.push((project.name.clone(), name.to_string(), "edited since it was generated"));
            }
        }
        // A hub still kept by hand is one the record cannot vouch for
        // either - and the one thing "every project through rigger" has
        // left to do. Named as such, so the list says how far along the
        // line is rather than staying silent about the files it skipped.
        if !by_hand.is_empty() {
            out.push((project.name.clone(), by_hand.join(", "), "kept by hand; `rigger export --adopt` hands it over"));
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

    // Where the plan and git disagree. Reported, never corrected: the record
    // cannot prove a tag's absence - it may simply not have been fetched -
    // and a silent correction would erase what the owner wrote (ADR 0005).
    let mut mismatches = Vec::new();
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
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "profile": profile_name,
                "database": path,
                "initialised": true,
                "schema_version": schema,
                "counts": counts,
                "backup": {
                    "newest_at": newest_backup,
                    "age_days": backup_age,
                    "copies": db.backups()?.len(),
                    "state": backup_state(backup_age),
                },
                "hubs": if hubs {
                    serde_json::to_value(
                        hub_drift(&db)?
                            .iter()
                            .map(|(project, file, why)| serde_json::json!({ "project": project, "file": file, "why": why }))
                            .collect::<Vec<_>>(),
                    )?
                } else {
                    serde_json::Value::Null
                },
                "closed_without_a_tag": mismatches
                    .iter()
                    .map(|(project, version)| serde_json::json!({ "project": project, "version": version }))
                    .collect::<Vec<_>>(),
                "never_synced": unsynced,
            }))?
        );
        return Ok(());
    }
    println!("database:  {}", path.display());
    println!("schema:    version {schema}");
    println!("projects:  {}", counts.projects);
    println!("versions:  {}", counts.versions);
    println!("tasks:     {}", counts.tasks);
    println!("sessions:  {}", counts.sessions);
    println!("events:    {}", counts.events);
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

    // A generated file edited by hand has stopped being a view of the
    // record, and the next export would overwrite the edit without saying
    // so. Off by default because it reads every hub from disk.
    if hubs {
        let drift = hub_drift(&db)?;
        println!();
        if drift.is_empty() {
            println!("hubs: every generated file matches the record");
        } else {
            println!("hubs the record cannot vouch for ({}):", drift.len());
            for (project, file, why) in &drift {
                println!("  {project:<12} {file:<14} {why}");
            }
            // The advice only fits an edit; a hub the record has never
            // seen needs the other sentence.
            if drift.iter().any(|(_, file, why)| file != "-" && why.starts_with("edited")) {
                println!("  edited: `rigger import` takes the edit into the record; `rigger export` discards it");
            }
        }
    }
    Ok(())
}
