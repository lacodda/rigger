//! `rigger pick` on a record built for the purpose.
//!
//! What is under test is the rule and the facts it stands on, not the
//! wording: that git is read before choosing, so a commit nobody synced
//! still counts; that the calendar outranks everything; that what the rule
//! leaves out is named with its reason; and that the brief and the MCP tool
//! name the same project the command does.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as TestCommand;
use serde_json::Value;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn git(dir: &Path, args: &[&str], at: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    if let Some(at) = at {
        cmd.env("GIT_AUTHOR_DATE", at).env("GIT_COMMITTER_DATE", at);
    }
    let out = cmd.output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit(root: &Path, file: &str, message: &str, at: Option<&str>) {
    std::fs::write(root.join(file), message).unwrap();
    git(root, &["add", "."], at);
    git(root, &["commit", "--quiet", "-m", message], at);
}

/// A project with a plan of the given stages and one release, v0.1.0, made
/// long before the test runs.
fn project(data: &Path, name: &str, stages: &[&str]) -> std::path::PathBuf {
    let root = data.join(name);
    let hub = data.join("hubs").join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&hub).unwrap();

    let mut plan = String::from("# План\n\n");
    for stage in stages {
        plan.push_str(&format!("## {stage}\n\n- [ ] a task\n\n"));
    }
    std::fs::write(hub.join("План.md"), plan).unwrap();

    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);
    let at = Some("2026-08-10T12:00:00Z");
    commit(&root, "a.txt", "feat: the first thing", at);
    git(&root, &["tag", "v0.1.0"], at);

    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    root
}

/// A sitting, opened and closed; the record keeps seconds, so the clock is
/// moved past the one it ended in.
fn sitting(data: &Path, name: &str) {
    rigger(data).args(["session", "start", name]).assert().success();
    rigger(data).args(["session", "end", name]).assert().success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
}

fn pick(data: &Path) -> Value {
    let out = rigger(data).args(["pick", "--json"]).assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn order(found: &Value) -> Vec<String> {
    std::iter::once(&found["pick"])
        .chain(found["then"].as_array().unwrap())
        .filter(|c| !c.is_null())
        .map(|c| c["project"].as_str().unwrap().to_string())
        .collect()
}

/// A change nobody synced still counts, because the pick reads git itself; and
/// it holds the line up however it was made - from a neighbour's sitting, or by
/// a sitting that knew and chose not to tag. Only a release settles it.
#[test]
fn a_commit_nobody_synced_puts_its_project_first_until_it_is_released() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "waiting", &["v0.2.0 · Second"]);
    let busy = project(data.path(), "busy", &["v0.2.0 · Second"]);
    sitting(data.path(), "waiting");
    sitting(data.path(), "busy");

    // Nothing presses yet: the one whose sitting is further back is first.
    assert_eq!(order(&pick(data.path())), vec!["waiting", "busy"]);
    assert_eq!(pick(data.path())["pick"]["reason"], "turn");

    commit(&busy, "b.txt", "feat(plugin): generate plugins for a neighbour", None);

    let found = pick(data.path());
    assert_eq!(order(&found), vec!["busy", "waiting"], "{found:#}");
    assert_eq!(found["pick"]["reason"], "unfinished");
    assert_eq!(found["pick"]["version"], "v0.2.0");
    assert_eq!(found["pick"]["unfinished"]["untagged"][0], "feat(plugin): generate plugins for a neighbour");

    let said = text(data.path(), &["pick"]);
    assert!(said.starts_with("Work on busy  v0.2.0 · Second"), "{said}");
    assert!(said.contains("not finished: 1 change since the last release"), "{said}");

    // A sitting that has seen the change does not settle it.
    sitting(data.path(), "busy");
    assert_eq!(pick(data.path())["pick"]["project"], "busy");

    // The tag does.
    git(&busy, &["tag", "v0.2.0"], None);
    let found = pick(data.path());
    assert!(found["pick"]["unfinished"].is_null(), "{found:#}");
    assert_eq!(found["pick"]["project"], "waiting", "{found:#}");
}

/// The measure the rule was drawn from: on the real line, a docs commit and a
/// chore commit after a tag sat in two projects with nothing wrong with them.
/// They wait for the next tag without holding anything up.
#[test]
fn docs_and_chores_after_a_tag_are_not_unfinished() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let root = project(data.path(), "alpha", &["v0.2.0 · Second"]);
    sitting(data.path(), "alpha");
    commit(&root, "b.txt", "docs: say how it is used", None);
    commit(&root, "c.txt", "chore(deps): take a newer lockfile", None);
    let found = pick(data.path());
    assert_eq!(found["pick"]["reason"], "turn", "{found:#}");
    assert!(found["pick"]["unfinished"].is_null(), "{found:#}");

    commit(&root, "d.txt", "fix: load the rows", None);
    let found = pick(data.path());
    assert_eq!(found["pick"]["reason"], "unfinished", "{found:#}");
    assert_eq!(found["pick"]["unfinished"]["untagged"].as_array().unwrap().len(), 1, "{found:#}");
}

/// Everything else git can say a repository holds, read from a real one.
#[test]
fn what_a_repository_holds_is_read_from_it() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let root = project(data.path(), "alpha", &["v0.2.0 · Second"]);
    sitting(data.path(), "alpha");
    let unfinished = || pick(data.path())["pick"]["unfinished"].clone();
    assert!(unfinished().is_null());

    std::fs::write(root.join("new.txt"), "new").unwrap();
    assert_eq!(unfinished()["untracked"], 1);
    std::fs::remove_file(root.join("new.txt")).unwrap();

    std::fs::write(root.join("a.txt"), "changed").unwrap();
    assert_eq!(unfinished()["changed"], 1);
    git(&root, &["add", "a.txt"], None);
    assert_eq!(unfinished()["changed"], 1, "staged is still not committed");
    git(&root, &["reset", "--quiet", "--hard"], None);
    assert!(unfinished().is_null());

    // A stash nobody means to finish by the next tag holds nothing up.
    std::fs::write(root.join("a.txt"), "stashed").unwrap();
    git(&root, &["stash", "--quiet"], None);
    assert!(unfinished().is_null(), "a stash stopped the line");
}

/// A repository with a remote: a commit that has not reached it holds the line
/// up, and so does a tag that has not. The fixture's remote is a path, which a
/// scratch record may ask.
#[test]
fn what_has_not_reached_the_remote_is_unfinished() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let root = project(data.path(), "alpha", &["v0.2.0 · Second", "v0.3.0 · Third"]);
    let bare = data.path().join("alpha.git");
    git(
        data.path(),
        &["init", "--quiet", "--bare", "--initial-branch", "main", bare.to_str().unwrap()],
        None,
    );
    git(&root, &["remote", "add", "origin", bare.to_str().unwrap()], None);
    git(&root, &["push", "--quiet", "--set-upstream", "origin", "main", "v0.1.0"], None);
    sitting(data.path(), "alpha");
    let unfinished = || pick(data.path())["pick"]["unfinished"].clone();
    assert!(unfinished().is_null(), "{}", unfinished());

    commit(&root, "b.txt", "docs: a line nobody has pushed", None);
    assert_eq!(unfinished()["unpushed"], 1);
    assert!(text(data.path(), &["pick"]).contains("1 commit not pushed"));
    git(&root, &["push", "--quiet"], None);
    assert!(unfinished().is_null(), "{}", unfinished());

    git(&root, &["tag", "v0.2.0"], None);
    assert_eq!(unfinished()["undelivered"], "v0.2.0: the tag is not on the remote");
    git(&root, &["push", "--quiet", "origin", "v0.2.0"], None);
    assert!(unfinished().is_null(), "{}", unfinished());

    // A release the record knows fell short holds it up as well, and one it
    // knows got out is not asked about again.
    rigger(data.path())
        .args(["note", "alpha", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "npm"])
        .assert()
        .success();
    git(data.path(), &["--git-dir", bare.to_str().unwrap(), "tag", "--delete", "v0.2.0"], None);
    assert!(unfinished().is_null(), "the record's word was asked about again: {}", unfinished());
}

/// Aiming a version at the week is the owner asking for it: it outranks
/// everything else, even a project the rule would leave out.
#[test]
fn the_calendar_comes_first() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "busy", &["v0.2.0 · Second"]);
    project(data.path(), "aimed", &["v0.2.0 · Second", "v0.3.0 · Third"]);
    sitting(data.path(), "aimed");
    sitting(data.path(), "busy");
    rigger(data.path()).args(["project", "tier", "aimed", "out"]).assert().success();

    let now = jiff::Zoned::now().date().iso_week_date();
    let now = format!("{}-W{:02}", now.year(), now.week());
    rigger(data.path())
        .args(["version", "plan", "aimed", "v0.3.0", "--week", &now])
        .assert()
        .success();
    let found = pick(data.path());
    assert_eq!(order(&found), vec!["aimed", "busy"], "{found:#}");
    assert_eq!(found["pick"]["reason"], "due");
    assert_eq!(found["pick"]["version"], "v0.3.0");
    assert!(found["pick"]["planned"].is_string(), "{found:#}");
}

/// What the rule does not consider is named, each with its reason, rather
/// than vanishing from the screen.
#[test]
fn what_is_left_out_is_named_with_its_reason() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "working", &["v0.2.0 · Second"]);
    project(data.path(), "aside", &["v0.2.0 · Second"]);
    project(data.path(), "bare", &[]);
    project(data.path(), "frozen", &["v0.2.0 · Second"]);
    rigger(data.path()).args(["project", "tier", "aside", "out"]).assert().success();
    let stage = text(data.path(), &["version", "show", "frozen", "--json"]);
    let stage: Value = serde_json::from_str(&stage).unwrap();
    let task = stage["tasks"][0]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["task", "freeze", &task]).assert().success();

    let found = pick(data.path());
    assert_eq!(order(&found), vec!["working"], "{found:#}");
    let aside: Vec<(String, String)> = found["left_out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["project"].as_str().unwrap().to_string(), l["why"].as_str().unwrap().to_string()))
        .collect();
    assert_eq!(
        aside,
        vec![
            ("aside".to_string(), "out".to_string()),
            ("bare".to_string(), "no-stage".to_string()),
            ("frozen".to_string(), "put-down".to_string()),
        ]
    );
    let said = text(data.path(), &["pick"]);
    assert!(said.contains("Left out"), "{said}");
    assert!(said.contains("everything planned is put down"), "{said}");
}

/// An answer the owner gave from the inbox, after the project's last
/// sitting, waits to be acted on.
#[test]
fn an_answer_given_outside_a_sitting_is_a_reason() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "asked", &["v0.2.0 · Second"]);
    project(data.path(), "quiet", &["v0.2.0 · Second"]);
    sitting(data.path(), "asked");
    sitting(data.path(), "quiet");
    let question = text(data.path(), &["note", "asked", "Which colour?", "--kind", "question", "--json"]);
    let question: Value = serde_json::from_str(&question).unwrap();
    let id = question["event"]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["resolve", "asked", &id, "Blue"]).assert().success();

    let found = pick(data.path());
    assert_eq!(order(&found), vec!["asked", "quiet"], "{found:#}");
    assert_eq!(found["pick"]["reason"], "answered");
    assert_eq!(found["pick"]["answered"], 1);
}

/// The brief and the command are one answer: when nothing is aimed at the
/// week, the brief names the project `pick` names - reading git first, as
/// `pick` does, so a commit nobody synced moves both alike.
#[test]
fn the_brief_names_the_same_pick() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "quiet", &["v0.2.0 · Second"]);
    let alpha = project(data.path(), "alpha", &["v0.2.0 · Second"]);
    sitting(data.path(), "quiet");
    sitting(data.path(), "alpha");
    commit(&alpha, "b.txt", "feat: work from a neighbour", None);

    let brief = text(data.path(), &["week"]);
    assert!(
        brief.contains("nothing is aimed at this week - rigger pick names alpha v0.2.0 (1 change since the last release)"),
        "{brief}"
    );
    let brief: Value = serde_json::from_str(&text(data.path(), &["week", "--json"])).unwrap();
    assert_eq!(brief["pick"]["project"], "alpha");
    assert_eq!(brief["pick"]["reason"], "unfinished");
    assert_eq!(brief["pick"]["reason"], pick(data.path())["pick"]["reason"]);

    // Another week's brief names no pick: the pick is about now.
    let other: Value = serde_json::from_str(&text(data.path(), &["week", "--week", "2026-W01", "--json"])).unwrap();
    assert!(other["pick"].is_null(), "{other:#}");
}
