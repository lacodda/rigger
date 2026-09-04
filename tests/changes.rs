//! Changes read from commits, and the two things that keep the record honest
//! as it grows: a plan that can be edited, and a packet the chronicle cannot
//! crowd out.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

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

/// A repository whose commits carry the given messages, in order.
fn repo_with_commits(root: &Path, messages: &[&str]) {
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "--quiet", "--initial-branch", "main"]);
    for (n, message) in messages.iter().enumerate() {
        std::fs::write(root.join(format!("f{n}.txt")), message).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", message]);
    }
}

fn recorded(data: &Path, project: &str) -> String {
    let out = rigger(data).args(["context", project]).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn only_commits_that_changed_the_product_become_events() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_commits(
        &root,
        &[
            "feat: the thing a session needs to know about",
            "chore(release): v0.1.0",
            "docs: changelog",
            "fix: a defect worth remembering",
            "test: another case",
            "refactor!: remove the deprecated flag",
        ],
    );
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("read       3 changes from commit messages"));

    let packet = recorded(data.path(), "proj");
    assert!(packet.contains("feat: the thing a session needs"), "{packet}");
    assert!(packet.contains("fix: a defect worth remembering"), "{packet}");
    // A breaking commit counts whatever its type, and says so.
    assert!(packet.contains("breaking: refactor: remove the deprecated flag"), "{packet}");
    // How the work was done belongs in git, where it already is.
    for noise in ["chore", "docs:", "test:"] {
        assert!(!packet.contains(noise), "{noise} should not reach the record: {packet}");
    }
}

#[test]
fn a_commit_is_recorded_once_however_often_it_is_read() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_commits(&root, &["feat: one change"]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    rigger(data.path()).args(["sync", "proj"]).assert().success();
    // The hash is the key, not the text: sync reads the same history on
    // every run, and this one is meant to run at the start of a session.
    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));

    let packet = recorded(data.path(), "proj");
    assert_eq!(packet.matches("feat: one change").count(), 1, "{packet}");
}

#[test]
fn an_event_is_dated_by_its_commit_not_by_the_sync() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("a.txt"), "old").unwrap();
    git(&root, &["add", "."]);
    // A change that landed long ago must not date from the sync that read
    // it, or a project's history arrives compressed into one day.
    let out = Command::new("git")
        .args(["commit", "--quiet", "-m", "feat: shipped back then"])
        .current_dir(&root)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_AUTHOR_DATE", "2026-01-15T12:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-15T12:00:00Z")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["sync", "proj"]).assert().success();

    let packet = recorded(data.path(), "proj");
    assert!(packet.contains("2026-01-15 · change · feat: shipped back then"), "{packet}");
}

#[test]
fn a_message_that_is_not_conventional_is_not_a_fact() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("old");
    repo_with_commits(&root, &["Initial commit", "update the parser", "feat: the one readable commit"]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // Repositories that predate the convention are not errors; their
    // commits are simply not facts this can read.
    rigger(data.path())
        .args(["sync", "old"])
        .assert()
        .success()
        .stdout(predicate::str::contains("read       1 change from commit messages"));
}

#[test]
fn the_chronicle_does_not_crowd_out_what_only_the_record_holds() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("busy");
    // Far more commits than the packet can hold.
    let messages: Vec<String> = (0..120).map(|n| format!("feat: change number {n} of many")).collect();
    let refs: Vec<&str> = messages.iter().map(String::as_str).collect();
    repo_with_commits(&root, &refs);

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    // Reasoning that exists nowhere but here.
    for n in 0..8 {
        rigger(data.path())
            .args(["note", "busy", &format!("A decision the code cannot explain, number {n}"), "--kind", "decision"])
            .assert()
            .success();
    }
    rigger(data.path()).args(["sync", "busy"]).assert().success();

    let packet = recorded(data.path(), "busy");
    // Every hand-written decision survives: a commit can be read again in
    // git, an argument cannot be recovered at all.
    for n in 0..8 {
        assert!(
            packet.contains(&format!("A decision the code cannot explain, number {n}")),
            "decision {n} was dropped for commit lines: {packet}"
        );
    }
    // And the chronicle is present but capped, rather than filling the packet.
    let commit_lines = packet.matches("change number").count();
    assert!(commit_lines > 0, "the chronicle should still be there: {packet}");
    assert!(commit_lines < 60, "the chronicle took over the packet ({commit_lines} lines): {packet}");
}

/// A hub whose plan can be rewritten per step.
fn plan(dir: &Path, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("План.md"), body).unwrap();
}

#[test]
fn rewording_a_task_edits_it_rather_than_adding_a_second() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    let hub = root.join("hub");
    std::fs::create_dir_all(&root).unwrap();
    plan(&hub, "# План\n\n## v0.1.0 · First\n\n- [ ] original wording\n- [ ] second task\n");

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub).assert().success();

    // A plan is edited - that is what a plan is for. Identifying a task by
    // its text alone left the old wording open for ever and showed a
    // session work it had already reworded.
    plan(&hub, "# План\n\n## v0.1.0 · First\n\n- [ ] reworded in place, same step\n- [ ] second task\n");
    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub)
        .assert()
        .success()
        .stdout(predicate::str::contains("tasks      0 added, 1 updated"));

    let packet = recorded(data.path(), "proj");
    assert!(packet.contains("reworded in place, same step"), "{packet}");
    assert!(!packet.contains("original wording"), "the old wording must go: {packet}");
    assert_eq!(packet.matches("second task").count(), 1, "{packet}");
}

#[test]
fn inserting_a_task_above_others_keeps_them_as_they_were() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    let hub = root.join("hub");
    std::fs::create_dir_all(&root).unwrap();
    plan(&hub, "# План\n\n## v0.1.0 · First\n\n- [ ] first step\n- [ ] second step\n");
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub).assert().success();

    // Matching by position alone would rename both existing tasks here.
    plan(
        &hub,
        "# План\n\n## v0.1.0 · First\n\n- [ ] a step inserted above\n- [ ] first step\n- [ ] second step\n",
    );
    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub)
        .assert()
        .success()
        .stdout(predicate::str::contains("tasks      1 added"));

    let packet = recorded(data.path(), "proj");
    for line in ["a step inserted above", "first step", "second step"] {
        assert_eq!(packet.matches(line).count(), 1, "{line} is wrong in: {packet}");
    }
}

#[test]
fn a_wish_can_be_sorted_and_a_question_answered() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    let hub = root.join("hub");
    std::fs::create_dir_all(&root).unwrap();
    plan(
        &hub,
        "# План\n\n## Ждёт решения владельца\n\n1. Which tier is this project?\n\n## v0.1.0 · First\n\n- [ ] a task\n",
    );
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub).assert().success();
    rigger(data.path()).args(["wish", "proj", "Count the days without a commit"]).assert().success();

    let packet = recorded(data.path(), "proj");
    let wish_id: i64 = packet
        .lines()
        .find(|l| l.contains("Count the days"))
        .and_then(|l| l.split(['[', ']']).nth(1).map(str::to_string))
        .expect("the packet lists wish ids")
        .parse()
        .unwrap();
    let question_id: i64 = packet
        .lines()
        .find(|l| l.contains("Which tier"))
        .and_then(|l| l.split(['[', ']']).nth(1).map(str::to_string))
        .expect("the packet lists question ids")
        .parse()
        .unwrap();

    rigger(data.path())
        .args(["resolve", "proj", &wish_id.to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sorted"));
    rigger(data.path())
        .args(["resolve", "proj", &question_id.to_string(), "Tier B, reviewed each cycle."])
        .assert()
        .success()
        .stdout(predicate::str::contains("Answered").and(predicate::str::contains("recorded as a decision")));

    let packet = recorded(data.path(), "proj");
    // Both leave the packet's waiting lists...
    assert!(!packet.contains("Wishes, not yet sorted"), "{packet}");
    assert!(!packet.contains("Waiting for the owner"), "{packet}");
    // ...and the answer stays in the record as what it is.
    assert!(packet.contains("Tier B, reviewed each cycle."), "{packet}");
    assert!(packet.contains("decision"), "{packet}");
}

#[test]
fn only_a_question_or_a_wish_can_be_resolved() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path())
        .args(["note", "proj", "A decision already taken", "--kind", "decision"])
        .assert()
        .success();

    // Resolving a decision would quietly rewrite the record's history.
    rigger(data.path())
        .args(["resolve", "proj", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a question or a wish"));
    rigger(data.path())
        .args(["resolve", "proj", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no open question or wish"));
}
