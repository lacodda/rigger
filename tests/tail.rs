//! A plan's tail, the numbers that steer it, and the big ideas it raises.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use serde_json::Value;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit(root: &Path, file: &str, message: &str) {
    std::fs::write(root.join(file), message).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", message]);
}

/// A project with a release behind it and the given number of versions planned.
fn project(data: &Path, name: &str, planned: usize) -> PathBuf {
    let root = data.join("repos").join(name);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    commit(&root, "a.txt", "feat: the first thing");
    git(&root, &["tag", "v0.1.0"]);
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    for n in 0..planned {
        let version = format!("v0.{}.0", n + 2);
        rigger(data).args(["version", "add", name, &version, "--task", "a task"]).assert().success();
    }
    rigger(data).args(["sync", name]).assert().success();
    root
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn json(data: &Path, args: &[&str]) -> Value {
    let out = rigger(data).args(args).arg("--json").assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn tail(data: &Path, name: &str) -> Value {
    json(data, &["tail", name])["tails"][0].clone()
}

fn init(data: &Path) {
    rigger(data).arg("init").assert().success();
}

/// At the floor a turn starts by growing the plan, and the first message says
/// so; above it nothing is asked. Once gone through, the same turn does not ask
/// again, and a release starts the next.
#[test]
fn a_short_tail_asks_to_be_grown_once_a_turn() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    let short = project(data.path(), "short", 3);
    project(data.path(), "long", 4);

    assert_eq!(tail(data.path(), "short")["due"], "grow");
    assert_eq!(tail(data.path(), "short")["planned"], 3);
    assert!(tail(data.path(), "long")["due"].is_null());
    assert!(text(data.path(), &["tail"]).contains("3 versions planned, at or under 3 - grow it"));

    let message = json(data.path(), &["go", "short"])["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("First the plan: 3 versions left, so grow it as `rigger tail ritual` says"),
        "{message}"
    );
    assert!(!message.contains(['\n', '"', '%']), "{message}");
    let packet = text(data.path(), &["context", "short"]);
    assert!(packet.contains("grow it"), "{packet}");
    let message = json(data.path(), &["go", "long"])["message"].as_str().unwrap().to_string();
    assert!(!message.contains("First the plan"), "{message}");

    assert!(text(data.path(), &["tail", "done", "short", "Nothing good to add yet"]).contains("short: 3 versions planned"));
    assert!(tail(data.path(), "short")["due"].is_null());
    assert!(!json(data.path(), &["go", "short"])["message"].as_str().unwrap().contains("First the plan"));

    // A release is a new turn, and the question comes back.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    commit(&short, "b.txt", "feat: the second thing");
    git(&short, &["tag", "v0.2.0"]);
    rigger(data.path()).args(["sync", "short"]).assert().success();
    assert_eq!(tail(data.path(), "short")["due"], "grow");
    assert_eq!(tail(data.path(), "short")["planned"], 2);
}

/// A project with nothing planned stays in the queue: its turn is what plans it.
#[test]
fn a_project_with_no_plan_is_in_the_queue() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "bare", 0);
    let found = json(data.path(), &["queue"]);
    assert_eq!(found["order"][0]["project"], "bare", "{found:#}");
    assert_eq!(found["order"][0]["version"], "unplanned");
    let message = json(data.path(), &["go"])["message"].as_str().unwrap().to_string();
    assert!(message.contains("no stage is planned, so plan one"), "{message}");
    assert!(message.contains("0 versions left"), "{message}");
}

/// A folder has no such plan, and a project set aside is not asked about.
#[test]
fn a_folder_and_a_project_set_aside_are_not_asked() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "aside", 0);
    rigger(data.path()).args(["priority", "aside", "0"]).assert().success();
    assert!(tail(data.path(), "aside")["due"].is_null());
    let notes = data.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&notes).assert().success();
    assert!(tail(data.path(), "notes")["due"].is_null());
}

/// A long plan is only sorted, and only when it has waited long enough; the
/// numbers can be set for the record or for one project, and cleared.
#[test]
fn the_numbers_steer_it_and_a_project_can_have_its_own() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "alpha", 5);
    project(data.path(), "beta", 5);

    let said = text(data.path(), &["config"]);
    assert!(said.contains("tail.floor"), "{said}");
    assert!(said.contains("tail.sort_days"), "{said}");
    let found = json(data.path(), &["config", "tail.floor"]);
    assert_eq!(found["settings"].as_array().unwrap().len(), 1);
    assert_eq!(found["settings"][0]["value"], 3);

    assert!(tail(data.path(), "alpha")["due"].is_null());
    text(data.path(), &["config", "tail.floor", "5"]);
    assert_eq!(tail(data.path(), "alpha")["due"], "grow");
    assert_eq!(tail(data.path(), "beta")["due"], "grow");
    assert!(text(data.path(), &["config", "tail.floor"]).contains("5 (default 3)"));

    // One project's own, beside the record's.
    text(data.path(), &["config", "tail.floor", "1", "--project", "beta"]);
    assert!(tail(data.path(), "beta")["due"].is_null());
    assert_eq!(tail(data.path(), "alpha")["due"], "grow");
    assert!(text(data.path(), &["config", "tail.floor"]).contains("1 for beta"));
    text(data.path(), &["config", "tail.floor", "--clear", "--project", "beta"]);
    assert_eq!(tail(data.path(), "beta")["due"], "grow");
    text(data.path(), &["config", "tail.floor", "--clear"]);
    assert!(tail(data.path(), "alpha")["due"].is_null());

    // Sorting comes round by days; 0 days since is already due at 0, and a pass resets it.
    text(data.path(), &["config", "tail.sort_days", "0"]);
    assert!(tail(data.path(), "alpha")["due"].is_null(), "0 turns sorting off");

    // The cap is said, not enforced: it is the ritual that stops adding.
    text(data.path(), &["config", "tail.cap", "4"]);
    assert!(text(data.path(), &["tail", "alpha"]).contains("at the cap of 4: small ideas wait"));

    for (args, says) in [
        (vec!["config", "no.such", "1"], "there is no setting"),
        (vec!["config", "no.such"], "there is no setting"),
        (vec!["config", "tail.floor", "--", "-1"], "cannot be below 0"),
    ] {
        rigger(data.path()).args(&args).assert().failure().stderr(predicates::str::contains(says));
    }
}

/// Days counted against the project's age when it was never gone through, and
/// against the pass when it was.
#[test]
fn a_long_plan_is_sorted_when_it_has_waited() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "alpha", 5);
    rigger(data.path()).args(["wish", "alpha", "One more thing"]).assert().success();

    let db = data.path().join("profiles").join("line").join("rigger.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute("UPDATE projects SET created_at = '2026-01-01T00:00:00Z'", []).unwrap();
    drop(conn);

    let found = tail(data.path(), "alpha");
    assert_eq!(found["due"], "sort", "{found:#}");
    assert_eq!(found["unsorted"], 1);
    let message = json(data.path(), &["go", "alpha"])["message"].as_str().unwrap().to_string();
    assert!(message.contains("First the plan: sort what has piled up"), "{message}");

    rigger(data.path()).args(["tail", "done", "alpha", "Sorted one wish"]).assert().success();
    assert!(tail(data.path(), "alpha")["due"].is_null());
    assert!(text(data.path(), &["find", "Sorted one wish"]).contains("Sorted one wish"));
}

/// A big idea waits apart from the inbox, comes up when it is time to ask, and
/// the answer either gives it a place among the wishes or puts it away.
#[test]
fn big_ideas_are_asked_about_together() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "alpha", 5);
    assert!(text(data.path(), &["ideas"]).contains("No big ideas wait"));
    assert!(!text(data.path(), &["queue"]).contains("big idea"));

    let raised = |text_: &str| {
        let found = json(data.path(), &["ideas", "add", "alpha", text_]);
        found["ideas"].as_array().unwrap().last().unwrap()["id"].as_i64().unwrap().to_string()
    };
    let take = raised("A companion program for the phone");
    let drop = raised("Rewrite it in another language");

    // Waiting apart from the questions, and said on the queue when it is time.
    assert!(text(data.path(), &["ideas"]).contains("2 big ideas wait"));
    assert!(text(data.path(), &["ideas"]).contains("A companion program"));
    assert!(text(data.path(), &["queue"]).contains("2 big ideas wait for your answer"));
    assert!(!text(data.path(), &["inbox"]).contains("companion"));
    assert!(!text(data.path(), &["context", "alpha"]).contains("companion"), "an idea is not work yet");

    // Asked: the count starts again, and the queue stops saying so.
    assert_eq!(json(data.path(), &["ideas"])["due"], true);
    text(data.path(), &["ideas", "asked"]);
    assert_eq!(json(data.path(), &["ideas"])["due"], false);
    assert!(!text(data.path(), &["queue"]).contains("big idea"));

    let said = text(data.path(), &["ideas", "take", "alpha", &take, "--note", "Yes, after the next major"]);
    assert!(said.contains(&format!("Taken [{take}]")), "{said}");
    let packet = json(data.path(), &["context", "alpha"]);
    assert_eq!(packet["wishes"][0]["head"], "A companion program for the phone", "it is a wish now: {packet:#}");
    assert!(text(data.path(), &["find", "after the next major"]).contains("decision"));

    assert!(text(data.path(), &["ideas", "drop", "alpha", &drop]).contains(&format!("Declined [{drop}]")));
    assert!(text(data.path(), &["ideas"]).contains("No big ideas wait"));
    assert!(!text(data.path(), &["context", "alpha"]).contains("another language"));

    // Only an idea is answered this way.
    rigger(data.path())
        .args(["ideas", "take", "alpha", &drop])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not an idea"));
    rigger(data.path()).args(["ideas", "take", "alpha", "99999"]).assert().failure();
}

#[test]
fn the_ritual_says_how() {
    let data = tempfile::tempdir().unwrap();
    let said = text(data.path(), &["tail", "ritual"]);
    assert!(said.contains("Small or big"), "{said}");
    assert!(said.contains("rigger ideas add"), "{said}");
    assert!(said.contains("rigger tail done"), "{said}");
}
