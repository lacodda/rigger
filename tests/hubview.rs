//! The hub as a view of the record, and a view that stays one.
//!
//! The export made the hub a picture of the record. What it did not do was
//! keep the picture: the record moved on after every sitting, the files did
//! not, and `doctor --hubs` could not tell a file the record had merely
//! moved past from one somebody had written in - on the line's hubs it once
//! named 66 files, two of which held an edit. These tests hold what changed
//! that: the record notes what it last wrote into each file, a session end
//! writes again what nobody touched, and nothing an import reads can strike,
//! wipe or delete what the file did not speak for.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;

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

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn json(data: &Path, args: &[&str]) -> serde_json::Value {
    let mut all = args.to_vec();
    all.push("--json");
    serde_json::from_str(&output(data, &all)).unwrap()
}

const PLAN: &str = "\
# План

## Ждёт решения владельца

1. Первый вопрос?

## v0.2.0 · Второй

- [ ] сделать одно

## v0.3.0 · Третий

- [ ] сделать другое
";

const CHANGES: &str = "\
# Изменения

## v0.1.0 · Первый — выпущена 2026-09-01

Что было сделано.
";

const DIARY: &str = "\
# Дневник

---

## 2026-09-02 · Основание

**Сделано.** Первая запись.
";

const README: &str = "\
# alpha

## Состояние

- **2026-09-02** — основание
";

/// A project with a hub of the four generated files, imported, and then
/// handed over to the record by a first export - the state every hub of
/// the line is in.
fn project(data: &Path) -> PathBuf {
    let root = data.join("alpha");
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), PLAN).unwrap();
    std::fs::write(hub.join("Изменения.md"), CHANGES).unwrap();
    std::fs::write(hub.join("Дневник.md"), DIARY).unwrap();
    std::fs::write(hub.join("README.md"), README).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("README.md"), "alpha").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "chore: start"]);

    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    rigger(data).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();
    rigger(data).args(["export", "alpha", "--adopt", "--hub"]).arg(&hub).assert().success();
    hub
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// The whole point of the digest: a file the record wrote and has since
/// moved past is behind it, which loses nothing; a file somebody wrote in
/// holds an edit. Told apart, the first stops burying the second.
#[test]
fn doctor_tells_a_file_behind_the_record_from_one_written_in() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    assert!(
        output(data.path(), &["doctor", "--hubs"]).contains("every file says what the record says"),
        "a hub just exported is the record"
    );

    // The record moves on: a question for the owner lands in the plan.
    rigger(data.path())
        .args(["note", "alpha", "Второй вопрос?", "--kind", "question"])
        .assert()
        .success();
    let behind = output(data.path(), &["doctor", "--hubs"]);
    assert!(!behind.contains("cannot vouch"), "a file the record moved past is no fault: {behind}");
    assert!(behind.contains("behind the record (1 file in 1 hub): alpha"), "{behind}");
    let doc = json(data.path(), &["doctor", "--hubs"]);
    assert_eq!(doc["hubs"], serde_json::json!([]), "{doc}");
    assert_eq!(doc["hubs_behind"][0]["file"], "План.md", "{doc}");

    // Somebody writes in a file: that is an edit, and it is named as one.
    std::fs::write(hub.join("Изменения.md"), format!("{}\nдописано руками\n", read(&hub.join("Изменения.md")))).unwrap();
    let edited = output(data.path(), &["doctor", "--hubs"]);
    assert!(edited.contains("cannot vouch for (1)"), "{edited}");
    assert!(
        edited.contains("Изменения.md") && edited.contains("edited since the record wrote it"),
        "{edited}"
    );
    let doc = json(data.path(), &["doctor", "--hubs"]);
    assert_eq!(doc["hubs"][0]["state"], "edited", "{doc}");
    assert_eq!(doc["hubs_behind"][0]["file"], "План.md", "the behind file stays behind: {doc}");
}

/// A session end writes again every file of the hub nobody touched since
/// the record wrote it, and leaves the one somebody wrote in - naming it.
#[test]
fn a_session_end_brings_the_hub_up_to_date_where_that_loses_nothing() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let written_in = format!("{}\nдописано руками\n", read(&hub.join("Изменения.md")));
    std::fs::write(hub.join("Изменения.md"), &written_in).unwrap();

    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    rigger(data.path())
        .args(["note", "alpha", "Решили одно.", "--kind", "decision"])
        .assert()
        .success();
    let end = output(data.path(), &["session", "end", "alpha", "--heading", "Сессия"]);

    let diary = read(&hub.join("Дневник.md"));
    assert!(diary.contains("· Сессия"), "the sitting reached the hub's diary: {diary}");
    assert!(diary.contains("Решили одно."), "{diary}");
    assert!(end.contains("brought the hub up to date: Дневник.md"), "{end}");
    // The file with an edit in it is left as it was, and named.
    assert_eq!(read(&hub.join("Изменения.md")), written_in, "an edit was overwritten");
    assert!(end.contains("the hub holds edits the record has not seen (Изменения.md)"), "{end}");
    // And what was brought up to date is now the record again.
    let doctor = json(data.path(), &["doctor", "--hubs"]);
    assert_eq!(doctor["hubs_behind"], serde_json::json!([]), "{doctor}");
}

/// The heading of a sitting's entry is the one its text carries, in the
/// record and in the file alike. The file used to get the text's heading
/// and the record one made from `--heading`, so the hub's diary read
/// differently from the record after any sitting that gave one of the two.
#[test]
fn an_entry_keeps_one_heading_in_the_record_and_in_the_file() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    rigger(data.path())
        .args(["note", "alpha", "Нашли одно.", "--kind", "finding"])
        .assert()
        .success();
    let entry = data.path().join("entry.md");
    std::fs::write(&entry, "## 2026-10-20 · Свой заголовок\r\n\r\nЧто было на самом деле.\r\n").unwrap();
    rigger(data.path())
        .args(["session", "end", "alpha", "--heading", "Другой", "--entry"])
        .arg(&entry)
        .arg("--diary")
        .arg(hub.join("Дневник.md"))
        .assert()
        .success();

    let diary = read(&hub.join("Дневник.md"));
    assert!(
        diary.contains("## 2026-10-20 · Свой заголовок\n\nЧто было на самом деле.\n\n## 2026-09-02"),
        "{diary}"
    );
    assert!(!diary.contains("Другой"), "{diary}");
    assert!(!diary.contains("\r\r"), "a carriage return was doubled: {diary:?}");
    // The file is what the record says, to the byte: nothing to vouch for.
    let doctor = output(data.path(), &["doctor", "--hubs"]);
    assert!(doctor.contains("every file says what the record says"), "{doctor}");

    // A diary kept by hand gets the same heading the record holds.
    let by_hand = data.path().join("by-hand.md");
    std::fs::write(&by_hand, "# Дневник\n\n## 2026-09-01\n\nРаньше.\n").unwrap();
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    rigger(data.path())
        .args(["note", "alpha", "Нашли другое.", "--kind", "finding"])
        .assert()
        .success();
    rigger(data.path())
        .args(["session", "end", "alpha", "--heading", "Второй раз", "--diary"])
        .arg(&by_hand)
        .assert()
        .success();
    let kept = read(&by_hand);
    let heading = kept
        .lines()
        .find(|l| l.starts_with("## ") && l.contains("Второй раз"))
        .unwrap_or_default()
        .to_string();
    assert!(!heading.is_empty(), "{kept}");
    assert!(kept.find(&heading).unwrap() < kept.find("## 2026-09-01").unwrap(), "newest first: {kept}");
    assert!(read(&hub.join("Дневник.md")).contains(&heading), "the record holds the same heading");

    // A generated diary somewhere else is written whole from the record, as
    // its mark says: a line written into it does not survive, and an entry
    // is not appended in a shape of its own.
    let elsewhere = data.path().join("elsewhere.md");
    let generated = read(&hub.join("Дневник.md"));
    std::fs::write(&elsewhere, generated.replacen("# Дневник", "# Дневник\n\nстрока, вписанная руками", 1)).unwrap();
    rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
    rigger(data.path())
        .args(["note", "alpha", "Нашли третье.", "--kind", "finding"])
        .assert()
        .success();
    rigger(data.path())
        .args(["session", "end", "alpha", "--diary"])
        .arg(&elsewhere)
        .assert()
        .success();
    assert_eq!(read(&elsewhere), read(&hub.join("Дневник.md")), "the generated diary is the record's");
}

/// Two sittings of one day under one heading are two entries, through an
/// export and an import. Matched an entry at a time, the second found the
/// first one's row and a merge deleted it: a release sitting of a real
/// project vanished on an ordinary import.
#[test]
fn two_entries_of_one_day_survive_an_import() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    for text in ["Первое за день.", "Второе за день."] {
        rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
        rigger(data.path()).args(["note", "alpha", text, "--kind", "finding"]).assert().success();
        rigger(data.path()).args(["session", "end", "alpha"]).assert().success();
    }
    let diary = read(&hub.join("Дневник.md"));
    let day = diary.lines().find(|l| l.starts_with("## 20") && !l.contains("Основание")).unwrap().to_string();
    assert_eq!(diary.matches(&format!("{day}\n")).count(), 2, "two entries under one heading: {diary}");

    // An edit to the first, so the import reads the diary back.
    std::fs::write(hub.join("Дневник.md"), diary.replacen("Второе за день.", "Второе за день, поправлено.", 1)).unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();
    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "alpha", "--hub"]).arg(&out).assert().success();
    let back = read(&out.join("Дневник.md"));
    assert!(back.contains("Первое за день."), "a sitting was deleted: {back}");
    assert!(back.contains("Второе за день, поправлено."), "{back}");
    assert_eq!(back.matches(&format!("{day}\n")).count(), 2, "{back}");

    // And each entry is still the entry of its own sitting: matched by
    // heading alone, the newest-first file paired the two the other way
    // round, and a sitting's diary came to describe its neighbour's work.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    for (finding, entry) in [("Первое за день.", "Первое за день."), ("Второе за день.", "Второе за день, поправлено.")]
    {
        let notes: String = db
            .query_row(
                "SELECT s.notes FROM sessions s JOIN events e ON e.session_id = s.id WHERE e.body = ?1",
                [finding],
                |r| r.get(0),
            )
            .unwrap();
        assert!(notes.contains(entry), "the sitting that found {finding:?} holds {notes:?}");
    }
}

/// A file that is not there says nothing. A hub without its plan named no
/// stage because it had no file to name them in, and the import struck
/// every planned stage on that reasoning; one without its README wiped the
/// state lines.
#[test]
fn a_hub_read_in_part_strikes_and_wipes_nothing() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    // A state line the record has and the README's own prose does not: the
    // prose would put the old lines back by itself, so only this one says
    // whether the record's lines were kept.
    rigger(data.path())
        .args(["note", "alpha", "сделано второе", "--kind", "state"])
        .assert()
        .success();
    let part = data.path().join("part");
    std::fs::create_dir_all(&part).unwrap();
    std::fs::write(part.join("Дневник.md"), "# Дневник\n\n## 2026-09-03 · Ещё\n\nДописали.\n").unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&part).assert().success();

    let show = json(data.path(), &["version", "show", "alpha", "v0.3.0"]);
    assert_ne!(show["status"], "dropped", "{show}");
    let inbox = output(data.path(), &["inbox"]);
    assert!(inbox.contains("Первый вопрос?"), "a question was withdrawn: {inbox}");
    // Reading a directory did not move the hub there.
    let project = json(data.path(), &["show", "alpha"]);
    assert!(project["hub_path"].as_str().unwrap().ends_with("hub"), "{project}");
    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "alpha", "--hub"]).arg(&out).assert().success();
    assert!(read(&out.join("README.md")).contains("сделано второе"), "the state lines were wiped");
    assert!(read(&out.join("Дневник.md")).contains("Дописали."), "the part that was there is read");
}

/// A generated file the record wrote and nobody touched is not read back:
/// it is the record as it stood at that export, and reading it would undo
/// what the record learnt since.
#[test]
fn an_import_does_not_read_back_what_the_record_wrote() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    rigger(data.path())
        .args(["note", "alpha", "сделано второе", "--kind", "state"])
        .assert()
        .success();
    let import = output(data.path(), &["import", "alpha", "--hub", hub.to_str().unwrap()]);
    assert!(import.contains("not read back"), "{import}");
    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "alpha", "--hub"]).arg(&out).assert().success();
    assert!(read(&out.join("README.md")).contains("сделано второе"), "the newer state line was undone");
}

/// Every `.md` of a hub is a document, not only the five with names the
/// reader knew: the texts nobody named a kind for lived in the vault alone.
#[test]
fn every_handwritten_file_of_a_hub_is_taken_in() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("Видение.md"), "# Видение\n\nЗачем.\n").unwrap();
    std::fs::write(hub.join("Стенды.md"), "# Стенды\n\n- один\n").unwrap();
    std::fs::create_dir_all(hub.join("Макеты")).unwrap();
    std::fs::write(hub.join("Макеты/Экран.md"), "Без заголовка.\n").unwrap();
    // A name that slugs to an address already taken keeps both texts.
    std::fs::write(hub.join("Vision.md"), "# Vision\n\nAnother text.\n").unwrap();
    // A generated file in an unexpected place is a view, not a text.
    std::fs::write(hub.join("Копия.md"), "<!-- generated by rigger; edits here are overwritten -->\n\nкопия\n").unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();

    let docs = json(data.path(), &["doc", "list", "alpha"]);
    let docs = docs.as_array().cloned().unwrap();
    let by = |file: &str| docs.iter().find(|d| d["file"] == file).cloned();
    let stands = by("Стенды.md").expect("Стенды.md is a document");
    assert_eq!(stands["kind"], "other");
    assert_eq!(stands["slug"], "стенды");
    let screen = by("Макеты/Экран.md").expect("a file in a folder is a document");
    assert_eq!(screen["slug"], "макеты-экран");
    assert_eq!(screen["title"], "Экран", "a text without a heading is called what its file is called");
    let vision = by("Видение.md").unwrap();
    let other = by("Vision.md").unwrap();
    assert_ne!(vision["slug"], other["slug"], "two texts, two addresses");
    assert!(by("Копия.md").is_none(), "a generated file was taken as a text");

    // Taken in, the file is one the record has accounted for: written in
    // afterwards, it holds an edit.
    std::fs::write(hub.join("Стенды.md"), "# Стенды\n\n- один\n- два\n").unwrap();
    let doctor = json(data.path(), &["doctor", "--hubs"]);
    let edited = doctor["hubs"].as_array().unwrap().iter().find(|d| d["file"] == "Стенды.md").cloned();
    assert_eq!(edited.map(|d| d["state"].clone()), Some(serde_json::json!("edited")), "{doctor}");
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();

    // Back out to the same paths, to the byte; and read again, nothing moves.
    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "alpha", "--docs", "--hub"]).arg(&out).assert().success();
    assert_eq!(read(&out.join("Макеты/Экран.md")), "Без заголовка.\n");
    assert_eq!(read(&out.join("Стенды.md")), read(&hub.join("Стенды.md")));
    let again = output(data.path(), &["import", "alpha", "--hub", hub.to_str().unwrap()]);
    assert!(again.contains("nothing changed"), "{again}");
}

/// A document read from a CRLF file goes back out in CRLF, once. The record
/// kept the carriage returns, and the export, converting the endings of
/// the file it wrote, turned each kept `\r\n` into `\r\r\n`.
#[test]
fn a_crlf_document_goes_back_out_unchanged() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let rituals = "# Ритуалы\r\n\r\n- первое\r\n- второе\r\n";
    std::fs::write(hub.join("Ритуалы.md"), rituals).unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();
    rigger(data.path()).args(["export", "alpha", "--docs", "--hub"]).arg(&hub).assert().success();
    assert_eq!(read(&hub.join("Ритуалы.md")), rituals);
    // And read again it is the same text: kept with its carriage returns,
    // the hub's copy would differ from the record's on every import.
    let again = output(data.path(), &["import", "alpha", "--hub", hub.to_str().unwrap()]);
    assert!(again.contains("nothing changed"), "{again}");
    let shown = json(data.path(), &["doc", "show", "alpha", "rituals"]);
    let body = shown["body"].as_str().or_else(|| shown["document"]["body"].as_str()).unwrap().to_string();
    assert!(!body.contains('\r'), "the record keeps LF: {body:?}");
}

/// The text a record already holds loses its carriage returns when the
/// record moves to the schema that keeps LF: a document, a sitting's entry
/// and an event read before that would otherwise go out as `\r\r\n` into
/// every CRLF hub.
#[test]
fn the_migration_takes_the_carriage_returns_out_of_what_the_record_holds() {
    let data = tempfile::tempdir().unwrap();
    project(data.path());
    let db = data.path().join("profiles").join("line").join("rigger.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    // The record as the schema before it left things: text with CRLF in it,
    // and no notes on hub files.
    conn.execute_batch(
        "DROP TABLE hub_files;
         UPDATE documents SET body = 'one' || char(13) || char(10) || 'two';
         UPDATE events SET body = 'three' || char(13) || char(10) || 'four' WHERE kind = 'question';
         UPDATE sessions SET notes = 'five' || char(13) || char(10) || 'six' WHERE notes IS NOT NULL;
         INSERT INTO documents (project_id, kind, slug, title, body, created_at, updated_at)
             VALUES (1, 'other', 'note', 'Note', 'a' || char(13) || char(10) || 'b', '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z');
         PRAGMA user_version = 29;",
    )
    .unwrap();
    drop(conn);

    rigger(data.path()).arg("doctor").assert().success();
    let conn = rusqlite::Connection::open(&db).unwrap();
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM documents WHERE instr(body, char(13)) > 0"), 0);
    assert_eq!(count("SELECT count(*) FROM events WHERE instr(body, char(13)) > 0"), 0);
    assert_eq!(count("SELECT count(*) FROM sessions WHERE instr(notes, char(13)) > 0"), 0);
    assert_eq!(
        count("SELECT count(*) FROM documents WHERE body = 'a' || char(10) || 'b'"),
        1,
        "the text itself is kept"
    );
    assert_eq!(count("SELECT count(*) FROM hub_files"), 0, "the table is there again, empty");
}

/// A text written through the tool has no file, and an address of its own
/// choosing. Written out by an export it stays one text: the export tells
/// it which file it lives in, prefers a file of the hub that already holds
/// the same words, and the import knows it by those words. Before, the
/// export made a name up every time and the next import took the file in
/// as a second document.
#[test]
fn a_text_written_through_the_tool_stays_one_text() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    for (title, slug, body) in [
        ("Голоса для чтения", "voices", "# Голоса\n\nКакие голоса звучат.\n"),
        ("HTTPS на стенде", "https-on-the-stand", "# HTTPS\n\nКорень CA на телефоне.\n"),
        ("Третий текст", "third", "# Третий\n\nЕщё один.\n"),
    ] {
        rigger(data.path())
            .args(["doc", "add", "alpha", title, "--kind", "research", "--slug", slug, "--body", body])
            .assert()
            .success();
    }
    let research = |data: &Path| json(data, &["doc", "list", "alpha", "--kind", "research"]).as_array().unwrap().clone();

    // The same words put into the hub by hand, read before any export: the
    // import knows them as the text they are.
    std::fs::create_dir_all(hub.join("Исследования")).unwrap();
    std::fs::write(hub.join("Исследования/2026-09-20 — Третий.md"), "# Третий\n\nЕщё один.\n").unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();
    let docs = research(data.path());
    assert_eq!(docs.len(), 3, "the hand copy came in as a second document: {docs:?}");
    let third = docs.iter().find(|d| d["slug"] == "third").unwrap();
    assert_eq!(third["file"], "Исследования/2026-09-20 — Третий.md", "{third}");

    // Another text put into the hub by hand under a name of its own: the
    // export writes to that file rather than making a second one.
    std::fs::write(hub.join("Исследования/2026-09-17 — HTTPS.md"), "# HTTPS\r\n\r\nКорень CA на телефоне.\r\n").unwrap();
    rigger(data.path()).args(["export", "alpha", "--docs", "--hub"]).arg(&hub).assert().success();
    let notes: Vec<String> = std::fs::read_dir(hub.join("Исследования"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(notes.len(), 3, "one file per text: {notes:?}");
    let docs = research(data.path());
    let https = docs.iter().find(|d| d["slug"] == "https-on-the-stand").unwrap();
    assert_eq!(https["file"], "Исследования/2026-09-17 — HTTPS.md", "{https}");

    // The file the export made for a text is that text's: written in
    // afterwards, it is still the same document, edited.
    let voices = docs.iter().find(|d| d["slug"] == "voices").unwrap()["file"].as_str().unwrap().to_string();
    std::fs::write(hub.join(&voices), "# Голоса\n\nКакие голоса звучат, и сколько их.\n").unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&hub).assert().success();
    let docs = research(data.path());
    assert_eq!(docs.len(), 3, "a text came back as a second document: {docs:?}");
    let shown = json(data.path(), &["doc", "show", "alpha", "voices"]);
    let body = shown["body"].as_str().or_else(|| shown["document"]["body"].as_str()).unwrap().to_string();
    assert!(body.contains("сколько их"), "{body}");
    let https = docs.iter().find(|d| d["slug"] == "https-on-the-stand").unwrap();
    assert_eq!(https["title"], "HTTPS на стенде", "the title it was given is kept");
    let again = output(data.path(), &["import", "alpha", "--hub", hub.to_str().unwrap()]);
    assert!(again.contains("nothing changed"), "{again}");
}

/// A diary file that names some of the sittings speaks for what it says
/// about them, not for the diary's order. Read as the whole diary, a file
/// an older `--diary` had appended to - or one handed a single entry to take
/// in - moved every sitting it did not name above every one it did.
#[test]
fn a_diary_read_in_part_keeps_the_order_it_does_not_name() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    for (heading, text) in [("Первая", "Нашли первое."), ("Вторая", "Нашли второе."), ("Третья", "Нашли третье.")]
    {
        rigger(data.path()).args(["session", "start", "alpha"]).assert().success();
        rigger(data.path()).args(["note", "alpha", text, "--kind", "finding"]).assert().success();
        rigger(data.path()).args(["session", "end", "alpha", "--heading", heading]).assert().success();
    }
    let diary = read(&hub.join("Дневник.md"));
    let at = diary.find("· Вторая").unwrap();
    let line_start = diary[..at].rfind("## ").unwrap();
    let block_end = line_start + diary[line_start..][3..].find("\n## ").map(|n| n + 3).unwrap();
    let block = &diary[line_start..block_end];
    let part = data.path().join("part");
    std::fs::create_dir_all(&part).unwrap();
    std::fs::write(
        part.join("Дневник.md"),
        format!("# Дневник\n\n{}\n", block.replacen("· Вторая", "· Вторая, поправлено", 1)),
    )
    .unwrap();
    rigger(data.path()).args(["import", "alpha", "--hub"]).arg(&part).assert().success();

    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "alpha", "--hub"]).arg(&out).assert().success();
    let back = read(&out.join("Дневник.md"));
    let order: Vec<&str> = back.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(order.len(), 4, "{back}");
    assert!(order[0].ends_with("· Третья"), "{order:?}");
    assert!(order[1].ends_with("· Вторая, поправлено"), "the named entry took the edit: {order:?}");
    assert!(order[2].ends_with("· Первая"), "{order:?}");
    assert!(order[3].ends_with("Основание"), "{order:?}");
    assert!(!back.contains("второе.\n## "), "the separator was taken from the part: {back}");
}
