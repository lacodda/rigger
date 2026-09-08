//! Recording a whole directory of repositories at once.
//!
//! Every repository and hub here is made in a temporary directory. The
//! line this was written for has seventeen projects side by side, eleven
//! strays beside them, and the hubs in a notes vault elsewhere; the
//! fixture is that shape in miniature - two checkouts, one hub, one
//! directory that is not a checkout at all.

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
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A checkout with one commit, tagged when asked.
fn checkout(root: &Path, tag: Option<&str>) {
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("README.md"), "x").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "feat: start"]);
    if let Some(tag) = tag {
        git(root, &["tag", tag]);
    }
}

/// A hub with one stage to come and one that shipped.
fn hub(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("План.md"), "# План\n\n## v0.2.0 · Next\n\n- [ ] one\n- [ ] two\n").unwrap();
    std::fs::write(dir.join("Изменения.md"), "# Изменения\n\n## v0.1.0 · First — выпущена 2026-09-01\n\nDone.\n").unwrap();
}

/// Two checkouts, a plain directory, and a hub for the first checkout.
fn line(data: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let repos = data.join("repos");
    checkout(&repos.join("alpha"), Some("v0.1.0"));
    checkout(&repos.join("beta"), None);
    std::fs::create_dir_all(repos.join("notes")).unwrap();
    std::fs::write(repos.join("notes").join("todo.md"), "not a repository").unwrap();

    let hubs = data.join("hubs");
    hub(&hubs.join("alpha"));
    // A directory with a project's name and nothing of a hub in it.
    std::fs::create_dir_all(hubs.join("beta")).unwrap();
    (repos, hubs)
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn a_checkout_with_a_hub_is_recorded_and_its_hub_and_tags_read() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, hubs) = line(data.path());

    let out = output(data.path(), &["adopt", repos.to_str().unwrap(), "--hubs", hubs.to_str().unwrap()]);
    assert!(out.contains("alpha  recorded"), "{out}");
    // Every kind the hub gave is named: the prose of its two files counts.
    assert!(out.contains("hub: 2 versions, 2 tasks, 2 files of prose new"), "{out}");
    assert!(out.contains("git: 1 version shipped"), "{out}");
    // The hub is what makes a checkout one of the line; beta has none.
    assert!(out.contains("beta   no hub"), "{out}");
    assert!(!out.contains("notes"), "a plain directory is not a project: {out}");
    assert!(
        out.contains("2 repositories: 1 recorded, 0 known, 1 without a hub, 0 skipped; 1 hub read."),
        "{out}"
    );

    let list = output(data.path(), &["project", "list"]);
    assert!(list.starts_with("alpha"), "{list}");
    assert!(!list.contains("beta"), "{list}");

    // The record knows where the hub is, so the check can find it.
    let doctor = output(data.path(), &["doctor", "--hubs"]);
    assert!(!doctor.contains("no hub recorded"), "{doctor}");
}

#[test]
fn without_a_hubs_directory_every_checkout_is_recorded() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, _) = line(data.path());

    let out = output(data.path(), &["adopt", repos.to_str().unwrap()]);
    assert!(out.contains("alpha  recorded     hub: none   git: 1 version shipped"), "{out}");
    // Its one commit is read as a change: untagged work is activity.
    assert!(out.contains("beta   recorded     hub: none   git: 0 versions shipped, 1 change read"), "{out}");
    assert!(out.contains("2 repositories: 2 recorded, 0 known, 0 skipped; 0 hubs read."), "{out}");
}

#[test]
fn running_it_again_records_nothing_twice() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, hubs) = line(data.path());
    let args = ["adopt", repos.to_str().unwrap(), "--hubs", hubs.to_str().unwrap()];
    output(data.path(), &args);

    let again = output(data.path(), &args);
    // The hub's changelog dates v0.1.0 a week before the tag's commit. The
    // tag's day stays: a second import must not put the hub's day back, or
    // every sync after it would report the version shipped anew.
    assert!(again.contains("alpha  known        hub: nothing new   git: nothing new"), "{again}");
    assert!(
        again.contains("2 repositories: 0 recorded, 1 known, 1 without a hub, 0 skipped; 1 hub read."),
        "{again}"
    );

    // The line has grown: a checkout with a hub is picked up, the rest left alone.
    checkout(&repos.join("gamma"), None);
    hub(&hubs.join("gamma"));
    let grown = output(data.path(), &args);
    assert!(grown.contains("gamma  recorded"), "{grown}");
    assert!(
        grown.contains("3 repositories: 1 recorded, 1 known, 1 without a hub, 0 skipped; 2 hubs read."),
        "{grown}"
    );
}

#[test]
fn check_writes_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, hubs) = line(data.path());

    let out = output(data.path(), &["adopt", repos.to_str().unwrap(), "--hubs", hubs.to_str().unwrap(), "--check"]);
    assert!(out.contains("alpha  would record hub: found"), "{out}");
    assert!(out.contains("beta   no hub"), "{out}");
    assert!(
        out.contains("2 repositories: 1 would be recorded, 0 known, 1 without a hub, 0 skipped; 0 hubs read."),
        "{out}"
    );
    assert!(out.contains("Nothing was written."), "{out}");

    let list = output(data.path(), &["project", "list"]);
    assert!(list.contains("No projects yet"), "{list}");
}

#[test]
fn a_name_already_taken_by_another_path_is_skipped_and_said() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, hubs) = line(data.path());
    let elsewhere = data.path().join("elsewhere").join("alpha");
    checkout(&elsewhere, None);
    rigger(data.path()).args(["project", "add"]).arg(&elsewhere).assert().success();

    let out = output(data.path(), &["adopt", repos.to_str().unwrap(), "--hubs", hubs.to_str().unwrap()]);
    assert!(out.contains("alpha  skipped"), "{out}");
    assert!(out.contains("a project named 'alpha' is recorded at"), "{out}");
    assert!(out.contains("0 recorded, 0 known, 1 without a hub, 1 skipped"), "{out}");
}

#[test]
fn a_directory_without_checkouts_is_an_error_that_says_what_a_checkout_is() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let empty = data.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    rigger(data.path())
        .args(["adopt", empty.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no repositories under").and(predicate::str::contains(".git")));
}

#[test]
fn json_carries_the_same_report() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let (repos, hubs) = line(data.path());

    let out = output(data.path(), &["adopt", repos.to_str().unwrap(), "--hubs", hubs.to_str().unwrap(), "--json"]);
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let rows = report.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "alpha");
    assert_eq!(rows[0]["status"], "recorded");
    assert_eq!(rows[0]["versions_added"], 2);
    assert_eq!(rows[0]["shipped"], 1);
    assert_eq!(rows[1]["status"], "no-hub");
    assert!(rows[1]["hub"].is_null(), "{}", rows[1]);
}
