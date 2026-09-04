//! Reading git, on repositories built for the purpose.
//!
//! Every repository here is made in a temporary directory and thrown away.
//! Testing against the owner's own checkouts would make the results depend
//! on what happened to be committed that morning, and would put the names of
//! real projects into the repository.
//!
//! The commits are made with `git` because that is what makes a fixture; the
//! product reads them with gix and never spawns anything.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

/// Runs git in `dir`, with an identity of its own so the fixture does not
/// depend on the machine's git configuration.
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

/// A repository with one commit per name given, tagging where asked.
///
/// `steps` is a list of (message, tag) - the tag is applied to that commit
/// when it is not empty.
fn repo_with_history(root: &Path, steps: &[(&str, &str)]) {
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "--quiet", "--initial-branch", "main"]);
    for (n, (message, tag)) in steps.iter().enumerate() {
        std::fs::write(root.join(format!("file{n}.txt")), message).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", message]);
        if !tag.is_empty() {
            git(root, &["tag", tag]);
        }
    }
}

/// A hub whose plan and changelog can be spelt per test.
fn hub(dir: &Path, plan: &str, changelog: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("План.md"), plan).unwrap();
    std::fs::write(dir.join("Изменения.md"), changelog).unwrap();
}

#[test]
fn a_tag_closes_the_version_whatever_the_plan_says() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0"), ("second", "v0.2.0"), ("third", "")]);
    // The plan still calls v0.2.0 open; git says otherwise, and git wins.
    hub(
        &root.join("hub"),
        "# План\n\n## v0.2.0 · Second stage\n\n- [ ] a task\n",
        "# Изменения\n\n## v0.1.0 · First stage — выпущен 2026-01-01\n",
    );

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shipped    v0.2.0"));

    // And the packet now names it as the newest release, rather than the
    // stage being built.
    rigger(data.path())
        .args(["context", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Last shipped: v0.2.0").and(predicate::str::contains("Current stage").not()));
}

#[test]
fn a_version_closed_without_a_tag_is_reported_not_corrected() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0")]);
    // The changelog claims a release git has never heard of.
    hub(
        &root.join("hub"),
        "# План\n\n## v0.3.0 · Next\n\n- [ ] a task\n",
        "# Изменения\n\n## v0.2.0 · Second — выпущен 2026-01-02\n\n## v0.1.0 · First — выпущен 2026-01-01\n",
    );

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no tag     v0.2.0 is closed in the plan"));

    // Reported, not corrected: the claim survives, because the record cannot
    // prove a tag's absence - it may simply never have been fetched.
    rigger(data.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"version\": \"v0.2.0\""));
    rigger(data.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("closed in the plan, no tag in git (1)"));
}

#[test]
fn a_release_the_plan_never_mentioned_is_named_when_it_is_news() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    // v0.1.0 predates the plan; v0.2.1 is a patch shipped without a stage.
    repo_with_history(&root, &[("first", "v0.1.0"), ("second", "v0.2.0"), ("patch", "v0.2.1")]);
    hub(
        &root.join("hub"),
        "# План\n\n## v0.3.0 · Next\n\n- [ ] a task\n",
        "# Изменения\n\n## v0.2.0 · Second — выпущен 2026-01-02\n",
    );

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    let out = rigger(data.path()).args(["sync", "proj"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    // The patch above the plan's floor is news.
    assert!(stdout.contains("v0.2.1"), "{stdout}");
    assert!(
        stdout.lines().any(|l| l.contains("v0.2.1") && l.contains("not in the plan")),
        "a patch shipped without a stage must be named: {stdout}"
    );
    // The release below it is history, not a plan that lost track: every hub
    // starts somewhere, and naming all of it buries what matters.
    assert!(
        !stdout.lines().any(|l| l.contains("v0.1.0") && l.contains("not in the plan")),
        "a release older than the plan is not news: {stdout}"
    );
}

#[test]
fn activity_counts_commits_since_the_newest_tag() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0"), ("work", ""), ("more work", ""), ("still", "")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // The first sync has news - two releases - so it says what has landed
    // since as well.
    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("activity   3 commits since v0.1.0"));

    // The packet carries it, because "how long has this actually been still"
    // is a different question from "when did anyone last write a note".
    rigger(data.path())
        .args(["context", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 commits since the last release"));
}

#[test]
fn a_tag_that_is_not_a_version_is_not_a_release() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0"), ("second", "latest")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    let out = rigger(data.path()).args(["sync", "proj"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("v0.1.0"), "{stdout}");
    // A moving pointer is not a release; recording it would put a phantom
    // version in the record and on the calendar.
    assert!(!stdout.contains("latest"), "{stdout}");
}

#[test]
fn an_annotated_tag_is_read_through_to_its_commit() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("a.txt"), "one").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "one"]);
    git(&root, &["tag", "-a", "v1.0.0", "-m", "the first release"]);

    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // An annotated tag points at a tag object, not at a commit; without
    // peeling, its date and its identity are both wrong.
    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shipped    v1.0.0"));
}

#[test]
fn syncing_twice_changes_nothing_the_second_time() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0"), ("second", "v0.2.0")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    rigger(data.path()).args(["sync", "proj"]).assert().success();
    // A command meant to run at the start of every session must be quiet
    // when nothing moved, or nobody will read its output at all.
    rigger(data.path())
        .args(["sync", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
}

#[test]
fn a_project_without_a_repository_is_reported_not_fatal() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("not-a-repo");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // A project can be recorded before its repository exists; one such
    // project must not stop a sync of the whole line.
    rigger(data.path())
        .args(["sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not a git repository"));
}

#[test]
fn an_empty_repository_has_no_releases_and_no_activity() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("fresh");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // No commits at all: a repository initialised and not yet used. Walking
    // history from a HEAD that does not exist must not be an error.
    rigger(data.path())
        .args(["sync", "fresh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
}

#[test]
fn a_project_never_synced_is_named_by_doctor() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // Before the first sync, doctor cannot tell a claim from a fact, and
    // says so instead of reporting a mismatch it has not checked for.
    rigger(data.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("never synced (1): proj"));

    rigger(data.path()).args(["sync", "proj"]).assert().success();
    rigger(data.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("never synced").not());
}

#[test]
fn syncing_the_whole_line_prints_only_what_moved() {
    let data = tempfile::tempdir().unwrap();
    let quiet = data.path().join("quiet");
    let noisy = data.path().join("noisy");
    repo_with_history(&quiet, &[("first", "v0.1.0")]);
    repo_with_history(&noisy, &[("first", "v0.1.0")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&quiet).assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&noisy).assert().success();
    rigger(data.path()).args(["sync"]).assert().success();

    // A second release lands in one of them only.
    std::fs::write(noisy.join("b.txt"), "two").unwrap();
    git(&noisy, &["add", "."]);
    git(&noisy, &["commit", "--quiet", "-m", "two"]);
    git(&noisy, &["tag", "v0.2.0"]);

    let out = rigger(data.path()).args(["sync"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("noisy"), "{stdout}");
    // Seventeen "nothing changed" lines hide the one that matters.
    assert!(!stdout.contains("quiet"), "a quiet project must stay quiet in a run over many: {stdout}");
}

#[test]
fn a_run_that_changed_nothing_says_only_that() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    repo_with_history(&root, &[("first", "v0.1.0"), ("work", ""), ("more", "")]);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path()).args(["sync", "proj"]).assert().success();

    // Activity is state, not news: it reads the same on every run until
    // someone commits. Printed beside "nothing changed" it contradicts it -
    // two lines that disagree about whether anything happened.
    let out = rigger(data.path()).args(["sync", "proj"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("nothing changed"), "{stdout}");
    assert!(
        !stdout.contains("activity"),
        "a run that changed nothing must not also report activity: {stdout}"
    );

    // It is not lost - it belongs in the packet, where a session reads it.
    rigger(data.path())
        .args(["context", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 commits since the last release"));
}
