//! A plan made with commands rather than a hub.
//!
//! A stage used to come from a heading in a markdown hub and from nowhere
//! else, so a person without one - anyone outside the line rigger was
//! built in - could not say what comes next. `version add` is that door.

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

fn project(data: &Path) {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
}

#[test]
fn a_stage_and_its_tasks_are_planned_without_a_hub() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());

    let said = output(
        data.path(),
        &[
            "version",
            "add",
            "sample",
            "v0.1.0",
            "--title",
            "First light",
            "--task",
            "read the sheet",
            "--task",
            "write the report",
            "--week",
            "2026-W43",
        ],
    );
    assert!(said.contains("Added v0.1.0 to sample, with 2 tasks"), "{said}");
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("## Current stage: v0.1.0 · First light"), "{packet}");
    assert!(packet.contains("- read the sheet") && packet.contains("- write the report"), "{packet}");

    // Asked again, a stage is found by value and a task by its text: only
    // what is new is added, last in the list.
    let again = output(
        data.path(),
        &["version", "add", "sample", "v0.1", "--task", "read the sheet", "--task", "draw the chart"],
    );
    assert!(again.contains("v0.1.0 was in the plan already; 1 task added"), "{again}");
    let shown: serde_json::Value = serde_json::from_str(&output(data.path(), &["version", "show", "sample", "v0.1.0", "--json"])).unwrap();
    let titles: Vec<&str> = shown["tasks"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["read the sheet", "write the report", "draw the chart"]);
    assert_eq!(shown["week"], "2026-W43");
    assert_eq!(shown["title"], "First light");
}

#[test]
fn what_is_not_a_version_and_what_has_shipped_are_refused() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    rigger(data.path())
        .args(["version", "add", "sample", "next"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a version"));

    rigger(data.path()).args(["version", "add", "sample", "v0.1.0"]).assert().success();
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.1.0"])
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "add", "sample", "v0.1.0", "--task", "one more thing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has shipped"));
}

#[test]
fn a_planned_stage_survives_the_trip_through_a_hub() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    rigger(data.path())
        .args(["version", "add", "sample", "v0.2.0", "--title", "Search", "--task", "an index"])
        .assert()
        .success();
    let hub = data.path().join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&hub).assert().success();
    let plan = std::fs::read_to_string(hub.join("План.md")).unwrap();
    assert!(plan.contains("v0.2.0 · Search") && plan.contains("- [ ] an index"), "{plan}");
    // Read back, the hub says what the record already holds.
    rigger(data.path())
        .args(["import", "sample", "--hub"])
        .arg(&hub)
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
}

#[test]
fn stages_planned_one_after_another_keep_their_order_through_a_hub() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    for (version, title) in [("v0.1.0", "First"), ("v0.2.0", "Second"), ("v0.3.0", "Third")] {
        rigger(data.path())
            .args(["version", "add", "sample", version, "--title", title, "--task", &format!("{title} task")])
            .assert()
            .success();
    }
    let hub = data.path().join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&hub).assert().success();
    let plan = std::fs::read_to_string(hub.join("План.md")).unwrap();
    let at = |what: &str| {
        plan.find(what).unwrap_or_else(|| {
            panic!(
                "{what} is not in the plan:
{plan}"
            )
        })
    };
    assert!(
        at("v0.1.0 · First") < at("v0.2.0 · Second") && at("v0.2.0 · Second") < at("v0.3.0 · Third"),
        "{plan}"
    );
    rigger(data.path())
        .args(["import", "sample", "--hub"])
        .arg(&hub)
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
}
