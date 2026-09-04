//! Asking the record questions, on a record built for the purpose.
//!
//! Both commands here were shaped by what the owner's real database did to
//! them - a search whose results did not show why they matched, a `why` that
//! returned a project's whole history, a Russian word that found a third of
//! what it should. Each of those is a test below, on a fixture that
//! reproduces the shape rather than the data.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

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

/// A project with two releases an hour apart on one day - the shape this
/// line actually has, where several versions ship in an afternoon.
fn two_releases_one_day(data: &Path) -> std::path::PathBuf {
    let root = data.join("proj");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);

    std::fs::write(root.join("a.txt"), "one").unwrap();
    git(&root, &["add", "."], None);
    git(&root, &["commit", "--quiet", "-m", "feat: the first thing"], Some("2026-03-01T09:00:00Z"));
    git(&root, &["tag", "v0.1.0"], None);

    std::fs::write(root.join("b.txt"), "two").unwrap();
    git(&root, &["add", "."], None);
    git(&root, &["commit", "--quiet", "-m", "feat: the second thing"], Some("2026-03-01T15:00:00Z"));
    git(&root, &["tag", "v0.2.0"], None);

    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["sync", "proj"]).assert().success();
    root
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn find_searches_every_project_at_once() {
    let data = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta"] {
        let root = data.path().join(name);
        std::fs::create_dir_all(&root).unwrap();
        if name == "alpha" {
            rigger(data.path()).arg("init").assert().success();
        }
        rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    }
    rigger(data.path())
        .args(["note", "alpha", "The budget is a gate, not a suggestion.", "--kind", "decision"])
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "beta", "A budget of ten thousand tokens is too many.", "--kind", "finding"])
        .assert()
        .success();

    let out = output(data.path(), &["find", "budget"]);
    assert!(out.contains("alpha"), "{out}");
    assert!(out.contains("beta"), "{out}");
    // The project column earns its place only when several are searched.
    let one = output(data.path(), &["find", "budget", "--project", "alpha"]);
    assert!(one.contains("gate"), "{one}");
    assert!(!one.contains("beta"), "{one}");
}

#[test]
fn a_result_shows_the_text_that_matched() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // The shape that broke this on the real record: a long decision whose
    // heading says the subject and whose match is hundreds of characters in.
    let body = format!(
        "**A heading that names something else entirely.**\n\n{}\n\nAnd then the migration is what this is really about.",
        "Padding that goes on for a while. ".repeat(20)
    );
    rigger(data.path()).args(["note", "proj", &body, "--kind", "decision"]).assert().success();

    let out = output(data.path(), &["find", "migration"]);
    assert!(out.contains("migration"), "a result must show why it matched, not the start of the body: {out}");
}

#[test]
fn a_bare_word_matches_its_inflections() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // Russian inflects and FTS5 has no stemmer for it. Searching the real
    // record for `бюджет` found two events where `бюджет*` found eight.
    rigger(data.path())
        .args(["note", "proj", "Решение про бюджета пакета контекста.", "--kind", "decision"])
        .assert()
        .success();

    let out = output(data.path(), &["find", "бюджет"]);
    assert!(out.contains("бюджета"), "an inflected form must be found: {out}");
}

#[test]
fn a_query_using_the_syntax_still_works() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path())
        .args(["note", "proj", "The packet holds a budget of three thousand.", "--kind", "decision"])
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "proj", "The packet is rendered from the record.", "--kind", "finding"])
        .assert()
        .success();

    // Both words required: the full FTS5 language stays available.
    let both = output(data.path(), &["find", "packet AND budget"]);
    assert!(both.contains("three thousand"), "{both}");
    assert!(!both.contains("rendered"), "AND must exclude the other: {both}");
}

#[test]
fn a_filter_on_kind_narrows_the_search() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path())
        .args(["note", "proj", "A decision about caching.", "--kind", "decision"])
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "proj", "A pitfall about caching.", "--kind", "pitfall"])
        .assert()
        .success();

    let out = output(data.path(), &["find", "caching", "--kind", "pitfall"]);
    assert!(out.contains("pitfall"), "{out}");
    assert!(!out.contains("A decision"), "{out}");
}

#[test]
fn a_search_that_matches_nothing_says_so_and_how_to_search_better() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    // An empty result is a result, not a failure.
    rigger(data.path())
        .args(["find", "zzznotawordanywhere"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing matches"));

    // A project that does not exist is a typo, and saying "nothing found"
    // would send someone looking for the wrong thing.
    rigger(data.path())
        .args(["find", "anything", "--project", "ghost"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

#[test]
fn why_shows_only_the_work_of_that_version() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());

    // Events either side of the second release, on the same day as both.
    rigger(data.path())
        .args(["note", "proj", "Belongs to the second version.", "--kind", "decision"])
        .assert()
        .success();

    let out = output(data.path(), &["why", "proj", "v0.2.0"]);
    assert!(out.contains("v0.2.0"), "{out}");
    assert!(out.contains("the second thing"), "{out}");
    // The defect this exists for: two versions shipped on one day, and a
    // day-wide window handed the second one the first one's work as well.
    assert!(!out.contains("the first thing"), "v0.1.0's work must not appear under v0.2.0: {out}");
}

#[test]
fn why_names_the_release_it_follows() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());

    let out = output(data.path(), &["why", "proj", "v0.2.0"]);
    assert!(out.contains("after v0.1.0"), "{out}");

    // The first release of all follows nothing, and says that rather than
    // pretending there was a predecessor.
    let first = output(data.path(), &["why", "proj", "v0.1.0"]);
    assert!(first.contains("from the start of the record"), "{first}");
    assert!(first.contains("the first thing"), "{first}");
}

#[test]
fn why_on_a_version_still_being_built_shows_the_work_so_far() {
    let data = tempfile::tempdir().unwrap();
    let root = two_releases_one_day(data.path());
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), "# План\n\n## v0.3.0 · Next\n\n- [ ] a task\n").unwrap();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub).assert().success();
    rigger(data.path())
        .args(["note", "proj", "Something decided while building v0.3.0.", "--kind", "decision"])
        .assert()
        .success();

    // An unshipped version has no upper bound, so its window runs to now -
    // which is exactly the work in progress a session wants to see.
    let out = output(data.path(), &["why", "proj", "v0.3.0"]);
    assert!(out.contains("being built"), "{out}");
    assert!(out.contains("while building v0.3.0"), "{out}");
}

#[test]
fn why_refuses_a_version_the_record_does_not_have() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());
    rigger(data.path())
        .args(["why", "proj", "v9.9.9"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no version 'v9.9.9'"));
}

#[test]
fn a_version_is_found_however_its_number_is_spelt() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());
    // Hubs write `v1.9` for a stage released as `v1.9.0`; a person types
    // whichever they remember.
    for spelling in ["v0.2.0", "0.2.0", "v0.2"] {
        let out = output(data.path(), &["why", "proj", spelling]);
        assert!(out.contains("v0.2.0"), "{spelling} did not find the version: {out}");
    }
}

#[test]
fn both_commands_answer_as_json() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());

    let found = output(data.path(), &["find", "second", "--json"]);
    let parsed: serde_json::Value = serde_json::from_str(&found).expect("find --json must be JSON");
    assert!(parsed.as_array().is_some_and(|a| !a.is_empty()), "{found}");

    let why = output(data.path(), &["why", "proj", "v0.2.0", "--json"]);
    let parsed: serde_json::Value = serde_json::from_str(&why).expect("why --json must be JSON");
    assert_eq!(parsed["version"]["name"], "v0.2.0", "{why}");
    assert_eq!(parsed["after"]["name"], "v0.1.0", "{why}");
}

#[test]
fn a_change_recorded_with_only_its_day_is_given_back_its_time() {
    let data = tempfile::tempdir().unwrap();
    two_releases_one_day(data.path());

    // What an earlier rigger stored: the day, which places every change of
    // a day at midnight - before any tag made that day, and so under the
    // wrong release.
    let db = data.path().join("rigger.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE events SET created_at = substr(created_at, 1, 10) || 'T00:00:00Z' WHERE author = 'git'",
        [],
    )
    .unwrap();
    drop(conn);

    // The defect, reproduced: with every change at midnight, none of them
    // falls inside a window bounded by tag moments - the second version's
    // own work vanishes, and both changes pile up under the first release.
    let before = output(data.path(), &["why", "proj", "v0.2.0"]);
    assert!(!before.contains("the second thing"), "the fixture should reproduce the defect first: {before}");
    let first = output(data.path(), &["why", "proj", "v0.1.0"]);
    assert!(first.contains("the second thing"), "both changes land under v0.1.0: {first}");

    // A sync corrects it: the same commit, dated by itself rather than by
    // what the reader could store at the time.
    rigger(data.path()).args(["sync", "proj"]).assert().success();

    let after = output(data.path(), &["why", "proj", "v0.2.0"]);
    assert!(after.contains("the second thing"), "{after}");
    assert!(
        !after.contains("the first thing"),
        "a corrected timestamp must file the change under its own release: {after}"
    );
}
