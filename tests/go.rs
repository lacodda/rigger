//! `rigger go` with a stand-in for the assistant: a script found on the `PATH`
//! the way an npm install is found, in a directory with a space in its name,
//! which writes down where it ran and what it was given and then does what the
//! test needs a session to have done.
//!
//! On Windows the stand-in is a batch file on purpose, because that is what
//! every npm install is there. The earlier `open` handed such an assistant a
//! packet on stdin, its test's stand-in read stdin happily, and Claude Code
//! itself refuses to start that way: a stand-in proves the path only if it
//! receives what the real thing would, so this one takes an argument and
//! nothing else.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use assert_cmd::cargo::cargo_bin;
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

/// A project with a stage planned and one release behind it, its last sitting
/// over: nothing presses, and nothing is left over.
fn project(data: &Path, name: &str) -> PathBuf {
    let root = data.join("repos").join(name);
    let hub = data.join("hubs").join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), "# План\n\n## v0.2.0 · Second\n\n- [ ] a task\n").unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    commit(&root, "a.txt", "feat: the first thing");
    git(&root, &["tag", "v0.1.0"]);
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    rigger(data).args(["session", "start", name]).assert().success();
    rigger(data).args(["session", "end", name]).assert().success();
    // The record keeps seconds, and the order goes by the sitting's end.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    root
}

fn init(data: &Path) {
    rigger(data).arg("init").assert().success();
}

/// A stand-in for the assistant. Returns the directory to put on the `PATH`
/// and the file it logs to. `work` is lines both `cmd` and `sh` can run.
fn stand_in(data: &Path, work: &[String], code: i32) -> (PathBuf, PathBuf) {
    let dir = data.join("bin dir");
    std::fs::create_dir_all(&dir).unwrap();
    let log = data.join("assistant.log");
    let work = work.join("\n");
    if cfg!(windows) {
        let script = format!(
            "@echo off\r\necho cwd=%CD%>>\"{log}\"\r\necho args=%*>>\"{log}\"\r\n{}\r\nexit /b {code}\r\n",
            work.replace('\n', "\r\n"),
            log = log.display(),
        );
        std::fs::write(dir.join("stand-in.cmd"), script).unwrap();
    } else {
        let script = format!(
            "#!/bin/sh\necho \"cwd=$(pwd)\" >> '{log}'\necho \"args=$*\" >> '{log}'\n{work}\nexit {code}\n",
            log = log.display()
        );
        let path = dir.join("stand-in");
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    (dir, log)
}

/// `rigger go` with the stand-in as the assistant.
fn go(data: &Path, bin: &Path, args: &[&str]) -> TestCommand {
    let path = std::env::join_paths(std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))).unwrap();
    let mut cmd = rigger(data);
    cmd.env("PATH", path)
        .env("RIGGER_ASSISTANT", "stand-in")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .arg("go")
        .args(args);
    cmd
}

fn stdout(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

fn json(cmd: &mut TestCommand) -> Value {
    serde_json::from_slice(&cmd.assert().success().get_output().stdout).unwrap()
}

/// The line the stand-in was given, and where it ran.
fn given(log: &Path) -> (String, String) {
    let text = std::fs::read_to_string(log).unwrap_or_else(|e| panic!("the assistant never ran: {e}"));
    let find = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .unwrap_or_else(|| panic!("no {key} in {text}"))
            .to_string()
    };
    (find("cwd="), find("args="))
}

/// A line of the stand-in's work that calls rigger. The record is inherited
/// through the environment, as a real session's would be.
fn rigger_line(args: &str) -> String {
    format!("\"{}\" {args}", cargo_bin("rigger").display())
}

/// The whole of the command, as the owner meets it: it starts the assistant in
/// the project that comes next, on one line that says what the sitting is for,
/// with the sitting open from the first moment, and closes it afterwards.
#[test]
fn go_starts_the_assistant_on_what_comes_next() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    project(data.path(), "second");

    let during = data.path().join("during.json");
    let (bin, log) = stand_in(data.path(), &[format!("{} > \"{}\"", rigger_line("queue --json"), during.display())], 0);
    let said = stdout(go(data.path(), &bin, &[]).assert().success());

    let (cwd, args) = given(&log);
    assert!(cwd.to_lowercase().ends_with("first"), "ran in {cwd}");
    assert!(args.contains("Work on first: take v0.2.0 to a release"), "{args}");
    assert!(args.contains("rigger context first"), "{args}");
    assert!(args.contains("rigger rules first"), "{args}");

    assert!(said.contains("Work on first  v0.2.0 · Second"), "{said}");
    assert!(said.contains("Starting stand-in in"), "{said}");

    // While the assistant runs, a second `go` can see someone is at it.
    let during: Value = serde_json::from_str(&std::fs::read_to_string(&during).unwrap()).unwrap();
    assert!(during["order"][0]["unfinished"]["sitting"].is_string(), "{during:#}");

    // And when it has gone, nobody is. Only a release ends a turn, so it is
    // still this project's.
    let after = json(rigger(data.path()).args(["queue", "--json"]));
    assert!(after["order"][0]["unfinished"].is_null(), "{after:#}");
    assert_eq!(after["order"][0]["project"], "first", "{after:#}");
    assert!(said.contains("first has no new tag, and nothing left over: it is still its turn."), "{said}");
    assert!(!said.contains("Next:"), "{said}");
}

/// A release is what a turn is for, and the screen says it happened - read from
/// the repository, not from what the assistant reported.
#[test]
fn a_release_is_said_and_the_next_project_named() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    project(data.path(), "second");

    let (bin, _) = stand_in(
        data.path(),
        &[
            "git commit --quiet --allow-empty -m \"chore(release): v0.2.0\"".to_string(),
            "git tag v0.2.0".to_string(),
        ],
        0,
    );
    let said = stdout(go(data.path(), &bin, &[]).assert().success());

    assert!(said.contains("first v0.2.0 went out."), "{said}");
    assert!(said.contains("Next: second v0.2.0"), "{said}");
    assert!(!said.contains("not finished"), "{said}");
}

/// The owner's rule. A turn left unfinished stops the line: the next `go`
/// returns to the same project and says what was left, and only `--force`
/// walks past it, saying whom it walked past.
#[test]
fn an_unfinished_turn_comes_back_until_forced_past() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    project(data.path(), "second");

    let (bin, log) = stand_in(data.path(), &["echo work> left.txt".to_string()], 0);
    let said = stdout(go(data.path(), &bin, &[]).assert().success());
    assert!(said.contains("first is not finished: 1 untracked file."), "{said}");
    assert!(said.contains("`rigger go` comes back to it; `rigger go --force` goes past it."), "{said}");
    assert!(!said.contains("Next:"), "{said}");

    std::fs::remove_file(&log).unwrap();
    let said = stdout(go(data.path(), &bin, &[]).assert().success());
    let (cwd, args) = given(&log);
    assert!(cwd.to_lowercase().ends_with("first"), "went on to {cwd}: {said}");
    assert!(args.contains("It was left unfinished (1 untracked file): finish that first."), "{args}");

    std::fs::remove_file(&log).unwrap();
    let said = stdout(go(data.path(), &bin, &["--force"]).assert().success());
    let (cwd, args) = given(&log);
    assert!(cwd.to_lowercase().ends_with("second"), "stayed in {cwd}: {said}");
    assert!(!args.contains("left unfinished"), "{args}");
    assert!(said.contains("Going past first."), "{said}");
}

/// A sitting open elsewhere may be someone at work: `go` starts nothing and
/// says so. Naming the project joins it, and leaves it open for whoever opened
/// it; `--force` goes past it.
#[test]
fn an_open_sitting_stops_go_and_naming_the_project_joins_it() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    project(data.path(), "second");
    rigger(data.path()).args(["session", "start", "second"]).assert().success();

    let (bin, log) = stand_in(data.path(), &[], 0);
    let failed = go(data.path(), &bin, &[]).assert().failure();
    let said = String::from_utf8(failed.get_output().stderr.clone()).unwrap();
    assert!(said.contains("second has a sitting open since"), "{said}");
    assert!(said.contains("rigger go --force"), "{said}");
    assert!(!log.exists(), "an assistant was started beside the one at work");

    go(data.path(), &bin, &["second"]).assert().success();
    let (cwd, _) = given(&log);
    assert!(cwd.to_lowercase().ends_with("second"), "{cwd}");
    let still = json(rigger(data.path()).args(["queue", "--json"]));
    assert_eq!(still["order"][0]["project"], "second", "{still:#}");
    assert!(
        still["order"][0]["unfinished"]["sitting"].is_string(),
        "the joined sitting was closed: {still:#}"
    );

    std::fs::remove_file(&log).unwrap();
    go(data.path(), &bin, &["--force"]).assert().success();
    let (cwd, _) = given(&log);
    assert!(cwd.to_lowercase().ends_with("first"), "{cwd}");
}

/// A blocker lets the line go on without `--force`, and says how it comes
/// back: `resolve`, or a later release.
#[test]
fn a_blocker_takes_a_project_out_of_the_way_and_is_lifted() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    let first = project(data.path(), "first");
    project(data.path(), "second");

    let (bin, _) = stand_in(
        data.path(),
        &[
            "echo work> left.txt".to_string(),
            rigger_line("note first \"The signing key has expired\" --kind blocker"),
        ],
        0,
    );
    let said = stdout(go(data.path(), &bin, &[]).assert().success());
    assert!(said.contains("first is blocked: The signing key has expired."), "{said}");
    assert!(said.contains("rigger resolve first"), "{said}");
    assert!(said.contains("Next: second v0.2.0"), "{said}");

    // No force needed, though first holds an untracked file.
    let found = json(rigger(data.path()).args(["queue", "--json"]));
    assert_eq!(found["order"][0]["project"], "second", "{found:#}");
    assert_eq!(found["left_out"][0]["why"], "blocked");
    let id = found["left_out"][0]["blocker"]["id"].as_i64().unwrap().to_string();

    // A session on it hears of it first.
    let packet = stdout(rigger(data.path()).args(["context", "first"]).assert().success());
    assert!(packet.contains("The signing key has expired"), "{packet}");
    assert!(packet.contains("## Blocked"), "{packet}");

    // Cleared, it is back - and still unfinished, so it comes first.
    let cleared = stdout(rigger(data.path()).args(["resolve", "first", &id]).assert().success());
    assert!(cleared.contains(&format!("Cleared [{id}]")), "{cleared}");
    let found = json(rigger(data.path()).args(["queue", "--json"]));
    assert_eq!(found["order"][0]["project"], "first", "{found:#}");
    assert_eq!(found["order"][0]["reason"], "unfinished");

    // A release settles one nobody cleared: it did not stop that tag.
    rigger(data.path())
        .args(["note", "first", "The runner is down", "--kind", "blocker"])
        .assert()
        .success();
    assert_eq!(json(rigger(data.path()).args(["queue", "--json"]))["order"][0]["project"], "second");
    std::fs::remove_file(first.join("left.txt")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    commit(&first, "b.txt", "feat: the second thing");
    git(&first, &["tag", "v0.2.0"]);
    let found = json(rigger(data.path()).args(["queue", "--json"]));
    assert!(found["left_out"].as_array().unwrap().iter().all(|l| l["why"] != "blocked"), "{found:#}");
}

/// Named outright, a project starts even though the rule leaves it out: that
/// is how work on a blocked one resumes. A name that is no project is a typo.
#[test]
fn a_named_project_starts_and_a_wrong_name_is_refused() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    project(data.path(), "second");
    rigger(data.path())
        .args(["note", "second", "No signing key", "--kind", "blocker"])
        .assert()
        .success();

    let (bin, log) = stand_in(data.path(), &[], 0);
    let said = stdout(go(data.path(), &bin, &["second"]).assert().success());
    let (cwd, _) = given(&log);
    assert!(cwd.to_lowercase().ends_with("second"), "{cwd}");
    assert!(said.contains("blocked since"), "{said}");

    std::fs::remove_file(&log).unwrap();
    go(data.path(), &bin, &["no-such-project"]).assert().failure();
    assert!(!log.exists());
}

#[test]
fn the_assistants_exit_status_becomes_ours_after_the_report() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    let (bin, _) = stand_in(data.path(), &[], 3);
    let said = stdout(go(data.path(), &bin, &[]).assert().code(3));
    assert!(said.contains("first has no new tag"), "{said}");
}

#[test]
fn a_missing_assistant_is_named() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    rigger(data.path())
        .env("RIGGER_ASSISTANT", "no-such-assistant-binary")
        .arg("go")
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot run `no-such-assistant-binary`"));
}

/// Flags travel with the command the owner names, on the way through `cmd` too.
#[test]
fn flags_given_with_the_assistant_reach_it() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    let (bin, log) = stand_in(data.path(), &[], 0);
    let path = std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))).unwrap();
    rigger(data.path())
        .env("PATH", path)
        .env("RIGGER_ASSISTANT", "stand-in --model opus")
        .arg("go")
        .assert()
        .success();
    let (_, args) = given(&log);
    assert!(args.starts_with("--model opus "), "{args}");
    assert!(args.contains("Work on first"), "{args}");
}

/// A program reading JSON is not a person about to work in a terminal: nothing
/// starts, and the answer says what would have.
#[test]
fn json_says_what_would_start_and_starts_nothing() {
    let data = tempfile::tempdir().unwrap();
    init(data.path());
    project(data.path(), "first");
    let second = project(data.path(), "second");
    std::fs::write(second.join("left.txt"), "work").unwrap();

    let (bin, log) = stand_in(data.path(), &[], 0);
    let found: Value = serde_json::from_slice(&go(data.path(), &bin, &["--json"]).assert().success().get_output().stdout).unwrap();
    assert!(!log.exists(), "--json started the assistant");
    assert_eq!(found["project"], "second");
    assert_eq!(found["version"], "v0.2.0");
    assert_eq!(found["reason"], "unfinished");
    assert_eq!(found["unfinished"]["untracked"], 1);
    assert!(found["message"].as_str().unwrap().starts_with("Work on second"));
    assert_eq!(found["passed_over"].as_array().unwrap().len(), 0);

    let forced: Value = serde_json::from_slice(&go(data.path(), &bin, &["--force", "--json"]).assert().success().get_output().stdout).unwrap();
    assert_eq!(forced["project"], "first");
    assert_eq!(forced["passed_over"][0], "second");
}
