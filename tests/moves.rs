//! What moves the queue on its own: what one project needs of another.
//!
//! On records built for the purpose, with real manifests: a release of a library
//! that breaks its users moves them up, an order built moves up who asked for it,
//! a stage waiting moves up what it waits on, and each of them is settled by the
//! next release rather than by anybody remembering.

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

/// The record's clock is whole seconds, and these tests are about what came
/// after what.
fn tick() {
    std::thread::sleep(std::time::Duration::from_millis(1100));
}

/// A project with a manifest, one release behind it and a stage planned.
fn project(data: &Path, name: &str, manifest: &str) -> PathBuf {
    let root = data.join("repos").join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("package.json"), manifest).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    commit(&root, "a.txt", "feat: the first thing");
    git(&root, &["tag", "v0.1.0"]);
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data)
        .args(["version", "add", name, "v0.2.0", "--title", "Second", "--task", "a task"])
        .assert()
        .success();
    rigger(data)
        .args(["version", "add", name, "v0.3.0", "--title", "Third", "--task", "a task"])
        .assert()
        .success();
    rigger(data).args(["sync", name]).assert().success();
    tick();
    root
}

const LIBRARY: &str = r#"{ "name": "widget-ui" }"#;
const APP: &str = r#"{ "name": "app", "private": true, "dependencies": { "widget-ui": "^0.1.0" } }"#;
const LONER: &str = r#"{ "name": "loner", "private": true }"#;

fn queue(data: &Path) -> Value {
    let out = rigger(data).args(["queue", "--json"]).assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn entry<'a>(found: &'a Value, name: &str) -> &'a Value {
    found["order"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["project"] == name)
        .unwrap_or_else(|| panic!("{name} is not in the order: {found:#}"))
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn release(root: &Path, message: &str, tag: &str) {
    commit(root, &format!("{tag}.txt"), message);
    git(root, &["tag", tag]);
    tick();
}

/// A release that breaks its users moves them up, and only them; nobody needs
/// to do anything but ship to settle it. One that does not break them moves
/// nothing: they take it up in their own turn.
#[test]
fn a_breaking_release_moves_up_who_uses_it() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let library = project(data.path(), "library", LIBRARY);
    let app = project(data.path(), "app", APP);
    project(data.path(), "loner", LONER);

    release(&library, "feat: a new part", "v0.2.0");
    let found = queue(data.path());
    assert_eq!(entry(&found, "app")["reason"], "turn", "an ordinary release moves nothing: {found:#}");

    release(&library, "feat!: change the part", "v0.3.0");
    let found = queue(data.path());
    assert_eq!(found["order"][0]["project"], "app", "{found:#}");
    assert_eq!(entry(&found, "app")["reason"], "moved");
    assert_eq!(entry(&found, "app")["moved"]["head"], "library broke what it uses: feat: change the part");
    assert_eq!(entry(&found, "loner")["reason"], "turn", "{found:#}");
    assert!(text(data.path(), &["queue"]).contains("moved up: library broke what it uses"));

    // Putting it back cannot undo this: what it uses has still broken.
    assert!(text(data.path(), &["queue", "back", "app"]).contains("stays moved up"));

    // Its release settles it, and it has spent its turn, not gained one.
    release(&app, "fix: take up the change", "v0.2.0");
    let found = queue(data.path());
    assert_eq!(entry(&found, "app")["reason"], "turn", "{found:#}");
    assert_eq!(entry(&found, "app")["waited"], 0, "{found:#}");
}

/// A change that broke something and has not been released has reached nobody.
#[test]
fn a_breaking_change_not_released_moves_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let library = project(data.path(), "library", LIBRARY);
    project(data.path(), "app", APP);
    commit(&library, "b.txt", "feat!: change the part");
    let found = queue(data.path());
    assert_eq!(entry(&found, "app")["reason"], "turn", "{found:#}");
    assert_eq!(entry(&found, "library")["reason"], "unfinished", "{found:#}");
}

/// A tie in the record counts the same as a manifest: what reads a command's
/// output has no dependency to name.
#[test]
fn a_tie_in_the_record_is_a_use() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let tool = project(data.path(), "tool", LONER);
    project(data.path(), "reader", LONER);
    rigger(data.path())
        .args(["link", "add", "reader", "tool", "--kind", "consumer", "--note", "reads its JSON"])
        .assert()
        .success();

    release(&tool, "feat!: rename a field", "v0.2.0");
    let found = queue(data.path());
    assert_eq!(entry(&found, "reader")["reason"], "moved", "{found:#}");

    let uses: Value = serde_json::from_str(&text(data.path(), &["link", "uses", "--json"])).unwrap();
    assert_eq!(uses["uses"].as_array().unwrap().len(), 1, "seen from both ends, it is one: {uses:#}");
    assert_eq!(uses["uses"][0]["consumer"], "reader");
    assert_eq!(uses["uses"][0]["supplier"], "tool");
    assert_eq!(uses["uses"][0]["note"], "reads its JSON");
}

/// Set aside on purpose, a project stays aside: only the owner's word brings it
/// back. And one that has not released yet has nothing to catch up with.
#[test]
fn a_project_set_aside_is_not_pulled_back() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let library = project(data.path(), "library", LIBRARY);
    project(data.path(), "app", APP);
    rigger(data.path()).args(["priority", "app", "0"]).assert().success();
    release(&library, "feat!: change the part", "v0.2.0");
    let found = queue(data.path());
    assert_eq!(found["left_out"][0]["project"], "app", "{found:#}");
    assert_eq!(found["left_out"][0]["why"], "out");
}

/// An order built moves up who asked for it - once it is released, and not
/// for an order only sorted into the plan, which is a promise and not a part.
#[test]
fn an_order_built_moves_up_who_asked() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let library = project(data.path(), "library", LIBRARY);
    let app = project(data.path(), "app", APP);
    let order = |text: &str| {
        let out = rigger(data.path())
            .args(["wish", "library", text, "--from", "app", "--json"])
            .assert()
            .success();
        let said: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
        said["event"]["id"].as_i64().unwrap().to_string()
    };

    let sorted = order("A slider");
    rigger(data.path()).args(["resolve", "library", &sorted]).assert().success();
    assert_eq!(entry(&queue(data.path()), "app")["reason"], "turn");

    let built = order("A date picker");
    commit(&library, "b.txt", "feat: a date picker");
    assert!(text(data.path(), &["resolve", "library", &built, "--built"]).contains(&format!("Built [{built}]")));
    let found = queue(data.path());
    assert_eq!(entry(&found, "app")["reason"], "turn", "built and not released: {found:#}");

    git(&library, &["tag", "v0.2.0"]);
    tick();
    let found = queue(data.path());
    assert_eq!(entry(&found, "app")["reason"], "moved", "{found:#}");
    assert_eq!(entry(&found, "app")["moved"]["head"], "library built what it asked for: A date picker");

    release(&app, "feat: use the date picker", "v0.2.0");
    assert_eq!(entry(&queue(data.path()), "app")["reason"], "turn");

    // What was built for someone else is theirs.
    project(data.path(), "other", APP);
    let out = rigger(data.path())
        .args(["wish", "library", "A chart", "--from", "other", "--json"])
        .assert()
        .success();
    let said: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let id = said["event"]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["resolve", "library", &id, "--built"]).assert().success();
    let found = queue(data.path());
    assert_eq!(entry(&found, "other")["reason"], "moved", "{found:#}");
    assert_eq!(entry(&found, "app")["reason"], "turn", "{found:#}");

    // Only a wish is built.
    let question = rigger(data.path())
        .args(["note", "library", "Which colour?", "--kind", "question", "--json"])
        .assert()
        .success();
    let question: Value = serde_json::from_slice(&question.get_output().stdout).unwrap();
    let id = question["event"]["id"].as_i64().unwrap().to_string();
    rigger(data.path())
        .args(["resolve", "library", &id, "--built"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("only a wish is built"));
}

/// A stage waiting on a neighbour moves the neighbour up, and says so in its
/// packet; resolving the order settles it, whichever way.
#[test]
fn a_stage_waiting_moves_up_what_it_waits_on() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "app", APP);
    project(data.path(), "library", LIBRARY);
    let before = queue(data.path());
    assert_eq!(before["order"][0]["project"], "app", "the older release comes first: {before:#}");

    // An order alone waits its turn.
    rigger(data.path())
        .args(["wish", "library", "A date picker", "--from", "app"])
        .assert()
        .success();
    assert_eq!(queue(data.path())["order"][0]["project"], "app");

    let out = rigger(data.path())
        .args(["wish", "library", "A reader block", "--from", "app", "--waits", "--json"])
        .assert()
        .success();
    let said: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let id = said["event"]["id"].as_i64().unwrap().to_string();
    let found = queue(data.path());
    assert_eq!(found["order"][0]["project"], "library", "{found:#}");
    assert_eq!(found["order"][0]["moved"]["head"], "app waits on it: A reader block");
    let packet = text(data.path(), &["context", "library"]);
    assert!(packet.contains("(app asks, and its stage waits on this) A reader block"), "{packet}");

    rigger(data.path()).args(["resolve", "library", &id, "--built"]).assert().success();
    assert_eq!(queue(data.path())["order"][0]["project"], "app");

    // `--waits` says whose stage: it means nothing without one.
    rigger(data.path()).args(["wish", "library", "Something", "--waits"]).assert().failure();
}

/// `link uses` says what the manifests say.
#[test]
fn what_is_used_is_listed() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "library", LIBRARY);
    project(data.path(), "app", APP);
    project(data.path(), "loner", LONER);

    let said = text(data.path(), &["link", "uses"]);
    assert_eq!(said.trim(), "app uses library - widget-ui ^0.1.0");
    assert_eq!(
        text(data.path(), &["link", "uses", "loner"]).trim(),
        "Nothing is drawn on: no manifest names a neighbour's package, and no tie says so."
    );
    assert!(text(data.path(), &["link", "uses", "library"]).contains("app uses library"));
    rigger(data.path()).args(["link", "uses", "no-such"]).assert().failure();
}
