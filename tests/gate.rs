//! `rigger gate`: the command that says a project is fit to commit.
//!
//! The record holds it once instead of seventeen skill files holding a copy
//! each, the packet prints it, and running it is witnessed rather than
//! reported. What the release promises: the gate is set, shown and cleared;
//! a run is recorded with its verdict; a red gate exits non-zero and is
//! remembered until it is run again green.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

/// A project whose directory exists, since a gate runs in it.
fn project(data: &Path, work: &Path) -> std::path::PathBuf {
    let root = work.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n").unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).args(["--name", "sample"]).assert().success();
    root
}

/// A shell line that succeeds on either shell a gate might be handed to.
const GREEN: &str = "cd .";
/// One that fails on either.
const RED: &str = "cd no-such-directory-here";

#[test]
fn a_gate_is_set_shown_and_cleared() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());

    rigger(data.path())
        .args(["project", "set", "sample", "--gate", GREEN])
        .assert()
        .success()
        .stdout(predicate::str::contains(GREEN));

    // Asking without setting is a question, not a silent no-op.
    rigger(data.path())
        .args(["project", "set", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains(GREEN));

    rigger(data.path())
        .args(["project", "show", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gate:"));

    rigger(data.path())
        .args(["project", "set", "sample", "--no-gate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no gate now"));

    rigger(data.path())
        .args(["project", "set", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("has no gate"));
}

#[test]
fn the_packet_prints_the_gate_on_one_line() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", GREEN]).assert().success();

    rigger(data.path())
        .args(["context", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Gate: {GREEN}")));
}

#[test]
fn without_a_gate_the_command_says_how_to_set_one() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());

    rigger(data.path())
        .args(["gate", "sample"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rigger project set sample --gate"));
}

#[test]
fn check_says_what_would_run_and_runs_nothing() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", RED]).assert().success();

    // A red gate, so a run would exit non-zero; `--check` must not.
    rigger(data.path())
        .args(["gate", "sample", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains(RED));

    // And nothing was recorded, because nothing ran.
    rigger(data.path())
        .args(["context", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("red (exit").not());
}

#[test]
fn a_green_run_succeeds_and_is_recorded() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", GREEN]).assert().success();

    rigger(data.path())
        .args(["gate", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("green"));

    rigger(data.path())
        .args(["context", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gate").and(predicate::str::contains("green")));
}

/// The exit code is the point: a hook or a script acts on it without
/// reading the text.
#[test]
fn a_red_run_exits_non_zero() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", RED]).assert().success();

    rigger(data.path())
        .args(["gate", "sample"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("red"));
}

#[test]
fn json_says_whether_it_passed() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", GREEN]).assert().success();

    let out = rigger(data.path()).args(["gate", "sample", "--json"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["passed"], true);
    assert_eq!(v["exit_code"], 0);
    assert_eq!(v["command"], GREEN);
}

/// Two runs of one command with one result are two facts. The ordinary
/// de-duplication of the record would have kept the first and dropped the
/// rest, so that "the gate is still green today" quietly became "the gate
/// was green once".
#[test]
fn every_run_is_kept_even_when_it_says_the_same_thing() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", GREEN]).assert().success();

    for _ in 0..3 {
        rigger(data.path()).args(["gate", "sample"]).assert().success();
    }

    let out = rigger(data.path()).args(["find", "green", "--json"]).output().unwrap();
    let found: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let runs = found.as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(runs, 3, "each run is its own fact: {found}");
}

/// The end-of-session reminder exists so that a red gate is not something
/// the next sitting discovers for itself.
#[test]
fn a_red_gate_is_remembered_at_the_end_of_the_session() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", RED]).assert().success();

    rigger(data.path()).args(["session", "start", "sample"]).assert().success();
    rigger(data.path()).args(["gate", "sample"]).assert().failure();

    rigger(data.path())
        .args(["session", "end", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("the gate was last red"));
}

/// And a gate run green again clears it: a reminder that never goes away is
/// a reminder nobody reads.
#[test]
fn running_it_green_again_takes_the_reminder_away() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", RED]).assert().success();

    rigger(data.path()).args(["session", "start", "sample"]).assert().success();
    rigger(data.path()).args(["gate", "sample"]).assert().failure();
    rigger(data.path()).args(["project", "set", "sample", "--gate", GREEN]).assert().success();
    rigger(data.path()).args(["gate", "sample"]).assert().success();

    rigger(data.path())
        .args(["session", "end", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("the gate was last red").not());
}

/// A gate that went red in some earlier sitting says nothing about this
/// one. Without the window the reminder would fire for ever.
#[test]
fn a_red_gate_from_an_earlier_session_is_not_this_sessions_reminder() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path()).args(["project", "set", "sample", "--gate", RED]).assert().success();

    rigger(data.path()).args(["session", "start", "sample"]).assert().success();
    rigger(data.path()).args(["gate", "sample"]).assert().failure();
    rigger(data.path()).args(["session", "end", "sample"]).assert().success();

    // A new sitting that never touched the gate.
    rigger(data.path()).args(["session", "start", "sample"]).assert().success();
    rigger(data.path())
        .args(["session", "end", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("the gate was last red").not());
}
