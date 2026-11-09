//! `rigger queue` on a record built for the purpose.
//!
//! What is under test is the rule and the facts it stands on, not the wording:
//! that git is read before choosing, so a commit nobody synced still counts;
//! that only a release settles what is unfinished; that a weight moves the order
//! at once; that what the rule leaves out is named with its reason; and that the
//! MCP tool and the command agree.

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

/// A project with a plan of the given stages and one release, v0.1.0, made at
/// the moment given: the order of the moments is the order of the turns.
fn project_released(data: &Path, name: &str, stages: &[&str], released: &str) -> std::path::PathBuf {
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
    commit(&root, "a.txt", "feat: the first thing", Some(released));
    git(&root, &["tag", "v0.1.0"], Some(released));

    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    root
}

fn project(data: &Path, name: &str, stages: &[&str]) -> std::path::PathBuf {
    project_released(data, name, stages, "2026-08-10T12:00:00Z")
}

/// A sitting, opened and closed.
fn sitting(data: &Path, name: &str) {
    rigger(data).args(["session", "start", name]).assert().success();
    rigger(data).args(["session", "end", name]).assert().success();
}

fn queue(data: &Path) -> Value {
    let out = rigger(data).args(["queue", "--json"]).assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn order(found: &Value) -> Vec<String> {
    found["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["project"].as_str().unwrap().to_string())
        .collect()
}

fn init(data: &Path) {
    rigger(data).arg("init").assert().success();
}

/// A change nobody synced still counts, because the queue reads git itself; and
/// it holds the line up however it was made - from a neighbour's sitting, or by
/// a sitting that knew and chose not to tag. Only a release settles it.
#[test]
fn a_commit_nobody_synced_puts_its_project_first_until_it_is_released() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "aaa", &["v0.2.0 · Second"]);
    let busy = project(data.path(), "busy", &["v0.2.0 · Second"]);

    // Nothing presses yet, and level on everything: the name decides.
    assert_eq!(order(&queue(data.path())), vec!["aaa", "busy"]);
    assert_eq!(queue(data.path())["order"][0]["reason"], "turn");

    commit(&busy, "b.txt", "feat(plugin): generate plugins for a neighbour", None);

    let found = queue(data.path());
    assert_eq!(order(&found), vec!["busy", "aaa"], "{found:#}");
    assert_eq!(found["order"][0]["reason"], "unfinished");
    assert_eq!(found["order"][0]["version"], "v0.2.0");
    assert_eq!(found["order"][0]["unfinished"]["untagged"][0], "feat(plugin): generate plugins for a neighbour");

    let said = text(data.path(), &["queue"]);
    assert!(said.starts_with("Next: busy  v0.2.0 · Second"), "{said}");
    assert!(said.contains("not finished: 1 change since the last release"), "{said}");

    // A sitting that has seen the change does not settle it.
    sitting(data.path(), "busy");
    assert_eq!(queue(data.path())["order"][0]["project"], "busy");

    // The tag does, and that was its turn: the other one is next.
    git(&busy, &["tag", "v0.2.0"], None);
    let found = queue(data.path());
    assert!(found["order"][0]["unfinished"].is_null(), "{found:#}");
    assert_eq!(found["order"][0]["project"], "aaa", "{found:#}");
    assert_eq!(found["order"][0]["waited"], 1, "{found:#}");
}

/// The measure the rule was drawn from: on the real line, a docs commit and a
/// chore commit after a tag sat in two projects with nothing wrong with them.
/// They wait for the next tag without holding anything up.
#[test]
fn docs_and_chores_after_a_tag_are_not_unfinished() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    let root = project(data.path(), "alpha", &["v0.2.0 · Second"]);
    commit(&root, "b.txt", "docs: say how it is used", None);
    commit(&root, "c.txt", "chore(deps): take a newer lockfile", None);
    let found = queue(data.path());
    assert_eq!(found["order"][0]["reason"], "turn", "{found:#}");
    assert!(found["order"][0]["unfinished"].is_null(), "{found:#}");

    commit(&root, "d.txt", "fix: load the rows", None);
    let found = queue(data.path());
    assert_eq!(found["order"][0]["reason"], "unfinished", "{found:#}");
    assert_eq!(found["order"][0]["unfinished"]["untagged"].as_array().unwrap().len(), 1, "{found:#}");
}

/// Everything else git can say a repository holds, read from a real one.
#[test]
fn what_a_repository_holds_is_read_from_it() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    let root = project(data.path(), "alpha", &["v0.2.0 · Second"]);
    let unfinished = || queue(data.path())["order"][0]["unfinished"].clone();
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
    init(data.path());
    let root = project(data.path(), "alpha", &["v0.2.0 · Second", "v0.3.0 · Third"]);
    let bare = data.path().join("alpha.git");
    git(
        data.path(),
        &["init", "--quiet", "--bare", "--initial-branch", "main", bare.to_str().unwrap()],
        None,
    );
    git(&root, &["remote", "add", "origin", bare.to_str().unwrap()], None);
    git(&root, &["push", "--quiet", "--set-upstream", "origin", "main", "v0.1.0"], None);
    let unfinished = || queue(data.path())["order"][0]["unfinished"].clone();
    assert!(unfinished().is_null(), "{}", unfinished());

    commit(&root, "b.txt", "docs: a line nobody has pushed", None);
    assert_eq!(unfinished()["unpushed"], 1);
    assert!(text(data.path(), &["queue"]).contains("1 commit not pushed"));
    git(&root, &["push", "--quiet"], None);
    assert!(unfinished().is_null(), "{}", unfinished());

    git(&root, &["tag", "v0.2.0"], None);
    assert_eq!(unfinished()["undelivered"], "v0.2.0: the tag is not on the remote");
    git(&root, &["push", "--quiet", "origin", "v0.2.0"], None);
    assert!(unfinished().is_null(), "{}", unfinished());

    // A release the record knows fell short holds it up as well, and one it
    // knows got out is not asked about again.
    rigger(data.path())
        .args(["note", "alpha", "--kind", "shipped", "--tag", "v0.2.0"])
        .assert()
        .success();
    assert_eq!(unfinished()["undelivered"], "v0.2.0: tag exists, no release");
    rigger(data.path())
        .args(["note", "alpha", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "npm"])
        .assert()
        .success();
    git(data.path(), &["--git-dir", bare.to_str().unwrap(), "tag", "--delete", "v0.2.0"], None);
    assert!(unfinished().is_null(), "the record's word was asked about again: {}", unfinished());
}

/// A turn is a release, whoever made it and however long ago: the order counts
/// releases, not days, and one made by hand moves it like any other.
#[test]
fn the_order_goes_by_releases_since_each_projects_last() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project_released(data.path(), "old", &["v0.2.0 · Second"], "2026-08-01T12:00:00Z");
    project_released(data.path(), "middle", &["v0.2.0 · Second"], "2026-08-02T12:00:00Z");
    let new = project_released(data.path(), "new", &["v0.2.0 · Second", "v0.3.0 · Third"], "2026-08-03T12:00:00Z");

    let found = queue(data.path());
    assert_eq!(order(&found), vec!["old", "middle", "new"], "{found:#}");
    assert_eq!(found["order"][0]["waited"], 2);
    assert_eq!(found["order"][2]["waited"], 0);
    assert!(text(data.path(), &["queue"]).contains("its turn: 2 releases since its last, at weight 2"));

    // A sitting is not a turn.
    sitting(data.path(), "old");
    assert_eq!(order(&queue(data.path())), vec!["old", "middle", "new"]);

    // A release is, and it goes to the back of the line.
    commit(&new, "b.txt", "docs: the release", Some("2026-08-04T12:00:00Z"));
    git(&new, &["tag", "v0.2.0"], Some("2026-08-04T12:00:00Z"));
    let found = queue(data.path());
    assert_eq!(found["order"][0]["waited"], 3, "the release counted for the rest: {found:#}");
    assert_eq!(found["order"][2]["project"], "new", "{found:#}");
}

/// A weight moves the order at once, with nothing to recompute: the same record
/// reads differently. Up, down and a number; 0 leaves the queue; a word that is
/// none of those changes nothing.
#[test]
fn a_weight_moves_the_order_at_once() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project_released(data.path(), "older", &["v0.2.0 · Second"], "2026-08-01T12:00:00Z");
    project_released(data.path(), "newer", &["v0.2.0 · Second"], "2026-08-02T12:00:00Z");
    assert_eq!(order(&queue(data.path())), vec!["older", "newer"]);

    // (1 + 1) * 2 against (0 + 1) * 2, until newer is worth more than that.
    assert!(text(data.path(), &["priority", "newer", "up"]).contains("newer has weight 3"));
    assert_eq!(order(&queue(data.path())), vec!["older", "newer"]);
    assert!(text(data.path(), &["priority", "newer", "5"]).contains("newer has weight 5"));
    assert_eq!(order(&queue(data.path())), vec!["newer", "older"]);
    assert!(text(data.path(), &["priority", "newer", "down"]).contains("newer has weight 4"));
    assert_eq!(order(&queue(data.path())), vec!["newer", "older"]);

    let found: Value = serde_json::from_str(&text(data.path(), &["priority", "--json"])).unwrap();
    assert_eq!(found["weights"][0]["project"], "newer");
    assert_eq!(found["weights"][0]["weight"], 4);
    let said = text(data.path(), &["priority"]);
    assert!(said.contains("4 turns in every 6"), "{said}");
    assert!(said.contains("2 turns in every 6"), "{said}");
    assert!(text(data.path(), &["priority", "newer"]).contains("newer has weight 4"));

    assert!(text(data.path(), &["priority", "newer", "0"]).contains("out of the queue"));
    assert_eq!(order(&queue(data.path())), vec!["older"]);
    assert_eq!(queue(data.path())["left_out"][0]["why"], "out");
    assert!(text(data.path(), &["priority"]).contains("out of the queue"));
    // Down from nothing stays at nothing.
    assert!(text(data.path(), &["priority", "newer", "down"]).contains("out of the queue"));

    rigger(data.path())
        .args(["priority", "newer", "higher"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a change of weight"));
    rigger(data.path()).args(["priority", "no-such", "up"]).assert().failure();
}

/// "It cannot wait" holds until its next release settles it, needs nobody to
/// undo it, and can be taken back.
#[test]
fn a_project_put_first_stays_first_until_it_ships() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project_released(data.path(), "older", &["v0.2.0 · Second"], "2026-08-01T12:00:00Z");
    let urgent = project_released(data.path(), "urgent", &["v0.2.0 · Second"], "2026-08-02T12:00:00Z");
    assert_eq!(order(&queue(data.path())), vec!["older", "urgent"]);

    let said = text(data.path(), &["queue", "first", "urgent", "--why", "needed for Monday"]);
    assert!(said.contains("urgent comes first until its next release: needed for Monday"), "{said}");
    let found = queue(data.path());
    assert_eq!(order(&found), vec!["urgent", "older"], "{found:#}");
    assert_eq!(found["order"][0]["reason"], "moved");
    assert_eq!(found["order"][0]["moved"][0]["head"], "needed for Monday");
    assert!(text(data.path(), &["queue"]).contains("moved up: needed for Monday"));

    // `rigger go` opens a sitting before the assistant starts, and sends it here
    // for what moved the project: the sitting must not hide the answer.
    rigger(data.path()).args(["session", "start", "urgent"]).assert().success();
    let found = queue(data.path());
    assert_eq!(found["order"][0]["reason"], "unfinished", "{found:#}");
    assert_eq!(found["order"][0]["moved"][0]["head"], "needed for Monday", "{found:#}");
    let said = text(data.path(), &["queue"]);
    assert!(said.contains("a sitting open since"), "{said}");
    assert!(said.contains("moved up: needed for Monday"), "{said}");
    rigger(data.path()).args(["session", "end", "urgent"]).assert().success();

    // Back in its turn.
    assert!(text(data.path(), &["queue", "back", "urgent"]).contains("urgent is back in its turn."));
    assert_eq!(order(&queue(data.path())), vec!["older", "urgent"]);
    assert!(text(data.path(), &["queue", "back", "urgent"]).contains("was not moved up"));

    // Without a reason, it is the owner's word; a release settles it.
    assert!(text(data.path(), &["queue", "first", "urgent"]).contains("the owner put it first"));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    commit(&urgent, "b.txt", "docs: the release", None);
    git(&urgent, &["tag", "v0.2.0"], None);
    let found = queue(data.path());
    assert_eq!(found["order"][0]["project"], "older", "{found:#}");
    assert_eq!(found["order"][1]["moved"], serde_json::json!([]), "{found:#}");

    // A project with no plan, put first, is asked for all the same.
    rigger(data.path()).args(["priority", "older", "0"]).assert().success();
    assert_eq!(queue(data.path())["left_out"][0]["project"], "older");
    rigger(data.path()).args(["queue", "first", "older"]).assert().success();
    assert_eq!(queue(data.path())["order"][0]["project"], "older");
}

/// What the rule does not consider is named, each with its reason, rather than
/// vanishing from the screen.
#[test]
fn what_is_left_out_is_named_with_its_reason() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "working", &["v0.2.0 · Second"]);
    project(data.path(), "aside", &["v0.2.0 · Second"]);
    project(data.path(), "frozen", &["v0.2.0 · Second"]);
    rigger(data.path()).args(["priority", "aside", "0"]).assert().success();
    let stage = text(data.path(), &["version", "show", "frozen", "--json"]);
    let stage: Value = serde_json::from_str(&stage).unwrap();
    let task = stage["tasks"][0]["id"].as_i64().unwrap().to_string();
    rigger(data.path()).args(["task", "freeze", &task]).assert().success();

    let found = queue(data.path());
    assert_eq!(order(&found), vec!["working"], "{found:#}");
    let aside: Vec<(String, String)> = found["left_out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["project"].as_str().unwrap().to_string(), l["why"].as_str().unwrap().to_string()))
        .collect();
    assert_eq!(
        aside,
        vec![("aside".to_string(), "out".to_string()), ("frozen".to_string(), "put-down".to_string()),]
    );
    let said = text(data.path(), &["queue"]);
    assert!(said.contains("Left out"), "{said}");
    assert!(said.contains("everything planned is put down"), "{said}");
}

/// A folder makes no releases: its turn is a sitting, and it needs no version
/// planned to have one.
#[test]
fn a_folders_turn_is_a_sitting_and_it_needs_no_plan() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project_released(data.path(), "repo", &["v0.2.0 · Second"], "2026-08-01T12:00:00Z");
    let notes = data.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&notes).assert().success();

    let found = queue(data.path());
    assert_eq!(order(&found), vec!["notes", "repo"], "it has never had a turn: {found:#}");
    assert_eq!(found["order"][0]["version"], "unplanned");
    assert_eq!(found["order"][0]["kind"], "folder");

    sitting(data.path(), "notes");
    let found = queue(data.path());
    assert_eq!(found["order"][0]["project"], "repo", "{found:#}");
}

/// `rigger` alone is the queue: the one screen worth seeing without asking.
/// Before there is a record there is no queue, and the help says how to start.
#[test]
fn rigger_alone_is_the_queue() {
    let data = tempfile::tempdir().unwrap();
    let before = text(data.path(), &[]);
    assert!(before.contains("Usage:"), "{before}");
    assert!(!data.path().join("profiles").exists(), "looking at the help made a record");

    init(data.path());
    project(data.path(), "alpha", &["v0.2.0 · Second"]);
    assert!(text(data.path(), &[]).starts_with("Next: alpha  v0.2.0 · Second"));
    let found: Value = serde_json::from_str(&text(data.path(), &["--json"])).unwrap();
    assert_eq!(found["order"][0]["project"], "alpha");
}

/// A program reading the help gets the whole list, and `--help --all` is that list
/// for a person. What a person sees at a terminal is a unit test of the short one.
#[test]
fn the_help_lists_everything_for_a_program() {
    let data = tempfile::tempdir().unwrap();
    for help in [text(data.path(), &["--help"]), text(data.path(), &["--help", "--all"])] {
        for command in ["go", "queue", "note", "context", "session", "export"] {
            assert!(
                help.lines().any(|l| l.trim_start().starts_with(command)),
                "{command} missing:
{help}"
            );
        }
    }
    assert!(text(data.path(), &["note", "--help"]).contains("--kind"));
}

/// The forecast, from a real record: a project moves on through its own plan,
/// and the screen and the JSON say the same.
#[test]
fn the_queue_plays_forward() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project_released(
        data.path(),
        "heavy",
        &["v0.2.0 · Second", "v0.3.0 · Third", "v0.4.0 · Fourth", "v0.5.0 · Fifth", "v0.6.0 · Sixth"],
        "2026-08-01T12:00:00Z",
    );
    project_released(
        data.path(),
        "light",
        &["v0.2.0 · Other", "v0.3.0 · More", "v0.4.0 · Again", "v0.5.0 · Yet", "v0.6.0 · Last"],
        "2026-08-02T12:00:00Z",
    );
    rigger(data.path()).args(["priority", "heavy", "3"]).assert().success();
    rigger(data.path()).args(["priority", "light", "1"]).assert().success();

    let found: Value = serde_json::from_str(&text(data.path(), &["queue", "--ahead", "5", "--json"])).unwrap();
    let turns: Vec<(String, String)> = found["ahead"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["project"].as_str().unwrap().to_string(), t["version"].as_str().unwrap().to_string()))
        .collect();
    let want = [
        ("heavy", "v0.2.0"),
        ("heavy", "v0.3.0"),
        ("heavy", "v0.4.0"),
        ("light", "v0.2.0"),
        ("heavy", "v0.5.0"),
    ];
    assert_eq!(turns, want.map(|(p, v)| (p.to_string(), v.to_string())), "{found:#}");
    assert_eq!(found["ahead"][0]["title"], "Second");
    assert_eq!(found["ahead"][0]["turn"], 1);

    let said = text(data.path(), &["queue", "--ahead", "5"]);
    assert!(said.lines().next().unwrap().contains("heavy"), "{said}");
    assert!(said.contains("Other"), "{said}");
    // Alone, the flag means the default.
    let found: Value = serde_json::from_str(&text(data.path(), &["queue", "--ahead", "--json"])).unwrap();
    assert_eq!(found["ahead"].as_array().unwrap().len(), 40);
}

/// "Due today" means the day where the owner is. Two zones more than a day apart
/// always disagree on the date, whatever the time of the run.
#[test]
fn today_is_the_day_where_the_owner_is() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "alpha", &["v0.2.0 · Second"]);
    let due = |zone: &str, question: &str| {
        let out = rigger(data.path())
            .env("TZ", zone)
            .args(["note", "alpha", question, "--kind", "question", "--due", "today", "--json"])
            .assert()
            .success();
        let said: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
        said["event"]["due"].as_str().unwrap().to_string()
    };
    let west = due("Etc/GMT+12", "From the far west?");
    let east = due("Etc/GMT-14", "From the far east?");
    assert!(west < east, "{west} and {east}: today did not follow the zone");
}
