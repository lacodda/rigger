//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod db;
mod hub;
mod import;
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
    /// Copy the database aside, stamped with the moment and its schema
    Backup,
    /// Show the database path, schema version and record counts
    Doctor {
        /// Print as JSON
        #[arg(long)]
        json: bool,
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
