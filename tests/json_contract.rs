//! The JSON every command prints is a contract, and this is its gate.
//!
//! `--json` is one flag on the root of the command line, so every command
//! accepts it by construction. Accepting is not keeping: a command that took
//! the flag and printed its sentences anyway would break the program reading
//! it, and a field renamed for taste would break it more quietly still.
//!
//! So every leaf command is run here with `--json` against one synthetic
//! record, rich enough to fill every field: its output has to parse as one
//! JSON document, and every field in it - each path through objects and
//! arrays, with its type - has to be written down in the "JSON" table of
//! the command's reference page, and every row of that table has to be
//! seen. A field renamed, removed, retyped or added without its row is a
//! red build rather than a surprise for whoever reads the output.
//!
//! The record is synthetic throughout: made-up projects, made-up tickets.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use serde_json::Value;

/// Commands with no `--json` document of their own, and why.
const EXEMPT: &[(&str, &str)] = &[("mcp", "it speaks JSON-RPC on stdin and stdout; there is no one document to print")];

/// Fewer leaves than this means the help went unread, not that the CLI shrank.
const AT_LEAST: usize = 70;

fn rigger_bin() -> TestCommand {
    TestCommand::cargo_bin("rigger").unwrap()
}

fn help_of(args: &[&str]) -> String {
    let out = rigger_bin().args(args).arg("--help").output().unwrap();
    assert!(out.status.success(), "rigger {args:?} --help failed");
    String::from_utf8(out.stdout).unwrap()
}

/// The names under `Commands:` in a help text.
fn commands_in(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in help.lines() {
        if line.trim_end() == "Commands:" {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().is_empty() {
                break;
            }
            if let Some(name) = line.strip_prefix("  ").and_then(|l| l.split_whitespace().next())
                && name != "help"
            {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Every leaf command, `task new` style, read from the help the binary
/// prints - so a command added to the CLI is a command this gate asks
/// about, without anybody remembering to list it.
fn leaves() -> Vec<String> {
    let mut out = Vec::new();
    for top in commands_in(&help_of(&[])) {
        let subs = commands_in(&help_of(&[&top]));
        if subs.is_empty() {
            out.push(top);
        } else {
            out.extend(subs.into_iter().map(|s| format!("{top} {s}")));
        }
    }
    out
}

/// A stand-in program: on Windows a batch file, elsewhere a shell script.
fn program(dir: &Path, name: &str, windows: &str, unix: &str) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join(format!("{name}.cmd"));
        std::fs::write(&path, format!("@echo off\r\n{}\r\n", windows.replace('\n', "\r\n"))).unwrap();
        path
    } else {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{unix}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit(dir: &Path, file: &str, message: &str) {
    std::fs::write(dir.join(file), message).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

/// What one run of the gate saw: for every leaf, every path with the types
/// it came in.
type Seen = BTreeMap<String, BTreeMap<String, BTreeSet<String>>>;

struct Record {
    data: PathBuf,
    env: Vec<(String, OsString)>,
    seen: Seen,
    /// Runs that failed or printed something other than one JSON document.
    /// Kept rather than panicked on, so one run of the gate names them all.
    broken: Vec<String>,
}

impl Record {
    /// Runs `rigger <args> --json`, insists it succeeds with one JSON
    /// document, and files every path of it under `leaf`.
    fn json(&mut self, leaf: &str, args: &[&str]) -> Value {
        let mut cmd = rigger_bin();
        cmd.env("RIGGER_DATA_DIR", &self.data);
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        let out = cmd.args(args).arg("--json").output().unwrap();
        if !out.status.success() {
            self.broken.push(format!(
                "{leaf}: rigger {args:?} --json failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
            return Value::Null;
        }
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                let head: String = text.lines().take(3).collect::<Vec<_>>().join(" / ");
                self.broken
                    .push(format!("{leaf}: rigger {args:?} --json printed what is not one JSON document ({e}): {head}"));
                return Value::Null;
            }
        };
        let paths = self.seen.entry(leaf.to_string()).or_default();
        walk(&value, "", paths);
        value
    }

    fn with(&mut self, key: &str, value: impl Into<OsString>) {
        self.env.retain(|(k, _)| k != key);
        self.env.push((key.to_string(), value.into()));
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Files every path under `at`: `key`, `key.inner`, `list[]`, `list[].key`.
/// The root itself is not a field and is not filed.
fn walk(value: &Value, at: &str, out: &mut BTreeMap<String, BTreeSet<String>>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let path = if at.is_empty() { key.clone() } else { format!("{at}.{key}") };
                out.entry(path.clone()).or_default().insert(kind_of(inner).to_string());
                walk(inner, &path, out);
            }
        }
        Value::Array(items) => {
            let path = format!("{at}[]");
            for inner in items {
                out.entry(path.clone()).or_default().insert(kind_of(inner).to_string());
                walk(inner, &path, out);
            }
        }
        _ => {}
    }
}

/// One row of a page's JSON table: a path, which may hold `*` for a key
/// that is data rather than a name, and the types it may come in.
struct Row {
    path: String,
    types: BTreeSet<String>,
}

/// A JSON table, and the leaves whose output it documents. Commands that
/// print one shape - every write that answers with the card it changed -
/// share one table: `<!-- json: task new, task open -->`. A command that
/// prints two shapes - `project tier` sets a tier or suggests them - may
/// sit in two tables; its output is then held to their rows together.
struct Table {
    page: String,
    leaves: Vec<String>,
    rows: Vec<Row>,
}

/// Leaves documented on another command's page.
const PAGE_OF: &[(&str, &str)] = &[("wish", "note")];

/// The reference page a leaf is documented on.
fn page_of(leaf: &str) -> String {
    let top = leaf.split(' ').next().unwrap_or_default();
    PAGE_OF
        .iter()
        .find(|(l, _)| *l == top)
        .map(|(_, page)| page.to_string())
        .unwrap_or_else(|| top.to_string())
}

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/content/docs/reference")
}

const MARKER: &str = "<!-- json: ";

/// Every JSON table of every reference page: the table that follows each
/// marker `<!-- json: <leaf>[, <leaf>...] -->`.
fn tables() -> Result<Vec<Table>, String> {
    let mut out = Vec::new();
    let mut pages: Vec<PathBuf> = std::fs::read_dir(docs_dir())
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    pages.sort();
    for page in pages {
        let name = page.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&page).map_err(|e| format!("{}: {e}", page.display()))?;
        for (at, _) in text.match_indices(MARKER) {
            let rest = &text[at + MARKER.len()..];
            let Some(close) = rest.find(" -->") else {
                return Err(format!("{name}.md: a JSON marker is never closed"));
            };
            let leaves: Vec<String> = rest[..close].split(',').map(|l| l.trim().to_string()).collect();
            let mut rows = Vec::new();
            let mut in_table = false;
            for line in rest[close..].lines().skip(1) {
                let line = line.trim();
                if !line.starts_with('|') {
                    if in_table {
                        break;
                    }
                    continue;
                }
                in_table = true;
                let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
                let Some(path) = cells.first().and_then(|c| c.strip_prefix('`')).and_then(|c| c.strip_suffix('`')) else {
                    continue; // the header and the separator
                };
                let types = cells.get(1).copied().unwrap_or_default().split(" or ").map(|t| t.trim().to_string()).collect();
                rows.push(Row { path: path.to_string(), types });
            }
            if rows.is_empty() {
                return Err(format!("{name}.md: the JSON table of {leaves:?} has no rows"));
            }
            out.push(Table {
                page: name.clone(),
                leaves,
                rows,
            });
        }
    }
    Ok(out)
}

/// Whether a documented path names an emitted one: equal, except that a
/// `*` segment stands for any key.
fn names(documented: &str, emitted: &str) -> bool {
    let d: Vec<&str> = documented.split('.').collect();
    let e: Vec<&str> = emitted.split('.').collect();
    d.len() == e.len()
        && d.iter().zip(&e).all(|(d, e)| {
            if d == e {
                return true;
            }
            // `*` is one key and nothing more: `*[]` is the elements under
            // it, and `*` alone does not reach into them.
            match d.strip_prefix('*').map(|rest| e.strip_suffix(rest)) {
                Some(Some(key)) => !key.is_empty() && !key.contains("[]"),
                _ => false,
            }
        })
}

/// Every way the documented and the printed disagree.
///
/// Forwards, per leaf: each path a command printed has a row, and came in
/// a type the row allows. Backwards, per table: each row was printed by
/// at least one of the commands that share it.
fn disagreements(leaves: &[String], seen: &Seen) -> Vec<String> {
    let tables = match tables() {
        Ok(tables) => tables,
        Err(why) => return vec![why],
    };
    let mut out = Vec::new();
    for table in &tables {
        for leaf in &table.leaves {
            if !leaves.contains(leaf) {
                out.push(format!("{}.md documents `{leaf}`, which the CLI does not have", table.page));
            } else if page_of(leaf) != table.page {
                out.push(format!("`{leaf}` is documented on {}.md; it belongs on {}.md", table.page, page_of(leaf)));
            }
        }
    }
    for leaf in leaves {
        if EXEMPT.iter().any(|(name, _)| name == leaf) {
            continue;
        }
        let Some(emitted) = seen.get(leaf) else {
            out.push(format!("{leaf}: never run here; add it to `fill`"));
            continue;
        };
        let rows: Vec<&Row> = tables.iter().filter(|t| t.leaves.contains(leaf)).flat_map(|t| &t.rows).collect();
        if rows.is_empty() {
            out.push(format!("{leaf}: {}.md has no `{MARKER}{leaf} -->` before a JSON table", page_of(leaf)));
            continue;
        }
        for (path, types) in emitted {
            match rows.iter().find(|r| names(&r.path, path)) {
                None => out.push(format!("{leaf}: `{path}` ({}) is printed but not documented", join(types))),
                Some(row) => {
                    let stray: Vec<&String> = types.iter().filter(|t| !row.types.contains(*t)).collect();
                    if !stray.is_empty() {
                        out.push(format!("{leaf}: `{path}` came as {stray:?}, documented as {}", join(&row.types)));
                    }
                }
            }
        }
    }
    for table in &tables {
        for row in &table.rows {
            let printed = table
                .leaves
                .iter()
                .filter_map(|leaf| seen.get(leaf))
                .any(|emitted| emitted.keys().any(|p| names(&row.path, p)));
            if !printed {
                out.push(format!("{}: `{}` is documented but never printed", table.leaves.join(", "), row.path));
            }
        }
    }
    out
}

fn join(types: &BTreeSet<String>) -> String {
    types.iter().cloned().collect::<Vec<_>>().join(" or ")
}

#[test]
fn the_help_is_read_whole() {
    // The sentry of the scanner below: a help layout it no longer reads
    // would find no commands, and a gate over no commands is green.
    let found = leaves();
    assert!(found.len() >= AT_LEAST, "only {} leaf commands read from --help: {found:?}", found.len());
    for known in ["init", "task new", "doc template", "link drift", "doctor", "mcp"] {
        assert!(found.iter().any(|l| l == known), "`{known}` was not read from --help: {found:?}");
    }
}

#[test]
fn every_command_prints_the_json_its_page_documents() {
    let data = tempfile::tempdir().unwrap();
    let mut r = Record {
        data: data.path().to_path_buf(),
        env: Vec::new(),
        seen: Seen::new(),
        broken: Vec::new(),
    };
    fill(&mut r, data.path());

    if let Some(dump) = std::env::var_os("RIGGER_JSON_CONTRACT_DUMP") {
        std::fs::write(dump, serde_json::to_string_pretty(&r.seen).unwrap()).unwrap();
    }

    let mut problems = r.broken.clone();
    problems.extend(disagreements(&leaves(), &r.seen));
    assert!(problems.is_empty(), "{} disagreements:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn a_path_with_a_star_stands_for_any_key() {
    assert!(names("card.key", "card.key"));
    assert!(names("weeks.*.focus", "weeks.2026-W41.focus"));
    assert!(names("links[].project", "links[].project"));
    assert!(names("*[].id", "alpha[].id"));
    assert!(!names("card.key", "card.title"));
    assert!(!names("card", "card.key"));
    assert!(!names("*[].id", "[].id"));
}

/// The ISO week of today, as `2026-W41`: the window `release-day` and
/// `digest` read is the current one.
fn this_week() -> String {
    let week = jiff::Zoned::now().date().iso_week_date();
    format!("{}-W{:02}", week.year(), week.week())
}

/// Builds the record and runs every command against it, in an order that
/// gives each one something to say: every list a command prints is meant
/// to hold at least one item here, so that the fields of its items are
/// seen and held to their rows.
fn fill(r: &mut Record, data: &Path) {
    let bin = data.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let now = this_week();

    // Stand-ins for what the record reaches outside itself.
    let inbox = bin.join("inbox.json");
    std::fs::write(
        &inbox,
        r#"{"issues":[
  {"key":"ACME-1","summary":"Export drops the footer","status":"Open","priority":"Medium","first_seen":"2026-09-20T09:00:00","pinned":false},
  {"key":"ACME-2","summary":"Import reads the wrong sheet","status":"In Progress","priority":"High","score":8.0,"url":"https://tracker.example.com/browse/ACME-2","first_seen":"2026-09-19T09:00:00","taken_at":"2026-09-21T10:00:00","pinned":true},
  {"key":"ACME-3","summary":"Old import bug","status":"Done","gone_at":"2026-09-24T10:00:00"},
  {"key":"ACME-4","summary":"Later, perhaps","status":"Open","snoozed_until":"2999-01-01T09:00:00"}
]}"#,
    )
    .unwrap();
    let kasl = program(&bin, "kasl", &format!("type \"{}\"", inbox.display()), &format!("cat '{}'", inbox.display()));
    let gh = program(
        &bin,
        "gh",
        "if \"%1 %2\"==\"release view\" goto view\nif \"%1 %2\"==\"release list\" goto list\nif \"%1 %2\"==\"run list\" goto runs\nexit /b 1\n:view\necho release not found 1>&2\nexit /b 1\n:list\necho [{\"tagName\":\"v0.0.9\"}]\nexit /b 0\n:runs\necho []\nexit /b 0",
        "case \"$1 $2\" in\n\"release view\") echo 'release not found' >&2; exit 1 ;;\n\"release list\") echo '[{\"tagName\":\"v0.0.9\"}]'; exit 0 ;;\n\"run list\") echo '[]'; exit 0 ;;\nesac\nexit 1",
    );
    let editor = program(&bin, "editor", "exit /b 0", "exit 0");
    let notify = program(&bin, "notify", "exit /b 0", "exit 0");
    let assistant = program(&bin, "assistant", "exit /b 0", "exit 0");
    r.with("RIGGER_KASL", &kasl);
    r.with("RIGGER_GH", &gh);
    r.with("RIGGER_EDITOR", editor.as_os_str());
    r.with("RIGGER_NOTIFY", &notify);
    r.with("RIGGER_ASSISTANT", &assistant);

    // ---- the record and its projects ------------------------------------
    r.json("init", &["init"]);
    r.json("init", &["init"]);

    let repos = data.join("repos");
    let alpha = repos.join("alpha");
    let beta = repos.join("beta");
    for root in [&alpha, &beta] {
        std::fs::create_dir_all(root).unwrap();
        git(root, &["init", "--quiet", "--initial-branch", "main"]);
    }
    git(&alpha, &["remote", "add", "origin", "https://github.com/acme/alpha.git"]);
    std::fs::write(
        alpha.join("Cargo.toml"),
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\ndescription = \"Reads every spreadsheet you have\"\n",
    )
    .unwrap();
    commit(&alpha, "a.txt", "feat: read the first file");
    // A tag no stage of the plan names, right beside a version the plan
    // closed without one: the plan and the tags one step apart.
    git(&alpha, &["tag", "v0.0.4"]);
    git(&alpha, &["tag", "v0.1.0"]);
    commit(&alpha, "b.txt", "fix: keep the footer");
    git(&alpha, &["tag", "v0.2.0"]);
    commit(&alpha, "c.txt", "feat(ACME-7310): export the footer");
    git(&alpha, &["branch", "fix/ACME-7310-footer"]);
    git(&alpha, &["update-ref", "refs/remotes/origin/feat/ACME-7311-sheet", "HEAD"]);
    commit(&beta, "a.txt", "feat: start");
    git(&beta, &["tag", "v0.1.0"]);
    // A directory recorded before it became a repository.
    let notes = repos.join("notes");
    std::fs::create_dir_all(&notes).unwrap();

    r.json("project add", &["project", "add", alpha.to_str().unwrap()]);
    r.json("project add", &["project", "add", beta.to_str().unwrap()]);
    r.json("project add", &["project", "add", notes.to_str().unwrap()]);
    r.json("project service", &["project", "service", "line"]);

    let hub = data.join("hubs").join("alpha");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План\n\n## v0.3.0 · Third stage\n\n- [ ] export the footer\n- [ ] read the sheet\n- [ ] draw the header\n- [ ] keep the margins\n\n## v0.3.1 · A patch\n\n- [ ] fix the margins\n\n## v0.4.0 · Fourth stage\n\n- [ ] later work\n- [ ] sleep on the colours\n- [ ] set the fonts aside\n",
    )
    .unwrap();
    std::fs::write(
        hub.join("Изменения.md"),
        "# Изменения\n\n## v0.2.0 · Second stage — выпущен 2026-09-25\n\n## v0.1.0 · First stage — выпущен 2026-09-11\n\n## v0.0.5 · Before the tags — выпущен 2026-09-01\n",
    )
    .unwrap();
    std::fs::write(
        hub.join("Хотелки.md"),
        "# Хотелки\n\nWhat to sort later.\n\n---\n\n- A reader for sheets by name\n",
    )
    .unwrap();
    r.json("import", &["import", "alpha", "--hub", hub.to_str().unwrap()]);
    let answers = data.join("answers.json");
    std::fs::write(
        &answers,
        r#"{"questionnaire":"plan review","answered":"2026-09-11",
  "forks":[{"question":"Where the code knowledge comes from","chosen":"a library","against":["an index of its own"],"why":"One index for the line."}],
  "ideas":[{"idea":"A registry as public JSON","taken":true},{"idea":"A second binary","taken":false,"why":"An alias is a link."}]}"#,
    )
    .unwrap();
    r.json("import", &["import", "alpha", "--answers", answers.to_str().unwrap(), "--check"]);
    r.json("import", &["import", "alpha", "--answers", answers.to_str().unwrap()]);

    r.json("sync", &["sync"]);
    r.json("sync", &["sync", "alpha"]);

    r.json(
        "project set",
        &["project", "set", "alpha", "--gate", "git --version", "--on-session-end", "git --version"],
    );
    r.json("project set", &["project", "set", "alpha"]);
    r.json(
        "project mark",
        &[
            "project",
            "mark",
            "alpha",
            "--code",
            "al",
            "--accent",
            "#3366ff",
            "--accent2",
            "#ff6633",
            "--form",
            "cli",
            "--docs",
            "https://example.com/alpha/",
        ],
    );
    r.json("project mark", &["project", "mark", "alpha"]);
    r.json("project tier", &["project", "tier", "alpha", "A"]);
    r.json("project tier", &["project", "tier", "beta", "B", "--rhythm", "3"]);
    r.json("project tier", &["project", "tier", "notes", "C"]);
    r.json("project tier", &["project", "tier", "--suggest"]);
    r.json("project list", &["project", "list"]);
    r.json("project show", &["project", "show", "alpha"]);
    r.json("project show", &["project", "show", "line"]);

    // ---- what is said about them -----------------------------------------
    r.json(
        "note",
        &[
            "note",
            "alpha",
            "Read git through one library",
            "--kind",
            "decision",
            "--principle",
            "one truth per thing",
        ],
    );
    r.json("note", &["note", "alpha", "The footer is drawn twice", "--kind", "finding"]);
    r.json("note", &["note", "alpha", "Which registry first?", "--kind", "question", "--due", "+3d"]);
    r.json("note", &["note", "beta", "Which registry first?", "--kind", "question"]);
    r.json("note", &["note", "alpha", "Export works, import next", "--kind", "state"]);
    r.json(
        "note",
        &["note", "alpha", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "crates.io"],
    );
    r.json("note", &["note", "alpha", "Start from the sheet reader", "--kind", "next"]);
    r.json("note", &["note", "notes", "The first page of notes", "--kind", "change"]);
    let wish = r.json("wish", &["wish", "alpha", "Read the sheet by name", "--from", "beta"]);
    r.json("wish", &["wish", "alpha", "Keep the margins wide", "--from", "beta"]);
    r.json("wish", &["wish", "alpha", "A second wish to sort"]);
    let wish_id = wish["event"]["id"].as_i64().unwrap_or_default().to_string();
    r.json("resolve", &["resolve", "alpha", &wish_id]);
    let question = r.json("note", &["note", "beta", "Is the reader public?", "--kind", "question"]);
    let question_id = question["event"]["id"].as_i64().unwrap_or_default().to_string();
    r.json("resolve", &["resolve", "beta", &question_id, "Yes, from the start"]);
    r.json("note", &["note", "beta", "Which colour?", "--kind", "question", "--due", "2026-01-01"]);

    r.json("link add", &["link", "add", "alpha@v0.3.0", "beta@v0.1.0", "--note", "the shared reader"]);
    r.json("link add", &["link", "add", "beta", "alpha", "--kind", "consumer"]);
    r.json("link add", &["link", "add", "alpha", "notes", "--note", "the notes of a release"]);
    r.json(
        "link add",
        &["link", "add", "beta@v0.1.0", "alpha@v0.4.0", "--note", "the reader and its next writer"],
    );
    let spare = r.json("link add", &["link", "add", "alpha", "line", "--kind", "donor"]);
    r.json("link list", &["link", "list"]);
    r.json("link list", &["link", "list", "alpha"]);
    r.json("link drift", &["link", "drift"]);
    let spare_id = spare["link"]["id"].as_i64().unwrap_or_default().to_string();
    r.json("link remove", &["link", "remove", &spare_id]);

    // ---- documents ----------------------------------------------------------
    r.json(
        "doc add",
        &[
            "doc",
            "add",
            "alpha",
            "Vision",
            "--kind",
            "vision",
            "--body",
            "# Vision\n\nRead every spreadsheet.",
        ],
    );
    r.json(
        "doc add",
        &["doc", "add", "alpha", "Scratch", "--kind", "research", "--body", "# Scratch\n\nNotes."],
    );
    r.json(
        "doc add",
        &[
            "doc",
            "add",
            "line",
            "Rituals",
            "--kind",
            "rituals",
            "--body",
            "# Rituals\n\nA stage ends in a tag.",
        ],
    );
    r.json(
        "doc add",
        &[
            "doc",
            "add",
            "alpha",
            "Rituals",
            "--kind",
            "rituals",
            "--body",
            "# Rituals of alpha\n\nThe reader first.",
        ],
    );
    r.json("doc list", &["doc", "list", "alpha"]);
    r.json("doc show", &["doc", "show", "alpha", "vision"]);
    r.json("doc edit", &["doc", "edit", "alpha", "vision", "--title", "Vision of alpha"]);
    r.json("doc edit", &["doc", "edit", "alpha", "vision", "--title", "Vision of alpha"]);
    r.json("doc remove", &["doc", "remove", "alpha", "scratch"]);
    r.json("doc template", &["doc", "template", "vision"]);
    r.json("doc template", &["doc", "template", "research", "--write"]);
    r.json("doc template", &["doc", "template", "research"]);
    r.json("rules", &["rules"]);
    r.json("rules", &["rules", "alpha"]);

    // ---- tasks of a plan, asleep and set aside --------------------------------
    let stage = r.json("version show", &["version", "show", "alpha"]);
    let task_id = |n: usize| stage["tasks"][n]["id"].as_i64().unwrap_or_default().to_string();
    r.json("task status", &["task", "status", &task_id(1), "active"]);
    let later = r.json("version show", &["version", "show", "alpha", "v0.4.0"]);
    let later_id = |n: usize| later["tasks"][n]["id"].as_i64().unwrap_or_default().to_string();
    r.json("task snooze", &["task", "snooze", &later_id(1), "--until", "+7d"]);
    r.json("task freeze", &["task", "freeze", &later_id(2), "--why", "waits for the margins"]);
    let patch = r.json("version show", &["version", "show", "alpha", "v0.3.1"]);
    let patch_id = patch["tasks"][0]["id"].as_i64().unwrap_or_default().to_string();
    r.json("task freeze", &["task", "freeze", &patch_id]);

    // ---- the calendar ---------------------------------------------------------
    r.json("version plan", &["version", "plan", "alpha", "v0.3.0", "--week", "2026-W45"]);
    r.json("version plan", &["version", "plan", "alpha", "v0.4.0", "--week", &now]);
    r.json("version show", &["version", "show", "alpha", "v0.3.0"]);
    r.json("version show", &["version", "show", "alpha", "v0.2.0"]);
    r.json("version plan", &["version", "plan", "alpha", "v0.2.0", "--week", "2026-W39"]);
    r.json("calendar", &["calendar", "--from", "2026-W38", "--weeks", "10"]);
    r.json(
        "calendar",
        &[
            "calendar",
            "--from",
            "2026-W38",
            "--weeks",
            "10",
            "--ics",
            data.join("line.ics").to_str().unwrap(),
        ],
    );
    r.json("calendar", &["calendar", "--from", "2026-W38", "--weeks", "10", "--ics", "-"]);
    r.json("next", &["next", "--week", "2026-W45"]);
    r.json("next", &["next", "--week", "2026-W52"]);
    r.json("week", &["week", "--week", "2026-W45"]);
    r.json("week", &["week"]);
    r.json("week", &["week", "--week", "2026-W52"]);

    // ---- a sitting -------------------------------------------------------------
    r.json("session start", &["session", "start", "alpha"]);
    // The record keeps moments to the second and a sitting holds what came
    // strictly after it began; a fixture this fast would otherwise put its
    // work in the second the sitting opened.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    r.json("note", &["note", "alpha", "Split the reader from the writer", "--kind", "change"]);
    r.json("note", &["note", "alpha", "Draw the footer once", "--kind", "decision"]);
    r.json("note", &["note", "alpha", "The template owns the footer", "--kind", "finding"]);
    r.json("note", &["note", "alpha", "A footer drawn twice looks right in print", "--kind", "pitfall"]);
    r.json("note", &["note", "alpha", "Should the header move too?", "--kind", "question"]);
    r.json("task status", &["task", "status", &task_id(0), "done"]);
    commit(&alpha, "d.txt", "fix: draw the footer once");
    git(&alpha, &["tag", "v0.3.0"]);
    r.json("sync", &["sync", "alpha"]);
    r.json("gate", &["gate", "alpha", "--check"]);
    r.json("gate", &["gate", "alpha"]);
    r.json("session draft", &["session", "draft", "alpha", "--heading", "The reader"]);
    let entry = data.join("entry.md");
    std::fs::write(&entry, "### The reader\n\nSplit the reader from the writer.\n").unwrap();
    let diary = data.join("diary.md");
    r.json(
        "session end",
        &[
            "session",
            "end",
            "alpha",
            "--entry",
            entry.to_str().unwrap(),
            "--diary",
            diary.to_str().unwrap(),
        ],
    );
    // A kasl that refuses: the sitting opens all the same, and says why.
    let refusing = program(
        &bin,
        "kasl-refusing",
        "echo focus is not enabled 1>&2\nexit /b 2",
        "echo 'focus is not enabled' >&2; exit 2",
    );
    r.with("RIGGER_KASL", &refusing);
    r.json("session start", &["session", "start", "beta"]);
    r.json("session end", &["session", "end", "beta"]);
    r.with("RIGGER_KASL", &kasl);

    r.json("release-day", &["release-day"]);
    r.json("retro", &["retro", "--to", "2026-W45", "--weeks", "8"]);
    r.json("retro", &["retro", "--to", "2026-W45", "--cycle", "--record"]);

    // ---- reading the record ----------------------------------------------------
    r.json("context", &["context", "alpha"]);
    r.json("context", &["context", "beta", "--budget", "400"]);
    r.json("context", &["context", "alpha", "--explain", "--budget", "300"]);
    r.json("show", &["show", "alpha"]);
    r.json("show", &["show", "notes"]);
    r.json("inbox", &["inbox"]);
    r.json("inbox", &["inbox", "--project", "alpha"]);
    r.json("digest", &["digest", "--since", "3650d"]);
    r.json(
        "digest",
        &["digest", "alpha", "--since", "3650d", "--md", data.join("daily.md").to_str().unwrap()],
    );
    r.json("find", &["find", "footer"]);
    r.json("find", &["find", "spreadsheet"]);
    r.json("find", &["find", "footer AND"]);
    r.json("find", &["find", "reader", "--project", "alpha", "--kind", "change"]);
    r.json("why", &["why", "alpha", "v0.3.0"]);
    r.json("why", &["why", "--principles"]);
    r.json("why", &["why", "--principle", "one truth per thing"]);

    // ---- cards -------------------------------------------------------------------
    r.json(
        "task new",
        &[
            "task",
            "new",
            "Export drops the footer",
            "--id",
            "ACME-7310",
            "--alias",
            "OPS-512",
            "--project",
            "alpha",
            "--branch",
            "fix/ACME-7310-footer",
            "--summary",
            "The footer is lost on export.",
        ],
    );
    let local = r.json("task new", &["task", "new", "Read the sheet by name", "--alias", "sheet-reader"]);
    let local_key = local["card"]["key"].as_str().unwrap_or_default().to_string();
    r.json(
        "task link",
        &["task", "link", &local_key, "beta", "--branch", "feat/ACME-7311", "--role", "only read"],
    );
    r.json("task link", &["task", "link", &local_key, "alpha"]);
    r.json("task rename", &["task", "rename", &local_key, "ACME-7311"]);
    r.json("task alias", &["task", "alias", "ACME-7311", "sheet-by-name"]);
    r.json(
        "task summary",
        &["task", "summary", "ACME-7311", "Follows ACME-7310: sheets are read by position, case 561207."],
    );
    r.json(
        "task note",
        &["task", "note", "ACME-7310", "The footer is drawn by the template", "--kind", "finding"],
    );
    r.json("task find", &["task", "find", "ACME-7310"]);
    r.json("task find", &["task", "find", "the footer on export"]);
    r.json(
        "task note",
        &["task", "note", "ACME-7311", "The same report as case 561207", "--kind", "finding"],
    );
    r.json("task find", &["task", "find", "case 561207"]);
    r.json("task open", &["task", "open", "ACME-7310"]);
    r.json("task active", &["task", "active"]);
    r.json("sync", &["sync", "alpha"]);
    r.json("task show", &["task", "show", "ACME-7310"]);
    r.json("task context", &["task", "context", "ACME-7310"]);
    r.json("task list", &["task", "list"]);
    r.json("task status", &["task", "status", "ACME-7311", "active"]);
    r.json("task snooze", &["task", "snooze", "ACME-7311", "--until", "+7d"]);
    r.json("task list", &["task", "list", "--status", "all", "--snoozed"]);
    r.json("task unsnooze", &["task", "unsnooze", "ACME-7311"]);
    r.json("task freeze", &["task", "freeze", "ACME-7311", "--why", "waits for the sheet format"]);
    r.json("task incoming", &["task", "incoming"]);
    r.json("task incoming", &["task", "incoming", "--all"]);
    r.json("task take", &["task", "take", "ACME-2", "--project", "alpha", "--branch", "fix/ACME-2"]);
    r.json("task alias", &["task", "alias", "ACME-2", "wrong-sheet"]);
    r.json("task take", &["task", "take", "ACME-2"]);
    r.json("task new", &["task", "new", "Old import bug", "--id", "ACME-3"]);
    r.json("task incoming", &["task", "incoming", "--all"]);
    r.json("task show", &["task", "show", "ACME-2"]);

    // ---- the tray of a card ----------------------------------------------------
    let drop = data.join("drop");
    std::fs::create_dir_all(&drop).unwrap();
    std::fs::write(drop.join("screen.png"), [0u8; 64]).unwrap();
    std::fs::write(drop.join("notes.txt"), "what the tester saw").unwrap();
    std::fs::write(drop.join("recording.mp4"), [0u8; 64]).unwrap();
    r.json("tray show", &["tray", "show", "ACME-7310"]);
    r.json("tray fetch", &["tray", "fetch", "ACME-7310", "--from", drop.to_str().unwrap()]);
    r.json("tray show", &["tray", "show", "ACME-7310"]);
    r.json("task show", &["task", "show", "ACME-7310"]);
    r.json("tray edit", &["tray", "edit", "ACME-7310"]);
    let intake = data.join("intake");
    std::fs::create_dir_all(&intake).unwrap();
    std::fs::write(intake.join("shot.png"), [0u8; 64]).unwrap();
    std::fs::write(intake.join("capture.mp4"), [0u8; 64]).unwrap();
    r.with("RIGGER_INTAKE_DIRS", &intake);
    r.json("tray intake", &["tray", "intake", "ACME-7310", "--since", "1d"]);
    r.json("tray list", &["tray", "list"]);
    let form = r.json("task handoff", &["task", "handoff", "ACME-7310", "--to", "alerts-api"]);
    if let Some(form) = form["files"]["form"].as_str() {
        let text = std::fs::read_to_string(form).unwrap();
        let filled = text
            .replace("## Ask (required)\n", "## Ask (required)\n\nPlease keep the footer on export.\n")
            .replace("## Now (required)\n", "## Now (required)\n\nThe exported file has no footer.\n")
            .replace("## Needed (required)\n", "## Needed (required)\n\nThe footer, as the template draws it.\n")
            .replace(
                "## Why (required)\n",
                "## Why (required)\n\nWhy: a document without its footer cannot be filed.\n",
            );
        std::fs::write(form, filled).unwrap();
    }
    r.json("task handoff", &["task", "handoff", "ACME-7310", "--to", "alerts-api"]);
    r.json("tray done", &["tray", "done", "ACME-7310"]);
    r.json("tray show", &["tray", "show", "ACME-7310"]);
    r.json("tray edit", &["tray", "edit", "ACME-7310"]);
    r.json("task show", &["task", "show", "ACME-7310"]);
    r.json("task close", &["task", "close", "ACME-7310"]);
    r.json("task active", &["task", "active"]);

    // ---- writing it back out ---------------------------------------------------
    let skills = data.join("skills");
    r.json("skill", &["skill", "alpha"]);
    r.json("skill", &["skill", "alpha", "--dir", skills.to_str().unwrap()]);
    r.json("skill", &["skill", "alpha", "--dir", skills.to_str().unwrap()]);
    r.json("skill", &["skill", "--line", "--dir", skills.to_str().unwrap()]);
    r.json("skill", &["skill", "--print-template"]);
    let out_hub = data.join("out").join("alpha");
    std::fs::create_dir_all(&out_hub).unwrap();
    r.json("export", &["export", "alpha", "--hub", out_hub.to_str().unwrap(), "--docs"]);
    r.json("export", &["export", "alpha", "--hub", out_hub.to_str().unwrap(), "--check"]);
    r.json("export", &["export", "--line"]);
    r.json("export", &["export", "--line", "--to", data.join("line.json").to_str().unwrap()]);
    r.json("open", &["open", "alpha"]);

    let others = data.join("others");
    let others_hubs = data.join("others-hubs");
    std::fs::create_dir_all(others.join("gamma")).unwrap();
    std::fs::create_dir_all(others.join("delta")).unwrap();
    std::fs::create_dir_all(others_hubs.join("gamma")).unwrap();
    std::fs::write(others_hubs.join("gamma").join("План.md"), "# План\n\n## v0.1.0 · First\n\n- [ ] begin\n").unwrap();
    git(&others.join("gamma"), &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::create_dir_all(others.join("alpha")).unwrap();
    std::fs::create_dir_all(others_hubs.join("alpha")).unwrap();
    git(&others.join("alpha"), &["init", "--quiet", "--initial-branch", "main"]);
    commit(&others.join("gamma"), "a.txt", "feat: begin");
    r.json(
        "adopt",
        &["adopt", others.to_str().unwrap(), "--hubs", others_hubs.to_str().unwrap(), "--check"],
    );
    r.json("adopt", &["adopt", others.to_str().unwrap(), "--hubs", others_hubs.to_str().unwrap()]);
    let late = repos.join("late");
    std::fs::create_dir_all(&late).unwrap();
    git(&late, &["init", "--quiet", "--initial-branch", "main"]);
    r.json("project add", &["project", "add", late.to_str().unwrap()]);

    for dir in [&hub, &out_hub] {
        std::fs::write(dir.join("Хотелки.md"), "# Хотелки\n\nWhat to sort later.\n\n---\n\n- Still kept here by hand\n").unwrap();
    }
    r.json("backup", &["backup"]);
    r.json("backup", &["backup", "--keep", "1"]);
    r.json("backup", &["backup", "--list"]);
    r.json("doctor", &["doctor"]);
    r.json("doctor", &["doctor", "--hubs"]);

    // ---- profiles, last: `use` moves every command after it ------------------
    r.json("profile list", &["profile", "list"]);
    r.json("profile show", &["profile", "show"]);
    r.json(
        "profile add",
        &[
            "profile",
            "add",
            "desk",
            "--kind",
            "tickets",
            "--root",
            repos.to_str().unwrap(),
            "--hubs",
            data.join("desk-hubs").to_str().unwrap(),
        ],
    );
    r.json(
        "profile set",
        &[
            "profile",
            "set",
            "desk",
            "--id-pattern",
            "[A-Z]+-[0-9]+",
            "--trays",
            data.join("desk-trays").to_str().unwrap(),
        ],
    );
    r.json("profile use", &["profile", "use", "desk"]);
    r.json("profile show", &["profile", "show", "desk"]);
    r.json("profile use", &["profile", "use", "line"]);
}
