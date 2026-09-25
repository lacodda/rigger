//! Task cards: the desk's unit of work.
//!
//! What the release promises: a line of text finds the card it means and
//! says how sure it is; a card is made, opened, linked, written against
//! and closed without a folder of markdown; the packet of a card holds
//! everything written against it; and an id that changed stays findable.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command as Process, Stdio};

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

/// A desk with one repository the cards can be worked in.
fn desk(data: &Path) {
    rigger(data).arg("init").assert().success();
    let repo = data.join("webapp");
    std::fs::create_dir_all(&repo).unwrap();
    rigger(data).args(["project", "add"]).arg(&repo).assert().success();
}

#[test]
fn a_card_is_made_with_a_ticket_id_or_a_local_key() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());

    let out = output(
        data.path(),
        &[
            "task",
            "new",
            "rtf files are not supported",
            "--id",
            "acme-7310",
            "--project",
            "webapp",
            "--branch",
            "fix/rtf",
        ],
    );
    assert!(out.contains("Made card ACME-7310 and opened it"), "{out}");
    assert!(out.contains("webapp") && out.contains("on fix/rtf"), "{out}");

    let local = output(data.path(), &["task", "new", "add a filter to the report"]);
    assert!(local.contains("Made card LOCAL-"), "{local}");
    assert!(local.contains("rename"), "a local key says how to replace itself: {local}");

    let list = output(data.path(), &["task", "list"]);
    assert!(list.contains("ACME-7310") && list.contains("LOCAL-"), "{list}");
    assert!(list.contains("new "), "{list}");

    rigger(data.path())
        .args(["task", "new", "again", "--id", "ACME-7310"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already names a card"));
}

#[test]
fn find_says_take_ask_or_new() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(
        data.path(),
        &["task", "new", "Не поддерживаются файлы rtf в родном формате", "--id", "ACME-7310"],
    );
    output(data.path(), &["task", "new", "Экспорт отчёта в файл rtf", "--id", "ACME-4200"]);

    let by_id = output(data.path(), &["task", "find", "ACME-7310 что-то совсем другое"]);
    assert!(by_id.contains("take ACME-7310"), "{by_id}");
    assert!(by_id.contains("id=ACME-7310"), "{by_id}");

    let by_title = output(data.path(), &["task", "find", "не поддерживаются файлы rtf"]);
    assert!(by_title.contains("take ACME-7310"), "{by_title}");

    let close = output(data.path(), &["task", "find", "экспорт файла"]);
    assert!(close.starts_with(" ") || close.contains("ACME-4200"), "{close}");
    assert!(close.contains("ask:"), "{close}");

    let nothing = output(data.path(), &["task", "find", "добавить новый фильтр в отчёт"]);
    assert!(nothing.contains("new:"), "{nothing}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["task", "find", "ACME-4200", "--json"])).unwrap();
    assert_eq!(json["verdict"], "take");
    assert_eq!(json["hits"][0]["key"], "ACME-4200");
}

#[test]
fn a_renamed_card_is_found_by_its_old_id_and_by_a_mention() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "rtf files", "--id", "ACME-7310"]);
    let renamed = output(data.path(), &["task", "rename", "ACME-7310", "api-77"]);
    assert!(renamed.contains("ACME-7310 is now API-77"), "{renamed}");

    let found = output(data.path(), &["task", "find", "ACME-7310"]);
    assert!(found.contains("take API-77"), "the old id is an alias: {found}");
    let show = output(data.path(), &["task", "show", "acme-7310"]);
    assert!(show.starts_with("API-77 · rtf files") && show.contains("aliases:  ACME-7310"), "{show}");

    // A case number mentioned only in what was written against a card.
    output(data.path(), &["task", "new", "another card", "--id", "ACME-9"]);
    output(
        data.path(),
        &["task", "note", "ACME-9", "--kind", "finding", "the customer case 561207 is the same defect"],
    );
    let mention = output(data.path(), &["task", "find", "561207"]);
    assert!(mention.contains("mentioned 561207: ACME-9"), "{mention}");
}

#[test]
fn a_card_is_opened_written_against_and_closed() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "rtf files", "--id", "ACME-7310"]);
    output(data.path(), &["task", "new", "other", "--id", "ACME-1"]);
    // The last card made is in hand; opening another replaces it.
    assert!(output(data.path(), &["task", "active"]).starts_with("ACME-1"));
    output(data.path(), &["task", "open", "acme-7310"]);
    assert!(output(data.path(), &["task", "active"]).starts_with("ACME-7310"));

    output(
        data.path(),
        &["task", "note", "ACME-7310", "--kind", "decision", "Fix the parser, not the export."],
    );
    output(
        data.path(),
        &["task", "note", "ACME-7310", "--kind", "plan", "Add the rtf branch to the loader."],
    );
    output(data.path(), &["task", "note", "ACME-7310", "--kind", "next", "Write the loader test first."]);
    output(
        data.path(),
        &["task", "link", "ACME-7310", "webapp", "--branch", "fix/rtf", "--role", "where it is fixed"],
    );

    let packet = output(data.path(), &["task", "context", "ACME-7310"]);
    assert!(packet.starts_with("# ACME-7310 · rtf files"), "{packet}");
    assert!(
        packet.contains("## Worked in\n- webapp") && packet.contains("on `fix/rtf` - where it is fixed"),
        "{packet}"
    );
    assert!(packet.contains("## Next step\nWrite the loader test first."), "{packet}");
    assert!(packet.contains("## Decisions\n") && packet.contains("Fix the parser"), "{packet}");
    assert!(packet.contains("## Plan\n") && packet.contains("rtf branch"), "{packet}");

    let show = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(show.contains("recorded: 1 decision, 1 next, 1 plan"), "{show}");
    assert!(show.contains("next:     Write the loader test first."), "{show}");

    // Closing the card in hand needs no name, and leaves nothing in hand.
    let closed = output(data.path(), &["task", "close"]);
    assert!(closed.contains("Closed ACME-7310 as done (was new)"), "{closed}");
    assert!(output(data.path(), &["task", "active"]).contains("No card is in hand"));
    assert!(!output(data.path(), &["task", "list"]).contains("ACME-7310"), "done leaves the open list");
    assert!(output(data.path(), &["task", "list", "--status", "all"]).contains("ACME-7310"));

    output(data.path(), &["task", "status", "ACME-1", "waiting-handoff"]);
    assert!(output(data.path(), &["task", "list"]).contains("waiting-handoff"));
    rigger(data.path())
        .args(["task", "close"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no card is in hand"));
}

/// The MCP door: a card is found and read, and an event written against
/// it lands on the card.
#[test]
fn over_mcp_a_card_is_found_read_and_written_against() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "rtf files are not supported", "--id", "ACME-7310"]);

    let mut child = Process::new(assert_cmd::cargo::cargo_bin("rigger"))
        .arg("mcp")
        .env("RIGGER_DATA_DIR", data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut call = |id: u64, method: &str, params: serde_json::Value| -> serde_json::Value {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(stdin, "{msg}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    call(
        1,
        "initialize",
        serde_json::json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
    );
    let text = |v: &serde_json::Value| v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();

    let found = text(&call(
        2,
        "tools/call",
        serde_json::json!({ "name": "task_find", "arguments": { "query": "acme-7310 rtf" } }),
    ));
    assert!(found.starts_with("take ACME-7310"), "{found}");

    let written = text(&call(
        3,
        "tools/call",
        serde_json::json!({ "name": "record_finding", "arguments": { "task": "ACME-7310", "text": "The loader drops rtf by extension." } }),
    ));
    assert!(written.contains("Recorded a finding on ACME-7310"), "{written}");
    let next = text(&call(
        4,
        "tools/call",
        serde_json::json!({ "name": "set_next_step", "arguments": { "task": "ACME-7310", "text": "Read the loader." } }),
    ));
    assert!(next.contains("ACME-7310"), "{next}");

    let packet = text(&call(
        5,
        "tools/call",
        serde_json::json!({ "name": "task_context", "arguments": { "task": "ACME-7310" } }),
    ));
    assert!(packet.contains("# ACME-7310 · rtf files are not supported"), "{packet}");
    assert!(packet.contains("## Findings") && packet.contains("drops rtf by extension"), "{packet}");
    assert!(packet.contains("## Next step\nRead the loader."), "{packet}");

    let refused = call(
        6,
        "tools/call",
        serde_json::json!({ "name": "record_finding", "arguments": { "task": "ACME-0", "text": "x" } }),
    );
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    drop(stdin);
    let _ = child.wait();
}
