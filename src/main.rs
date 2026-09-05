//! rigger - one seat for all your projects and tasks.
//!
//! The command surface grows one release at a time; this release brings the
//! database, projects and `doctor`.

mod calendar;
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
mod retro;
mod search;
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
            ProjectCommand::Service { name } => project_service(&name),
            ProjectCommand::List { json } => project_list(json),
            ProjectCommand::Show { name, json } => project_show(&name, json),
            ProjectCommand::Tier { name, tier, rhythm } => project_tier(&name, &tier, rhythm),
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
