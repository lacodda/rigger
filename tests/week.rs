//! The owner's week, on a record built for the purpose.
//!
//! Three screens read one week: `week` is the Monday brief, `release-day` is
//! the shopfront queue, and `next` carries the tier signals. What is under
//! test is not the wording but the agreement between them - all three read
//! the same versions, and a version counted as queued by one and as shipped
//! by another would make them argue in front of the owner.
//!
//! Every fixture dates its commits, so "shipped on the Friday" is a fact of
//! the repository rather than of the morning the test ran.

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
    // A repository with no releases still needs a commit, or there is no
    // history for `sync` to read and no week it was last touched in.
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

/// The brief answers the three questions a Monday opens with, and answers
/// all three at once: that is the whole of it. Each was already available
/// somewhere - the point is that they arrive together, before the week is
/// spent rather than after.
#[test]
fn the_brief_names_the_focus_the_friday_and_what_waits() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First", "v0.2.0 · Second"], &[]);
    rigger(data.path()).args(["project", "tier", "alpha", "B"]).assert().success();
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "alpha", "Which day do we announce on?", "--kind", "finding"])
        .assert()
        .success();

    let out = output(data.path(), &["week", "--week", "2026-W37"]);
    // The week is named by its days, not by its number: nobody pictures W37.
    assert!(out.contains("2026-08-31") || out.contains("2026-09-07"), "{out}");
    assert!(out.contains("2026-09-11"), "no Friday named: {out}");
    assert!(out.contains("Focus"), "{out}");
    assert!(out.contains("v0.1.0"), "{out}");
    assert!(out.contains("First"), "{out}");
    assert!(out.contains("Waiting on you"), "{out}");
}

/// A question is what waits, and a note of another kind is not. The brief
/// would be useless if everything recorded turned up in it.
#[test]
fn only_a_question_counts_as_waiting_on_the_owner() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    let out = output(data.path(), &["week", "--week", "2026-W37"]);
    assert!(out.contains("Waiting on you"), "{out}");
    // Nothing recorded, so nothing waits - and the brief says so rather
    // than leaving the reader to infer it from an empty space.
    let waiting = out.split("Waiting on you").nth(1).unwrap_or("");
    assert!(waiting.contains("nothing"), "{out}");
}

/// The shopfront rule, kept: one release, on the Friday of its week.
#[test]
fn a_tag_on_the_friday_reads_as_the_rule_kept() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // 2026-09-11 is the Friday of 2026-W37.
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);

    let out = output(data.path(), &["release-day", "--week", "2026-W37"]);
    assert!(out.contains("Friday"), "{out}");
    assert!(out.contains("v0.1.0"), "{out}");
    // Nothing is wrong, so nothing is said about it: the two complaints
    // only appear when there is something to complain about.
    assert!(!out.contains("before Friday"), "{out}");
    assert!(!out.contains("past the one release"), "{out}");
}

/// The rule broken, both ways at once: tags earlier in the week, and more
/// than one of them. Two releases in a day read as one burst from outside;
/// two in different weeks read as a rhythm. That is what the queue is for.
#[test]
fn releases_before_friday_and_past_the_slot_are_both_counted() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second", "v0.3.0 · Third"],
        // Monday, Tuesday and the Friday of W37.
        &[("v0.1.0", "2026-09-07"), ("v0.2.0", "2026-09-08"), ("v0.3.0", "2026-09-11")],
    );

    let out = output(data.path(), &["release-day", "--week", "2026-W37"]);
    assert!(out.contains("2 releases past the one release this week has room for"), "{out}");
    assert!(out.contains("2 releases went out before Friday"), "{out}");

    // And the same facts as data, for whatever reads them next.
    let json = output(data.path(), &["release-day", "--week", "2026-W37", "--json"]);
    assert!(json.contains("\"over_the_slot\": 2"), "{json}");
    assert!(json.contains("\"early\": 2"), "{json}");
}

/// The fold the real record forced. A week of this line holds ninety-four
/// releases, and a line each puts the two numbers that answer the question
/// below the fold - which is exactly how the calendar grid failed at
/// v0.10.0, on the same week of the same history.
#[test]
fn a_busy_week_stays_one_screen() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();

    let versions: Vec<String> = (1..=12).map(|n| format!("v0.{n}.0")).collect();
    let stages: Vec<String> = versions.iter().map(|v| format!("{v} · Stage")).collect();
    let stage_refs: Vec<&str> = stages.iter().map(String::as_str).collect();
    // All twelve on the Tuesday of W37: one day, one project, one burst.
    let releases: Vec<(&str, &str)> = versions.iter().map(|v| (v.as_str(), "2026-09-08")).collect();
    project(data.path(), "alpha", &stage_refs, &releases);

    let out = output(data.path(), &["release-day", "--week", "2026-W37"]);
    // One line for the day, not twelve for the releases.
    let listed = out.lines().filter(|l| l.contains("2026-09-08")).count();
    assert_eq!(listed, 1, "a line each rather than a folded day: {out}");
    // The run is spanned and counted, which is what keeps it to one line.
    assert!(out.contains("v0.1.0..v0.12.0 (12)"), "{out}");
    // And the numbers that answer the question are still on the screen.
    assert!(out.contains("11 releases past"), "{out}");
    // The whole screen fits a terminal: this is the property that broke.
    assert!(out.lines().count() < 15, "{} lines: {out}", out.lines().count());
    assert!(out.lines().all(|l| l.chars().count() < 120), "a line ran past the screen: {out}");
}

/// Tier A promised not to miss more than one cycle in a row. The signal is
/// therefore not the rhythm lapse `next` already shows: the lapse fires at
/// the first week over, and the promise allows a whole cycle before it is
/// broken.
#[test]
fn a_carrying_product_is_allowed_one_missed_cycle_and_no_more() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "tier", "alpha", "A"]).assert().success();

    // Rhythm of two weeks. Four weeks on is one cycle missed - late by the
    // rhythm, and still inside what the tier allows.
    let out = output(data.path(), &["next", "--week", "2026-W41"]);
    assert!(out.contains("Behind their rhythm"), "{out}");
    assert!(!out.contains("more than one cycle"), "still inside the allowance: {out}");

    // Five weeks on, the allowance is spent.
    let out = output(data.path(), &["next", "--week", "2026-W42"]);
    assert!(out.contains("more than one cycle"), "{out}");
    assert!(out.contains("alpha"), "{out}");
}

/// Tier B promised a turn in the focus every six weeks, and a turn is not a
/// release: a week spent on a product that shipped nothing was still spent.
/// Measuring this by tags would call a worked-on product neglected.
#[test]
fn a_growing_product_is_measured_by_its_last_turn_not_its_last_tag() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Tagged in W37, then worked on in W41 without another release. Both
    // dates are the repository's, not the clock's, so the week being read
    // is a fixed distance from each of them however the test is run.
    project(data.path(), "alpha", &["v0.1.0 · First", "v0.2.0 · Second"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "tier", "alpha", "B"]).assert().success();

    // Seven weeks past the tag and nothing since: the turn is overdue.
    let out = output(data.path(), &["next", "--week", "2026-W44"]);
    assert!(out.contains("no turn in the focus"), "{out}");

    // Now a commit in W41 - work that shipped nothing. Three weeks later
    // the tag is eleven weeks old and the signal is still down, which it
    // could only be by reading the turn rather than the release.
    let root = data.path().join("alpha");
    std::fs::write(root.join("work.txt"), "in progress").unwrap();
    git(&root, &["add", "."], Some("2026-10-08T12:00:00Z"));
    git(&root, &["commit", "--quiet", "-m", "feat: work on the decoder"], Some("2026-10-08T12:00:00Z"));
    rigger(data.path()).args(["sync", "alpha"]).assert().success();

    let out = output(data.path(), &["next", "--week", "2026-W44"]);
    assert!(!out.contains("no turn in the focus"), "a worked-on product read as neglected: {out}");
    // And the tag really is old enough to have raised it: without this the
    // line above would pass for the wrong reason.
    assert!(out.contains("Behind their rhythm"), "the tag was not old enough to mean anything: {out}");
}

/// Tier C promised to start one declared product at a time: the second does
/// not begin until the first has shipped. This is the one rule of the three
/// that is about a pair rather than a clock, and the one the written
/// calendar had least chance of catching.
#[test]
fn a_second_declared_product_started_out_of_turn_is_named_with_the_first() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // `first` began in W33 and `second` in W36, both without a release. The
    // dates are the repositories', so which one started earlier is a fact
    // of the fixture rather than of the alphabet or the morning.
    started(data.path(), "first", "2026-08-12T12:00:00Z");
    started(data.path(), "second", "2026-09-02T12:00:00Z");

    let out = output(data.path(), &["next", "--week", "2026-W40"]);
    // The later start is the one out of turn, and the earlier is what it
    // should have waited for: "started out of turn" without the other name
    // does not say which of the two to put down.
    let line = out
        .lines()
        .find(|l| l.contains("started before"))
        .unwrap_or_else(|| panic!("no second-start signal: {out}"));
    assert!(line.starts_with("second"), "the wrong product was called out of turn: {out}");
    assert!(line.contains("before first"), "the wrong product was named as the one to wait for: {out}");
    // Exactly one of the pair is called out; both would be no guidance.
    assert_eq!(out.matches("started before").count(), 1, "{out}");
}

/// A declared product with a start and no release: a commit dated by the
/// fixture, so the week it began in does not move with the clock.
fn started(data: &Path, name: &str, at: &str) {
    let root = data.join(name);
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("План.md"),
        "# План

## v0.1.0 · First

- [ ] a task
",
    )
    .unwrap();

    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);
    std::fs::write(root.join("README.md"), name).unwrap();
    git(&root, &["add", "."], Some(at));
    git(&root, &["commit", "--quiet", "-m", "feat: begin"], Some(at));

    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
    rigger(data).args(["project", "tier", name, "C"]).assert().success();
}

/// A project set to `out` is out by decision, and a decision the record
/// keeps is one it does not second-guess.
#[test]
fn a_project_out_of_the_rotation_raises_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "tier", "alpha", "out"]).assert().success();

    let out = output(data.path(), &["next", "--week", "2026-W50"]);
    assert!(!out.contains("Their tier asks for more"), "{out}");
    assert!(!out.contains("Behind their rhythm"), "{out}");
}

/// The three screens read one week, so they cannot disagree about it: a
/// version queued in the brief is queued in the release day, and one that
/// shipped is shipped in both.
#[test]
fn the_brief_and_the_queue_agree_about_the_week() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First", "v0.2.0 · Second"], &[("v0.1.0", "2026-09-08")]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.2.0", "--week", "2026-W37"])
        .assert()
        .success();

    let brief = output(data.path(), &["week", "--week", "2026-W37", "--json"]);
    let queue = output(data.path(), &["release-day", "--week", "2026-W37", "--json"]);
    let brief: serde_json::Value = serde_json::from_str(&brief).unwrap();
    let queue: serde_json::Value = serde_json::from_str(&queue).unwrap();

    assert_eq!(brief["shipping"], queue["queued"], "the two screens disagree about the queue");
    assert_eq!(brief["shipped"], queue["shipped"], "the two screens disagree about what shipped");
    // And the queue holds the unshipped version, not the tagged one.
    assert_eq!(queue["queued"][0]["version"], "v0.2.0");
    assert_eq!(queue["shipped"][0]["version"], "v0.1.0");
}

/// A week nobody planned and nothing shipped in is a real answer, not an
/// error - and each screen has to say so in its own words rather than
/// printing a heading over nothing.
#[test]
fn an_empty_week_says_so_on_every_screen() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    let brief = output(data.path(), &["week", "--week", "2026-W50"]);
    assert!(brief.contains("nothing is aimed at this week"), "{brief}");
    assert!(brief.contains("nothing is queued"), "{brief}");

    let queue = output(data.path(), &["release-day", "--week", "2026-W50"]);
    assert!(queue.contains("Nothing is waiting for Friday"), "{queue}");
}

/// The digest folds a project that has not moved into one quiet line, and a
/// project raising a signal is exactly the one that must not be folded: a
/// carrying product that stopped releasing is quiet by definition, and the
/// fold is how it would go on being unnoticed.
#[test]
fn a_signalled_project_is_named_in_the_digest_rather_than_folded_into_the_quiet_line() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Two declared products both started and neither shipped: the later
    // start raises a signal, and neither has moved this week.
    started(data.path(), "first", "2026-08-12T12:00:00Z");
    started(data.path(), "second", "2026-09-02T12:00:00Z");

    let out = output(data.path(), &["digest", "--since", "1d"]);
    let quiet = out.lines().find(|l| l.starts_with("Quiet:")).unwrap_or("");
    assert!(!quiet.contains("second"), "a signalled project was folded away: {out}");
    // And the signal is the reason it is named, in its own words.
    assert!(out.contains("asks for more"), "{out}");
    assert!(out.contains("started before first"), "{out}");

    // The one without a signal is still quiet: the exception is the signal,
    // not the pair. Without this the test would pass on a digest that had
    // simply stopped folding anything.
    assert!(quiet.contains("first"), "the fold stopped working entirely: {out}");
}

#[test]
fn a_week_that_is_not_a_week_is_refused_by_both_screens() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();

    for command in ["week", "release-day"] {
        rigger(data.path())
            .args([command, "--week", "next tuesday"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("is not a week"));
    }
}
