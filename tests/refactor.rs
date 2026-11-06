//! Whose turn it is to be refactored: one at a time across the line, by how much
//! has been done to the code since the last one.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use serde_json::Value;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
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

/// A project with this many releases behind it, and plenty planned ahead.
fn project(data: &Path, name: &str, releases: usize) -> PathBuf {
    let root = data.join("repos").join(name);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    git(&root, &["commit", "--quiet", "--allow-empty", "-m", "feat: begin"]);
    for n in 0..releases {
        git(&root, &["tag", &format!("v0.{n}.0")]);
    }
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    for n in 0..5 {
        rigger(data)
            .args(["version", "add", name, &format!("v9.{n}.0"), "--task", "a task"])
            .assert()
            .success();
    }
    rigger(data).args(["sync", name]).assert().success();
    root
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn json(data: &Path, args: &[&str]) -> Value {
    let out = rigger(data).args(args).arg("--json").assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn init(data: &Path) {
    rigger(data).arg("init").assert().success();
}

/// Below the minimum nobody is due; at it the one who has gone longest is, and
/// its first message says so instead of the stage.
#[test]
fn the_one_with_the_most_versions_goes_first_past_the_minimum() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "small", 9);
    assert!(json(data.path(), &["refactor"])["next"].is_null());
    assert!(text(data.path(), &["refactor"]).contains("Nobody has gone long enough"));

    project(data.path(), "medium", 10);
    project(data.path(), "large", 12);
    let rota = json(data.path(), &["refactor"]);
    assert_eq!(rota["next"], "large", "{rota:#}");
    assert_eq!(rota["standings"][0]["project"], "large");
    assert_eq!(rota["standings"][0]["versions"], 12);
    assert_eq!(rota["standings"][1]["project"], "medium");
    assert!(text(data.path(), &["refactor"]).contains("Next to refactor: large, on its next turn"));

    // It takes the project's own turn, and moves nothing.
    let queue = json(data.path(), &["queue"]);
    let entry = queue["order"].as_array().unwrap().iter().find(|c| c["project"] == "large").unwrap();
    assert_eq!(entry["refactor"], true);
    assert_eq!(entry["reason"], "turn");
    let other = queue["order"].as_array().unwrap().iter().find(|c| c["project"] == "medium").unwrap();
    assert_eq!(other["refactor"], false);

    let message = json(data.path(), &["go", "large"])["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("This turn is a refactoring instead of the stage, 12 versions since the last"),
        "{message}"
    );
    assert!(message.contains("rigger refactor done large"), "{message}");
    assert!(!message.contains("First the plan"), "one thing to a turn: {message}");
    let message = json(data.path(), &["go", "medium"])["message"].as_str().unwrap().to_string();
    assert!(!message.contains("refactoring"), "{message}");
}

/// One a week, across the line, and the next one waits for next week. The count
/// starts again for the one that was done.
#[test]
fn one_a_week_and_the_count_starts_again() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "large", 12);
    project(data.path(), "medium", 10);

    assert!(text(data.path(), &["refactor", "done", "large", "The loader: 900 lines to 300"]).contains("large starts counting again"));
    let rota = json(data.path(), &["refactor"]);
    assert!(rota["next"].is_null(), "the week's slot is taken: {rota:#}");
    assert_eq!(rota["this_week"][0]["project"], "large");
    assert_eq!(rota["standings"][0]["project"], "medium", "{rota:#}");
    assert_eq!(rota["standings"][1]["versions"], 0);
    assert!(text(data.path(), &["refactor"]).contains("This week's slot is taken: large"));
    assert!(!json(data.path(), &["go", "medium"])["message"].as_str().unwrap().contains("refactoring"));

    // A wider allowance, and the next one is due the same week.
    text(data.path(), &["config", "refactor.per_week", "2"]);
    assert_eq!(json(data.path(), &["refactor"])["next"], "medium");

    // Last week's does not take this week's slot.
    let db = data.path().join("profiles").join("line").join("rigger.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute("UPDATE events SET created_at = '2026-01-05T12:00:00Z' WHERE kind = 'refactor'", [])
        .unwrap();
    drop(conn);
    text(data.path(), &["config", "refactor.per_week", "--clear"]);
    let rota = json(data.path(), &["refactor"]);
    assert_eq!(rota["this_week"].as_array().unwrap().len(), 0, "{rota:#}");
    assert_eq!(rota["next"], "large", "its versions count again from that week: {rota:#}");
}

/// A project may set its own minimum, and the ones not coming up are not asked.
#[test]
fn the_minimum_is_a_projects_own_and_some_are_not_asked() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "small", 3);
    assert!(json(data.path(), &["refactor"])["next"].is_null());
    text(data.path(), &["config", "refactor.min_versions", "2", "--project", "small"]);
    assert_eq!(json(data.path(), &["refactor"])["next"], "small");

    rigger(data.path()).args(["priority", "small", "0"]).assert().success();
    assert!(json(data.path(), &["refactor"])["next"].is_null(), "at weight 0 it is not coming up");
    rigger(data.path()).args(["priority", "small", "2"]).assert().success();
    rigger(data.path()).args(["note", "small", "No key", "--kind", "blocker"]).assert().success();
    assert!(json(data.path(), &["refactor"])["next"].is_null(), "nor blocked");

    // A folder has no code to refactor.
    let notes = data.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&notes).assert().success();
    let rota = json(data.path(), &["refactor"]);
    assert!(rota["standings"].as_array().unwrap().iter().all(|s| s["project"] != "notes"), "{rota:#}");
}

/// The ritual holds the rules that make a refactoring safe, as commands and
/// steps someone can follow.
#[test]
fn the_ritual_says_how() {
    let data = tempfile::tempdir().unwrap();
    let said = text(data.path(), &["refactor", "ritual"]);
    for must in [
        "Measure first",
        "git log",
        "characterisation tests",
        "One hat at a time",
        "EXPLAIN",
        "expand first",
        "What does not move",
        "revert",
        "rigger refactor done",
    ] {
        assert!(said.contains(must), "the ritual lost `{must}`");
    }
}
