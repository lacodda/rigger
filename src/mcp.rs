//! The assistant's door: the record served over MCP.
//!
//! `rigger context` hands an assistant the packet once, at the start. This
//! serves the same packet and, more to the point, the writing side: an
//! assistant that decides something records it here, in the same breath,
//! instead of a session's findings living only in its own transcript until
//! someone edits markdown afterwards.
//!
//! The transport is the one MCP defines for a local server: JSON-RPC 2.0,
//! one message per line, requests on stdin and responses on stdout. Nothing
//! else may be written to stdout - a stray `println!` is a protocol error to
//! the client - so every diagnostic goes to stderr.
//!
//! The protocol is spoken directly rather than through an SDK: the stdio
//! half of MCP is a handful of methods, and a synchronous loop over lines
//! matches a binary whose database access is synchronous anyway.

use std::io::{BufRead, Write};

use anyhow::{Result, bail};
use serde_json::{Map, Value, json};

use crate::context;
use crate::db::{Db, Project};
use crate::paths;

/// The protocol version this server speaks. A client that asks for another
/// one is answered with this: the specification says the server names the
/// version it supports, and the client decides whether it can live with it.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC error codes used here, from the specification.
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Serves the record on stdin/stdout until the client closes the stream.
///
/// The database is opened once: a session is a long conversation, and
/// re-opening the file for every tool call would only add failure modes.
pub fn serve() -> Result<()> {
    let db = Db::open(&paths::db_path()?)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        if stdin.lock().read_line(&mut line)? == 0 {
            return Ok(());
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(err) => {
                // A line that is not JSON has no id to answer against, so
                // there is nobody to answer; saying so on stderr is all a
                // server can do without corrupting the stream.
                eprintln!("rigger mcp: ignoring a line that is not JSON: {err}");
                continue;
            }
        };
        let Some(response) = handle(&db, &request) else {
            // A notification: the specification forbids a response.
            continue;
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
}

/// Answers one message, or `None` when it is a notification.
fn handle(db: &Db, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    // Notifications carry no id and expect nothing back.
    id.as_ref()?;
    let id = id.unwrap_or(Value::Null);

    let result = match method {
        "initialize" => Ok(initialize()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(db, &params),
        "prompts/list" => Ok(json!({ "prompts": prompts() })),
        "prompts/get" => get_prompt(db, &params),
        "resources/list" => resources(db),
        "resources/read" => read_resource(db, &params),
        other => Err(Failure {
            code: METHOD_NOT_FOUND,
            message: format!("rigger does not serve `{other}`"),
        }),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(failure) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": failure.code, "message": failure.message },
        }),
    })
}

/// A protocol-level failure: the request itself was wrong. A tool that runs
/// and fails is not one of these - it answers with its own text, so that the
/// assistant reads what went wrong instead of the client swallowing it.
struct Failure {
    code: i64,
    message: String,
}

fn initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {},
            "prompts": {},
            "resources": {},
        },
        "serverInfo": {
            "name": "rigger",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "The record of this project: what is done, what is next, what waits for the owner. \
    Start a session with the `context` tool, or the `start session` prompt. Record what you decide, find or \
    break with `record_decision`, `record_finding`, `record_pitfall` and `record_change` as it happens, not \
    at the end. Leave the next session one line with `set_next_step`. Anything only the owner can settle goes \
    to `ask_owner`; anything for the plan later goes to `wish`.",
    })
}

/// One entry of the tool list, with the shape of its arguments.
fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
    })
}

/// The argument every tool takes: which project it speaks about.
fn project_arg() -> Value {
    json!({ "type": "string", "description": "Project name, as `rigger project list` shows it" })
}

fn text_arg(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// The tools, in the order a session uses them: read the record, then write
/// to it.
fn tools() -> Vec<Value> {
    let record = |kind: &str, what: &str| {
        tool(
            &format!("record_{kind}"),
            what,
            json!({ "project": project_arg(), "text": text_arg("What happened, in full sentences") }),
            &["project", "text"],
        )
    };
    vec![
        tool(
            "context",
            "Where the project stands: what shipped, what is being built, what waits for the owner, what happened recently, and the line the last session left behind. Read this before doing anything else in a project.",
            json!({
                "project": project_arg(),
                "budget": { "type": "integer", "description": "Token budget for the packet; the default is 3000" },
            }),
            &["project"],
        ),
        tool(
            "plan",
            "The stage being built and its open tasks, each with the id `close_task` takes.",
            json!({ "project": project_arg() }),
            &["project"],
        ),
        record(
            "decision",
            "Record a decision and the reason behind it. Reasons outlive the decision, so write why, not only what.",
        ),
        record(
            "finding",
            "Record something learnt about the code or the domain that the next session would otherwise learn again.",
        ),
        record("pitfall", "Record a trap worth remembering: what looked right, what actually happened."),
        record("change", "Record something that changed in the product."),
        tool(
            "set_next_step",
            "Leave the next session one line to start from. The newest one wins; it is not a list.",
            json!({ "project": project_arg(), "text": text_arg("The one line the next session starts from") }),
            &["project", "text"],
        ),
        tool(
            "ask_owner",
            "Ask the owner something only they can settle. The question waits in the packet until they answer it - it does not reach them now, so do not block on it.",
            json!({ "project": project_arg(), "text": text_arg("The question, with enough context to answer it cold") }),
            &["project", "text"],
        ),
        tool(
            "wish",
            "Record something to sort into the plan later.",
            json!({ "project": project_arg(), "text": text_arg("What you want, and why it would help") }),
            &["project", "text"],
        ),
        tool(
            "resolve",
            "Answer a question waiting for the owner, or mark a wish sorted into the plan, so it leaves the packet. Ids come from the packet. Answer a question only when the owner has actually said something - this records their answer, not your guess at it.",
            json!({
                "project": project_arg(),
                "id": { "type": "integer", "description": "Id of the question or wish, as the packet lists it" },
                "answer": text_arg("What the owner decided; recorded as a decision. Omit for a wish."),
            }),
            &["project", "id"],
        ),
        tool(
            "close_task",
            "Mark a task of the current stage done. Ids come from `plan`.",
            json!({
                "project": project_arg(),
                "task": { "type": "integer", "description": "Task id, as `plan` lists it" },
            }),
            &["project", "task"],
        ),
    ]
}

fn call_tool(db: &Db, params: &Value) -> Result<Value, Failure> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| Failure {
        code: INVALID_PARAMS,
        message: "a tool call needs a `name`".into(),
    })?;
    let empty = Map::new();
    let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);

    // Anything the tool itself can go wrong with is reported as a failed
    // tool result rather than as a protocol error: the assistant is the one
    // who can act on "no project named that", and a JSON-RPC error is
    // reported to the client instead of to the model.
    Ok(match run_tool(db, name, args) {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(err) => json!({
            "content": [{ "type": "text", "text": format!("{err:#}") }],
            "isError": true,
        }),
    })
}

fn run_tool(db: &Db, name: &str, args: &Map<String, Value>) -> Result<String> {
    let project = |db: &Db| -> Result<Project> {
        let Some(name) = args.get("project").and_then(Value::as_str) else {
            bail!("this tool needs a `project`");
        };
        match db.project_by_name(name)? {
            Some(project) => Ok(project),
            None => bail!("no project named '{name}'; the projects are listed by `rigger project list`"),
        }
    };
    let text = || -> Result<&str> {
        match args.get("text").and_then(Value::as_str) {
            Some(text) if !text.trim().is_empty() => Ok(text.trim()),
            Some(_) => bail!("`text` is empty; a record with nothing in it helps nobody"),
            None => bail!("this tool needs a `text`"),
        }
    };

    match name {
        "context" => {
            let project = project(db)?;
            let budget = args
                .get("budget")
                .and_then(Value::as_u64)
                .map(|b| b as usize)
                .unwrap_or(context::DEFAULT_BUDGET);
            Ok(context::render(&context::build(db, &project, budget)?))
        }
        "plan" => {
            let project = project(db)?;
            Ok(render_plan(db, &project)?)
        }
        "resolve" => {
            let project = project(db)?;
            let Some(id) = args.get("id").and_then(Value::as_i64) else {
                bail!("this tool needs an `id`; the packet lists them");
            };
            let answer = args.get("answer").and_then(Value::as_str).map(str::trim).filter(|a| !a.is_empty());
            let (kind, body) = db.resolve_event(project.id, id, answer)?;
            let first_line = body.lines().next().unwrap_or(&body);
            Ok(match (kind.as_str(), answer) {
                ("question", Some(_)) => format!("Answered [{id}]: {first_line}\nThe answer is recorded as a decision."),
                ("question", None) => format!("Closed question [{id}]: {first_line}"),
                _ => format!("Sorted [{id}]: {first_line}"),
            })
        }
        "close_task" => {
            let project = project(db)?;
            let Some(task) = args.get("task").and_then(Value::as_i64) else {
                bail!("this tool needs a `task` id; `plan` lists them");
            };
            let (title, change) = db.close_task(project.id, task)?;
            Ok(match change {
                crate::db::Change::Unchanged => format!("Task {task} was already done: {title}"),
                _ => format!("Closed task {task}: {title}"),
            })
        }
        _ => {
            let kind = match name {
                "record_decision" => "decision",
                "record_finding" => "finding",
                "record_pitfall" => "pitfall",
                "record_change" => "change",
                "set_next_step" => "next",
                "ask_owner" => "question",
                "wish" => "wish",
                other => bail!("rigger serves no tool named `{other}`"),
            };
            let project = project(db)?;
            let text = text()?;
            db.record_event(project.id, kind, text, &crate::db::now(), "assistant")?;
            Ok(match kind {
                "next" => format!("The next session for {} starts from this line.", project.name),
                "question" => format!("Asked the owner; it waits in {}'s packet until they answer.", project.name),
                "wish" => format!("Recorded a wish for {}.", project.name),
                _ => format!("Recorded a {kind} for {}.", project.name),
            })
        }
    }
}

/// The stage and its tasks, with the ids `close_task` takes.
fn render_plan(db: &Db, project: &Project) -> Result<String> {
    let Some(stage) = db.current_stage(project.id)? else {
        return Ok(format!("{}: no stage is planned.", project.name));
    };
    let mut out = format!("{} · {}", project.name, stage.version);
    if let Some(title) = &stage.title {
        out.push_str(&format!(" · {title}"));
    }
    out.push('\n');
    if stage.tasks.is_empty() {
        out.push_str("Every task of this stage is done.\n");
    }
    for task in &stage.tasks {
        out.push_str(&format!("- [{}] {}\n", task.id, task.title));
    }
    Ok(out)
}

/// The packet as a prompt, so that a session starts with `/rigger` rather
/// than with the assistant remembering to call a tool.
fn prompts() -> Vec<Value> {
    vec![json!({
        "name": "start session",
        "description": "Where the project stands, as the first message of a session",
        "arguments": [{
            "name": "project",
            "description": "Project name, as `rigger project list` shows it",
            "required": true,
        }],
    })]
}

fn get_prompt(db: &Db, params: &Value) -> Result<Value, Failure> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    if name != "start session" {
        return Err(Failure {
            code: INVALID_PARAMS,
            message: format!("rigger serves no prompt named `{name}`"),
        });
    }
    let project = params
        .get("arguments")
        .and_then(|a| a.get("project"))
        .and_then(Value::as_str)
        .ok_or_else(|| Failure {
            code: INVALID_PARAMS,
            message: "the prompt needs a `project` argument".into(),
        })?;
    let packet = packet_for(db, project).map_err(|err| Failure {
        code: INVALID_PARAMS,
        message: format!("{err:#}"),
    })?;

    Ok(json!({
        "description": format!("Where {project} stands, from rigger"),
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": crate::open::first_message_over_mcp(&packet) },
        }],
    }))
}

/// Every project as a resource, so a client can browse the record rather
/// than having to know a project's name to ask for it.
fn resources(db: &Db) -> Result<Value, Failure> {
    let projects = db.projects().map_err(|err| Failure {
        code: INVALID_PARAMS,
        message: format!("{err:#}"),
    })?;
    let resources: Vec<Value> = projects
        .iter()
        .map(|p| {
            json!({
                "uri": format!("rigger://{}/context", p.name),
                "name": format!("{} context", p.name),
                "description": format!("Where {} stands: shipped, current stage, open questions, recent events", p.name),
                "mimeType": "text/markdown",
            })
        })
        .collect();
    Ok(json!({ "resources": resources }))
}

fn read_resource(db: &Db, params: &Value) -> Result<Value, Failure> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
    let project = parse_uri(uri).ok_or_else(|| Failure {
        code: INVALID_PARAMS,
        message: format!("`{uri}` is not a rigger resource; they are `rigger://<project>/context`"),
    })?;
    let packet = packet_for(db, project).map_err(|err| Failure {
        code: INVALID_PARAMS,
        message: format!("{err:#}"),
    })?;
    Ok(json!({
        "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": packet }],
    }))
}

/// The project named by a `rigger://<project>/context` URI.
fn parse_uri(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("rigger://")?;
    let project = rest.strip_suffix("/context")?;
    (!project.is_empty()).then_some(project)
}

fn packet_for(db: &Db, name: &str) -> Result<String> {
    let Some(project) = db.project_by_name(name)? else {
        bail!("no project named '{name}'; the projects are listed by `rigger project list`");
    };
    Ok(context::render(&context::build(db, &project, context::DEFAULT_BUDGET)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_named(name: &str) -> Value {
        tools()
            .into_iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("no tool named {name}"))
    }

    #[test]
    fn the_stage_lists_every_tool_the_plan_promised() {
        let names: Vec<String> = tools().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        for promised in [
            "context",
            "plan",
            "record_decision",
            "record_finding",
            "record_pitfall",
            "record_change",
            "set_next_step",
            "ask_owner",
            "wish",
            "resolve",
            "close_task",
        ] {
            assert!(names.contains(&promised.to_string()), "{promised} is missing from {names:?}");
        }
    }

    #[test]
    fn every_tool_declares_the_shape_of_its_arguments() {
        // A tool whose schema is wrong is called wrong, and the model has no
        // way to find out why: the schema is the only documentation it gets.
        for tool in tools() {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "{}", tool["name"]);
            let required = schema["required"].as_array().expect("required");
            for name in required {
                let name = name.as_str().unwrap();
                assert!(
                    schema["properties"].get(name).is_some(),
                    "{} requires `{name}` without describing it",
                    tool["name"]
                );
            }
            assert!(
                tool["description"].as_str().is_some_and(|d| d.len() > 30),
                "{} needs a description a model can act on",
                tool["name"]
            );
        }
    }

    #[test]
    fn close_task_takes_an_id_because_a_title_is_not_a_name() {
        let schema = &tool_named("close_task")["inputSchema"];
        assert_eq!(schema["properties"]["task"]["type"], "integer");
    }

    #[test]
    fn a_notification_is_not_answered() {
        // The specification forbids a response to a message without an id;
        // answering one breaks clients that count messages.
        let db_free = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        let dir = tempfile::tempdir().unwrap();
        let db = Db::create(&dir.path().join("rigger.db")).unwrap();
        assert!(handle(&db, &db_free).is_none());
    }

    #[test]
    fn an_unknown_method_is_a_protocol_error_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::create(&dir.path().join("rigger.db")).unwrap();
        let response = handle(&db, &json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/subscribe" })).unwrap();
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
        assert!(response["result"].is_null());
    }

    #[test]
    fn a_tool_that_fails_answers_the_model_rather_than_the_client() {
        // isError keeps the reason in the conversation: a JSON-RPC error is
        // reported to the client, and the model never learns why.
        let dir = tempfile::tempdir().unwrap();
        let db = Db::create(&dir.path().join("rigger.db")).unwrap();
        let call = json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "record_finding", "arguments": { "project": "nowhere", "text": "x" } },
        });
        let response = handle(&db, &call).unwrap();
        assert!(response["error"].is_null(), "{response}");
        assert_eq!(response["result"]["isError"], true);
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("no project named 'nowhere'"), "{text}");
    }

    #[test]
    fn an_empty_record_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::create(&dir.path().join("rigger.db")).unwrap();
        db.add_project("sample", "/tmp/sample", None).unwrap();
        let mut args = Map::new();
        args.insert("project".into(), json!("sample"));
        args.insert("text".into(), json!("   "));
        let err = run_tool(&db, "record_finding", &args).unwrap_err().to_string();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn a_resource_uri_names_a_project() {
        assert_eq!(parse_uri("rigger://sample/context"), Some("sample"));
        assert_eq!(parse_uri("rigger:///context"), None);
        assert_eq!(parse_uri("https://example.com/context"), None);
        assert_eq!(parse_uri("rigger://sample"), None);
    }

    #[test]
    fn initialize_names_the_protocol_and_the_build() {
        let init = initialize();
        assert_eq!(init["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(init["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        // Without the tools capability a client never asks for the tools.
        assert!(init["capabilities"]["tools"].is_object());
    }
}
