//! Handing a card over to the team that owns the fix.
//!
//! What the release promises: the first run writes a form into the card's
//! tray with what the record knows; a form left blank is refused by name;
//! a filled one becomes the same text twice - markdown and Jira markup -
//! the card waits on a handoff and says so; the tray does not take the
//! handoff for material to sort; and the card's screens name what went out.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env_remove("RIGGER_INTAKE_DIRS");
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn handoff_dir(data: &Path) -> PathBuf {
    data.join("profiles").join("line").join("trays").join("ACME-7310").join("handoff")
}

/// The one form in the handoff folder.
fn form(data: &Path) -> PathBuf {
    let found: Vec<PathBuf> = std::fs::read_dir(handoff_dir(data))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".form.md"))
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    found.into_iter().next().unwrap()
}

fn fill(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let text = text
        .replace("## Ask (required)\n", "## Ask (required)\n\nPlease return whether an alert is archived.\n")
        .replace("## Now (required)\n", "## Now (required)\n\nThe client gets no such flag and **guesses**.\n")
        .replace("## Needed (required)\n", "## Needed (required)\n\nAn explicit field, `isArchived`.\n")
        .replace("## Why (required)\n", "## Why (required)\n\nWhy: the archive cannot be shown or filtered.\n");
    std::fs::write(path, text).unwrap();
}

fn desk(data: &Path) {
    rigger(data).arg("init").assert().success();
    output(data, &["task", "new", "Archived alerts look live", "--id", "ACME-7310"]);
    output(data, &["task", "note", "ACME-7310", "the list response has no archive flag"]);
}

#[test]
fn a_card_is_handed_over_through_a_form() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());

    let out = output(data.path(), &["task", "handoff", "--to", "Alerts API"]);
    assert!(out.contains("Wrote the form") && out.contains("alerts-api"), "{out}");
    let path = form(data.path());
    let blank = std::fs::read_to_string(&path).unwrap();
    assert!(blank.contains("\nArchived alerts look live\n"), "the record fills in the title: {blank}");
    assert!(
        blank.contains("> - ") && blank.contains("the list response has no archive flag"),
        "and quotes the material: {blank}"
    );

    rigger(data.path())
        .args(["task", "handoff", "ACME-7310", "--to", "alerts-api"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Ask, Now, Needed, Why empty"));
    let shown = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(
        shown.contains("status:   active") || shown.contains("status:   new"),
        "a refused handoff changes nothing: {shown}"
    );

    fill(&path);
    let out = output(data.path(), &["task", "handoff", "ACME-7310", "--to", "alerts-api"]);
    assert!(out.starts_with("### Archived alerts look live\n"), "{out}");
    assert!(out.contains("is now waiting-handoff"), "{out}");
    let markdown = std::fs::read_to_string(path.to_string_lossy().replace(".form.md", ".md")).unwrap();
    let jira = std::fs::read_to_string(path.to_string_lossy().replace(".form.md", ".jira.txt")).unwrap();
    assert!(markdown.contains("**guesses**") && markdown.contains("Related task: ACME-7310."), "{markdown}");
    assert!(!markdown.contains("archive flag"), "the material is not sent: {markdown}");
    assert!(
        jira.starts_with("h3. Archived alerts look live") && jira.contains("*guesses*") && jira.contains("{{isArchived}}"),
        "{jira}"
    );

    let shown = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(shown.contains("status:   waiting-handoff"), "{shown}");
    assert!(shown.contains("handoff:  to alerts-api on "), "{shown}");
    let packet = output(data.path(), &["task", "context", "ACME-7310"]);
    assert!(packet.contains("## Handed over\n- to alerts-api on "), "{packet}");
    assert!(
        packet.contains("Handed over to alerts-api: Archived alerts look live (handoff/alerts-api-"),
        "{packet}"
    );

    // What went out is not material that came in: sorting the tray leaves
    // it. A screenshot waits beside it, so the sorting really happens - an
    // empty tray is not sorted at all and would prove nothing.
    std::fs::write(handoff_dir(data.path()).parent().unwrap().join("shot.png"), b"png").unwrap();
    let tray = output(data.path(), &["tray", "show", "ACME-7310"]);
    assert!(tray.contains("shot.png") && !tray.contains("handoff/"), "{tray}");
    let sorted = output(data.path(), &["tray", "done", "ACME-7310"]);
    assert!(sorted.contains("1 entry moved") || sorted.contains("1 entries moved"), "{sorted}");
    assert!(path.is_file(), "the handoff stays where it is");

    // Run again, it makes the same texts and records nothing twice.
    output(data.path(), &["task", "handoff", "ACME-7310", "--to", "alerts-api"]);
    let packet = output(data.path(), &["task", "context", "ACME-7310"]);
    assert_eq!(packet.matches("Handed over to alerts-api").count(), 1, "{packet}");
}

#[test]
fn a_long_text_without_a_tldr_is_refused() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "handoff", "ACME-7310", "--to", "desktop"]);
    let path = form(data.path());
    fill(&path);
    let text = std::fs::read_to_string(&path).unwrap().replace(
        "## Example (optional)\n",
        &format!("## Example (optional)\n\n{}", "A line of the example.\n".repeat(14)),
    );
    std::fs::write(&path, &text).unwrap();
    rigger(data.path())
        .args(["task", "handoff", "ACME-7310", "--to", "desktop"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no TL;DR"));
    let text = text.replace(
        "## TL;DR (required when the text runs long)\n",
        "## TL;DR (required when the text runs long)\n\nThe list needs an archive flag.\n",
    );
    std::fs::write(&path, text).unwrap();
    let out = output(data.path(), &["task", "handoff", "ACME-7310", "--to", "desktop"]);
    assert!(out.contains("**TL;DR:** The list needs an archive flag."), "{out}");
}

#[test]
fn a_handoff_needs_someone_to_go_to() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path())
        .args(["task", "handoff", "ACME-7310", "--to", "//"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("say who it goes to"));
}
