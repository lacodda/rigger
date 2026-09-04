//! The owner's two screens, on a record built for the purpose.
//!
//! These were shaped by what the owner's real database did to them: a
//! subject line that panicked on a Cyrillic dash, headings buried under
//! checkboxes and status glyphs, and a stage name dragging a parenthetical
//! from its hub. Each is a test here.

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

/// A project whose hub carries a plan, a changelog and questions.
fn project(data: &Path, name: &str, plan: &str) -> std::path::PathBuf {
    let root = data.join(name);
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), plan).unwrap();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    root
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// The plan shape a hub actually has: a waiting list, then stages.
fn plan_with_questions(questions: &[&str], stage: &str) -> String {
    let mut plan = String::from("# План\n\n## Ждёт решения владельца\n\n");
    for (n, q) in questions.iter().enumerate() {
        plan.push_str(&format!("{}. {q}\n", n + 1));
    }
    plan.push_str(&format!("\n## {stage}\n\n- [ ] a task\n"));
    plan
}

#[test]
fn the_inbox_gathers_questions_from_every_project() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &plan_with_questions(&["**Pick the release day.** It sets the rhythm."], "v0.1.0 · First"),
    );
    project(
        data.path(),
        "beta",
        &plan_with_questions(&["**Sign the binaries?** The work machine may refuse them."], "v0.2.0 · Second"),
    );

    let out = output(data.path(), &["inbox"]);
    assert!(out.contains("2 questions in 2 projects"), "{out}");
    assert!(out.contains("Pick the release day"), "{out}");
    assert!(out.contains("Sign the binaries?"), "{out}");
    // And how to answer one, since the point is to clear the queue.
    assert!(out.contains("rigger resolve"), "{out}");
}

#[test]
fn one_question_asked_by_several_projects_is_grouped() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // The case this exists for: three hubs carrying one question, and the
    // owner answering it three times without noticing.
    for name in ["alpha", "beta", "gamma"] {
        project(
            data.path(),
            name,
            &plan_with_questions(&["**Место в производственном календаре:** ярус не назначен."], "v0.1.0 · First"),
        );
    }

    let out = output(data.path(), &["inbox"]);
    assert!(out.contains("Asked by several projects"), "{out}");
    // The group block, not the question line that also names a project.
    let group = out
        .lines()
        .skip_while(|l| !l.contains("Asked by several projects"))
        .find(|l| l.contains("Место в производственном календаре"))
        .unwrap_or("");
    for name in ["alpha", "beta", "gamma"] {
        assert!(group.contains(name), "{name} missing from the group: {out}");
    }
}

#[test]
fn a_question_in_one_project_only_is_not_a_group() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &plan_with_questions(&["**Only here.** Nobody else asks this."], "v0.1.0 · First"),
    );

    let out = output(data.path(), &["inbox"]);
    assert!(out.contains("Only here"), "{out}");
    assert!(!out.contains("Asked by several projects"), "{out}");
}

#[test]
fn a_subject_written_in_cyrillic_does_not_panic() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // The first real hub this met panicked here: the sentence end was found
    // as a byte index and the slice landed inside an em dash.
    project(
        data.path(),
        "alpha",
        &plan_with_questions(
            &[
                "**Отозванная зависимость в `Cargo.lock`** — держать или ждать.",
                "⚪ **Trusted publishing на crates.io** — привязать после первой публикации.",
                "[ ] **Модель эмбеддингов**: размер против качества.",
            ],
            "v0.1.0 · First",
        ),
    );

    let out = output(data.path(), &["inbox"]);
    assert!(out.contains("Отозванная зависимость"), "{out}");
    // A status glyph and a checkbox are not part of the question.
    assert!(out.contains("Trusted publishing на crates.io"), "{out}");
    assert!(!out.contains("⚪"), "a glyph is not part of the subject: {out}");
    assert!(out.contains("Модель эмбеддингов"), "{out}");
    assert!(!out.contains("[ ] **"), "{out}");
}

#[test]
fn an_empty_inbox_says_so() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", "# План\n\n## v0.1.0 · First\n\n- [ ] a task\n");

    rigger(data.path())
        .arg("inbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing is waiting on you"));
    rigger(data.path())
        .args(["inbox", "--project", "alpha"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha is waiting on nothing"));
    // A project that does not exist is a typo, not an empty queue.
    rigger(data.path())
        .args(["inbox", "--project", "ghost"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

#[test]
fn answering_a_question_takes_it_off_the_queue() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &plan_with_questions(&["**Pick the release day.** It sets the rhythm."], "v0.1.0 · First"),
    );

    let out = output(data.path(), &["inbox"]);
    let id: i64 = out
        .lines()
        .find(|l| l.contains("Pick the release day"))
        .and_then(|l| l.split(['[', ']']).nth(1).map(|s| s.trim().to_string()))
        .expect("the inbox prints ids")
        .parse()
        .unwrap();

    rigger(data.path()).args(["resolve", "alpha", &id.to_string(), "Friday."]).assert().success();
    rigger(data.path())
        .arg("inbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing is waiting on you"));
}

/// A repository with one tagged release, for the digest tests.
fn released(data: &Path, name: &str) -> std::path::PathBuf {
    let root = data.join(name);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("a.txt"), "one").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feat: the thing that shipped"]);
    git(&root, &["tag", "v0.1.0"]);
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    root
}

#[test]
fn a_digest_is_five_lines_at_most() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    released(data.path(), "busy");
    let hub = data.path().join("busy").join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План\n\n## Ждёт решения владельца\n\n1. **A question.** Detail.\n\n## v0.2.0 · Next *(deferred from v0.1.9, owner's call)*\n\n- [ ] a task\n",
    )
    .unwrap();
    rigger(data.path()).args(["import", "busy", "--hub"]).arg(&hub).assert().success();
    for n in 0..6 {
        rigger(data.path())
            .args(["note", "busy", &format!("A decision, number {n}."), "--kind", "decision"])
            .assert()
            .success();
    }

    let out = output(data.path(), &["digest", "busy"]);
    let lines: Vec<&str> = out.lines().filter(|l| l.starts_with("  ")).collect();
    assert!(lines.len() <= 5, "a digest is five lines at most: {out}");
    assert!(out.contains("shipped v0.1.0"), "{out}");
    assert!(out.contains("waiting on you: 1 question"), "{out}");
    // The hub's aside has no room in five lines.
    assert!(out.contains("next: v0.2.0 · Next"), "{out}");
    assert!(!out.contains("deferred from"), "the aside must go: {out}");
}

#[test]
fn a_digest_of_the_line_names_only_what_moved() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    released(data.path(), "moved");
    // A project recorded but untouched.
    let still = data.path().join("still");
    std::fs::create_dir_all(&still).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&still).assert().success();

    let out = output(data.path(), &["digest"]);
    assert!(out.contains("moved"), "{out}");
    // Seventeen projects at five lines each is not a digest; the quiet ones
    // are one line between them.
    assert!(out.contains("Quiet: still"), "{out}");

    // The quiet project must appear *only* in that line - not as a heading
    // with lines of its own above it. Counting occurrences is what catches
    // a listing that names it twice.
    assert_eq!(
        out.matches("still").count(),
        1,
        "a quiet project belongs in the Quiet line and nowhere else: {out}"
    );
    // And no project gets a block that says nothing happened.
    assert!(!out.contains("nothing this week"), "an empty block is what the Quiet line replaces: {out}");
}

#[test]
fn the_window_can_be_narrowed_and_is_reported() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    released(data.path(), "proj");

    let out = output(data.path(), &["digest", "proj", "--since", "30d"]);
    assert!(out.starts_with("Since 2026-"), "the window is stated, not assumed: {out}");

    // A project whose only release predates the window is quiet in it, not
    // absent - five empty lines would say the same thing at five times the
    // length.
    let old = data.path().join("ancient");
    std::fs::create_dir_all(&old).unwrap();
    git(&old, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(old.join("a.txt"), "old").unwrap();
    git(&old, &["add", "."]);
    let out = Command::new("git")
        .args(["commit", "--quiet", "-m", "feat: shipped long ago"])
        .current_dir(&old)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_AUTHOR_DATE", "2020-01-15T12:00:00Z")
        .env("GIT_COMMITTER_DATE", "2020-01-15T12:00:00Z")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(out.status.success());
    git(&old, &["tag", "v0.1.0"]);
    rigger(data.path()).args(["project", "add"]).arg(&old).assert().success();
    rigger(data.path()).args(["sync", "ancient"]).assert().success();

    let narrow = output(data.path(), &["digest", "--since", "7d"]);
    assert!(narrow.contains("Quiet: ancient"), "a project outside the window is one line: {narrow}");
    assert!(!narrow.contains("shipped long ago"), "{narrow}");

    rigger(data.path())
        .args(["digest", "--since", "soon"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a number of days"));
}

#[test]
fn both_screens_answer_as_json() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    released(data.path(), "proj");
    let hub = data.path().join("proj").join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), plan_with_questions(&["**A question.** Detail."], "v0.2.0 · Next")).unwrap();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub).assert().success();

    let inbox: serde_json::Value = serde_json::from_str(&output(data.path(), &["inbox", "--json"])).expect("inbox --json");
    assert_eq!(inbox["waiting"].as_array().map(Vec::len), Some(1), "{inbox}");
    assert!(inbox["waiting"][0]["id"].is_i64(), "{inbox}");

    let digest: serde_json::Value = serde_json::from_str(&output(data.path(), &["digest", "--json"])).expect("digest --json");
    assert!(digest["since"].is_string(), "{digest}");
    assert!(digest["projects"][0]["lines"].is_array(), "{digest}");
}
