//! The binary surface as a user meets it, against a throwaway data directory.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn repo(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"declared-elsewhere\"\nversion = \"0.1.0\"\n").unwrap();
}

#[test]
fn version_prints_the_crate_version() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_describes_the_product() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("One seat for all your projects and tasks"));
}

#[test]
fn init_creates_the_database_and_is_idempotent() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created").and(predicate::str::contains("schema version 1")));
    assert!(data.path().join("rigger.db").exists());

    rigger(data.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Already initialised"));
}

#[test]
fn commands_before_init_point_at_init() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .args(["project", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run `rigger init` first"));
}

#[test]
fn project_add_names_the_project_after_its_directory() {
    // The manifest declares another name on purpose: the crate or package
    // name is the publishing name, not the one the owner calls the project.
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("repos").join("folder-name");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 'folder-name'").and(predicate::str::contains("remote: none")));

    rigger(data.path())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("folder-name"));
}

#[test]
fn project_add_reads_the_origin_remote() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("remoted");
    repo(&root);
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(
        root.join(".git").join("config"),
        "[remote \"origin\"]\n\turl = https://example.com/remoted.git\n",
    )
    .unwrap();
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .success()
        .stdout(predicate::str::contains("remote: https://example.com/remoted.git"));

    rigger(data.path())
        .args(["project", "show", "remoted", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"remote\": \"https://example.com/remoted.git\""));
}

#[test]
fn name_can_be_overridden_and_must_be_unique() {
    let data = tempfile::tempdir().unwrap();
    let first = data.path().join("a").join("same");
    let second = data.path().join("b").join("same");
    repo(&first);
    repo(&second);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path()).args(["project", "add"]).arg(&first).assert().success();
    rigger(data.path())
        .args(["project", "add"])
        .arg(&second)
        .assert()
        .failure()
        .stderr(predicate::str::contains("a project named 'same' already exists"));
    rigger(data.path())
        .args(["project", "add", "--name", "other"])
        .arg(&second)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 'other'"));
}

#[test]
fn the_same_path_is_not_recorded_twice() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("once");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already recorded as project 'once'"));
}

#[test]
fn show_of_an_unknown_project_fails_with_a_pointer() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .args(["project", "show", "ghost"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

#[test]
fn doctor_reports_the_database_before_and_after_init() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("missing - run `rigger init`"));

    let root = data.path().join("one");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema:    version 1").and(predicate::str::contains("projects:  1")));
    rigger(data.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 1"));
}
