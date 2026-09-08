//! What "every project through rigger" needs from the record.
//!
//! A hub that has lived by hand for a month is not the hub the first
//! import read: stages were renumbered, tasks reworded and struck, and the
//! diary was read once by an older reader. And a hub written from the
//! record has to go on growing without a hand: the diary entry a sitting
//! leaves, the line the README's state block gains. Each test here is one
//! of those, found on the line before it was written down.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    hub
}

fn import(data: &Path, hub: &Path) {
    rigger(data).args(["import", "sample", "--hub"]).arg(hub).assert().success();
}

/// The generated file without its mark.
fn body(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    text.split_once('\n').map(|(_, rest)| rest.trim_start_matches('\n').to_string()).unwrap_or(text)
}

fn export(data: &Path, file: &str) -> String {
    let out = data.join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    body(&out.join(file))
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// A task struck from the plan by hand is struck in the record: not shown
/// by the export, not counted as open, and back the moment the plan lists
/// it again.
#[test]
fn a_task_struck_from_the_plan_is_dropped_and_comes_back_when_listed_again() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let two = "# План\n\n## v0.2.0 · Второй\n\n- [ ] сделать одно\n- [ ] сделать другое\n";
    let one = "# План\n\n## v0.2.0 · Второй\n\n- [ ] сделать одно\n";
    std::fs::write(hub.join("План.md"), two).unwrap();
    import(data.path(), &hub);
    assert!(output(data.path(), &["context", "sample"]).contains("2 tasks open"));

    std::fs::write(hub.join("План.md"), one).unwrap();
    let report = output(data.path(), &["import", "sample", "--hub", hub.to_str().unwrap()]);
    assert!(report.contains("0 versions and 1 task struck from the hub"), "{report}");
    assert_eq!(export(data.path(), "План.md"), one);
    assert!(output(data.path(), &["context", "sample"]).contains("1 task open"));

    std::fs::write(hub.join("План.md"), two).unwrap();
    import(data.path(), &hub);
    assert_eq!(export(data.path(), "План.md"), two);
    assert!(output(data.path(), &["context", "sample"]).contains("2 tasks open"));
}

/// A stage renumbered by hand leaves no ghost under its old number.
#[test]
fn a_stage_renumbered_by_hand_drops_its_old_number() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("План.md"), "# План\n\n## v1.10 · Триаж\n\n- [ ] snooze\n").unwrap();
    import(data.path(), &hub);

    let renumbered = "# План\n\n## v1.11 · Триаж\n\n- [ ] snooze\n";
    std::fs::write(hub.join("План.md"), renumbered).unwrap();
    let report = output(data.path(), &["import", "sample", "--hub", hub.to_str().unwrap()]);
    assert!(report.contains("1 version and"), "{report}");
    assert_eq!(export(data.path(), "План.md"), renumbered);
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("1 version planned, 1 task open"), "{packet}");
    assert!(!packet.contains("v1.10"), "{packet}");
}

/// A diary entry read by an older reader - its date kept apart from its
/// heading - is the same entry as the one read now, not a second one.
#[test]
fn a_diary_entry_read_without_its_date_is_merged_with_itself() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let diary = "# Дневник\n\n## 2026-09-03 (ночь) · v0.2.1: знак\n\n**Сделано.** Знак в шапке.\n";
    std::fs::write(hub.join("Дневник.md"), diary).unwrap();
    // The row the older reader left.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    db.execute(
        "INSERT INTO sessions (project_id, started_at, ended_at, day, heading, notes, followed_by_rule, gap_after, rank) \
         VALUES (1, '2026-09-03T00:00:00Z', '2026-09-03T00:00:00Z', '2026-09-03', '(ночь) · v0.2.1: знак', '**Сделано.** Знак в шапке.', 0, 1, 0)",
        [],
    )
    .unwrap();
    drop(db);

    import(data.path(), &hub);
    let written = export(data.path(), "Дневник.md");
    assert_eq!(written, diary, "one entry, under the heading the hub writes");
    assert_eq!(written.matches("Знак в шапке").count(), 1);
}

/// The entry a sitting leaves goes into the record, where an export of a
/// hub written from the record reads it - newest first.
#[test]
fn a_session_leaves_its_diary_entry_in_the_record() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("Дневник.md"), "# Дневник\n\n## 2026-09-01 · Основание\n\n**Сделано.** Начало.\n").unwrap();
    import(data.path(), &hub);

    rigger(data.path()).args(["session", "start", "sample"]).assert().success();
    rigger(data.path())
        .args(["note", "sample", "--kind", "decision", "Решено так."])
        .assert()
        .success();
    rigger(data.path())
        .args(["session", "end", "sample", "--heading", "v0.2.0 «Второй»"])
        .assert()
        .success();

    let written = export(data.path(), "Дневник.md");
    let today = written
        .lines()
        .find(|l| l.starts_with("## 20") && l.contains("v0.2.0 «Второй»"))
        .unwrap_or_default();
    assert!(!today.is_empty(), "the sitting's entry is in the diary: {written}");
    assert!(written.contains("Решено так."), "{written}");
    assert!(
        written.find("v0.2.0 «Второй»").unwrap() < written.find("Основание").unwrap(),
        "newest first: {written}"
    );
    // A second export is the same bytes: the entry is in the record, not composed on the way out.
    assert_eq!(export(data.path(), "Дневник.md"), written);
}

/// A sitting that shifts the state says so with one line, and the README
/// gains it at the top of its state block.
#[test]
fn a_state_line_lands_at_the_top_of_the_readme_block() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let readme = "# sample\n\n## Состояние\n\n- **2026-09-01** — **Основан.** Репозиторий заведён.\n";
    std::fs::write(hub.join("README.md"), readme).unwrap();
    import(data.path(), &hub);

    rigger(data.path())
        .args(["note", "sample", "--kind", "state", "**v0.2.0 выпущен.** Второй этап закрыт."])
        .assert()
        .success()
        .stdout(predicate::str::contains("state line"));

    let written = export(data.path(), "README.md");
    let block = written.split("## Состояние").nth(1).unwrap();
    let first = block.lines().find(|l| l.starts_with("- ")).unwrap();
    assert!(first.contains("v0.2.0 выпущен"), "{written}");
    assert!(first.starts_with("- **20"), "dated today: {first}");
    assert!(block.contains("Основан."), "the older line stays: {written}");
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
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

/// A version the record knows only from a tag is a fact, not an entry:
/// the changelog does not gain a bare heading for it.
#[test]
fn a_version_known_only_from_a_tag_is_not_written_into_the_changelog() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let root = data.path().join("sample");
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("README.md"), "x").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feat: start"]);
    git(&root, &["tag", "v0.1.1"]);

    let changes = "# Изменения\n\n## v0.1.0 · Первый — выпущена 2026-09-01\n\nБыло.\n";
    std::fs::write(hub.join("Изменения.md"), changes).unwrap();
    import(data.path(), &hub);
    rigger(data.path()).args(["sync", "sample"]).assert().success();

    assert_eq!(export(data.path(), "Изменения.md"), changes);
    // It is still a fact the record holds.
    let packet = output(data.path(), &["context", "sample"]);
    assert!(packet.contains("Last shipped: v0.1.1"), "{packet}");
}

/// A question the owner struck from the hub is withdrawn; one an assistant
/// asked was never in the file and stays.
#[test]
fn a_question_struck_from_the_hub_is_withdrawn_but_an_assistants_stays() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let two = "# План\n\n## Ждёт решения владельца\n\n1. Первый вопрос?\n2. Второй вопрос?\n\n## v0.2.0 · Второй\n\n- [ ] одно\n";
    std::fs::write(hub.join("План.md"), two).unwrap();
    import(data.path(), &hub);
    rigger(data.path()).args(["wish", "sample", "Хочется третьего."]).assert().success();
    let asked = "Ассистент спрашивает?";
    // The way `ask_owner` records it: a question by the assistant.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    db.execute(
        "INSERT INTO events (project_id, kind, body, author, created_at) VALUES (1, 'question', ?1, 'assistant', '2026-09-08T10:00:00Z')",
        [asked],
    )
    .unwrap();
    drop(db);

    let one = "# План\n\n## Ждёт решения владельца\n\n1. Второй вопрос?\n\n## v0.2.0 · Второй\n\n- [ ] одно\n";
    std::fs::write(hub.join("План.md"), one).unwrap();
    let report = output(data.path(), &["import", "sample", "--hub", hub.to_str().unwrap()]);
    assert!(report.contains("withdrawn  1 question struck from the hub"), "{report}");

    let packet = output(data.path(), &["context", "sample"]);
    assert!(!packet.contains("Первый вопрос"), "{packet}");
    assert!(packet.contains("Второй вопрос"), "{packet}");
    assert!(packet.contains(asked), "the assistant's question stays: {packet}");
}

/// A hub still kept by hand is named as such: it is what "every project
/// through rigger" has left to do.
#[test]
fn doctor_names_the_hubs_still_kept_by_hand() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("План.md"), "# План\n\n## v0.2.0 · Второй\n\n- [ ] одно\n").unwrap();
    import(data.path(), &hub);

    let before = output(data.path(), &["doctor", "--hubs"]);
    assert!(before.contains("sample") && before.contains("kept by hand"), "{before}");
    assert!(before.contains("План.md"), "{before}");

    rigger(data.path())
        .args(["export", "sample", "--hub"])
        .arg(&hub)
        .arg("--adopt")
        .assert()
        .success();
    let after = output(data.path(), &["doctor", "--hubs"]);
    assert!(after.contains("every generated file matches the record"), "{after}");
}
