//! `rigger rules`: how the work is done, read from the record.
//!
//! The line's rituals and a project's are two documents, not one glued
//! copy per skill file. These check that both are found, that each is found
//! without the other, and that a project of the profile's own name cannot
//! quietly become the place the line's rituals live.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn repo(root: &Path, name: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n")).unwrap();
}

/// A record with one project, ready for documents.
fn record(data: &Path, work: &Path, name: &str) {
    rigger(data).arg("init").assert().success();
    repo(&work.join(name), name);
    rigger(data)
        .args(["project", "add"])
        .arg(work.join(name))
        .args(["--name", name])
        .assert()
        .success();
}

fn write_rituals(data: &Path, project: &str, title: &str, body: &str) {
    rigger(data)
        .args(["doc", "add", project, title, "--kind", "rituals", "--body", body])
        .assert()
        .success();
}

#[test]
fn the_line_and_the_project_are_printed_together() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "widget");

    write_rituals(data.path(), "line", "Rituals", "A stage ends in a tag.");
    write_rituals(data.path(), "widget", "Rituals", "Widgets are measured before they are claimed.");

    rigger(data.path())
        .args(["rules", "widget"])
        .assert()
        .success()
        .stdout(predicate::str::contains("A stage ends in a tag.").and(predicate::str::contains("Widgets are measured")));

    // Without a project named, only the line's - a rule of the line is not
    // a rule of some project that happened to be asked about.
    rigger(data.path())
        .arg("rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("A stage ends in a tag.").and(predicate::str::contains("Widgets are measured").not()));
}

#[test]
fn a_project_without_its_own_still_gets_the_lines() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "widget");
    write_rituals(data.path(), "line", "Rituals", "A stage ends in a tag.");

    rigger(data.path())
        .args(["rules", "widget"])
        .assert()
        .success()
        .stdout(predicate::str::contains("A stage ends in a tag."));
}

#[test]
fn with_nothing_written_the_way_to_write_it_is_the_whole_message() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "widget");

    rigger(data.path())
        .args(["rules", "widget"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rigger doc add line").and(predicate::str::contains("rigger doc add widget")));
}

/// The address a document lands on comes from its title, and an English
/// title is the only one that makes `rituals` out of itself. A Russian one
/// falls through to a made-up address, an imported one keeps the address
/// the hub gave it, and `--slug` says outright. `rules` asks by kind so
/// that none of that decides whether the rituals of the line are found.
#[test]
fn rituals_are_found_by_kind_not_by_the_address_a_title_happens_to_make() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "widget");

    rigger(data.path())
        .args([
            "doc",
            "add",
            "line",
            "Ритуалы линейки",
            "--kind",
            "rituals",
            "--slug",
            "how-the-line-works",
            "--body",
            "A stage ends in a tag.",
        ])
        .assert()
        .success();

    // The address is deliberately not the kind: looking the document up by
    // slug would find nothing here, and the rituals of the line would go
    // silently missing from every sitting that asked for them.
    let listed = rigger(data.path()).args(["doc", "list", "line", "--json"]).output().unwrap();
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed[0]["slug"], "how-the-line-works");
    assert_eq!(listed[0]["kind"], "rituals");

    rigger(data.path())
        .arg("rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("A stage ends in a tag."));
}

#[test]
fn json_names_both_halves() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "widget");
    write_rituals(data.path(), "line", "Rituals", "A stage ends in a tag.");

    let out = rigger(data.path()).args(["rules", "widget", "--json"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["line"], "A stage ends in a tag.");
    assert_eq!(v["project"], "widget");
    assert!(v["project_rules"].is_null(), "widget has written none of its own");
}

/// The line's rituals hang off a service project named after the profile.
/// A repository of that name is a collision, and a collision that is
/// guessed around files the rules of a line under one product.
#[test]
fn a_repository_named_after_the_profile_is_refused_rather_than_used() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    record(data.path(), work.path(), "line");

    // `rules` reads, and a repository called `line` is simply not the
    // service project, so nothing of the line is found.
    rigger(data.path())
        .arg("rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("has not written down how the work is done"));
}
