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

/// A hub in the shape the real ones have: a plan with open stages and
/// questions, a changelog of shipped ones, a dated decision log.
fn hub(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("План.md"),
        "# План разработки\n\n## Ждёт решения владельца\n\n1. Pick a colour.\n2. Sign the binary.\n\n## v0.3.0 · Third stage\n\n- [ ] first task\n- [ ] second task\n\n## Бэклог без версии\n\n- [ ] someday, not a task of a stage\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Изменения.md"),
        "# Изменения\n\n## v0.2.0 · Second stage — выпущена 2026-09-03\n\n- Something shipped.\n\n## v0.1.0 · First stage — выпущен 2026-09-01\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Решения.md"),
        "# Журнал решений\n\n---\n\n## 2026-09-03 · The record is the database\n\nBecause prose cannot be filtered.\n",
    )
    .unwrap();
}

/// A database with one project named `proj`, whose repository carries a hub.
fn imported_project(data: &Path) -> std::path::PathBuf {
    let root = data.join("proj");
    repo(&root);
    hub(&root.join("hub"));
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    root
}

#[test]
fn import_reads_stages_tasks_decisions_and_questions() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(root.join("hub"))
        .assert()
        .success()
        .stdout(
            predicate::str::contains("versions   3 added")
                .and(predicate::str::contains("tasks      2 added"))
                .and(predicate::str::contains("decisions  1 added"))
                .and(predicate::str::contains("questions  2 added")),
        );

    rigger(data.path()).arg("doctor").assert().success().stdout(
        predicate::str::contains("versions:  3")
            .and(predicate::str::contains("tasks:     2"))
            .and(predicate::str::contains("events:    3")),
    );
}

#[test]
fn importing_an_unchanged_hub_again_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");

    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();
    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));

    // And the record did not grow behind the report.
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("versions:  3").and(predicate::str::contains("events:    3")));
}

#[test]
fn a_stage_that_shipped_is_updated_rather_than_duplicated() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();

    // The stage moves from the plan to the changelog, as it does when a
    // version ships: same version, now with a date.
    std::fs::write(hub_dir.join("План.md"), "# План\n\n## Ждёт решения владельца\n\n- (пусто)\n").unwrap();
    std::fs::write(
        hub_dir.join("Изменения.md"),
        "# Изменения\n\n## v0.3.0 · Third stage — выпущена 2026-09-05\n\n## v0.2.0 · Second stage — выпущена 2026-09-03\n\n## v0.1.0 · First stage — выпущен 2026-09-01\n",
    )
    .unwrap();

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("versions   0 added, 1 updated"));

    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("versions:  3"));
}

#[test]
fn import_of_a_missing_hub_reports_the_files_it_wanted() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(root.join("no-such-hub"))
        .assert()
        .success()
        .stdout(predicate::str::contains("План.md is missing").and(predicate::str::contains("nothing changed")));
}

#[test]
fn import_of_an_unknown_project_fails() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path())
        .args(["import", "ghost", "--hub"])
        .arg(root.join("hub"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

#[test]
fn backup_copies_the_database_beside_itself() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .arg("backup")
        .assert()
        .success()
        .stdout(predicate::str::contains("Copied to"));

    let copies: Vec<_> = std::fs::read_dir(data.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bak"))
        .collect();
    assert_eq!(copies.len(), 1, "expected one backup, found {copies:?}");
    assert!(copies[0].file_name().to_string_lossy().starts_with("rigger.v1-"));
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
