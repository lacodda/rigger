//! Putting work down: a task asleep until a day, and a task frozen on
//! purpose.
//!
//! What the release promises: a task put to sleep leaves the packet, the
//! list of cards and the hand until its day, and is counted where it left
//! so a stage does not silently read as smaller; its day coming wakes it
//! with nobody doing anything; `unsnooze` wakes it sooner. A version whose
//! every open task is frozen or asleep stops being the stage being built,
//! and the packet names it as set aside. `freeze` records why.

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

fn db(data: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(data.join("profiles").join("line").join("rigger.db")).unwrap()
}

/// A project with two stages; returns nothing, the ids come from `task_id`.
fn project(data: &Path) {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План\n\n## v0.2.0 · Second\n\n- [ ] do one thing\n- [ ] do another\n\n## v0.3.0 · Third\n\n- [ ] do the third\n",
    )
    .unwrap();
    rigger(data).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
}

fn task_id(data: &Path, title: &str) -> String {
    let id: i64 = db(data).query_row("SELECT id FROM tasks WHERE title = ?1", [title], |r| r.get(0)).unwrap();
    id.to_string()
}

#[test]
fn a_task_asleep_leaves_the_packet_and_is_counted_there() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let id = task_id(data.path(), "do one thing");

    let out = output(data.path(), &["task", "snooze", &id, "--until", "2w"]);
    assert!(out.contains("sleeps until") && out.contains("do one thing"), "{out}");
    let packet = output(data.path(), &["context", "sample"]);
    assert!(!packet.contains("do one thing"), "an asleep task is out of sight: {packet}");
    assert!(packet.contains("- do another\n"), "{packet}");
    assert!(packet.contains("(1 task asleep until "), "and counted, not forgotten: {packet}");
    assert!(packet.contains("Current stage: v0.2.0"), "one task awake keeps the stage: {packet}");

    let out = output(data.path(), &["task", "unsnooze", &id]);
    assert!(out.contains("is awake (it slept until"), "{out}");
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("- do one thing\n") && !packet.contains("asleep"), "{packet}");
}

#[test]
fn a_day_that_has_come_wakes_the_task_by_itself() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let id = task_id(data.path(), "do one thing");
    output(data.path(), &["task", "snooze", &id, "--until", "+3d"]);
    // The day passes: nothing but the date changes.
    db(data.path())
        .execute("UPDATE tasks SET snoozed_until = '2001-01-01' WHERE id = ?1", [&id])
        .unwrap();
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("- do one thing\n") && !packet.contains("asleep"), "{packet}");
}

#[test]
fn a_sleep_must_end_on_a_day_still_to_come() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let id = task_id(data.path(), "do one thing");
    rigger(data.path())
        .args(["task", "snooze", &id, "--until", "today"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not after today"));
    rigger(data.path())
        .args(["task", "snooze", &id, "--until", "someday"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a day"));
    output(data.path(), &["task", "status", &id, "done"]);
    rigger(data.path())
        .args(["task", "snooze", &id, "--until", "3d"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("only work still to do"));
}

#[test]
fn a_stage_with_all_its_work_put_down_is_not_the_stage_being_built() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let one = task_id(data.path(), "do one thing");
    let another = task_id(data.path(), "do another");

    let out = output(data.path(), &["task", "freeze", &one, "--why", "waits for the server"]);
    assert!(out.contains("is frozen (was new)"), "{out}");
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("Current stage: v0.2.0"), "one task still awake holds the stage: {packet}");

    output(data.path(), &["task", "snooze", &another, "--until", "2w"]);
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("Current stage: v0.3.0"), "all put down: the next stage is built: {packet}");
    assert!(packet.contains("- do the third"), "{packet}");
    assert!(packet.contains("(set aside: v0.2.0 · Second (1 frozen, 1 asleep))"), "{packet}");
    let version = output(data.path(), &["version", "show", "sample"]);
    assert!(version.contains("v0.3.0"), "every screen names the same stage: {version}");

    // The reason is on the record, where the next person to find the task
    // will look for it.
    let why: String = db(data.path())
        .query_row("SELECT body FROM events WHERE task_id = ?1 AND kind = 'decision'", [&one], |r| r.get(0))
        .unwrap();
    assert_eq!(why, "Set aside: waits for the server");

    // Taking the frozen one up again brings its stage back.
    output(data.path(), &["task", "status", &one, "active"]);
    let packet = output(data.path(), &["context", "sample"]);
    assert!(
        packet.contains("Current stage: v0.2.0") && packet.contains("- do one thing (active)"),
        "{packet}"
    );
}

#[test]
fn a_stage_whose_tasks_are_all_done_stays_current_until_its_tag() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    for title in ["do one thing", "do another"] {
        output(data.path(), &["task", "status", &task_id(data.path(), title), "done"]);
    }
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("Current stage: v0.2.0"), "finished is not set aside: {packet}");
}

#[test]
fn a_card_asleep_is_out_of_the_list_and_out_of_hand() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    output(data.path(), &["task", "new", "Archived alerts look live", "--id", "ACME-7310"]);
    output(data.path(), &["task", "new", "Export is slow", "--id", "ACME-7311"]);
    output(data.path(), &["task", "open", "ACME-7310"]);

    output(data.path(), &["task", "snooze", "ACME-7310", "--until", "3d"]);
    let active = output(data.path(), &["task", "active"]);
    assert!(active.contains("No card is in hand"), "a card put down is not in hand: {active}");
    let list = output(data.path(), &["task", "list"]);
    assert!(!list.contains("ACME-7310") && list.contains("ACME-7311"), "{list}");
    assert!(list.contains("1 card asleep; `rigger task list --snoozed`"), "{list}");
    let list = output(data.path(), &["task", "list", "--snoozed"]);
    assert!(list.contains("ACME-7310") && list.contains("(asleep until "), "{list}");
    let shown = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(shown.contains("asleep:   until "), "{shown}");

    // Opened again while asleep, it is still not in hand: the hand and the
    // sleep would be two answers to one question.
    output(data.path(), &["task", "open", "ACME-7310"]);
    let active = output(data.path(), &["task", "active"]);
    assert!(active.contains("No card is in hand"), "{active}");
    output(data.path(), &["task", "unsnooze", "ACME-7310"]);
    let active = output(data.path(), &["task", "active"]);
    assert!(active.contains("ACME-7310"), "{active}");
}
