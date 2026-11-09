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
  {"key":"ACME-1","summary":"Export drops the footer","status":"Open","priority":"Medium","first_seen":"2026-09-20T09:00:00","pinned":false},
  {"key":"ACME-2","summary":"rtf files are not supported","status":"In Progress","priority":"High","score":8.0,"url":"https://tracker.example.com/browse/ACME-2","first_seen":"2026-09-19T09:00:00","taken_at":"2026-09-21T10:00:00","pinned":true},
  {"key":"ACME-3","summary":"Old import bug","status":"Done","gone_at":"2026-09-24T10:00:00"},
  {"key":"ACME-4","summary":"Later, perhaps","status":"Open","snoozed_until":"2999-01-01T09:00:00"}
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
    rigger(data.path()).args(["task", "new", "Old import bug", "--id", "ACME-3"]).assert().success();

    let out = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(
        out.contains("kasl's inbox: 3 tickets - 2 without a card, 1 gone with the card still open"),
        "{out}"
    );
    let two = out.find("ACME-2").expect("ACME-2 is listed");
    let one = out.find("ACME-1").expect("ACME-1 is listed");
    assert!(two < one, "the ticket taken in kasl comes first:\n{out}");
    assert!(!out.contains("ACME-4"), "a sleeping ticket is not waiting:\n{out}");
    assert!(out.contains("rigger task take <KEY>"), "{out}");
    let gone = out.split("The ticket has gone").nth(1).expect("the gone section");
    assert!(gone.contains("ACME-3") && gone.contains("gone 2026-09-24"), "{out}");
    assert!(gone.contains("rigger task close <KEY>"), "{out}");

    let asked = std::fs::read_to_string(&log).unwrap();
    assert_eq!(asked.trim(), "inbox list --all --snoozed --json", "the question is the contract");
}

#[test]
fn a_closed_card_is_not_named_again_when_its_ticket_goes() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    rigger(data.path()).args(["task", "new", "Old import bug", "--id", "ACME-3"]).assert().success();
    rigger(data.path()).args(["task", "close", "ACME-3"]).assert().success();
    let out = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(out.contains("0 gone with the card still open"), "{out}");
    assert!(!out.contains("The ticket has gone"), "{out}");
}

#[test]
fn a_card_is_taken_from_the_inbox_by_its_key_alone() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);

    let out = output(with_kasl(data.path(), &kasl), &["task", "take", "acme-2"]);
    assert!(
        out.contains("Made card ACME-2 from kasl's inbox and opened it: rtf files are not supported"),
        "{out}"
    );
    assert!(out.contains("In Progress · priority High · score 8"), "{out}");
    assert!(out.contains("https://tracker.example.com/browse/ACME-2"), "{out}");
    let active: serde_json::Value = serde_json::from_str(&output(rigger(data.path()), &["task", "active", "--json"])).unwrap();
    assert_eq!(active["key"], "ACME-2");
    assert_eq!(active["title"], "rtf files are not supported");

    // Taking it again makes no second card.
    let again = output(with_kasl(data.path(), &kasl), &["task", "take", "ACME-2"]);
    assert!(again.contains("ACME-2 already has a card"), "{again}");
    let cards: serde_json::Value = serde_json::from_str(&output(rigger(data.path()), &["task", "list", "--status", "all", "--json"])).unwrap();
    assert_eq!(cards.as_array().unwrap().len(), 1);

    let incoming = output(with_kasl(data.path(), &kasl), &["task", "incoming"]);
    assert!(incoming.contains("1 without a card"), "{incoming}");

    with_kasl(data.path(), &kasl)
        .args(["task", "take", "ACME-77"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not know ACME-77").and(predicate::str::contains("rigger task new")));
}

#[test]
fn a_card_shows_its_ticket_and_its_packet_carries_it() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    output(with_kasl(data.path(), &kasl), &["task", "take", "ACME-2"]);

    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "ACME-2"]);
    assert!(
        out.contains("ticket:   In Progress · priority High · score 8 · taken 2026-09-21 (kasl)"),
        "{out}"
    );
    let packet = output(with_kasl(data.path(), &kasl), &["task", "context", "ACME-2"]);
    assert!(packet.contains("## Ticket\nIn Progress · priority High"), "{packet}");
    assert!(packet.contains("https://tracker.example.com/browse/ACME-2"), "{packet}");

    // A card found under an alias is the same ticket.
    rigger(data.path())
        .args(["task", "new", "import", "--id", "LOC-1", "--alias", "ACME-3"])
        .assert()
        .success();
    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "LOC-1"]);
    assert!(out.contains("gone 2026-09-24 - closed or reassigned"), "{out}");
}

#[test]
fn a_kasl_that_cannot_answer_is_said_so_only_where_the_inbox_was_asked_for() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path()).args(["task", "new", "rtf", "--id", "ACME-2"]).assert().success();
    let kasl = old_kasl(data.path());

    with_kasl(data.path(), &kasl)
        .args(["task", "incoming"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("did not list its inbox (unexpected argument '--json' found)"));
    // Taking a ticket kasl cannot give says how to make the card without it.
    with_kasl(data.path(), &kasl)
        .args(["task", "take", "ACME-9"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rigger task new \"<title>\" --id ACME-9"));
    let out = output(with_kasl(data.path(), &kasl), &["task", "show", "ACME-2"]);
    assert!(!out.contains("ticket:") && !out.contains("kasl"), "{out}");

    // No kasl at all: a scratch record reaches none it was not given.
    rigger(data.path())
        .args(["task", "incoming"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no kasl here"));

    // An answer that is not the contract is not read as an empty inbox.
    let (garbled, _) = fake_kasl(data.path(), "[{\"key\":\"ACME-1\"}]");
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
fn a_card_taken_from_the_inbox_shows_its_ticket_and_tray_as_json() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, _) = fake_kasl(data.path(), INBOX);
    output(with_kasl(data.path(), &kasl), &["task", "take", "ACME-2"]);
    output(rigger(data.path()), &["tray", "show"]);
    std::fs::write(data.path().join("profiles/line/trays/ACME-2/shot.png"), "x").unwrap();
    output(rigger(data.path()), &["tray", "done"]);
    std::fs::write(data.path().join("profiles/line/trays/ACME-2/shot.png"), "x").unwrap();

    // The fields themselves are held to task.md by the JSON contract gate
    // (tests/json_contract.rs); this holds the values.
    let show: serde_json::Value = serde_json::from_str(&output(with_kasl(data.path(), &kasl), &["task", "show", "ACME-2", "--json"])).unwrap();
    assert_eq!(show["ticket"]["key"], "ACME-2");
    assert_eq!(show["tray"]["files"][0]["path"], "shot.png");
}

/// Five tickets taken in kasl, none with a card yet, beside one that is not
/// taken: more than the inbox names in its line.
const SIX_TAKEN: &str = r#"{"issues":[
  {"key":"ACME-11","summary":"one","taken_at":"2026-09-21T10:00:00"},
  {"key":"ACME-12","summary":"two","taken_at":"2026-09-20T10:00:00"},
  {"key":"ACME-13","summary":"three","taken_at":"2026-09-22T10:00:00"},
  {"key":"ACME-14","summary":"four","taken_at":"2026-09-23T10:00:00"},
  {"key":"ACME-15","summary":"five","taken_at":"2026-09-24T10:00:00"},
  {"key":"ACME-16","summary":"six","taken_at":"2026-09-25T10:00:00"},
  {"key":"ACME-17","summary":"not taken"}
]}"#;

fn inbox_json(cmd: TestCommand) -> serde_json::Value {
    serde_json::from_str(&output(cmd, &["inbox", "--json"])).unwrap()
}

/// The owner looks at the inbox for everything waiting; at a desk that
/// includes the tickets they took in kasl and never made a card for. Only a
/// ticket profile asks, and only for the whole inbox - a ticket is the
/// desk's, not one project's. The taken ones only: a ticket nobody took is
/// not yet anybody's work.
#[test]
fn a_ticket_profiles_inbox_names_the_tickets_taken_in_kasl_without_a_card() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let (kasl, log) = fake_kasl(data.path(), INBOX);

    // The line's inbox is about questions, and does not ask kasl at all.
    let out = output(with_kasl(data.path(), &kasl), &["inbox"]);
    assert_eq!(out.trim(), "Nothing is waiting on you.", "{out}");
    let json = inbox_json(with_kasl(data.path(), &kasl));
    assert!(json["taken_without_card"].is_null() && json["kasl_said"].is_null(), "{json}");
    assert!(!log.exists(), "a line profile asked kasl");

    output(rigger(data.path()), &["profile", "add", "desk", "--kind", "tickets", "--use"]);
    let out = output(with_kasl(data.path(), &kasl), &["inbox"]);
    assert_eq!(out.trim(), "1 ticket taken in kasl has no card: ACME-2 - rigger task incoming", "{out}");
    let json = inbox_json(with_kasl(data.path(), &kasl));
    let taken = json["taken_without_card"].as_array().expect("an array once kasl was read");
    assert_eq!(taken.len(), 1, "{json}");
    assert_eq!(taken[0]["key"], "ACME-2");
    assert_eq!(taken[0]["summary"], "rtf files are not supported");
    assert_eq!(taken[0]["taken_at"], "2026-09-21T10:00:00");
    assert!(json["kasl_said"].is_null(), "{json}");

    // Beside a question, after it; with one project named, not asked.
    let root = data.path().join("alpha");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    output(rigger(data.path()), &["note", "alpha", "Which sheet comes first?", "--kind", "question"]);
    let out = output(with_kasl(data.path(), &kasl), &["inbox"]);
    let question = out.find("Which sheet comes first?").expect("the question");
    let ticket = out.find("1 ticket taken in kasl").expect("the ticket line");
    assert!(question < ticket, "{out}");
    let out = output(with_kasl(data.path(), &kasl), &["inbox", "--project", "alpha"]);
    assert!(!out.contains("kasl"), "{out}");
    assert!(inbox_json(with_kasl(data.path(), &kasl))["taken_without_card"].is_array());
    let json: serde_json::Value = serde_json::from_str(&output(with_kasl(data.path(), &kasl), &["inbox", "--project", "alpha", "--json"])).unwrap();
    assert!(json["taken_without_card"].is_null(), "{json}");
    rigger(data.path())
        .args(["resolve", "alpha", &question_id(&json), "the first"])
        .assert()
        .success();

    // Its card made, it is not waiting any more.
    rigger(data.path()).args(["task", "new", "rtf", "--id", "ACME-2"]).assert().success();
    let out = output(with_kasl(data.path(), &kasl), &["inbox"]);
    assert_eq!(out.trim(), "Nothing is waiting on you.", "{out}");
    let json = inbox_json(with_kasl(data.path(), &kasl));
    assert_eq!(json["taken_without_card"], serde_json::json!([]), "read, and none: {json}");

    // More than the line names: the longest taken first, the rest counted.
    let (many, _) = fake_kasl(data.path(), SIX_TAKEN);
    let out = output(with_kasl(data.path(), &many), &["inbox"]);
    assert_eq!(
        out.trim(),
        "6 tickets taken in kasl have no card: ACME-12, ACME-11, ACME-13, ACME-14, ACME-15 and 1 more - rigger task incoming",
        "{out}"
    );
    assert_eq!(inbox_json(with_kasl(data.path(), &many))["taken_without_card"].as_array().unwrap().len(), 6);
}

fn question_id(inbox: &serde_json::Value) -> String {
    inbox["waiting"][0]["id"].as_i64().expect("a question").to_string()
}

/// The questions do not need kasl, so the inbox never fails for want of it.
#[test]
fn a_ticket_profiles_inbox_stands_without_kasl() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(rigger(data.path()), &["profile", "add", "desk", "--kind", "tickets", "--use"]);

    // No kasl here: nothing to say about it.
    let out = output(rigger(data.path()), &["inbox"]);
    assert_eq!(out.trim(), "Nothing is waiting on you.", "{out}");
    let json = inbox_json(rigger(data.path()));
    assert!(json["taken_without_card"].is_null() && json["kasl_said"].is_null(), "{json}");

    // A kasl that cannot list its inbox is said so, in a line.
    let old = old_kasl(data.path());
    let out = output(with_kasl(data.path(), &old), &["inbox"]);
    assert_eq!(
        out.trim(),
        "kasl did not list its inbox (unexpected argument '--json' found): the tickets taken there are not counted",
        "{out}"
    );
    let json = inbox_json(with_kasl(data.path(), &old));
    assert!(json["taken_without_card"].is_null(), "{json}");
    assert_eq!(json["kasl_said"], "unexpected argument '--json' found");

    // An answer that is not the contract is not an empty inbox either.
    let (garbled, _) = fake_kasl(data.path(), "[{\"key\":\"ACME-1\"}]");
    let out = output(with_kasl(data.path(), &garbled), &["inbox"]);
    assert!(out.contains("not the inbox rigger reads"), "{out}");
}
