//! Rhythm and facts: what the week says out loud, and what a release proves.
//!
//! Each outside party this stage talks to - GitHub, kasl, the desktop - is a
//! program named in the environment, standing in for the real one. A scratch
//! record reaches the real ones not at all (`paths::scratch`), so these tests
//! cannot pop a toast on the machine running them or start an interval in
//! its kasl; what they read is what the stand-in wrote down.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn git(dir: &Path, args: &[&str], at: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    if let Some(at) = at {
        cmd.env("GIT_AUTHOR_DATE", at).env("GIT_COMMITTER_DATE", at);
    }
    let out = cmd.output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

/// A project with a plan, a GitHub remote, and tags on the days given.
fn project(data: &Path, name: &str, stages: &[&str], releases: &[(&str, &str)]) -> PathBuf {
    let root = data.join(name);
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    let mut plan = String::from("# План\n\n");
    for stage in stages {
        plan.push_str(&format!("## {stage}\n\n- [ ] a task\n- [ ] another task\n\n"));
    }
    std::fs::write(hub.join("План.md"), plan).unwrap();

    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);
    git(&root, &["remote", "add", "origin", &format!("https://github.com/acme/{name}.git")], None);
    std::fs::write(root.join("README.md"), name).unwrap();
    git(&root, &["add", "."], Some("2026-08-03T12:00:00Z"));
    git(&root, &["commit", "--quiet", "-m", "chore: start"], Some("2026-08-03T12:00:00Z"));
    for (n, (version, date)) in releases.iter().enumerate() {
        let at = format!("{date}T12:00:00Z");
        std::fs::write(root.join(format!("file{n}.txt")), version).unwrap();
        git(&root, &["add", "."], Some(&at));
        git(&root, &["commit", "--quiet", "-m", &format!("feat: {version}")], Some(&at));
        git(&root, &["tag", version], Some(&at));
    }
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    root
}

fn record(data: &Path) {
    rigger(data).arg("init").assert().success();
}

/// The week the test runs in, as `2026-W39`: whatever is "late" is late
/// against the real clock.
fn this_week() -> String {
    let week = jiff::Zoned::now().date().iso_week_date();
    format!("{}-W{:02}", week.year(), week.week())
}

/// A stand-in program: on Windows a batch file, elsewhere a shell script.
/// `windows` and `unix` are its body in each language.
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

/// A `gh` that answers the three questions `sync` asks. `view` is what
/// `release view` prints, or `None` for "release not found"; `list` is what
/// `release list` prints, and `runs` what `run list` prints.
fn fake_gh(dir: &Path, view: Option<&str>, list: &str, runs: &str) -> PathBuf {
    let log = dir.join("gh.log");
    let view_win = match view {
        Some(json) => format!("echo {json}\nexit /b 0"),
        None => "echo release not found 1>&2\nexit /b 1".to_string(),
    };
    let view_sh = match view {
        Some(json) => format!("echo '{json}'; exit 0"),
        None => "echo 'release not found' >&2; exit 1".to_string(),
    };
    program(
        dir,
        "gh",
        &format!(
            "echo %*>>\"{log}\"\nif \"%1 %2\"==\"release view\" goto view\nif \"%1 %2\"==\"release list\" goto list\nif \"%1 %2\"==\"run list\" goto runs\nexit /b 1\n:view\n{view_win}\n:list\necho {list}\nexit /b 0\n:runs\necho {runs}\nexit /b 0",
            log = log.display()
        ),
        &format!(
            "echo \"$@\" >> '{log}'\ncase \"$1 $2\" in\n\"release view\") {view_sh} ;;\n\"release list\") echo '{list}'; exit 0 ;;\n\"run list\") echo '{runs}'; exit 0 ;;\nesac\nexit 1",
            log = log.display()
        ),
    )
}

/// What a stand-in wrote down. Read without insisting on UTF-8: a batch
/// file writes in the console's code page, and a test about which calls
/// were made should not fail on how a middle dot was spelt.
fn read_log(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned()
}

/// A program that writes down its arguments, one call per line.
fn recorder(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let log = dir.join(format!("{name}.log"));
    let path = program(
        dir,
        name,
        &format!("echo %*>>\"{}\"", log.display()),
        &format!("echo \"$@\" >> '{}'", log.display()),
    );
    (path, log)
}

// ---------------------------------------------------------------------------
// Questions with a day

/// A question can be asked from the command line, with the day its answer
/// is needed by; once that day has gone, every screen that shows it says
/// so in words - colour is only for a person at a terminal.
#[test]
fn a_question_past_its_day_is_named_overdue_everywhere() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);

    let yesterday = jiff::Zoned::now().date().yesterday().unwrap().to_string();
    rigger(data.path())
        .args(["note", "widget", "Which registry goes first?", "--kind", "question", "--due", &yesterday])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("due {yesterday}")));
    rigger(data.path())
        .args(["note", "widget", "Which icon?", "--kind", "question", "--due", "+30d"])
        .assert()
        .success();

    let inbox = output(data.path(), &["inbox"]);
    assert!(inbox.contains(&format!("overdue since {yesterday}")), "{inbox}");
    assert!(inbox.contains("1 overdue"), "{inbox}");
    // A pipe gets no escape codes: colour is for a terminal only.
    assert!(!inbox.contains('\x1b'), "{inbox}");
    // The one with time left says its day, not "overdue".
    assert!(
        inbox.lines().any(|l| l.contains("Which icon?") && l.contains("due ") && !l.contains("overdue")),
        "{inbox}"
    );

    let packet = output(data.path(), &["context", "widget"]);
    assert!(packet.contains(&format!("(overdue since {yesterday}) Which registry goes first?")), "{packet}");

    let brief = output(data.path(), &["week"]);
    assert!(brief.contains("Which registry goes first") && brief.contains("overdue since"), "{brief}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["inbox", "--json"])).unwrap();
    let late: Vec<&serde_json::Value> = json["waiting"].as_array().unwrap().iter().filter(|q| q["overdue"] == true).collect();
    assert_eq!(late.len(), 1, "{json}");
    assert_eq!(late[0]["due"], yesterday.as_str());

    // Answering it is what takes it off every screen.
    let id = late[0]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["resolve", "widget", &id, "crates.io"]).assert().success();
    let inbox = output(data.path(), &["inbox"]);
    assert!(!inbox.contains("overdue"), "{inbox}");
}

/// A day belongs to a question, and a day that is not a day is refused -
/// before anything is written, so a typo never leaves a question without
/// the deadline it was asked with.
#[test]
fn a_due_day_is_checked_before_anything_is_written() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);

    rigger(data.path())
        .args(["note", "widget", "Something learnt", "--due", "tomorrow"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--due belongs to a question"));
    rigger(data.path())
        .args(["note", "widget", "When?", "--kind", "question", "--due", "someday"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a day"));
    let inbox = output(data.path(), &["inbox"]);
    assert!(inbox.contains("Nothing is waiting"), "the refused question was recorded anyway: {inbox}");
}

// ---------------------------------------------------------------------------
// A release, as the engine reports it

/// The release engine's word closes the version, says how far it got, and
/// leaves a `shipped` event in the history.
#[test]
fn a_release_reported_by_the_engine_closes_the_version() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First", "v0.2.0 · Second"], &[]);

    rigger(data.path())
        .args([
            "note",
            "widget",
            "--kind",
            "shipped",
            "--tag",
            "v0.1.0",
            "--release",
            "--registry",
            "crates.io",
            "--registry",
            "npm",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("v0.1.0 shipped - released and published"))
        .stdout(predicate::str::contains("reached crates.io, npm"));

    let card: serde_json::Value = serde_json::from_str(&output(data.path(), &["version", "show", "widget", "v0.1.0", "--json"])).unwrap();
    assert_eq!(card["status"], "shipped");
    assert_eq!(card["delivery"], "published");
    assert_eq!(card["registries"], serde_json::json!(["crates.io", "npm"]));

    // The stage being built moves on to the next one.
    let next: serde_json::Value = serde_json::from_str(&output(data.path(), &["version", "show", "widget", "--json"])).unwrap();
    assert_eq!(next["version"], "v0.2.0");

    let history = output(data.path(), &["find", "went out", "--project", "widget"]);
    assert!(history.contains("v0.1.0 went out: the release with its archives"), "{history}");
}

/// The flags of a release belong to a release, and a release is known by
/// its tag.
#[test]
fn a_release_needs_its_tag_and_its_flags_need_a_release() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);

    rigger(data.path())
        .args(["note", "widget", "--kind", "shipped"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tag"));
    rigger(data.path())
        .args(["note", "widget", "a finding", "--registry", "npm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--kind shipped"));
    rigger(data.path())
        .args(["note", "widget", "--kind", "shipped", "--tag", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a version tag"));
}

// ---------------------------------------------------------------------------
// GitHub, as sync reads it

/// A tag with no release is the v0.4.0 lesson: counted as shipped by every
/// screen, installable by nobody. `sync` says so on every run until it is
/// fixed, and `show` carries it beside the version.
#[test]
fn a_tag_without_a_release_is_said_out_loud() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-01")]);
    let gh = fake_gh(data.path(), None, r#"[{"tagName":"v0.0.9"}]"#, "[]");

    let out = rigger(data.path()).env("RIGGER_GH", &gh).args(["sync"]).assert().success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(out.contains("delivery   v0.1.0: tag exists, no release"), "{out}");
    let asked = read_log(&data.path().join("gh.log"));
    assert!(asked.contains("acme/widget"), "gh was not asked about the remote's repository: {asked}");

    // Said again on the next run: a standing fault is not news, but it is
    // not fixed either.
    let again = rigger(data.path()).env("RIGGER_GH", &gh).args(["sync"]).assert().success();
    let again = String::from_utf8(again.get_output().stdout.clone()).unwrap();
    assert!(again.contains("tag exists, no release"), "{again}");

    let screen = output(data.path(), &["show", "widget"]);
    assert!(screen.contains("Last shipped v0.1.0 on 2026-09-01 - tag exists, no release"), "{screen}");
    let packet = output(data.path(), &["context", "widget"]);
    assert!(packet.contains("Last shipped: v0.1.0 on 2026-09-01 - tag exists, no release"), "{packet}");
}

/// A repository that makes no releases at all tags by way of working, and
/// calling each of its tags "no release" would be a warning for ever.
#[test]
fn a_repository_that_never_releases_is_not_warned_about() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-01")]);
    let gh = fake_gh(data.path(), None, "[]", "[]");

    let out = rigger(data.path()).env("RIGGER_GH", &gh).args(["sync", "widget"]).assert().success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!out.contains("delivery"), "{out}");
}

/// A green publish is the full delivery; it is said once, when learnt.
#[test]
fn a_green_publish_is_learnt_once() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-01")]);
    let gh = fake_gh(
        data.path(),
        Some(r#"{"assets":[{"name":"widget.zip"}],"isDraft":false}"#),
        r#"[{"tagName":"v0.1.0"}]"#,
        r#"[{"workflowName":"Publish","status":"completed","conclusion":"success"}]"#,
    );
    let first = rigger(data.path()).env("RIGGER_GH", &gh).args(["sync", "widget"]).assert().success();
    let first = String::from_utf8(first.get_output().stdout.clone()).unwrap();
    assert!(first.contains("delivery   v0.1.0: released and published"), "{first}");

    // Published is final: the next sync does not ask again.
    std::fs::remove_file(data.path().join("gh.log")).unwrap();
    rigger(data.path()).env("RIGGER_GH", &gh).args(["sync", "widget"]).assert().success();
    assert!(!data.path().join("gh.log").exists(), "a published version was asked about again");
}

/// The engine is the thing that made the release; a look at GitHub does
/// not overwrite what it said.
#[test]
fn what_the_engine_said_is_not_overwritten_by_a_look() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-01")]);
    rigger(data.path())
        .args(["note", "widget", "--kind", "shipped", "--tag", "v0.1.0", "--registry", "crates.io"])
        .assert()
        .success()
        .stdout(predicate::str::contains("had shipped already"));
    let gh = fake_gh(data.path(), None, r#"[{"tagName":"v0.0.9"}]"#, "[]");
    rigger(data.path()).env("RIGGER_GH", &gh).args(["sync", "widget"]).assert().success();

    let card: serde_json::Value = serde_json::from_str(&output(data.path(), &["version", "show", "widget", "v0.1.0", "--json"])).unwrap();
    assert_eq!(card["delivery"], "published", "{card}");
    // And the tag's day stayed the tag's: the engine does not move a
    // version git already proved.
    assert_eq!(card["shipped_at"], "2026-09-01", "{card}");
}

/// A scratch record never reaches the real GitHub: with no `gh` named, it
/// does not ask at all - which is also what keeps these tests off the
/// network.
#[test]
fn a_scratch_record_asks_no_github_it_did_not_name() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-01")]);
    let out = output(data.path(), &["sync", "widget", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(json[0].get("delivery").is_none(), "{json}");
}

// ---------------------------------------------------------------------------
// Contracts

/// Every key of a JSON value, at every depth.
fn keys(value: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                out.insert(key.clone());
                keys(inner, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| keys(item, out)),
        _ => {}
    }
}

/// Holds a command's JSON to its reference page: every field it prints is
/// named there, in backticks. A release engine reads this shape, and a field
/// renamed in code and not on the page is a consumer broken without notice.
fn documented(page: &str, json: &serde_json::Value) {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/content/docs/reference").join(page)).unwrap();
    let mut found = std::collections::BTreeSet::new();
    keys(json, &mut found);
    let missing: Vec<&String> = found.iter().filter(|k| !text.contains(&format!("`{k}`"))).collect();
    assert!(missing.is_empty(), "{page} does not name these fields of the JSON: {missing:?}\n{json:#}");
}

#[test]
fn version_show_json_is_the_documented_contract() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First", "v0.2.0 · Second"], &[]);
    rigger(data.path())
        .args(["note", "widget", "--kind", "shipped", "--tag", "v0.1.0", "--release", "--registry", "crates.io"])
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "plan", "widget", "v0.2.0", "--week", &this_week()])
        .assert()
        .success();

    for args in [
        ["version", "show", "widget", "v0.1.0", "--json"],
        ["version", "show", "widget", "v0.2.0", "--json"],
    ] {
        let json: serde_json::Value = serde_json::from_str(&output(data.path(), &args)).unwrap();
        documented("version.md", &json);
    }
    let planned: serde_json::Value = serde_json::from_str(&output(data.path(), &["version", "show", "widget", "--json"])).unwrap();
    assert_eq!(planned["version"], "v0.2.0");
    assert_eq!(planned["title"], "Second");
    assert_eq!(planned["week"], this_week());
    assert_eq!(planned["open_tasks"], 2);
    assert!(planned["friday"].as_str().is_some_and(|f| f.len() == 10), "{planned}");
}

/// The fixture fills every list `next --json` has, so that the fields of a
/// focus, a lapse, a signal and a parted pair are all there to be checked.
#[test]
fn next_json_is_the_documented_contract() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second", "v0.3.0 · Third"],
        &[("v0.1.0", "2026-08-10")],
    );
    project(data.path(), "beta", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["project", "tier", "alpha", "A"]).assert().success();
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.2.0", "--week", "2026-W50"])
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.3.0", "--week", "2026-W45"])
        .assert()
        .success();
    rigger(data.path())
        .args(["link", "add", "alpha@v0.1.0", "beta@v0.1.0", "--kind", "pair"])
        .assert()
        .success();

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["next", "--week", "2026-W50", "--json"])).unwrap();
    for list in ["focus", "overdue", "lapsed", "signals", "parted"] {
        assert!(
            json[list].as_array().is_some_and(|l| !l.is_empty()),
            "the fixture left `{list}` empty: {json:#}"
        );
    }
    documented("next.md", &json);
}

// ---------------------------------------------------------------------------
// Tiers from the numbers

#[test]
fn a_tier_is_suggested_and_nothing_is_set() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "idle", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["project", "tier", "idle", "B"]).assert().success();

    let out = output(data.path(), &["project", "tier", "--suggest"]);
    assert!(out.contains("stalled in B"), "{out}");
    assert!(out.contains("rigger project tier idle C"), "{out}");
    // Suggesting is not setting.
    let screen = output(data.path(), &["project", "show", "idle"]);
    assert!(!screen.contains("tier       C") && !screen.contains("tier C"), "{screen}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["project", "tier", "--suggest", "--json"])).unwrap();
    let idle = &json["suggestions"][0];
    assert_eq!(idle["verdict"], "stalled");
    assert_eq!(idle["suggested"], "C");
}

// ---------------------------------------------------------------------------
// The phone's calendar

#[test]
fn the_calendar_is_written_for_the_phone() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First", "v0.2.0 · Second"], &[]);
    rigger(data.path())
        .args(["version", "plan", "widget", "v0.1.0", "--week", "2026-W50"])
        .assert()
        .success();
    let file = data.path().join("rigger.ics");
    rigger(data.path())
        .args(["calendar", "--from", "2026-W49", "--weeks", "3", "--ics"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("1 release Friday and 1 week of focus"));
    let ics = std::fs::read_to_string(&file).unwrap();
    assert!(ics.contains("UID:release-widget-v0.1.0@rigger\r\n"), "{ics}");
    // 2026-W50 runs from Monday the 7th to Friday the 11th of December.
    assert!(ics.contains("DTSTART;VALUE=DATE:20261211\r\n"), "{ics}");
    assert!(ics.contains("UID:focus-2026-W50@rigger\r\n"), "{ics}");
    assert!(ics.contains("DTSTART;VALUE=DATE:20261207\r\n"), "{ics}");
    // A version with no week has no Friday to be on.
    assert!(!ics.contains("v0.2.0"), "{ics}");
}

// ---------------------------------------------------------------------------
// The week, as a toast

#[test]
fn the_week_is_toasted_once_a_week() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);
    rigger(data.path())
        .args(["note", "widget", "Which registry?", "--kind", "question"])
        .assert()
        .success();
    let (notify, log) = recorder(data.path(), "notify");

    // Nothing so far has toasted: a scratch record with no notifier named
    // does not reach the desktop, and does not use up the week either.
    assert!(!log.exists());

    rigger(data.path()).env("RIGGER_NOTIFY", &notify).args(["inbox"]).assert().success();
    let said = read_log(&log);
    // The week, not the whole title: a batch file writes what it was given
    // in the console's code page, and the title's middle dot does not
    // survive that - the week is what says the right toast went out.
    assert!(said.contains(&this_week()) && said.contains("releases on"), "{said}");

    // The second run of the week says nothing.
    rigger(data.path()).env("RIGGER_NOTIFY", &notify).args(["inbox"]).assert().success();
    assert_eq!(read_log(&log), said, "the week was toasted twice");

    // A new week is claimed again.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    db.execute("UPDATE settings SET value = '2020-W01' WHERE key = 'week_toast'", []).unwrap();
    rigger(data.path()).env("RIGGER_NOTIFY", &notify).args(["inbox"]).assert().success();
    assert_eq!(read_log(&log).lines().count(), 2);
}

// ---------------------------------------------------------------------------
// kasl

#[test]
fn a_session_opens_and_closes_an_interval_in_kasl() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);
    let (kasl, log) = recorder(data.path(), "kasl");

    rigger(data.path())
        .env("RIGGER_KASL", &kasl)
        .args(["session", "start", "widget"])
        .assert()
        .success();
    // Joining the sitting already open is not a second interval.
    rigger(data.path())
        .env("RIGGER_KASL", &kasl)
        .args(["session", "start", "widget"])
        .assert()
        .success();
    rigger(data.path())
        .env("RIGGER_KASL", &kasl)
        .args(["session", "end", "widget"])
        .assert()
        .success();

    let said = read_log(&log);
    let calls: Vec<&str> = said.lines().collect();
    assert_eq!(calls.len(), 2, "{said}");
    assert!(
        calls[0].contains("focus start") && calls[0].contains("widget v0.1.0") && calls[0].contains("--from rigger"),
        "{said}"
    );
    assert!(calls[1].contains("focus stop") && calls[1].contains("--from rigger"), "{said}");
}

/// A kasl that does not know `focus` - every kasl today - is silence: the
/// session opens and closes as it always did.
#[test]
fn a_kasl_that_says_no_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);
    let kasl = program(
        data.path(),
        "kasl",
        "echo unrecognized subcommand 'focus' 1>&2\nexit /b 2",
        "echo \"unrecognized subcommand 'focus'\" >&2; exit 2",
    );

    let out = rigger(data.path())
        .env("RIGGER_KASL", &kasl)
        .args(["session", "start", "widget"])
        .assert()
        .success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!out.contains("kasl"), "{out}");
    let json = rigger(data.path())
        .env("RIGGER_KASL", &kasl)
        .args(["session", "end", "widget", "--json"])
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&json.get_output().stdout).unwrap();
    assert_eq!(json["kasl"]["state"], "refused", "{json}");
}

// ---------------------------------------------------------------------------
// The diary's draft

/// The draft's "done" is the tasks the sitting closed, the changes it
/// recorded and the commits it made - and an edited draft is what `end`
/// writes.
#[test]
fn the_diary_is_drafted_from_the_sitting_and_the_edit_is_kept() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    let root = project(data.path(), "widget", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["session", "start", "widget"]).assert().success();
    // The sitting began a minute ago. Stamps are kept to the second, and a
    // commit made in the same second the session opened would be read as
    // made before it - true of the stamps, not of the work.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    let earlier = jiff::Timestamp::from_second(jiff::Timestamp::now().as_second() - 60).unwrap().to_string();
    db.execute("UPDATE sessions SET started_at = ?1 WHERE ended_at IS NULL", [&earlier]).unwrap();

    let plan = output(data.path(), &["version", "show", "widget", "--json"]);
    let plan: serde_json::Value = serde_json::from_str(&plan).unwrap();
    let task = plan["tasks"][0]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["task", "status", &task, "done"]).assert().success();
    std::fs::write(root.join("new.txt"), "x").unwrap();
    git(&root, &["add", "."], None);
    git(&root, &["commit", "--quiet", "-m", "feat: the grid"], None);

    let draft = output(data.path(), &["session", "draft", "widget", "--heading", "the grid"]);
    assert!(draft.contains("**Сделано.**"), "{draft}");
    assert!(draft.contains("- a task"), "the closed task is not in what was done: {draft}");
    assert!(draft.contains("- feat: the grid"), "the session's commit is not in what was done: {draft}");

    let edited = data.path().join("entry.md");
    std::fs::write(&edited, "## today · the grid\n\nThe grid went in, and the week reads right.\n").unwrap();
    let diary = data.path().join("Дневник.md");
    rigger(data.path())
        .args(["session", "end", "widget", "--entry"])
        .arg(&edited)
        .arg("--diary")
        .arg(&diary)
        .assert()
        .success();
    let written = std::fs::read_to_string(&diary).unwrap();
    assert!(written.contains("The grid went in, and the week reads right."), "{written}");
    assert!(!written.contains("**Сделано.**"), "the composed entry was written over the edit: {written}");
}

// ---------------------------------------------------------------------------
// The digest in a note

#[test]
fn the_digest_goes_into_a_note_once() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1.0 · First"], &[]);
    rigger(data.path())
        .args(["note", "widget", "Weeks are ISO weeks", "--kind", "decision"])
        .assert()
        .success();
    let note = data.path().join("2026-09-22.md");
    std::fs::write(&note, "# Tuesday\n\nMy own words.\n").unwrap();

    rigger(data.path())
        .args(["digest", "--md"])
        .arg(&note)
        .assert()
        .success()
        .stdout(predicate::str::contains("Added the digest"));
    let first = std::fs::read_to_string(&note).unwrap();
    assert!(first.starts_with("# Tuesday\n\nMy own words.\n"), "the owner's words moved: {first}");
    assert!(first.contains("**widget**") && first.contains("- recorded 1 decision"), "{first}");

    // Written again, it replaces itself rather than adding a second copy.
    rigger(data.path())
        .args(["digest", "--md"])
        .arg(&note)
        .assert()
        .success()
        .stdout(predicate::str::contains("already holds"));
    let second = std::fs::read_to_string(&note).unwrap();
    assert_eq!(second.matches("<!-- rigger digest -->").count(), 1, "{second}");
}

/// GitHub is asked about the tag as the tag is spelt. A plan that writes
/// `v0.1` for the release tagged `v0.1.0` is matched by value in the record,
/// and asking GitHub about `v0.1` answers "no release" for a version that
/// has one - which is what the first run over the real line did.
#[test]
fn github_is_asked_about_the_tag_not_the_plans_spelling() {
    let data = tempfile::tempdir().unwrap();
    record(data.path());
    project(data.path(), "widget", &["v0.1 · First"], &[("v0.1.0", "2026-09-01")]);
    let gh = fake_gh(
        data.path(),
        Some(r#"{"assets":[],"isDraft":false}"#),
        r#"[{"tagName":"v0.1.0"}]"#,
        r#"[{"workflowName":"Publish","status":"completed","conclusion":"success"}]"#,
    );
    let out = rigger(data.path()).env("RIGGER_GH", &gh).args(["sync", "widget"]).assert().success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let asked = read_log(&data.path().join("gh.log"));
    assert!(asked.contains("release view v0.1.0 "), "{asked}");
    // A release with nothing attached and a green publish is a library's
    // whole delivery, not a shortfall.
    assert!(out.contains("released and published"), "{out}");
}
