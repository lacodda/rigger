//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod commit;
mod context;
mod db;
mod hub;
mod import;
mod mcp;
mod open;
mod owner;
mod paths;
mod repo;
mod search;
mod sync;

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
        Command::Mcp => mcp::serve(),
        Command::Resolve { project, id, answer } => resolve(&project, id, answer.as_deref()),
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

/// Reads git for one project, or for every recorded project.
fn sync_projects(project: Option<&str>, json: bool) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let projects = match project {
        Some(name) => vec![open_project(&db, name)?],
        None => db.projects()?,
    };
    let mut reports = Vec::new();
    for project in &projects {
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

fn note(project: &str, kind: &str, text: &str) -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let project = open_project(&db, project)?;
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

    if json {
        let payload: Vec<_> = reports
            .iter()
            .map(|(name, facts, next, lines)| serde_json::json!({ "project": name, "facts": facts, "next": next, "lines": lines }))
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

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
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

    // Where the plan and git disagree. Reported, never corrected: the record
    // cannot prove a tag's absence - it may simply not have been fetched -
    // and a silent correction would erase what the owner wrote (ADR 0005).
    let mut mismatches = Vec::new();
    let mut unsynced = Vec::new();
    for project in db.projects()? {
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
                "database": path,
                "initialised": true,
                "schema_version": schema,
                "counts": counts,
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
    Ok(())
}
