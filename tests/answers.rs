//! `import --answers`: a questionnaire the owner answered, into the record.
//!
//! The page that produced it is otherwise the only place these choices are
//! written down, and the plan made from them says what was decided without
//! saying what it was decided *against* - which is the half that answers
//! "why this and not that" a year later.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    root
}

fn answers_file(data: &Path, json: &str) -> std::path::PathBuf {
    let path = data.join("answers.json");
    std::fs::write(&path, json).unwrap();
    path
}

const FILLED: &str = r#"{
  "questionnaire": "plan review 03ab557f",
  "answered": "2026-09-11",
  "forks": [{
    "question": "Where knowledge of the code comes from",
    "chosen": "nooma as a library",
    "against": ["a tree-sitter index of its own", "asking an assistant each time"],
    "why": "One index for the line, not one per product."
  }],
  "ideas": [
    {"idea": "A registry of the line as public JSON", "taken": true},
    {"idea": "A second binary for the alias", "taken": false, "why": "An alias is a link."}
  ]
}"#;

#[test]
fn a_fork_becomes_a_decision_that_names_what_it_beat() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), FILLED);

    rigger(data.path())
        .args(["import", "sample", "--answers"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("3 added"));

    rigger(data.path()).args(["context", "sample"]).assert().success().stdout(
        predicate::str::contains("nooma as a library")
            .and(predicate::str::contains("Rather than"))
            .and(predicate::str::contains("tree-sitter")),
    );
}

#[test]
fn a_taken_idea_is_a_wish_and_an_untaken_one_is_a_decision() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), FILLED);
    rigger(data.path()).args(["import", "sample", "--answers"]).arg(&file).assert().success();

    let out = rigger(data.path()).args(["context", "sample"]).output().unwrap();
    let packet = String::from_utf8(out.stdout).unwrap();
    let (_, wishes) = packet.split_once("## Wishes, not yet sorted\n").expect("the taken idea is a wish");
    let wishes = wishes.split("\n## ").next().unwrap_or("");
    assert!(wishes.contains("A registry of the line"), "{wishes}");
    assert!(!wishes.contains("second binary"), "an untaken idea is not a wish: {wishes}");
    assert!(packet.contains("Not taking: A second binary"), "{packet}");
}

/// The events belong to the day the owner answered, not to the day somebody
/// got round to importing the file - otherwise the reasoning is filed after
/// the work it explains.
#[test]
fn the_events_carry_the_day_it_was_answered() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), FILLED);
    rigger(data.path()).args(["import", "sample", "--answers"]).arg(&file).assert().success();

    rigger(data.path())
        .args(["context", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2026-09-11 · decision"));
}

/// Reading the same page twice is ordinary - it is how a correction to it
/// is applied - so it must not pile up copies.
#[test]
fn reading_the_same_answers_twice_records_them_once() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), FILLED);
    rigger(data.path()).args(["import", "sample", "--answers"]).arg(&file).assert().success();

    rigger(data.path())
        .args(["import", "sample", "--answers"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 added").and(predicate::str::contains("3 already recorded")));
}

#[test]
fn check_says_what_it_would_take_and_records_nothing() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), FILLED);

    rigger(data.path())
        .args(["import", "sample", "--answers"])
        .arg(&file)
        .arg("--check")
        .assert()
        .success()
        .stdout(predicate::str::contains("would take 3 events"));

    rigger(data.path())
        .args(["context", "sample"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nooma as a library").not());
}

#[test]
fn a_file_that_answers_nothing_is_refused() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), r#"{"questionnaire":"empty"}"#);

    rigger(data.path())
        .args(["import", "sample", "--answers"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("answers nothing"));
}

#[test]
fn a_file_that_is_not_a_questionnaire_is_refused_by_name() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let file = answers_file(data.path(), "not json at all");

    rigger(data.path())
        .args(["import", "sample", "--answers"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a questionnaire rigger understands"));
}

/// `--hub` and `--answers` are two sources, not two halves of one: asking
/// for both is a mistake worth naming rather than resolving by precedence.
#[test]
fn a_hub_and_a_questionnaire_are_not_asked_for_together() {
    let data = tempfile::tempdir().unwrap();
    let root = project(data.path());
    let file = answers_file(data.path(), FILLED);

    rigger(data.path())
        .args(["import", "sample", "--hub"])
        .arg(&root)
        .arg("--answers")
        .arg(&file)
        .assert()
        .failure();

    // And neither is no source at all.
    rigger(data.path()).args(["import", "sample"]).assert().failure();
}
