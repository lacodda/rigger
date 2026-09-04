//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod context;
mod db;
mod hub;
mod import;
mod mcp;
mod open;
mod paths;
mod repo;

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
    /// Serve the record over MCP, on stdin and stdout
    Mcp,
    /// Record a wish: something to sort into the plan later
    Wish {
        /// Project name
        project: String,
        /// What you want
        text: String,
    },
    /// Copy the database aside, stamped with the moment and its schema
    Backup,
    /// Show the database path, schema version and record counts
    Doctor {
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
}

impl NoteKind {
    fn as_str(self) -> &'static str {
        match self {
            NoteKind::Decision => "decision",
            NoteKind::Finding => "finding",
            NoteKind::Pitfall => "pitfall",
            NoteKind::Change => "change",
            NoteKind::Next => "next",
        }
    }
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
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init => init(),
        Command::Project { command } => match command {
            ProjectCommand::Add { path, name } => project_add(path, name),
            ProjectCommand::List { json } => project_list(json),
            ProjectCommand::Show { name, json } => project_show(&name, json),
        },
        Command::Import { project, hub, json } => import_hub(&project, &hub, json),
        Command::Context {
            project,
            json,
            explain,
            budget,
        } => show_context(&project, json, explain, budget),
        Command::Open { project, print, budget } => open_session(&project, print, budget),
        Command::Note { project, text, kind } => note(&project, kind.as_str(), &text),
        Command::Mcp => mcp::serve(),
        Command::Wish { project, text } => note(&project, "wish", &text),
        Command::Backup => backup(),
        Command::Doctor { json } => doctor(json),
    }
}

fn import_hub(project: &str, hub_dir: &Path, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let Some(project) = db.project_by_name(project)? else {
        bail!("no project named '{project}'; see `rigger project list`");
    };
    let hub = hub::read(hub_dir)?;
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
    if report.decisions_added > 0 {
        println!("  {:<10} {} added", "decisions", report.decisions_added);
    }
    if report.questions_added > 0 {
        println!("  {:<10} {} added", "questions", report.questions_added);
    }
    Ok(())
}

fn open_project(db: &Db, name: &str) -> Result<db::Project> {
    match db.project_by_name(name)? {
        Some(project) => Ok(project),
        None => bail!("no project named '{name}'; see `rigger project list`"),
    }
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

fn note(project: &str, kind: &str, text: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
    db.record_event(project.id, kind, text, &db::now(), "assistant")?;
    println!("Recorded a {kind} for {}", project.name);
    Ok(())
}

fn backup() -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let target = db.backup()?;
    println!("Copied to {}", target.display());
    Ok(())
}

fn init() -> Result<()> {
    let path = paths::db_path()?;
    if path.exists() {
        Db::open(&path)?;
        println!("Already initialised: {}", path.display());
        return Ok(());
    }
    let db = Db::create(&path)?;
    println!("Created {} (schema version {})", db.path().display(), db.schema_version()?);
    println!("Next: rigger project add <path>");
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
    let project = db.add_project(&name, &root.to_string_lossy(), remote.as_deref())?;
    println!("Recorded '{}' at {}", project.name, project.path);
    match &project.remote {
        Some(url) => println!("  remote: {url}"),
        None => println!("  remote: none (no origin in .git/config)"),
    }
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
        println!("{:width$}  {}", p.name, p.path);
    }
    Ok(())
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
    println!("  path:    {}", project.path);
    println!("  remote:  {}", project.remote.as_deref().unwrap_or("none"));
    println!("  since:   {}", project.created_at);
    Ok(())
}

fn doctor(json: bool) -> Result<()> {
    let path = paths::db_path()?;
    if !path.exists() {
        if json {
            println!("{}", serde_json::json!({ "database": path, "initialised": false }));
        } else {
            println!("database:  {} (missing - run `rigger init`)", path.display());
        }
        return Ok(());
    }
    let db = Db::open(&path)?;
    let schema = db.schema_version()?;
    let counts = db.counts()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "database": path,
                "initialised": true,
                "schema_version": schema,
                "counts": counts,
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
    Ok(())
}
