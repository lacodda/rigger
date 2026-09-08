//! A task's status is a word from a vocabulary, not one of two.
//!
//! `new` is what a plan's empty box means; `active`, `waiting-handoff`
//! and `frozen` are what a desk needs to say about work that is neither
//! untouched nor done; `done` and `dropped` are the end. What the release
//! promises: the words are given and shown, a hub's box does not reset
//! them, and the count of open work counts all of them but the last two.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План\n\n## v0.2.0 · Второй\n\n- [ ] сделать одно\n- [ ] сделать другое\n",
    )
    .unwrap();
    rigger(data).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    hub
}

/// The id of the first task, as the plan lists it.
fn first_task(data: &Path) -> String {
    let db = rusqlite::Connection::open(data.join("profiles").join("line").join("rigger.db")).unwrap();
    let id: i64 = db.query_row("SELECT id FROM tasks ORDER BY id LIMIT 1", [], |r| r.get(0)).unwrap();
    id.to_string()
}

#[test]
fn a_status_is_given_shown_and_counted() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let id = first_task(data.path());

    let out = output(data.path(), &["task", "status", &id, "active"]);
    assert!(out.contains("is now active (was new)"), "{out}");
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("- сделать одно (active)"), "{packet}");
    assert!(packet.contains("- сделать другое\n"), "a new task carries no mark: {packet}");
    assert!(packet.contains("2 tasks open"), "active is still open work: {packet}");

    output(data.path(), &["task", "status", &id, "waiting-handoff"]);
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("(waiting-handoff)") && packet.contains("2 tasks open"), "{packet}");

    output(data.path(), &["task", "status", &id, "done"]);
    let packet = output(data.path(), &["context", "sample"]);
    assert!(!packet.contains("сделать одно"), "done leaves the stage's list: {packet}");
    assert!(packet.contains("1 task open"), "{packet}");
}

#[test]
fn a_status_outside_the_vocabulary_is_refused_by_name() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let id = first_task(data.path());
    rigger(data.path())
        .args(["task", "status", &id, "sleeping"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("'sleeping' is not a status").and(predicate::str::contains("waiting-handoff")));
    rigger(data.path())
        .args(["task", "status", &id, "dropped"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a status a task can be given"));
}

/// A hub's empty box means "not done": reading the hub again leaves an
/// active task active, and exporting it writes the box back empty.
#[test]
fn a_hubs_box_neither_resets_nor_loses_a_status() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let id = first_task(data.path());
    output(data.path(), &["task", "status", &id, "frozen"]);

    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("(frozen)"), "a re-read hub does not reset the status: {packet}");

    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    let plan = std::fs::read_to_string(out.join("План.md")).unwrap();
    assert!(plan.contains("- [ ] сделать одно\n- [ ] сделать другое\n"), "{plan}");

    // And once done, the box is ticked, whatever the word before was.
    output(data.path(), &["task", "status", &id, "done"]);
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    let plan = std::fs::read_to_string(out.join("План.md")).unwrap();
    assert!(plan.contains("- [x] сделать одно\n- [ ] сделать другое\n"), "{plan}");
}

/// A record from before the vocabulary had `open`; the migration reads it
/// as `new`, and nothing about it is lost.
#[test]
fn an_open_task_from_before_reads_as_new() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let db_path = data.path().join("profiles").join("line").join("rigger.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    // What an older rigger wrote, and the version it wrote it at - with
    // what later schemas added taken away, so the migrations run again.
    db.execute_batch(
        "UPDATE tasks SET status = 'open'; \
         DROP TABLE task_projects; DROP TABLE settings; DROP INDEX tasks_by_key; \
         ALTER TABLE tasks DROP COLUMN key; ALTER TABLE tasks DROP COLUMN aliases; \
         ALTER TABLE tasks DROP COLUMN summary; ALTER TABLE tasks DROP COLUMN updated_at; \
         PRAGMA user_version = 16;",
    )
    .unwrap();
    drop(db);

    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("2 tasks open"), "{packet}");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    let left: i64 = db.query_row("SELECT COUNT(*) FROM tasks WHERE status = 'open'", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0, "no task is left under the old word");
}
