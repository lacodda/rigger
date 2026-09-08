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
            "wa-4130",
            "--project",
            "webapp",
            "--branch",
            "fix/rtf",
        ],
    );
    assert!(out.contains("Made card WA-4130 and opened it"), "{out}");
    assert!(out.contains("webapp") && out.contains("on fix/rtf"), "{out}");

    let local = output(data.path(), &["task", "new", "add a filter to the report"]);
    assert!(local.contains("Made card LOCAL-"), "{local}");
    assert!(local.contains("rename"), "a local key says how to replace itself: {local}");

    let list = output(data.path(), &["task", "list"]);
    assert!(list.contains("WA-4130") && list.contains("LOCAL-"), "{list}");
    assert!(list.contains("new "), "{list}");

    rigger(data.path())
        .args(["task", "new", "again", "--id", "WA-4130"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already names a card"));
}

#[test]
fn find_says_take_ask_or_new() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "Не поддерживаются файлы rtf в родном формате", "--id", "WA-4130"]);
    output(data.path(), &["task", "new", "Экспорт отчёта в файл rtf", "--id", "WA-4200"]);

    let by_id = output(data.path(), &["task", "find", "WA-4130 что-то совсем другое"]);
    assert!(by_id.contains("take WA-4130"), "{by_id}");
    assert!(by_id.contains("id=WA-4130"), "{by_id}");

    let by_title = output(data.path(), &["task", "find", "не поддерживаются файлы rtf"]);
    assert!(by_title.contains("take WA-4130"), "{by_title}");

    let close = output(data.path(), &["task", "find", "экспорт файла"]);
    assert!(close.starts_with(" ") || close.contains("WA-4200"), "{close}");
    assert!(close.contains("ask:"), "{close}");

    let nothing = output(data.path(), &["task", "find", "добавить новый фильтр в отчёт"]);
    assert!(nothing.contains("new:"), "{nothing}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["task", "find", "WA-4200", "--json"])).unwrap();
    assert_eq!(json["verdict"], "take");
    assert_eq!(json["hits"][0]["key"], "WA-4200");
}

#[test]
fn a_renamed_card_is_found_by_its_old_id_and_by_a_mention() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "rtf files", "--id", "WA-4130"]);
    let renamed = output(data.path(), &["task", "rename", "WA-4130", "api-77"]);
    assert!(renamed.contains("WA-4130 is now API-77"), "{renamed}");

    let found = output(data.path(), &["task", "find", "WA-4130"]);
    assert!(found.contains("take API-77"), "the old id is an alias: {found}");
    let show = output(data.path(), &["task", "show", "wa-4130"]);
    assert!(show.starts_with("API-77 · rtf files") && show.contains("aliases:  WA-4130"), "{show}");

    // A case number mentioned only in what was written against a card.
    output(data.path(), &["task", "new", "another card", "--id", "WA-9"]);
    output(
        data.path(),
        &["task", "note", "WA-9", "--kind", "finding", "the customer case 323858 is the same defect"],
    );
    let mention = output(data.path(), &["task", "find", "323858"]);
    assert!(mention.contains("mentioned 323858: WA-9"), "{mention}");
}

#[test]
fn a_card_is_opened_written_against_and_closed() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["task", "new", "rtf files", "--id", "WA-4130"]);
    output(data.path(), &["task", "new", "other", "--id", "WA-1"]);
    // The last card made is in hand; opening another replaces it.
    assert!(output(data.path(), &["task", "active"]).starts_with("WA-1"));
    output(data.path(), &["task", "open", "wa-4130"]);
    assert!(output(data.path(), &["task", "active"]).starts_with("WA-4130"));

    output(
        data.path(),
        &["task", "note", "WA-4130", "--kind", "decision", "Fix the parser, not the export."],
    );
    output(data.path(), &["task", "note", "WA-4130", "--kind", "plan", "Add the rtf branch to the loader."]);
    output(data.path(), &["task", "note", "WA-4130", "--kind", "next", "Write the loader test first."]);
    output(
        data.path(),
        &["task", "link", "WA-4130", "webapp", "--branch", "fix/rtf", "--role", "where it is fixed"],
    );

    let packet = output(data.path(), &["task", "context", "WA-4130"]);
    assert!(packet.starts_with("# WA-4130 · rtf files"), "{packet}");
    assert!(
        packet.contains("## Worked in\n- webapp") && packet.contains("on `fix/rtf` - where it is fixed"),
        "{packet}"
    );
    assert!(packet.contains("## Next step\nWrite the loader test first."), "{packet}");
    assert!(packet.contains("## Decisions\n") && packet.contains("Fix the parser"), "{packet}");
    assert!(packet.contains("## Plan\n") && packet.contains("rtf branch"), "{packet}");

    let show = output(data.path(), &["task", "show", "WA-4130"]);
    assert!(show.contains("recorded: 1 decision, 1 next, 1 plan"), "{show}");
    assert!(show.contains("next:     Write the loader test first."), "{show}");

    // Closing the card in hand needs no name, and leaves nothing in hand.
    let closed = output(data.path(), &["task", "close"]);
    assert!(closed.contains("Closed WA-4130 as done (was new)"), "{closed}");
    assert!(output(data.path(), &["task", "active"]).contains("No card is in hand"));
    assert!(!output(data.path(), &["task", "list"]).contains("WA-4130"), "done leaves the open list");
    assert!(output(data.path(), &["task", "list", "--status", "all"]).contains("WA-4130"));

    output(data.path(), &["task", "status", "WA-1", "waiting-handoff"]);
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
    output(data.path(), &["task", "new", "rtf files are not supported", "--id", "WA-4130"]);

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
        serde_json::json!({ "name": "task_find", "arguments": { "query": "wa-4130 rtf" } }),
    ));
    assert!(found.starts_with("take WA-4130"), "{found}");

    let written = text(&call(
        3,
        "tools/call",
        serde_json::json!({ "name": "record_finding", "arguments": { "task": "WA-4130", "text": "The loader drops rtf by extension." } }),
    ));
    assert!(written.contains("Recorded a finding on WA-4130"), "{written}");
    let next = text(&call(
        4,
        "tools/call",
        serde_json::json!({ "name": "set_next_step", "arguments": { "task": "WA-4130", "text": "Read the loader." } }),
    ));
    assert!(next.contains("WA-4130"), "{next}");

    let packet = text(&call(
        5,
        "tools/call",
        serde_json::json!({ "name": "task_context", "arguments": { "task": "WA-4130" } }),
    ));
    assert!(packet.contains("# WA-4130 · rtf files are not supported"), "{packet}");
    assert!(packet.contains("## Findings") && packet.contains("drops rtf by extension"), "{packet}");
    assert!(packet.contains("## Next step\nRead the loader."), "{packet}");

    let refused = call(
        6,
        "tools/call",
        serde_json::json!({ "name": "record_finding", "arguments": { "task": "WA-0", "text": "x" } }),
    );
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    drop(stdin);
    let _ = child.wait();
}
