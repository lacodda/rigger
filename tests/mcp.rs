//! The MCP server as a client meets it: a real process, real JSON-RPC lines.
//!
//! The unit tests beside the code check the shapes; these check the wire. A
//! server that builds a correct tool list and then writes something else to
//! stdout - a stray print, a missing flush, an answer to a notification - is
//! broken in exactly the way a client cannot recover from, and only a
//! process test sees it.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Value, json};

/// A running server, with the pipes a client talks through.
struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn start(data: &Path) -> Server {
        let mut child = Command::cargo_bin("rigger")
            .unwrap()
            .arg("mcp")
            .env("RIGGER_DATA_DIR", data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Server { child, stdin, stdout }
    }

    /// Sends a request and reads the one line that answers it.
    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("not JSON: {line:?} ({e})"));
        assert_eq!(response["jsonrpc"], "2.0", "{response}");
        assert_eq!(response["id"], id, "an answer must carry the id it answers");
        response
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Calls a tool and returns its text, asserting it did not fail.
    fn call(&mut self, id: i64, name: &str, arguments: Value) -> String {
        let response = self.request(id, "tools/call", json!({ "name": name, "arguments": arguments }));
        let result = &response["result"];
        assert!(result["isError"] != json!(true), "{name} failed: {result}");
        result["content"][0]["text"].as_str().unwrap().to_string()
    }

    /// Closes stdin and waits: the server must end when the client goes away.
    fn finish(mut self) {
        drop(self.stdin);
        let status = self.child.wait().unwrap();
        assert!(status.success(), "the server exited with {status}");
    }
}

fn rigger(data: &Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

/// A database with one project whose hub has been read: a stage with open
/// tasks, a shipped version, a question waiting for the owner.
fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("proj");
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"proj\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План разработки\n\n## Ждёт решения владельца\n\n1. Pick a colour.\n\n## v0.2.0 · Second stage\n\n- [ ] first task\n- [ ] second task\n",
    )
    .unwrap();
    std::fs::write(
        hub.join("Изменения.md"),
        "# Изменения\n\n## v0.1.0 · First stage — выпущен 2026-09-01\n\n- Something shipped.\n",
    )
    .unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", "proj", "--hub"]).arg(&hub).assert().success();
    root
}

#[test]
fn a_session_reads_the_record_and_writes_to_it() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let mut server = Server::start(data.path());

    let init = server.request(1, "initialize", json!({ "protocolVersion": "2025-06-18" }));
    assert_eq!(init["result"]["serverInfo"]["name"], "rigger");
    assert!(init["result"]["capabilities"]["tools"].is_object(), "{init}");
    assert!(init["result"]["capabilities"]["prompts"].is_object(), "{init}");

    // The specification forbids answering a notification; a client that
    // counts messages hangs forever if the server does.
    server.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));

    let listed = server.request(2, "tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"context"), "{names:?}");
    assert!(names.contains(&"record_decision"), "{names:?}");

    let packet = server.call(3, "context", json!({ "project": "proj" }));
    assert!(packet.contains("Last shipped: v0.1.0"), "{packet}");
    assert!(packet.contains("Current stage: v0.2.0"), "{packet}");
    assert!(packet.contains("Pick a colour"), "the owner's question belongs in the packet: {packet}");

    let recorded = server.call(4, "record_decision", json!({ "project": "proj", "text": "The record is the database." }));
    assert!(recorded.contains("Recorded a decision"), "{recorded}");

    server.call(5, "set_next_step", json!({ "project": "proj", "text": "Ship the exporter." }));
    server.call(6, "ask_owner", json!({ "project": "proj", "text": "Which tier is this project?" }));
    server.call(7, "wish", json!({ "project": "proj", "text": "Count days without a commit." }));

    // Everything written comes back through the same door it went in.
    let packet = server.call(8, "context", json!({ "project": "proj" }));
    assert!(packet.contains("The record is the database."), "{packet}");
    assert!(packet.contains("Ship the exporter."), "{packet}");
    assert!(packet.contains("Which tier is this project?"), "{packet}");
    assert!(packet.contains("Count days without a commit."), "{packet}");

    server.finish();
}

#[test]
fn the_plan_hands_out_the_ids_close_task_takes() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let mut server = Server::start(data.path());

    let plan = server.call(1, "plan", json!({ "project": "proj" }));
    assert!(plan.contains("v0.2.0"), "{plan}");
    assert!(plan.contains("first task"), "{plan}");

    // The id the plan printed is the one close_task accepts - a title would
    // not do: two stages can spell the same step.
    let id: i64 = plan
        .lines()
        .find(|l| l.contains("first task"))
        .and_then(|l| l.split(['[', ']']).nth(1).map(str::to_string))
        .expect("the plan must print an id")
        .parse()
        .unwrap();

    let closed = server.call(2, "close_task", json!({ "project": "proj", "task": id }));
    assert!(closed.contains("first task"), "{closed}");

    let plan = server.call(3, "plan", json!({ "project": "proj" }));
    assert!(!plan.contains("first task"), "a closed task must leave the plan: {plan}");
    assert!(plan.contains("second task"), "{plan}");

    // Closing it twice says so rather than pretending it did something.
    let again = server.call(4, "close_task", json!({ "project": "proj", "task": id }));
    assert!(again.contains("already done"), "{again}");

    server.finish();
}

#[test]
fn the_packet_is_served_as_a_prompt_and_as_a_resource() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let mut server = Server::start(data.path());

    let prompts = server.request(1, "prompts/list", json!({}));
    assert_eq!(prompts["result"]["prompts"][0]["name"], "start session");

    let prompt = server.request(2, "prompts/get", json!({ "name": "start session", "arguments": { "project": "proj" } }));
    let text = prompt["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("Current stage: v0.2.0"), "{text}");
    assert!(text.contains("record_decision"), "the prompt must point at the tools, not at the CLI: {text}");

    let resources = server.request(3, "resources/list", json!({}));
    let uri = resources["result"]["resources"][0]["uri"].as_str().unwrap().to_string();
    assert_eq!(uri, "rigger://proj/context");

    let read = server.request(4, "resources/read", json!({ "uri": uri }));
    let text = read["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("Current stage: v0.2.0"), "{text}");

    server.finish();
}

#[test]
fn a_failure_is_reported_where_the_model_can_read_it() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let mut server = Server::start(data.path());

    // A tool that runs and fails answers the model, not the client.
    let response = server.request(
        1,
        "tools/call",
        json!({ "name": "record_finding", "arguments": { "project": "ghost", "text": "x" } }),
    );
    assert!(response["error"].is_null(), "{response}");
    assert_eq!(response["result"]["isError"], true, "{response}");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("no project named 'ghost'"), "{text}");
    assert!(!text.contains("stack backtrace"), "an error must read as a sentence: {text}");

    // A method the server does not serve is a protocol error, and the server
    // keeps going: one bad request does not end a session.
    let response = server.request(2, "resources/subscribe", json!({}));
    assert_eq!(response["error"]["code"], -32601);

    let packet = server.call(3, "context", json!({ "project": "proj" }));
    assert!(packet.contains("Current stage"), "the server must survive a bad request: {packet}");

    server.finish();
}

#[test]
fn a_line_that_is_not_json_does_not_end_the_session() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let mut server = Server::start(data.path());

    writeln!(server.stdin, "this is not JSON").unwrap();
    server.stdin.flush().unwrap();

    // Nothing was written to stdout for that line, so this answer is the
    // next request's - if the server had answered the noise, the ids would
    // not line up.
    let packet = server.call(1, "context", json!({ "project": "proj" }));
    assert!(packet.contains("Current stage"), "{packet}");

    server.finish();
}

#[test]
fn without_a_database_the_server_says_so_rather_than_serving_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .arg("mcp")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains("run `rigger init` first"));
}
