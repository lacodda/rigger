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

/// What the server says about itself when asked whether it is there.
///
/// `doctor` asks this rather than starting a process and speaking the
/// protocol down a pipe. The question "does the server answer" is about
/// this code, and a subprocess would test the shell, the PATH and the
/// binary on disk instead - three things that can be wrong while the
/// server is perfectly fine, and one - a stale binary earlier in PATH -
/// that would make a broken install look healthy.
pub fn self_check(db: &Db) -> Result<Check> {
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let Some(response) = handle(db, &request) else {
        bail!("the server returned nothing to a request that requires an answer");
    };
    if let Some(error) = response.get("error") {
        bail!("the server answered with an error: {error}");
    }
    let tools = response
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    Ok(Check {
        protocol: PROTOCOL_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        tools,
        prompts: prompts().len(),
    })
}

/// What `doctor` prints about the server.
#[derive(Debug, serde::Serialize)]
pub struct Check {
    pub protocol: &'static str,
    pub version: &'static str,
    pub tools: usize,
    pub prompts: usize,
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

fn task_arg() -> Value {
    json!({ "type": "string", "description": "A card's key, alias or id, to write against the card instead of a project" })
}

/// The tools, in the order a session uses them: read the record, then write
/// to it.
fn tools() -> Vec<Value> {
    let record = |kind: &str, what: &str| {
        tool(
            &format!("record_{kind}"),
            what,
            json!({ "project": project_arg(), "task": task_arg(), "text": text_arg("What happened, in full sentences") }),
            &["text"],
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
        tool(
            "record_decision",
            "Record a decision and the reason behind it. Reasons outlive the decision, so write why, not only what. Name the `principle` when the decision is one more instance of something this line already believes - `why_principle` then reads the whole thread.",
            json!({
                "project": project_arg(),
                "task": task_arg(),
                "text": text_arg("What happened, in full sentences"),
                "principle": { "type": "string", "description": "The principle this stands on, in the words the record already uses; `principles` lists them" },
            }),
            &["text"],
        ),
        record(
            "finding",
            "Record something learnt about the code or the domain that the next session would otherwise learn again.",
        ),
        record("pitfall", "Record a trap worth remembering: what looked right, what actually happened."),
        record("change", "Record something that changed in the product."),
        tool(
            "task_find",
            "Find the card a line of text means - by ticket id, by title, by case number - and say whether to take it, ask, or make a new one. Give the task exactly as it was handed over.",
            json!({ "query": text_arg("The task as it was handed over: id, title, numbers, any of them") }),
            &["query"],
        ),
        tool(
            "task_context",
            "Where a task stands: what it is, where it is worked, and everything written against it - decisions, findings, pitfalls, the plan of edits, changes, the next step. A session on a card starts here.",
            json!({
                "task": { "type": "string", "description": "Card key, alias or id" },
                "budget": { "type": "integer", "description": "Token budget for the packet; the default is 3000" },
            }),
            &["task"],
        ),
        tool(
            "record_plan",
            "A step of the plan of edits for a card: what will be changed, where, in what order. Give `task`.",
            json!({ "project": project_arg(), "task": task_arg(), "text": text_arg("The step, in full sentences") }),
            &["text"],
        ),
        tool(
            "record_state",
            "Put one line at the top of the hub's state block: where the project stands after this sitting, the way a README's «Состояние» tells it. Only when the state shifted - a release, a block closed, a plan reground.",
            json!({ "project": project_arg(), "text": text_arg("One sentence, bold headline first, for the README's state block") }),
            &["project", "text"],
        ),
        tool(
            "set_next_step",
            "Leave the next session one line to start from. The newest one wins; it is not a list. With `task`, the line belongs to the card.",
            json!({ "project": project_arg(), "task": task_arg(), "text": text_arg("The one line the next session starts from") }),
            &["text"],
        ),
        tool(
            "ask_owner",
            "Ask the owner something only they can settle. The question waits in the packet until they answer it - it does not reach them now, so do not block on it.",
            json!({ "project": project_arg(), "text": text_arg("The question, with enough context to answer it cold") }),
            &["project", "text"],
        ),
        tool(
            "wish",
            "Record something to sort into the plan later. Give `from` when one project is asking another for something: the order then shows on the neighbour's packet and in the owner's inbox as theirs.",
            json!({
                "project": project_arg(),
                "text": text_arg("What you want, and why it would help"),
                "from": { "type": "string", "description": "The project asking, when a neighbour is placing the order" },
            }),
            &["project", "text"],
        ),
        tool(
            "principles",
            "The principles the record stands on, with how often each has been invoked. Read this before naming one, so a decision joins a thread rather than starting a synonym of it.",
            json!({}),
            &[],
        ),
        tool(
            "why_principle",
            "Every decision across the line that stands on one principle, oldest first: how the line came to believe it, and what it has cost.",
            json!({ "principle": { "type": "string", "description": "The name, as `principles` spells it" } }),
            &["principle"],
        ),
        tool(
            "link",
            "Tie two projects together so the record can see the line as a graph. `pair` is two halves of one capability and neither ships alone; `consumer` is this project drawing on the other; `donor` is the other drawing on this one. Anchor a version on a side by writing it as `kasl@v1.13.0`. Only pairs are checked for drift - `week` and `retro` say when one half shipped without the other.",
            json!({
                "from": { "type": "string", "description": "Project on this side, or `project@version`" },
                "to": { "type": "string", "description": "Project on the other side, or `project@version`" },
                "kind": { "type": "string", "description": "pair, consumer or donor; pair when omitted" },
                "note": { "type": "string", "description": "A sentence saying what the two share" },
            }),
            &["from", "to"],
        ),
        tool(
            "links",
            "What a project is tied to, read from its side: its pairs, what it draws on, what draws on it, and the orders its neighbours have placed.",
            json!({ "project": project_arg() }),
            &["project"],
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
            "set_task_status",
            "Give a task of the current stage a status: new, active, waiting-handoff, frozen or done. Ids come from `plan`. `active` is the one being worked on; `waiting-handoff` is handed to someone else; `frozen` is set aside on purpose.",
            json!({
                "project": project_arg(),
                "task": { "type": "integer", "description": "Task id, as `plan` lists it" },
                "status": { "type": "string", "description": "One of new, active, waiting-handoff, frozen, done" },
            }),
            &["project", "task", "status"],
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
        tool(
            "doc_show",
            "Read a handwritten text of the project: its vision, its rituals, a research note. \
             Without a `slug`, lists what there is.",
            json!({
                "project": project_arg(),
                "slug": { "type": "string", "description": "Which document; omit to list them" },
            }),
            &["project"],
        ),
        tool(
            "doc_write",
            "Write a handwritten text into the record: a research note you were asked to make, \
             or a correction to the vision. Replaces the whole body of the document, so read it \
             with `doc_show` first and send back the whole of it.",
            json!({
                "project": project_arg(),
                "slug": { "type": "string", "description": "Which document; a new one is made if there is none" },
                "kind": { "type": "string", "description": "vision, decisions, research, rituals or other; only for a new one" },
                "title": { "type": "string", "description": "What it is called; only needed for a new one" },
                "body": { "type": "string", "description": "The whole text of the document, in markdown" },
            }),
            &["project", "slug", "body"],
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
    // A card named instead of a project: the event goes against the card,
    // under the desk the cards live in.
    let card = |db: &Db| -> Result<Option<crate::card::Card>> {
        match args.get("task").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) {
            Some(text) => match db.card_by_ref(text)? {
                Some(card) => Ok(Some(card)),
                None => bail!("no card named '{text}'; `task_find` looks one up"),
            },
            None => Ok(None),
        }
    };
    let project = |db: &Db| -> Result<Project> {
        if card(db)?.is_some() {
            return db.desk_project();
        }
        let Some(name) = args.get("project").and_then(Value::as_str) else {
            bail!("this tool needs a `project`, or a `task` to write against");
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
        "task_find" => {
            let Some(query) = args.get("query").and_then(Value::as_str).map(str::trim).filter(|q| !q.is_empty()) else {
                bail!("this tool needs a `query`: the task as it was handed over");
            };
            let parsed = crate::card::parse(query);
            let cards = db.cards(None)?;
            let (hits, verdict) = crate::card::find(&parsed, &cards, 8);
            let mut out = match verdict {
                crate::card::Verdict::Take => format!("take {}: it is the one.\n", hits[0].key),
                crate::card::Verdict::Ask => "ask: show these and let the owner pick, or say it is new.\n".to_string(),
                crate::card::Verdict::New => "new: nothing is close; make a card with `rigger task new`.\n".to_string(),
            };
            for h in &hits {
                out.push_str(&format!("{:>3}  {}  {}  {}  ({})\n", h.score, h.key, h.status, h.title, h.why));
            }
            let shown: Vec<String> = hits.iter().map(|h| h.key.clone()).collect();
            for needle in parsed.ids.iter().chain(parsed.numbers.iter()) {
                for (key, title) in db.cards_mentioning(needle, &shown)? {
                    out.push_str(&format!("mentioned {needle}: {key} - {title}\n"));
                }
            }
            Ok(out)
        }
        "task_context" => {
            let Some(card) = card(db)? else {
                bail!("this tool needs a `task`: a card's key, alias or id")
            };
            let budget = args
                .get("budget")
                .and_then(Value::as_u64)
                .map(|b| b as usize)
                .unwrap_or(crate::context::DEFAULT_BUDGET);
            let first = "This is where the task stands, from rigger. Pick up from the next step; record against it with the `task` argument \
                 of `record_decision`, `record_finding`, `record_pitfall`, `record_plan` and `record_change`, and leave the next line \
                 with `set_next_step`.\n\n";
            Ok(first.to_string() + &crate::render_card(db, &card, budget)?)
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
        "set_task_status" => {
            let project = project(db)?;
            let Some(task) = args.get("task").and_then(Value::as_i64) else {
                bail!("this tool needs a `task` id; `plan` lists them");
            };
            let Some(status) = args.get("status").and_then(Value::as_str) else {
                bail!("this tool needs a `status`: one of {}", crate::db::TASK_STATUSES[..5].join(", "));
            };
            let (title, was) = db.set_task_status(project.id, task, status)?;
            Ok(match was == status {
                true => format!("Task {task} was already {status}: {title}"),
                false => format!("Task {task} is now {status} (was {was}): {title}"),
            })
        }
        "doc_show" => {
            let project = project(db)?;
            let Some(slug) = args.get("slug").and_then(Value::as_str) else {
                let docs = db.documents(project.id, None)?;
                if docs.is_empty() {
                    return Ok(format!("{} has no documents yet.", project.name));
                }
                let mut out = String::new();
                for d in &docs {
                    out.push_str(&format!("{} ({}) - {}\n", d.slug, d.kind, d.title));
                }
                return Ok(out);
            };
            let Some(doc) = db.document(project.id, slug)? else {
                let known = db.documents(project.id, None)?;
                let names = known.iter().map(|d| d.slug.as_str()).collect::<Vec<_>>().join(", ");
                bail!("{} has no document at '{slug}'; it has {names}", project.name);
            };
            Ok(doc.body)
        }
        "doc_write" => {
            let project = project(db)?;
            let Some(slug) = args.get("slug").and_then(Value::as_str) else {
                bail!("this tool needs a `slug`; `doc_show` lists them");
            };
            let Some(body) = args.get("body").and_then(Value::as_str) else {
                bail!("this tool needs a `body`: the whole text of the document");
            };
            if body.trim().is_empty() {
                bail!("the body is empty; a document is removed with `rigger doc remove`, not by emptying it");
            }
            let existing = db.document(project.id, slug)?;
            // A rewrite keeps what it was not told to change, so an
            // assistant correcting a vision cannot silently retitle it or
            // turn it into a research note.
            let kind = args
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| existing.as_ref().map(|d| d.kind.clone()))
                .unwrap_or_else(|| "other".to_string());
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| existing.as_ref().map(|d| d.title.clone()))
                .unwrap_or_else(|| slug.to_string());
            crate::doc::check_kind(&kind)?;
            let written = db.write_document(project.id, &kind, slug, &title, body)?;
            Ok(match existing {
                Some(_) => format!("Rewrote {} ({}), {} bytes", written.slug, written.kind, written.body.len()),
                None => format!("Wrote {} ({}), {} bytes", written.slug, written.kind, written.body.len()),
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
        "principles" => {
            let principles = db.principles()?;
            if principles.is_empty() {
                return Ok(
                    "No decision names a principle yet. Name one on the next decision that is an instance of something this line already believes.".to_string(),
                );
            }
            let mut out = String::new();
            for (principle, count) in &principles {
                out.push_str(&format!("{principle} ({count})\n"));
            }
            Ok(out)
        }
        "why_principle" => {
            let Some(principle) = args.get("principle").and_then(Value::as_str) else {
                bail!("this tool needs a `principle`; `principles` lists them");
            };
            let events = db.on_principle(principle)?;
            if events.is_empty() {
                let known = db.principles()?.into_iter().map(|(n, _)| n).collect::<Vec<_>>().join(", ");
                // Far likelier a synonym than a new principle: the names
                // are typed by hand, months apart, in two languages.
                bail!("nothing stands on '{principle}'; the record knows {known}");
            }
            let mut out = format!("{principle}\n\n");
            for event in &events {
                let day = event.at.split('T').next().unwrap_or(&event.at);
                out.push_str(&format!("{day} · {} · {}\n{}\n\n", event.project, event.kind, event.body));
            }
            Ok(out)
        }
        "link" => {
            let side = |key: &str| -> Result<(String, Option<String>)> {
                let Some(text) = args.get(key).and_then(Value::as_str) else {
                    bail!("this tool needs `{key}`: a project, or `project@version`");
                };
                let (project, version) = crate::link_side(text);
                Ok((project.to_string(), version.map(str::to_string)))
            };
            let (from_name, from_version) = side("from")?;
            let (to_name, to_version) = side("to")?;
            let kind = match args.get("kind").and_then(Value::as_str) {
                Some(text) => {
                    crate::link::Kind::parse(text).ok_or_else(|| anyhow::anyhow!("a link is a pair, a consumer or a donor; '{text}' is none of them"))?
                }
                None => crate::link::Kind::Pair,
            };
            let from = named_project(db, &from_name)?;
            let to = named_project(db, &to_name)?;
            if from.id == to.id {
                bail!("a link joins two projects; {} is one", from.name);
            }
            let note = args.get("note").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
            let (id, change, anchored) = db.add_link(&crate::db::NewLink {
                kind,
                from_project: from.id,
                from_version: from_version.as_deref(),
                to_project: to.id,
                to_version: to_version.as_deref(),
                note,
            })?;
            let arrow = if kind.symmetric() { "<->" } else { "->" };
            let side = |name: &str, version: Option<&String>| match version {
                Some(v) => format!("{name} {v}"),
                None => name.to_string(),
            };
            let both = format!(
                "{} {arrow} {} ({kind})",
                side(&from_name, anchored.from.as_ref()),
                side(&to_name, anchored.to.as_ref())
            );
            Ok(match change {
                crate::db::Change::Added => format!("[{id}] {both}"),
                _ => format!("[{id}] already recorded: {both}"),
            })
        }
        "links" => {
            let project = project(db)?;
            let links = db.links_of(project.id)?;
            let asked = db.asked_wishes(project.id)?;
            if links.is_empty() && asked.is_empty() {
                return Ok(format!("{} is tied to nothing in the record.", project.name));
            }
            let mut out = String::new();
            for found in &links {
                out.push_str(&crate::render_link(found));
                out.push('\n');
            }
            for wish in &asked {
                let first = wish.body.lines().next().unwrap_or("").trim();
                out.push_str(&format!("[{}] asks     {} — {}\n", wish.id, wish.asked_by, first));
            }
            Ok(out)
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
                "record_state" => "state",
                "record_plan" => "plan",
                other => bail!("rigger serves no tool named `{other}`"),
            };
            let project = project(db)?;
            let text = text()?;
            if kind == "state" {
                db.add_state_line(project.id, &crate::db::today(), text)?;
                return Ok(format!("Added a state line for {}; `rigger export` writes it into the README.", project.name));
            }
            if let Some(card) = card(db)? {
                db.record_task_event(project.id, card.id, kind, text, &crate::db::now(), "assistant")?;
                return Ok(match kind {
                    "next" => format!("The next session on {} starts from this line.", card.key),
                    _ => format!("Recorded a {kind} on {}.", card.key),
                });
            }
            // A principle belongs to a decision and an asker to a wish;
            // both are read before the event is written so that a name
            // that does not resolve refuses the whole call rather than
            // leaving the event recorded and the tie silently missing.
            let principle = match kind {
                "decision" => args.get("principle").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty()),
                _ => None,
            };
            let asker = match kind {
                "wish" => match args.get("from").and_then(Value::as_str).map(str::trim).filter(|f| !f.is_empty()) {
                    Some(name) => Some(named_project(db, name)?),
                    None => None,
                },
                _ => None,
            };
            if let Some(asker) = &asker
                && asker.id == project.id
            {
                bail!("{} cannot be the neighbour asking itself for something", asker.name);
            }
            db.record_event(project.id, kind, text, &crate::db::now(), "assistant")?;
            if let Some(id) = db.latest_event_id(project.id, kind)? {
                if let Some(principle) = principle {
                    db.name_principle(id, principle)?;
                }
                if let Some(asker) = &asker {
                    db.name_asker(id, asker.id)?;
                }
            }
            Ok(match (kind, &asker) {
                ("next", _) => format!("The next session for {} starts from this line.", project.name),
                ("question", _) => format!("Asked the owner; it waits in {}'s packet until they answer.", project.name),
                ("wish", Some(asker)) => format!("Recorded a wish for {} from {}.", project.name, asker.name),
                ("wish", None) => format!("Recorded a wish for {}.", project.name),
                _ => match principle {
                    Some(principle) => format!("Recorded a {kind} for {}, on the principle '{principle}'.", project.name),
                    None => format!("Recorded a {kind} for {}.", project.name),
                },
            })
        }
    }
}

/// A project by the name a tool was given, refusing one the record does
/// not have.
///
/// A name that resolves to nothing is a typo, not an empty answer: a link
/// to a project that does not exist, or a wish from one, would be recorded
/// against nothing and never fire.
fn named_project(db: &Db, name: &str) -> Result<Project> {
    match db.project_by_name(name)? {
        Some(project) => Ok(project),
        None => bail!("no project named '{name}'; the projects are listed by `rigger project list`"),
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
        match task.status.as_str() {
            "new" => out.push_str(&format!("- [{}] {}\n", task.id, task.title)),
            status => out.push_str(&format!("- [{}] {} ({status})\n", task.id, task.title)),
        }
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
            "record_state",
            "record_plan",
            "task_find",
            "task_context",
            "set_next_step",
            "ask_owner",
            "wish",
            "resolve",
            "close_task",
            "set_task_status",
            "principles",
            "why_principle",
            "link",
            "links",
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
        db.add_project("sample", "/tmp/sample", None, crate::db::Kind::Repo).unwrap();
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
