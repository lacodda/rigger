//! A sitting, and what it holds.
//!
//! What is under test is the boundary. Before this, an event knew its
//! project and its day and nothing else, so "what did we do last time" could
//! only be answered by a window of days - wrong in both directions: two
//! sittings in an evening became one blur, and one spanning midnight became
//! two. A session records the boundary instead of guessing it.
//!
//! The second half is the end-of-session ritual, which has always been a
//! list in a skill file that the assistant had to remember at exactly the
//! moment it was running out of context.

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

fn project(data: &Path, name: &str, releases: &[(&str, &str)]) {
    let root = data.join(name);
    let hub = root.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), "# План\n\n## v0.1.0 · First\n\n- [ ] a task\n").unwrap();

    git(&root, &["init", "--quiet", "--initial-branch", "main"], None);
    std::fs::write(root.join("README.md"), name).unwrap();
    git(&root, &["add", "."], Some("2026-08-10T12:00:00Z"));
    git(&root, &["commit", "--quiet", "-m", "chore: start"], Some("2026-08-10T12:00:00Z"));
    for (n, (version, date)) in releases.iter().enumerate() {
        let at = format!("{date}T12:00:00Z");
        std::fs::write(root.join(format!("f{n}.txt")), version).unwrap();
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

fn note(data: &Path, project: &str, kind: &str, text: &str) {
    rigger(data).args(["note", project, text, "--kind", kind]).assert().success();
}

/// The whole point: an event written while a session is open belongs to it,
/// and one written outside does not. Nothing has to be told to do this - the
/// assistant records as it always has, and the record applies the boundary.
#[test]
fn events_written_while_a_session_is_open_belong_to_it() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    // Before the sitting.
    note(data.path(), "alpha", "finding", "written beforehand");

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "decision", "written inside");
    note(data.path(), "alpha", "change", "also inside");

    let json = output(data.path(), &["session", "end", "alpha", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let session = &json["session"];
    assert_eq!(session["decisions"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(session["changes"].as_array().unwrap().len(), 1, "{json}");
    // The one written before the sitting is not in it - which is the claim,
    // and what a window of days could never get right.
    let findings = session["findings"].as_array().unwrap();
    assert!(findings.is_empty(), "an event from before the session was counted in it: {json}");
}

/// An event written after a session closed belongs to no session, and must
/// not be swept into the next one that happens to open.
#[test]
fn an_event_written_between_sessions_belongs_to_neither() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "decision", "inside the first");
    rigger(data.path()).args(["session", "end", "alpha"]).assert().success();

    note(data.path(), "alpha", "finding", "in between");

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["session", "end", "alpha", "--json"])).unwrap();
    let session = &json["session"];
    assert!(session["findings"].as_array().unwrap().is_empty(), "{json}");
    assert!(session["decisions"].as_array().unwrap().is_empty(), "{json}");
}

/// Starting twice joins the sitting rather than splitting it. An assistant
/// that lost its place, or a hook that fired again, would otherwise orphan
/// half its events under a session nobody ever ends.
#[test]
fn starting_a_second_time_joins_the_sitting() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "decision", "before the second start");
    let again = output(data.path(), &["session", "start", "alpha"]);
    assert!(again.contains("already open"), "{again}");
    note(data.path(), "alpha", "finding", "after it");

    // Both events are in one session, not one each in two.
    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["session", "end", "alpha", "--json"])).unwrap();
    assert_eq!(json["session"]["decisions"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(json["session"]["findings"].as_array().unwrap().len(), 1, "{json}");

    // And there is nothing left open to end.
    let after = output(data.path(), &["session", "end", "alpha"]);
    assert!(after.contains("No session is open"), "{after}");
}

/// The chronicle read out of commit messages is what git already says, not
/// what the sitting did. Counting it would make any session that happened to
/// run `sync` look productive.
#[test]
fn the_chronicle_from_commits_is_not_counted_as_the_sessions_work() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    // A commit made and synced during the sitting: its change event is git's
    // sentence, not the session's.
    let root = data.path().join("alpha");
    std::fs::write(root.join("work.txt"), "x").unwrap();
    git(&root, &["add", "."], Some("2026-09-04T12:00:00Z"));
    git(&root, &["commit", "--quiet", "-m", "feat: a committed change"], Some("2026-09-04T12:00:00Z"));
    rigger(data.path()).args(["sync", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "a change the session wrote");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["session", "end", "alpha", "--json"])).unwrap();
    let changes = json["session"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{json}");
    assert_eq!(changes[0], "a change the session wrote");
}

/// The ritual asks for a next step and for a reason behind the work. These
/// are the last two steps of a sitting, so they are the two most often
/// missing - a session ends when attention has run out.
#[test]
fn the_end_says_what_the_ritual_asked_for_and_did_not_get() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "did a thing");
    let out = output(data.path(), &["session", "end", "alpha"]);
    assert!(out.contains("The ritual asks for"), "{out}");
    assert!(out.contains("no next step"), "{out}");
    assert!(out.contains("nothing says why"), "{out}");

    // With both, nothing is asked for.
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "did another thing");
    note(data.path(), "alpha", "decision", "and here is why");
    note(data.path(), "alpha", "next", "carry on tomorrow");
    let out = output(data.path(), &["session", "end", "alpha"]);
    assert!(!out.contains("The ritual asks for"), "{out}");
}

/// A day spent deciding is not a day that owes an explanation. Without this
/// the check would tell a session off for the very thing it did.
#[test]
fn a_session_that_only_reasoned_is_not_asked_why() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "decision", "we will use ISO weeks");
    note(data.path(), "alpha", "next", "start on it");
    let out = output(data.path(), &["session", "end", "alpha"]);
    assert!(!out.contains("nothing says why"), "{out}");
}

/// A hook fires on every stop. One that speaks every time is one nobody
/// reads, so `--remind` says something only when something is missing - and
/// having no session open at all is ordinary, not a failure.
#[test]
fn a_reminder_speaks_only_when_there_is_something_to_say() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    // No session: silent, and successful.
    let out = output(data.path(), &["session", "end", "alpha", "--remind"]);
    assert!(out.trim().is_empty(), "a hook spoke about nothing: {out}");

    // A complete session: also silent.
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "a change");
    note(data.path(), "alpha", "decision", "and why");
    note(data.path(), "alpha", "next", "onwards");
    let out = output(data.path(), &["session", "end", "alpha", "--remind"]);
    assert!(out.trim().is_empty(), "a hook spoke about a complete session: {out}");

    // An incomplete one: speaks, and names what is missing.
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "a lone change");
    let out = output(data.path(), &["session", "end", "alpha", "--remind"]);
    assert!(out.contains("nothing says why"), "{out}");
}

/// The entry is the session's own sentences, arranged. rigger has no opinion
/// about the day, and inventing one would put words in the owner's diary
/// that nobody said.
#[test]
fn the_diary_entry_is_written_from_what_was_recorded() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);
    let diary = data.path().join("diary.md");
    std::fs::write(
        &diary,
        "# Дневник работы\n\nОдна запись на сессию, новые сверху.\n\n---\n\n## 2026-09-01 · раньше\n\nЧто-то было.\n",
    )
    .unwrap();

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "added the calendar");
    note(data.path(), "alpha", "pitfall", "the grid wraps at 46 releases");
    note(data.path(), "alpha", "next", "ship the retro");

    rigger(data.path())
        .args(["session", "end", "alpha", "--heading", "v0.1.0 «First»", "--diary"])
        .arg(&diary)
        .assert()
        .success();

    let text = std::fs::read_to_string(&diary).unwrap();
    // The preamble survives, the new entry is above the old one, and the old
    // one is untouched: a diary is appended to, never rewritten.
    assert!(text.starts_with("# Дневник работы"), "{text}");
    let new_at = text.find("v0.1.0 «First»").unwrap_or_else(|| panic!("no new entry: {text}"));
    let old_at = text.find("## 2026-09-01 · раньше").unwrap_or_else(|| panic!("the old entry was lost: {text}"));
    assert!(new_at < old_at, "the new entry went below the old one: {text}");
    assert!(text.contains("Что-то было."), "{text}");

    assert!(text.contains("added the calendar"), "{text}");
    assert!(text.contains("the grid wraps at 46 releases"), "{text}");
    assert!(text.contains("**Следующий шаг.** ship the retro"), "{text}");
    // An empty section is not printed as a bare heading.
    assert!(!text.contains("**Решения.**"), "{text}");
}

/// A diary that does not exist yet is made, rather than being a reason to
/// fail at the end of a session.
#[test]
fn a_diary_is_created_if_it_is_not_there() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);
    let diary = data.path().join("new-diary.md");

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "change", "the first thing");
    rigger(data.path()).args(["session", "end", "alpha", "--diary"]).arg(&diary).assert().success();

    let text = std::fs::read_to_string(&diary).unwrap();
    assert!(text.contains("the first thing"), "{text}");
    assert!(text.starts_with("## "), "{text}");
}

/// The question an assistant returning to a project actually has is not
/// "what has been going on" but "what changed while I was away". Without the
/// line, every session re-reads the same history unable to tell which part
/// of it is new.
#[test]
fn the_packet_says_what_happened_since_the_last_sitting() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    // No session has ever ended, so there is nothing to be since.
    let packet = output(data.path(), &["context", "alpha"]);
    assert!(!packet.contains("Last session ended"), "{packet}");

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    note(data.path(), "alpha", "decision", "inside the sitting");
    rigger(data.path()).args(["session", "end", "alpha"]).assert().success();

    // Straight after, nothing has happened since - and saying so is the
    // point: it tells the reader the events below are all from before.
    let packet = output(data.path(), &["context", "alpha"]);
    assert!(packet.contains("nothing since"), "{packet}");

    // Recorded immediately, which is the case that nearly went missing:
    // timestamps are kept to the second, so the first thing written after a
    // sitting usually shares the second the sitting closed in. With a strict
    // boundary it would vanish from the one line meant to report it.
    note(data.path(), "alpha", "finding", "after hours");
    let packet = output(data.path(), &["context", "alpha"]);
    assert!(packet.contains("1 event recorded"), "{packet}");
    assert!(!packet.contains("nothing since"), "{packet}");
}

/// The boundary second belongs to "since". Timestamps are kept to the
/// second, so an event written in the same second as a session closed is not
/// strictly after it - and that is the most likely second for the next thing
/// to be written in.
#[test]
fn an_event_in_the_second_the_session_closed_counts_as_since() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["session", "end", "alpha", "--json"])).unwrap();
    let ended_at = json["session"]["ended_at"].as_str().unwrap().to_string();

    // Recorded again and again until one lands in the closing second, so
    // the case is exercised rather than hoped for. A handful of tries is
    // plenty: notes take milliseconds.
    let mut hit = false;
    for n in 0..40 {
        note(data.path(), "alpha", "finding", &format!("attempt {n}"));
        let packet: serde_json::Value = serde_json::from_str(&output(data.path(), &["context", "alpha", "--json"])).unwrap();
        let events = packet["state"]["since_last_session"]["events"].as_u64().unwrap_or(0);
        // Whatever else is true, every note written after the close must be
        // counted - including the one sharing its second.
        assert_eq!(events, n + 1, "an event after the close went uncounted (ended {ended_at}): {packet}");
        hit = true;
    }
    assert!(hit);
}

/// Work committed between sessions is counted apart from work recorded in
/// them: most of what happens to a project is commits, and a returning
/// assistant needs to know which of the two it missed.
#[test]
fn commits_since_the_last_sitting_are_counted_apart_from_events() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    rigger(data.path()).args(["session", "end", "alpha"]).assert().success();

    // Dated after the sitting, because a commit event carries its commit's
    // own date: work committed with an older date is not work done since,
    // however recently it was pushed, and "since" has to mean since.
    let root = data.path().join("alpha");
    std::fs::write(root.join("later.txt"), "x").unwrap();
    let after = "2099-01-01T12:00:00Z";
    git(&root, &["add", "."], Some(after));
    git(&root, &["commit", "--quiet", "-m", "feat: done outside a session"], Some(after));
    rigger(data.path()).args(["sync", "alpha"]).assert().success();

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["context", "alpha", "--json"])).unwrap();
    let since = &json["state"]["since_last_session"];
    assert_eq!(since["commits"], 1, "{json}");
    assert_eq!(since["events"], 0, "{json}");

    let packet = output(data.path(), &["context", "alpha"]);
    assert!(packet.contains("1 change committed"), "{packet}");
}

/// The budget is a gate, and a new section in the packet is exactly the kind
/// of thing that quietly pushes it over.
#[test]
fn the_packet_still_fits_its_budget_with_a_session_line() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    for n in 0..60 {
        note(
            data.path(),
            "alpha",
            "finding",
            &format!("a finding numbered {n}, with enough words in it to cost something worth counting"),
        );
    }
    rigger(data.path()).args(["session", "end", "alpha"]).assert().success();

    let explained = output(data.path(), &["context", "alpha", "--explain"]);
    let total: usize = explained
        .lines()
        .find(|l| l.trim_start().starts_with("total"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no total in --explain: {explained}"));
    assert!(total <= 3000, "the packet went over its budget: {total}");
    assert!(explained.contains("Last session ended"), "{explained}");
}

/// A hook has no project name to pass: the Stop hook of an assistant is
/// handed a working directory and nothing else. But it runs in the project,
/// and the record knows every project by its path - so the directory is the
/// name, and the hook needs to be told nothing.
#[test]
fn the_working_directory_names_the_project_when_nothing_else_does() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);
    let root = data.path().join("alpha");

    rigger(data.path()).current_dir(&root).args(["session", "start"]).assert().success();
    note(data.path(), "alpha", "change", "recorded from the root");

    // And from a subdirectory, because a session ends wherever the last
    // command left the shell - which is rarely the checkout's root.
    let deep = root.join("src").join("deep");
    std::fs::create_dir_all(&deep).unwrap();
    let out = rigger(data.path()).current_dir(&deep).args(["session", "end"]).assert().success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(out.contains("alpha"), "{out}");
    assert!(out.contains("1 change"), "{out}");
}

/// A hook fires in every directory, most of which are not projects. Failing
/// there would turn every unrelated session into an error message - and an
/// assistant's Stop hook that exits non-zero can hold the turn open, so the
/// cost of getting this wrong is not cosmetic.
#[test]
fn a_reminder_outside_any_project_is_silent_and_successful() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    project(data.path(), "alpha", &[]);

    let elsewhere = data.path().join("not-a-project");
    std::fs::create_dir_all(&elsewhere).unwrap();

    let out = rigger(data.path())
        .current_dir(&elsewhere)
        .args(["session", "end", "--remind"])
        .assert()
        .success();
    assert!(String::from_utf8(out.get_output().stdout.clone()).unwrap().trim().is_empty());

    // Asked directly, in the same place, it says what is wrong: the silence
    // is for the hook, not a general refusal to explain itself.
    rigger(data.path())
        .current_dir(&elsewhere)
        .args(["session", "end"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project recorded"));
}

#[test]
fn a_project_nobody_recorded_is_refused_by_both_halves() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();

    for args in [vec!["session", "start", "ghost"], vec!["session", "end", "ghost"]] {
        rigger(data.path()).args(&args).assert().failure().stderr(predicate::str::contains("ghost"));
    }
}
