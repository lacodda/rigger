//! A project taken out of the work: gone from every list that looks ahead,
//! readable by name in every one that looks back, and closed to new
//! history - by the commands that know its name, and by the schema for
//! every writer that does not.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use assert_cmd::Command;
use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Value, json};

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env("RIGGER_SKILLS_DIR", data.join("skills"));
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn refusal(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().failure();
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

/// An ISO week as `2026-W41`, counted from this one.
fn week(offset: i64) -> String {
    let day = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(offset * 7)).unwrap();
    let week = day.iso_week_date();
    format!("{}-W{:02}", week.year(), week.week())
}

/// Two products of a line, tied as a pair, both in the rotation, with the
/// line's skill installed; `attic` has a stage aimed at this week, a
/// question for the owner and a wish.
fn line(data: &Path) {
    rigger(data).arg("init").assert().success();
    for name in ["sample", "attic"] {
        let root = data.join(name);
        std::fs::create_dir_all(&root).unwrap();
        std::process::Command::new("git")
            .args(["init", "--quiet", "--initial-branch", "main"])
            .current_dir(&root)
            .status()
            .unwrap();
        rigger(data).args(["project", "add"]).arg(&root).assert().success();
        rigger(data).args(["project", "tier", name, "A"]).assert().success();
    }
    rigger(data).args(["skill", "--line", "--install"]).assert().success();
    rigger(data)
        .args([
            "version",
            "add",
            "attic",
            "v0.2.0",
            "--title",
            "Shelves",
            "--task",
            "build the shelves",
            "--week",
        ])
        .arg(week(0))
        .assert()
        .success();
    rigger(data)
        .args(["note", "attic", "--kind", "question", "Which wood for the shelves?"])
        .assert()
        .success();
    rigger(data).args(["wish", "attic", "A ladder to the top shelf"]).assert().success();
    rigger(data).args(["version", "add", "sample", "v0.1.0"]).assert().success();
    rigger(data).args(["link", "add", "sample@v0.1.0", "attic@v0.2.0"]).assert().success();
    // One half out in the world and the other not: a pair parted, which
    // the brief warns about every week while both are in the work.
    rigger(data).args(["note", "sample", "--kind", "shipped", "--tag", "v0.1.0"]).assert().success();
}

fn retire(data: &Path) {
    rigger(data)
        .args(["project", "retire", "attic", "--why", "Folded into sample; nobody climbs up there any more."])
        .assert()
        .success();
}

fn installed_skill(data: &Path) -> String {
    std::fs::read_to_string(data.join("skills").join("line").join("SKILL.md")).unwrap()
}

fn database(data: &Path) -> PathBuf {
    let doctor: Value = serde_json::from_str(&output(data, &["doctor", "--json"])).unwrap();
    PathBuf::from(doctor["database"].as_str().unwrap())
}

#[test]
fn a_retired_project_leaves_every_list_that_looks_ahead() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    // Seen before, so that its absence after is the retirement's doing.
    assert!(installed_skill(data.path()).contains("`attic`"));
    assert!(output(data.path(), &["calendar"]).contains("attic"));
    assert!(output(data.path(), &["inbox"]).contains("Which wood"));
    assert!(output(data.path(), &["next"]).contains("attic"));

    let said = output(
        data.path(),
        &["project", "retire", "attic", "--why", "Folded into sample; nobody climbs up there any more."],
    );
    assert!(said.contains("attic is retired, as of "), "{said}");
    assert!(said.contains("1 question, 1 wish, 1 open task"), "what it leaves open is counted: {said}");
    assert!(said.contains("now lists 1 project"), "the installed skill is written again: {said}");

    let skill = installed_skill(data.path());
    assert!(!skill.contains("`attic`"), "the line's skill offers a project nobody works on:\n{skill}");
    assert!(skill.contains("`sample`"), "{skill}");

    let registry: Value = serde_json::from_str(&output(data.path(), &["export", "--line"])).unwrap();
    let products: Vec<&str> = registry["products"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(products, ["sample"], "the registry says what the line ships");

    for screen in [
        vec!["calendar"],
        vec!["next"],
        vec!["week"],
        vec!["release-day"],
        vec!["inbox"],
        vec!["digest"],
        vec!["sync"],
        vec!["project", "tier", "--suggest"],
        vec!["link", "list"],
    ] {
        let text = output(data.path(), &screen);
        assert!(
            !text.contains("attic"),
            "`rigger {}` still names the retired project:\n{text}",
            screen.join(" ")
        );
    }
    let ics = data.path().join("line.ics");
    rigger(data.path()).args(["calendar", "--ics"]).arg(&ics).assert().success();
    assert!(!std::fs::read_to_string(&ics).unwrap().contains("attic"));

    // The index of the record still has it, marked.
    let list = output(data.path(), &["project", "list"]);
    assert!(list.lines().any(|l| l.starts_with("attic") && l.contains("(retired ")), "{list}");
    assert!(output(data.path(), &["doctor"]).contains("projects:  2 (1 retired)"));
}

#[test]
fn a_retired_project_reads_by_name() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    let packet = output(data.path(), &["context", "attic"]);
    assert!(
        packet.contains("Retired since "),
        "a session opened on it learns first that it is retired:\n{packet}"
    );
    assert!(packet.contains("Retired: Folded into sample"), "and why:\n{packet}");
    assert!(packet.contains("rigger project unretire attic"), "{packet}");
    let json: Value = serde_json::from_str(&output(data.path(), &["context", "attic", "--json"])).unwrap();
    let decision = json["state"]["retired"]["decision"].as_i64().expect("the decision is named by id");
    assert!(output(data.path(), &["event", "attic", &decision.to_string()]).contains("nobody climbs up there"));

    assert!(output(data.path(), &["show", "attic"]).contains("retired"));
    assert!(output(data.path(), &["project", "show", "attic"]).contains("retired: "));
    assert!(
        output(data.path(), &["find", "climbs"]).contains("attic"),
        "its history is searched with the rest"
    );
    let shown: Value = serde_json::from_str(&output(data.path(), &["project", "show", "attic", "--json"])).unwrap();
    assert!(shown["retired_at"].is_string(), "{shown}");
    assert_eq!(shown["retired_by"], decision);
}

#[test]
fn a_retired_project_takes_nothing_new_and_says_which_one() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    for command in [
        vec!["note", "attic", "--kind", "decision", "Paint it after all"],
        vec!["wish", "attic", "A window"],
        vec!["session", "start", "attic"],
        vec!["version", "add", "attic", "v0.3.0"],
        vec!["project", "tier", "attic", "B"],
        vec!["link", "add", "sample", "attic"],
        vec!["skill", "attic"],
        vec!["sync", "attic"],
    ] {
        let said = refusal(data.path(), &command);
        assert!(
            said.contains("attic is retired since "),
            "`rigger {}` must refuse by name, before the schema does:\n{said}",
            command.join(" ")
        );
    }
    // Putting away what it holds is tidying, not new history.
    let json: Value = serde_json::from_str(&output(data.path(), &["context", "attic", "--json"])).unwrap();
    let id = json["wishes"][0]["id"].as_i64().expect("its wish is still in its packet");
    rigger(data.path()).args(["resolve", "attic", &id.to_string()]).assert().success();
}

/// Every writer the commands do not guard - an import, a second front
/// door, a future command that forgets - meets the schema.
#[test]
fn the_schema_refuses_new_history_whoever_writes_it() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    let conn = rusqlite::Connection::open(database(data.path())).unwrap();
    let attic: i64 = conn.query_row("SELECT id FROM projects WHERE name = 'attic'", [], |r| r.get(0)).unwrap();
    let now = "2026-10-13T10:00:00Z";
    for (table, sql) in [
        (
            "events",
            "INSERT INTO events (project_id, kind, body, created_at) VALUES (?1, 'finding', 'a draught', ?2)",
        ),
        ("sessions", "INSERT INTO sessions (project_id, started_at) VALUES (?1, ?2)"),
        ("versions", "INSERT INTO versions (project_id, name, title) VALUES (?1, 'v9.0.0', ?2)"),
        ("tasks", "INSERT INTO tasks (project_id, title, created_at) VALUES (?1, 'sweep', ?2)"),
    ] {
        let err = conn.execute(sql, rusqlite::params![attic, now]).expect_err(table).to_string();
        assert!(err.contains("the project is retired"), "{table}: {err}");
    }
    // The working project next door is untouched by it.
    let sample: i64 = conn.query_row("SELECT id FROM projects WHERE name = 'sample'", [], |r| r.get(0)).unwrap();
    conn.execute(
        "INSERT INTO events (project_id, kind, body, created_at) VALUES (?1, 'finding', 'still here', ?2)",
        rusqlite::params![sample, now],
    )
    .unwrap();
}

#[test]
fn the_assistant_door_refuses_it_too() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    let mut child = std::process::Command::cargo_bin("rigger")
        .unwrap()
        .arg("mcp")
        .env("RIGGER_DATA_DIR", data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |id: i64, method: &str, params: Value| -> Value {
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    };
    ask(1, "initialize", json!({ "protocolVersion": "2025-06-18" }));

    let tools = [
        ("record_decision", json!({ "project": "attic", "text": "Paint it" })),
        ("set_next_step", json!({ "project": "attic", "text": "Paint it" })),
        ("wish", json!({ "project": "sample", "text": "Shelves", "from": "attic" })),
        ("link", json!({ "from": "sample", "to": "attic" })),
    ];
    for (n, (tool, arguments)) in tools.into_iter().enumerate() {
        let answer = ask(10 + n as i64, "tools/call", json!({ "name": tool, "arguments": arguments }));
        assert_eq!(answer["result"]["isError"], true, "{tool}: {answer}");
        let text = answer["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("attic is retired since "), "{tool}: {text}");
    }
    // Reading it is what the door is still for.
    let packet = ask(20, "tools/call", json!({ "name": "context", "arguments": { "project": "attic" } }));
    assert!(packet["result"]["content"][0]["text"].as_str().unwrap().contains("Retired since "), "{packet}");
    let listed = ask(21, "resources/list", json!({}));
    assert!(!listed.to_string().contains("attic"), "the resources are the projects in the work: {listed}");

    drop(stdin);
    child.wait().unwrap();
}

#[test]
fn retiring_asks_for_a_reason_and_an_ended_sitting() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());

    let said = refusal(data.path(), &["project", "retire", "attic", "--why", "  "]);
    assert!(said.contains("say why"), "{said}");

    rigger(data.path()).args(["session", "start", "attic"]).assert().success();
    let said = refusal(data.path(), &["project", "retire", "attic", "--why", "Done with it"]);
    assert!(said.contains("rigger session end attic"), "{said}");
    rigger(data.path()).args(["session", "end", "attic"]).assert().success();

    rigger(data.path()).args(["project", "service", "line"]).assert().success();
    let said = refusal(data.path(), &["project", "retire", "line", "--why", "Tidy"]);
    assert!(said.contains("a place the record keeps for itself"), "{said}");

    retire(data.path());
    let again = output(data.path(), &["project", "retire", "attic", "--why", "Twice"]);
    assert!(again.contains("was retired already"), "{again}");
    let decisions = output(data.path(), &["find", "Retired", "--project", "attic"]);
    assert_eq!(decisions.matches("Retired:").count(), 1, "a second retirement records nothing:\n{decisions}");
}

#[test]
fn unretiring_brings_it_back_where_it_was() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    let said = output(data.path(), &["project", "unretire", "attic", "--why", "The ladder arrived"]);
    assert!(said.contains("attic is back in the work"), "{said}");
    assert!(said.contains("its tier is A"), "the tier it had is kept: {said}");
    assert!(installed_skill(data.path()).contains("`attic`"));
    assert!(output(data.path(), &["calendar"]).contains("attic"));
    assert!(output(data.path(), &["inbox"]).contains("Which wood"));
    rigger(data.path())
        .args(["note", "attic", "--kind", "decision", "Paint it after all"])
        .assert()
        .success();

    let history = output(data.path(), &["find", "ladder", "--project", "attic"]);
    assert!(history.contains("Back in the work"), "{history}");
    assert!(output(data.path(), &["project", "unretire", "attic"]).contains("not retired"));
}

/// A look back keeps what a retired project did and drops what it was
/// never going to do.
#[test]
fn a_retro_keeps_its_past_and_drops_its_future() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    rigger(data.path())
        .args(["version", "add", "attic", "v0.1.0", "--week"])
        .arg(week(0))
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "attic", "--kind", "shipped", "--tag", "v0.1.0"])
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "add", "attic", "v0.1.5", "--week"])
        .arg(week(-1))
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "plan", "attic", "v0.2.0", "--week", &week(1)])
        .assert()
        .success();
    retire(data.path());

    let retro: Value = serde_json::from_str(&output(data.path(), &["retro", "--weeks", "3", "--to", &week(1), "--json"])).unwrap();
    let named = |list: &str| -> Vec<String> {
        retro[list]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["project"] == "attic")
            .map(|v| v["version"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(named("shipped"), ["v0.1.0"], "what it shipped happened: {retro}");
    assert_eq!(named("missed"), ["v0.1.5"], "aimed before it left is missed; aimed after is not: {retro}");
    let standing = retro["standings"].as_array().unwrap().iter().find(|s| s["project"] == "attic").unwrap();
    assert!(standing["tier"].is_null(), "no tier is held against it: {standing}");
}

/// Its checkout is still on disk, and a sweep of the directory finds it.
#[test]
fn adopt_leaves_a_retired_checkout_alone() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    retire(data.path());

    let said = output(data.path(), &["adopt", data.path().to_str().unwrap()]);
    assert!(said.contains("retired since"), "{said}");
    assert!(output(data.path(), &["context", "attic"]).contains("Retired since "));
}
