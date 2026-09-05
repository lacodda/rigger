//! The look back, on a record built for the purpose.
//!
//! A retro reads a window of the record backwards: what the plan said, what
//! the tags say, and where the two parted company. Every fixture here dates
//! its commits, so "slipped by two weeks" and "shipped inside the window"
//! are facts of the repository rather than of the morning the test ran.
//!
//! The second half is about the place a retro leaves its summary. That
//! place has no repository, and the parts of rigger that read git have to
//! know it - otherwise the record nags on every sync about a project that
//! is working exactly as intended.

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

/// A project whose plan names stages and whose repository tags some of them
/// on dates the test chooses.
fn project(data: &Path, name: &str, stages: &[&str], releases: &[(&str, &str)]) {
    let root = data.join(name);
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();

    let mut plan = String::from("# План\n\n");
    for stage in stages {
        plan.push_str(&format!("## {stage}\n\n- [ ] a task\n\n"));
    }
    std::fs::write(hub.join("План.md"), plan).unwrap();

    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);
    for (n, (version, date)) in releases.iter().enumerate() {
        let at = format!("{date}T12:00:00Z");
        std::fs::write(root.join(format!("file{n}.txt")), version).unwrap();
        git(&root, &["add", "."], Some(&at));
        git(&root, &["commit", "--quiet", "-m", &format!("feat: {version}")], Some(&at));
        git(&root, &["tag", version], Some(&at));
    }
    if releases.is_empty() {
        std::fs::write(root.join("README.md"), name).unwrap();
        git(&root, &["add", "."], Some("2026-08-10T12:00:00Z"));
        git(&root, &["commit", "--quiet", "-m", "chore: start"], Some("2026-08-10T12:00:00Z"));
    }

    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// The three ways a release can relate to its plan, which is the whole of
/// what the check is for: what shipped when it said it would, what moved,
/// and what nobody ever aimed anywhere.
#[test]
fn a_retro_sorts_every_release_into_on_time_slipped_or_unplanned() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second", "v0.3.0 · Third"],
        // W37 is 07-11 Sep; W39 is 21-25 Sep.
        &[("v0.1.0", "2026-09-11"), ("v0.2.0", "2026-09-25"), ("v0.3.0", "2026-09-25")],
    );
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.2.0", "--week", "2026-W37"])
        .assert()
        .success();

    let out = output(data.path(), &["retro", "--to", "2026-W40", "--weeks", "6"]);
    assert!(out.contains("3 shipped"), "{out}");
    assert!(out.contains("1 on time"), "{out}");
    assert!(out.contains("1 slipped"), "{out}");
    assert!(out.contains("1 unplanned"), "{out}");
    // The slip is named with its size: the grid says a release moved, only
    // a number says how far, and "what turned out dearer" was the question.
    assert!(out.contains("2 weeks late"), "{out}");
}

/// The number the written calendar could never produce, and the one that
/// says whether planning happened at all.
#[test]
fn the_share_of_planned_releases_is_reported() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second"],
        &[("v0.1.0", "2026-09-11"), ("v0.2.0", "2026-09-11")],
    );
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();

    let out = output(data.path(), &["retro", "--to", "2026-W37", "--weeks", "4"]);
    assert!(out.contains("50% of what shipped had been planned"), "{out}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["retro", "--to", "2026-W37", "--weeks", "4", "--json"])).unwrap();
    assert_eq!(json["planned_share"], 50);
}

/// A window is a window: what happened outside it is another retro's
/// business, however interesting.
#[test]
fn only_what_happened_inside_the_window_is_counted() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second"],
        // One inside W37, one three months earlier.
        &[("v0.1.0", "2026-06-12"), ("v0.2.0", "2026-09-11")],
    );

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["retro", "--to", "2026-W37", "--weeks", "2", "--json"])).unwrap();
    let shipped = json["shipped"].as_array().unwrap();
    assert_eq!(shipped.len(), 1, "{json}");
    assert_eq!(shipped[0]["version"], "v0.2.0");
}

/// A look back has to give the same answer whenever it is run, so a version
/// that was planned and never shipped is late against the window's end and
/// not against today - otherwise the number grows every time it is asked.
#[test]
fn a_missed_release_is_measured_against_the_window_not_the_clock() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();

    let first = output(data.path(), &["retro", "--to", "2026-W40", "--weeks", "4", "--json"]);
    let later = output(data.path(), &["retro", "--to", "2026-W40", "--weeks", "4", "--json"]);
    assert_eq!(first, later, "the same window gave two answers");

    let json: serde_json::Value = serde_json::from_str(&first).unwrap();
    let missed = json["missed"].as_array().unwrap();
    assert_eq!(missed.len(), 1, "{json}");
    // W37 to W40 is three weeks, measured to the window's end.
    assert_eq!(missed[0]["weeks"], 3);
}

/// "Do the tiers need moving" was the third question the written calendar
/// asked. The two directions are different problems and are shown apart:
/// the real record came back with seven of fifteen projects called misfits,
/// and the two that had stalled sat buried among five that were racing.
#[test]
fn the_two_directions_of_a_misfit_are_shown_apart() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();

    // Tier C asks for one release in seven weeks; this shipped four.
    project(
        data.path(),
        "busy",
        &["v0.1.0 · A", "v0.2.0 · B", "v0.3.0 · C", "v0.4.0 · D"],
        &[
            ("v0.1.0", "2026-09-11"),
            ("v0.2.0", "2026-09-11"),
            ("v0.3.0", "2026-09-18"),
            ("v0.4.0", "2026-09-18"),
        ],
    );
    rigger(data.path()).args(["project", "tier", "busy", "C"]).assert().success();

    // Tier C asks for one and this shipped none at all.
    project(data.path(), "stalled", &["v0.1.0 · A"], &[]);
    rigger(data.path()).args(["project", "tier", "stalled", "C"]).assert().success();

    let out = output(data.path(), &["retro", "--to", "2026-W40", "--cycle"]);
    let stalled_at = out.find("Nothing shipped, and their tier asked for something:");
    let outgrown_at = out.find("Shipping past their tier");
    assert!(stalled_at.is_some(), "{out}");
    assert!(outgrown_at.is_some(), "{out}");
    // Stalled comes first: a product that shipped nothing is the one that
    // needs a decision, and burying it under the busy ones is the defect
    // this split exists to fix.
    assert!(stalled_at < outgrown_at, "{out}");

    let stalled_block = &out[stalled_at.unwrap()..outgrown_at.unwrap()];
    assert!(stalled_block.contains("stalled"), "{out}");
    assert!(!stalled_block.contains("busy"), "a racing project was called stalled: {out}");

    let outgrown_block = &out[outgrown_at.unwrap()..];
    assert!(outgrown_block.contains("busy"), "{out}");
    assert!(outgrown_block.contains("4x"), "{out}");
}

/// A tier that is doing its job is named in neither list. Without this the
/// screen would call every project a misfit and mean nothing by it.
#[test]
fn a_tier_that_is_working_is_not_named() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Tier A over seven weeks at a release every two asks for three.
    project(
        data.path(),
        "steady",
        &["v0.1.0 · A", "v0.2.0 · B", "v0.3.0 · C"],
        &[("v0.1.0", "2026-09-11"), ("v0.2.0", "2026-09-18"), ("v0.3.0", "2026-09-25")],
    );
    rigger(data.path()).args(["project", "tier", "steady", "A"]).assert().success();

    let out = output(data.path(), &["retro", "--to", "2026-W40", "--cycle"]);
    assert!(out.contains("3 shipped"), "{out}");
    assert!(!out.contains("Shipping past their tier"), "a working tier was called a misfit: {out}");
    assert!(!out.contains("Nothing shipped, and their tier"), "{out}");
}

/// A project deliberately set aside promised nothing, so it cannot fall
/// short of anything. Being out of the rotation is the point of `out`.
#[test]
fn a_project_out_of_the_rotation_is_never_a_misfit() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "aside", &["v0.1.0 · A"], &[]);
    rigger(data.path()).args(["project", "tier", "aside", "out"]).assert().success();

    let out = output(data.path(), &["retro", "--to", "2026-W40", "--cycle"]);
    assert!(!out.contains("aside"), "{out}");
}

/// The summary belongs to no single project, because it is about all of
/// them - so without a place to keep it the command says so and names the
/// fix, rather than filing it somewhere misleading.
#[test]
fn recording_without_a_place_to_keep_it_says_what_to_do() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);

    rigger(data.path())
        .args(["retro", "--to", "2026-W37", "--record"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rigger project service"));
}

/// The same retro of the same weeks is the same fact however often it is
/// asked for. Stamping it with the moment it ran filed a fresh copy every
/// time, which is how a record fills with restatements of one conclusion.
#[test]
fn the_same_retro_is_kept_once() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "service", "line"]).assert().success();

    let first = output(data.path(), &["retro", "--to", "2026-W37", "--record"]);
    assert!(first.contains("Kept in the record"), "{first}");

    let again = output(data.path(), &["retro", "--to", "2026-W37", "--record"]);
    assert!(again.contains("already in the record"), "{again}");

    // Idempotence by the window, not by luck. Two runs a second apart share
    // a timestamp, so a retro stamped with "now" would also look kept-once
    // here - and would file a fresh copy the next day. The date proves
    // which of the two is happening: it is the window's Friday, not today.
    let packet = output(data.path(), &["context", "line"]);
    assert!(packet.contains("2026-09-11"), "the retro is not dated by its window: {packet}");

    // One event under the place it is kept, not two - which is the claim,
    // and the count is what proves it rather than the wording. Counted
    // there rather than across the record: `sync` writes a change event of
    // its own from the fixture's commit, and a total would move for
    // reasons that have nothing to do with this.
    assert_eq!(kept(data.path()), 1, "{}", output(data.path(), &["context", "line"]));

    // A different window is a different fact, and is kept beside it. It has
    // to be a window that saw something: a quiet one is deliberately not
    // kept, so using one here would prove the opposite of the claim.
    output(data.path(), &["retro", "--to", "2026-W38", "--record"]);
    assert_eq!(kept(data.path()), 2, "{}", output(data.path(), &["context", "line"]));
}

/// How many retros the record is keeping.
fn kept(data: &Path) -> usize {
    let json = output(data, &["digest", "line", "--since", "3650d", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    json["projects"][0]["facts"]["changes"].as_u64().unwrap_or(0) as usize
}

/// A window where nothing happened is a real answer and not one worth
/// filing: a retro is kept so a later one can find what was concluded, and
/// "nothing" concludes nothing.
#[test]
fn a_quiet_window_is_not_kept() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["project", "service", "line"]).assert().success();

    let out = output(data.path(), &["retro", "--to", "2026-W44", "--record"]);
    assert!(out.contains("Nothing shipped"), "{out}");
    assert!(out.contains("Nothing to keep"), "{out}");

    let doctor: serde_json::Value = serde_json::from_str(&output(data.path(), &["doctor", "--json"])).unwrap();
    assert_eq!(doctor["counts"]["events"], 0, "{doctor}");
}

/// The defect that made the kind a column at all. Recorded as a repo-less
/// repository, the place a retro keeps its summary made `sync` warn on
/// every run and `doctor` advise a command that could not help it - the
/// record nagging about a project working exactly as intended.
#[test]
fn a_place_the_record_keeps_for_itself_is_not_asked_about_git() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "service", "line"]).assert().success();

    // Syncing everything says nothing about it: there is no repository to
    // read, and that is not a warning.
    let out = output(data.path(), &["sync"]);
    assert!(!out.contains("not a git repository"), "{out}");

    // Named on its own it explains itself once, rather than failing.
    let named = output(data.path(), &["sync", "line"]);
    assert!(named.contains("no repository"), "{named}");

    // And it never joins the list of projects waiting to be synced, where
    // it would have sat for ever.
    let doctor = output(data.path(), &["doctor"]);
    assert!(!doctor.contains("line"), "{doctor}");

    // The alpha project, which git can answer for, is unaffected.
    let doctor: serde_json::Value = serde_json::from_str(&output(data.path(), &["doctor", "--json"])).unwrap();
    assert_eq!(doctor["counts"]["projects"], 2, "{doctor}");
}

/// Nowhere does the marker in its path column reach a screen: it is
/// bookkeeping, and shown as a location it reads as a broken path.
#[test]
fn a_service_project_never_shows_a_path_it_does_not_have() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "service", "line"]).assert().success();

    for args in [vec!["project", "list"], vec!["project", "show", "line"], vec!["context", "line"]] {
        let out = output(data.path(), &args);
        assert!(!out.contains("service:line"), "{args:?} showed the marker: {out}");
        assert!(out.contains("line"), "{args:?}: {out}");
    }
}

#[test]
fn a_window_of_no_weeks_is_refused() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path())
        .args(["retro", "--weeks", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one"));
    rigger(data.path())
        .args(["retro", "--to", "never"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a week"));
}

/// A cycle is seven weeks, as the owner's calendar set it - long enough for
/// tier A to get two turns and every other tier one.
#[test]
fn a_cycle_is_the_seven_weeks_the_calendar_named() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["retro", "--cycle", "--to", "2026-W40", "--json"])).unwrap();
    assert_eq!(json["weeks"], 7);
    assert_eq!(json["from"], "2026-W34");
    assert_eq!(json["to"], "2026-W40");
}
