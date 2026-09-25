//! Tickets, read from kasl's inbox.
//!
//! What the release promises: rigger does not ask the tracker a second time;
//! it reads what kasl already holds, through `kasl inbox list --all
//! --snoozed --json`. The tickets without a card are named - the ones taken
//! in kasl first, the sleeping ones not at all - and so are the open cards
//! whose ticket has gone; `task take` makes a card from a ticket by its key
//! alone; a card shows its ticket's facts. A kasl that cannot answer is
//! said so where the person asked for the inbox, and silence where they
//! only looked at a card.
//!
//! kasl here is a stand-in named in the environment: it prints the answer
//! the test wrote and writes down how it was asked.

use std::path::{Path, PathBuf};

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env_remove("RIGGER_KASL");
    cmd
}

fn with_kasl(data: &Path, kasl: &Path) -> TestCommand {
    let mut cmd = rigger(data);
    cmd.env("RIGGER_KASL", kasl);
    cmd
}

fn output(mut cmd: TestCommand, args: &[&str]) -> String {
    let out = cmd.args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// A program at `dir/name` that runs the lines given, as a batch file on
/// Windows and a shell script elsewhere.
fn program(dir: &Path, name: &str, windows: &str, unix: &str) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join(format!("{name}.cmd"));
        std::fs::write(&path, format!("@echo off\r\n{}\r\n", windows.replace('\n', "\r\n"))).unwrap();
        path
    } else {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{unix}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

/// Four tickets: one waiting, one taken in kasl, one gone from the tracker,
/// one asleep until a day that will not come.
const INBOX: &str = r#"{"issues":[
  {"key":"WA-1","summary":"Export drops the footer","status":"Open","priority":"Medium","first_seen":"2026-09-20T09:00:00","pinned":false},
  {"key":"WA-2","summary":"rtf files are not supported","status":"In Progress","priority":"High","score":8.0,"url":"https://tracker.example.com/browse/WA-2","first_seen":"2026-09-19T09:00:00","taken_at":"2026-09-21T10:00:00","pinned":true},
  {"key":"WA-3","summary":"Old import bug","status":"Done","gone_at":"2026-09-24T10:00:00"},
  {"key":"WA-4","summary":"Later, perhaps","status":"Open","snoozed_until":"2999-01-01T09:00:00"}
]}"#;

/// A kasl that answers with `inbox`, and writes down its arguments.
fn fake_kasl(dir: &Path, inbox: &str) -> (PathBuf, PathBuf) {
    let answer = dir.join("inbox.json");
    std::fs::write(&answer, inbox).unwrap();
    let log = dir.join("kasl.log");
    let kasl = program(
        dir,
        "kasl",
        &format!("echo %*>>\"{}\"\ntype \"{}\"", log.display(), answer.display()),
        &format!("echo \"$@\" >> '{}'\ncat '{}'", log.display(), answer.display()),
    );
    (kasl, log)
}

/// A kasl of today: its inbox has no `--json`, and clap says so.
fn old_kasl(dir: &Path) -> PathBuf {
    program(
        dir,
        "kasl",
        "echo error: unexpected argument '--json' found 1>&2\nexit /b 2",
        "echo \"error: unexpected argument '--json' found\" >&2; exit 2",
    )
}

fn desk(data: &Path) {
    rigger(data).arg("init").assert().success();
}

#[test]
fn the_tickets_without_a_card_are_named_taken_first_and_sleeping_ones_not_at_all() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, log) = fake_kasl(data.path(), INBOX);
    rigger(data.path()).args(["task", "new", "Old import bug", "--id", "WA-3"]).assert().success();

    let out = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(
        out.contains("kasl's inbox: 3 tickets - 2 without a card, 1 gone with the card still open"),
        "{out}"
    );
    let two = out.find("WA-2").expect("WA-2 is listed");
    let one = out.find("WA-1").expect("WA-1 is listed");
    assert!(two < one, "the ticket taken in kasl comes first:\n{out}");
    assert!(!out.contains("WA-4"), "a sleeping ticket is not waiting:\n{out}");
    assert!(out.contains("rigger task take <KEY>"), "{out}");
    let gone = out.split("The ticket has gone").nth(1).expect("the gone section");
    assert!(gone.contains("WA-3") && gone.contains("gone 2026-09-24"), "{out}");
    assert!(gone.contains("rigger task close <KEY>"), "{out}");

    let asked = std::fs::read_to_string(&log).unwrap();
    assert_eq!(asked.trim(), "inbox list --all --snoozed --json", "the question is the contract");
}

#[test]
fn a_closed_card_is_not_named_again_when_its_ticket_goes() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    rigger(data.path()).args(["task", "new", "Old import bug", "--id", "WA-3"]).assert().success();
    rigger(data.path()).args(["task", "close", "WA-3"]).assert().success();
    let out = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(out.contains("0 gone with the card still open"), "{out}");
    assert!(!out.contains("The ticket has gone"), "{out}");
}

#[test]
fn a_card_is_taken_from_the_inbox_by_its_key_alone() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);

    let out = output(with_kasl(data.path(), &kasl), &["task", "take", "wa-2"]);
    assert!(
        out.contains("Made card WA-2 from kasl's inbox and opened it: rtf files are not supported"),
        "{out}"
    );
    assert!(out.contains("In Progress · priority High · score 8"), "{out}");
    assert!(out.contains("https://tracker.example.com/browse/WA-2"), "{out}");
    let active: serde_json::Value = serde_json::from_str(&output(rigger(data.path()), &["task", "active", "--json"])).unwrap();
    assert_eq!(active["key"], "WA-2");
    assert_eq!(active["title"], "rtf files are not supported");

    // Taking it again makes no second card.
    let again = output(with_kasl(data.path(), &kasl), &["task", "take", "WA-2"]);
    assert!(again.contains("WA-2 already has a card"), "{again}");
    let cards: serde_json::Value = serde_json::from_str(&output(rigger(data.path()), &["task", "list", "--status", "all", "--json"])).unwrap();
    assert_eq!(cards.as_array().unwrap().len(), 1);

    let incoming = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(incoming.contains("1 without a card"), "{incoming}");

    with_kasl(data.path(), &kasl)
        .args(["task", "take", "WA-77"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not know WA-77").and(predicate::str::contains("rigger task new")));
}

#[test]
fn a_card_shows_its_ticket_and_its_packet_carries_it() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    output(with_kasl(data.path(), &kasl), &["task", "take", "WA-2"]);

    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "WA-2"]);
    assert!(
        out.contains("ticket:   In Progress · priority High · score 8 · taken 2026-09-21 (kasl)"),
        "{out}"
    );
    let packet = output(with_kasl(data.path(), &kasl), &["task", "context", "WA-2"]);
    assert!(packet.contains("## Ticket\nIn Progress · priority High"), "{packet}");
    assert!(packet.contains("https://tracker.example.com/browse/WA-2"), "{packet}");

    // A card found under an alias is the same ticket.
    rigger(data.path())
        .args(["task", "new", "import", "--id", "LOC-1", "--alias", "WA-3"])
        .assert()
        .success();
    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "LOC-1"]);
    assert!(out.contains("gone 2026-09-24 - closed or reassigned"), "{out}");
}

#[test]
fn a_kasl_that_cannot_answer_is_said_so_only_where_the_inbox_was_asked_for() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path()).args(["task", "new", "rtf", "--id", "WA-2"]).assert().success();
    let kasl = old_kasl(data.path());

    with_kasl(data.path(), &kasl)
        .args(["task", "incoming"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("did not list its inbox (unexpected argument '--json' found)"));
    // Taking a ticket kasl cannot give says how to make the card without it.
    with_kasl(data.path(), &kasl)
        .args(["task", "take", "WA-9"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rigger task new \"<title>\" --id WA-9"));
    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "WA-2"]);
    assert!(!out.contains("ticket:") && !out.contains("kasl"), "{out}");

    // No kasl at all: a scratch record reaches none it was not given.
    rigger(data.path())
        .args(["task", "incoming"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no kasl here"));

    // An answer that is not the contract is not read as an empty inbox.
    let (garbled, _) = fake_kasl(data.path(), "[{\"key\":\"WA-1\"}]");
    with_kasl(data.path(), &garbled)
        .args(["task", "incoming"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not the inbox rigger reads"));
}

#[test]
fn an_inbox_larger_than_a_pipe_is_read_whole() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    // Some 160 KB: past the pipe buffer of every platform the tests run on.
    let filler = "a summary long enough to fill a pipe quickly ".repeat(8);
    let issues: Vec<String> = (1..=400)
        .map(|n| format!(r#"{{"key":"BIG-{n}","summary":"{filler}{n}","status":"Open"}}"#))
        .collect();
    let (kasl, _) = fake_kasl(data.path(), &format!("{{\"issues\":[{}]}}", issues.join(",\n")));
    let json: serde_json::Value = serde_json::from_str(&output(with_kasl(data.path(), &kasl), &["task", "incoming", "--json"])).unwrap();
    assert_eq!(json["issues"].as_array().unwrap().len(), 400);
    let out = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(out.contains("and 390 more: rigger task incoming --all"), "{out}");
}

#[test]
fn the_json_of_the_inbox_and_of_a_card_is_the_documented_contract() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    output(with_kasl(data.path(), &kasl), &["task", "take", "WA-2"]);
    output(rigger(data.path()), &["tray", "show"]);
    std::fs::write(data.path().join("profiles/line/trays/WA-2/shot.png"), "x").unwrap();
    output(rigger(data.path()), &["tray", "done"]);
    std::fs::write(data.path().join("profiles/line/trays/WA-2/shot.png"), "x").unwrap();

    let page = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/content/docs/reference/task.md")).unwrap();
    for args in [&["task", "incoming", "--json"][..], &["task", "show", "WA-2", "--json"][..]] {
        let json: serde_json::Value = serde_json::from_str(&output(with_kasl(data.path(), &kasl), args)).unwrap();
        let mut found = std::collections::BTreeSet::new();
        keys(&json, &mut found);
        let missing: Vec<&String> = found.iter().filter(|k| !page.contains(&format!("`{k}`"))).collect();
        assert!(missing.is_empty(), "task.md does not name these fields of {args:?}: {missing:?}\n{json:#}");
    }
    let show: serde_json::Value = serde_json::from_str(&output(with_kasl(data.path(), &kasl), &["task", "show", "WA-2", "--json"])).unwrap();
    assert_eq!(show["ticket"]["key"], "WA-2");
    assert_eq!(show["tray"]["files"][0]["path"], "shot.png");
}

fn keys(value: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                out.insert(key.clone());
                keys(inner, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| keys(item, out)),
        _ => {}
    }
}
