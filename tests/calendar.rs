//! The release calendar, on a record built for the purpose.
//!
//! The thing under test is not the grid but the comparison behind it: a
//! version is aimed at a week by hand, and the week it actually shipped in
//! comes from a tag. Every fixture here therefore dates its commits, so
//! that "slipped by two weeks" is a fact of the repository rather than of
//! the morning the test ran.

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

/// A project whose plan names stages, and whose repository tags some of
/// them on dates the test chooses.
///
/// `releases` is (version, ISO date) - one dated commit per entry, tagged.
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

    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["sync", name]).assert().success();
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn a_tier_carries_a_rhythm_without_being_asked_for_one() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    let out = output(data.path(), &["project", "tier", "alpha", "A"]);
    assert!(out.contains("tier A"), "{out}");
    assert!(out.contains("every 2 weeks"), "{out}");

    // And a project can keep its tier's company without its pace.
    let out = output(data.path(), &["project", "tier", "alpha", "A", "--rhythm", "3"]);
    assert!(out.contains("every 3 weeks"), "{out}");

    // Out of the rotation is a decision, and it has no rhythm to keep.
    let out = output(data.path(), &["project", "tier", "alpha", "out"]);
    assert!(out.contains("no rhythm"), "{out}");

    rigger(data.path())
        .args(["project", "tier", "alpha", "D"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a tier"));
    rigger(data.path())
        .args(["project", "tier", "alpha", "A", "--rhythm", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a rhythm"));
}

#[test]
fn a_tier_survives_being_written_and_is_shown_again() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["project", "tier", "alpha", "B"]).assert().success();

    // The point of the column: a later process reads back what was set.
    let out = output(data.path(), &["project", "show", "alpha", "--json"]);
    assert!(out.contains("\"tier\": \"B\""), "{out}");
    assert!(out.contains("\"rhythm_weeks\": 4"), "{out}");
}

#[test]
fn only_a_version_the_record_holds_can_be_planned() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    let out = output(data.path(), &["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"]);
    assert!(out.contains("2026-W37"), "{out}");
    // The week is named by the day the line releases on, not by its number
    // alone - a week number is not a date anyone can picture.
    assert!(out.contains("2026-09-11"), "{out}");

    // A version nobody has heard of is a typo. Accepting it would put a row
    // in the calendar that no plan, changelog or tag knows about.
    rigger(data.path())
        .args(["version", "plan", "alpha", "v9.9.9", "--week", "2026-W37"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no version 'v9.9.9'"));

    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "next tuesday"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a week"));

    // Saying nothing at all is a mistake worth naming: without this the
    // command would silently clear the week it was meant to set.
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--week"));
}

#[test]
fn planning_the_same_week_twice_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();
    let out = output(data.path(), &["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"]);
    assert!(out.contains("nothing changed"), "{out}");

    let out = output(data.path(), &["version", "plan", "alpha", "v0.1.0", "--clear"]);
    assert!(out.contains("off the calendar"), "{out}");
    // And once it is off, it is off the grid too.
    let grid = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "4"]);
    assert!(!grid.contains("v0.1.0"), "{grid}");
}

/// The comparison the whole release exists for: a version aimed at one week
/// whose tag landed in another is shown where the tag put it, and named as
/// having slipped - by a number, not by a shrug.
#[test]
fn a_release_that_landed_late_is_shown_where_it_landed() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(
        data.path(),
        "alpha",
        &["v0.1.0 · First", "v0.2.0 · Second"],
        // W37 is 07-11 Sep 2026; this tag is in W39.
        &[("v0.1.0", "2026-09-25")],
    );
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "4"]);
    // Shown in W39 with the slipped mark, not in the week it was aimed at.
    // Which column it lands in is the whole claim, so it is read by column
    // rather than by presence: the mark alone would be right either way.
    let heading = out.lines().find(|l| l.contains("2026-W37")).unwrap_or("");
    let row = out.lines().find(|l| l.starts_with("alpha")).unwrap_or("");
    let w37 = heading.find("2026-W37").unwrap_or_default();
    let w39 = heading.find("2026-W39").unwrap_or_default();
    let mark = row.find(">v0.1.0").unwrap_or_else(|| panic!("no slipped mark: {out}"));
    assert!(mark >= w39, "shown before the week its tag is in: {out}");
    assert!(mark > w37, "shown in the week it was aimed at, not where it landed: {out}");
    assert!(!row.contains("+v0.1.0"), "{out}");
    // And how far it slipped, which the grid alone cannot say.
    assert!(out.contains("2 weeks late"), "{out}");
}

#[test]
fn a_release_that_landed_in_its_week_is_not_called_slipped() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "2"]);
    assert!(out.contains("+v0.1.0"), "{out}");
    assert!(!out.contains("late"), "nothing slipped: {out}");
}

#[test]
fn a_release_nobody_planned_still_appears() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Tagged, never aimed at a week. Hiding it would let the calendar
    // disagree with the tags, which is the failure it exists to prevent.
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "2"]);
    assert!(out.contains("*v0.1.0"), "{out}");
    assert!(!out.contains("late"), "an unplanned release cannot be late: {out}");
}

#[test]
fn the_grid_names_the_week_being_read_from() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W38"])
        .assert()
        .success();

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "3"]);
    for week in ["2026-W37", "2026-W38", "2026-W39"] {
        assert!(out.contains(week), "{week} missing: {out}");
    }
    // Outside the span asked for, nothing is shown at all.
    let out = output(data.path(), &["calendar", "--from", "2026-W20", "--weeks", "2"]);
    assert!(out.contains("Nothing is on the calendar"), "{out}");
    assert!(out.contains("rigger version plan"), "an empty grid should say how to fill it: {out}");
}

#[test]
fn an_empty_span_is_refused_rather_than_printed_blank() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .args(["calendar", "--weeks", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one"));
    rigger(data.path())
        .args(["calendar", "--from", "not-a-week"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a week"));
}

#[test]
fn next_shows_what_is_aimed_at_the_week_and_what_is_past_it() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First", "v0.2.0 · Second"], &[]);
    project(data.path(), "beta", &["v0.3.0 · Third"], &[]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.2.0", "--week", "2026-W38"])
        .assert()
        .success();
    // Aimed at a week already gone, and never tagged.
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W35"])
        .assert()
        .success();
    // Someone else's week, so it must not appear in this one.
    rigger(data.path())
        .args(["version", "plan", "beta", "v0.3.0", "--week", "2026-W40"])
        .assert()
        .success();

    let out = output(data.path(), &["next", "--week", "2026-W38"]);
    assert!(out.contains("2026-W38"), "{out}");
    assert!(out.contains("2026-09-18"), "the Friday it releases on: {out}");
    assert!(out.contains("v0.2.0"), "{out}");
    assert!(out.contains("Second"), "the stage title, not just its number: {out}");
    assert!(out.contains("Past their week"), "{out}");
    assert!(out.contains("v0.1.0"), "{out}");
    assert!(out.contains("3 weeks ago"), "{out}");
    assert!(!out.contains("v0.3.0"), "another week's focus is not this week's: {out}");
}

/// A version planned for this week is not late yet - only a week already
/// gone is. Getting this wrong would mark the current focus as overdue on
/// the Monday it began.
#[test]
fn the_current_week_is_not_late() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W38"])
        .assert()
        .success();

    let out = output(data.path(), &["next", "--week", "2026-W38"]);
    assert!(out.contains("v0.1.0"), "{out}");
    assert!(!out.contains("Past their week"), "{out}");

    // And the grid agrees: the mark for this week is "planned", not
    // "overdue". Checked here because `next` decides the focus before it
    // asks about lateness, so it would go on looking right either way.
    let grid = output(data.path(), &["calendar", "--from", "2026-W38", "--weeks", "1"]);
    assert!(grid.contains("·v0.1.0"), "this week is planned, not late: {grid}");
    assert!(!grid.contains("!v0.1.0"), "{grid}");
}

#[test]
fn an_empty_week_says_so_rather_than_printing_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);

    let out = output(data.path(), &["next", "--week", "2026-W38"]);
    assert!(out.contains("Nothing is aimed at this week"), "{out}");
}

/// The failure the written calendar could never see: a product that quietly
/// stopped releasing. Nothing there ever compared the rotation to the tags.
#[test]
fn a_project_behind_its_rhythm_is_named() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Tier A asks for a release every two weeks; this one last shipped in
    // W37 and is being read in W44.
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-11")]);
    rigger(data.path()).args(["project", "tier", "alpha", "A"]).assert().success();
    // Tier B asks for four weeks, and this one shipped last week.
    project(data.path(), "beta", &["v0.1.0 · First"], &[("v0.1.0", "2026-10-23")]);
    rigger(data.path()).args(["project", "tier", "beta", "B"]).assert().success();

    let out = output(data.path(), &["next", "--week", "2026-W44"]);
    assert!(out.contains("Behind their rhythm"), "{out}");
    // Read the one section, not the whole screen: `next` names a project
    // again further down when its tier's own minimum is broken, and
    // counting across the output would call that a second lapse.
    let behind: Vec<&str> = out
        .lines()
        .skip_while(|l| !l.contains("Behind their rhythm"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter(|l| l.contains("alpha") || l.contains("beta"))
        .collect();
    assert_eq!(behind.len(), 1, "only alpha is behind: {out}");
    assert!(behind[0].contains("alpha"), "{out}");
    assert!(behind[0].contains("7 weeks"), "{out}");
}

/// Being out of the rotation is a decision, and the point of it is not
/// being nagged. A project with no tier at all is out by omission and gets
/// the same silence - the calendar does not invent a schedule for it.
#[test]
fn a_project_out_of_the_rotation_is_not_nagged() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-01-09")]);
    rigger(data.path()).args(["project", "tier", "alpha", "out"]).assert().success();
    // Never given a tier at all.
    project(data.path(), "beta", &["v0.1.0 · First"], &[("v0.1.0", "2026-01-09")]);
    // And out with a rhythm spelt out by hand: being out of the rotation is
    // the decision, not the absence of a number. A single-layer guard here
    // would start nagging the moment someone typed --rhythm.
    project(data.path(), "gamma", &["v0.1.0 · First"], &[("v0.1.0", "2026-01-09")]);
    rigger(data.path())
        .args(["project", "tier", "gamma", "out", "--rhythm", "2"])
        .assert()
        .success();

    let out = output(data.path(), &["next", "--week", "2026-W44"]);
    assert!(!out.contains("Behind their rhythm"), "{out}");
}

#[test]
fn a_project_that_never_shipped_is_behind_from_the_start() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[]);
    rigger(data.path()).args(["project", "tier", "alpha", "C"]).assert().success();

    let out = output(data.path(), &["next", "--week", "2026-W44"]);
    assert!(out.contains("Behind their rhythm"), "{out}");
    assert!(out.contains("never shipped"), "{out}");
}

#[test]
fn both_screens_answer_as_json() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &["v0.1.0 · First"], &[("v0.1.0", "2026-09-25")]);
    rigger(data.path()).args(["project", "tier", "alpha", "A"]).assert().success();
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.1.0", "--week", "2026-W37"])
        .assert()
        .success();

    let grid: serde_json::Value = serde_json::from_str(&output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "4", "--json"])).unwrap();
    assert_eq!(grid["weeks"][0], "2026-W37");
    assert_eq!(grid["projects"][0]["project"], "alpha");
    assert_eq!(grid["projects"][0]["tier"], "A");
    assert_eq!(grid["projects"][0]["cells"][0]["week"], "2026-W39");
    assert_eq!(grid["projects"][0]["cells"][0]["standing"], "slipped");

    let next: serde_json::Value = serde_json::from_str(&output(data.path(), &["next", "--week", "2026-W44", "--json"])).unwrap();
    assert_eq!(next["week"], "2026-W44");
    assert_eq!(next["friday"], "2026-10-30");
    assert_eq!(next["lapsed"][0]["project"], "alpha");
    assert_eq!(next["lapsed"][0]["rhythm_weeks"], 2);
}

/// A busy week must not stretch the grid out of shape.
///
/// Found on the owner's own record rather than on a fixture: one week of
/// one project held forty-six releases, the column grew past three hundred
/// characters, every row wrapped and the heading no longer lined up with
/// anything. Fixtures never showed it because they ship one release a week.
#[test]
fn a_week_full_of_releases_is_counted_rather_than_listed() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    // Eight releases in one week, and two in the next.
    let mut stages = Vec::new();
    let mut releases = Vec::new();
    for n in 1..=10 {
        stages.push(format!("v0.{n}.0 · Stage {n}"));
        let day = if n <= 8 { "2026-09-11" } else { "2026-09-18" };
        releases.push((format!("v0.{n}.0"), day.to_string()));
    }
    let stages: Vec<&str> = stages.iter().map(String::as_str).collect();
    let releases: Vec<(&str, &str)> = releases.iter().map(|(v, d)| (v.as_str(), d.as_str())).collect();
    project(data.path(), "alpha", &stages, &releases);

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "2"]);
    let row = out.lines().find(|l| l.starts_with("alpha")).unwrap_or("");
    // The span and the count, not eight names.
    assert!(row.contains("(8)"), "a busy week should be counted: {out}");
    assert!(row.contains("v0.1.0..v0.8.0"), "{out}");
    // Two still fit, because two is not a wall of text.
    assert!(row.contains("*v0.9.0 *v0.10.0"), "{out}");
    // And the row stays readable. The defect was a line of 300+ characters.
    assert!(row.chars().count() < 120, "the row is {} characters wide: {out}", row.chars().count());
}

/// Inside a counted week, the mark shown is the one most worth seeing: a
/// release that slipped into a busy week must not be hidden by the eight
/// that landed on time beside it.
#[test]
fn a_counted_week_shows_its_worst_standing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let stages: Vec<String> = (1..=4).map(|n| format!("v0.{n}.0 · Stage {n}")).collect();
    let stages: Vec<&str> = stages.iter().map(String::as_str).collect();
    let releases: Vec<(&str, &str)> = vec![
        ("v0.1.0", "2026-09-11"),
        ("v0.2.0", "2026-09-11"),
        ("v0.3.0", "2026-09-11"),
        ("v0.4.0", "2026-09-11"),
    ];
    project(data.path(), "alpha", &stages, &releases);
    // Three landed in the week they were aimed at; one slipped into it.
    for version in ["v0.1.0", "v0.2.0", "v0.3.0"] {
        rigger(data.path())
            .args(["version", "plan", "alpha", version, "--week", "2026-W37"])
            .assert()
            .success();
    }
    rigger(data.path())
        .args(["version", "plan", "alpha", "v0.4.0", "--week", "2026-W35"])
        .assert()
        .success();

    let out = output(data.path(), &["calendar", "--from", "2026-W37", "--weeks", "1"]);
    let row = out.lines().find(|l| l.starts_with("alpha")).unwrap_or("");
    assert!(row.contains(">v0.1.0..v0.4.0 (4)"), "the slip must survive the count: {out}");
    // And it is still named underneath with its distance.
    assert!(out.contains("2 weeks late"), "{out}");
}
