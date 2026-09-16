//! Reading a hub into a record that already holds it.
//!
//! The first import of a hub is the easy case, and the one every other
//! test covers: an empty record takes whatever the hub says. A line lives
//! in the other case. Its hubs were read once, months of work were
//! written into them by hand since, and rigger itself moved on - so the
//! record holds stages it read before it kept their shape, and stages the
//! owner has since moved. Both were found by adopting the line and
//! exporting a hub back: every stage of the plan came out at the wrong
//! depth, in the wrong place.

use std::path::Path;

use assert_cmd::Command;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

const PLAN: &str = "\
# План

## Блок «Первый»

### v0.2.0 · Второй

- [ ] сделать одно

### v0.3.0 · Третий

- [ ] сделать другое
";

fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    hub
}

/// The generated file without its mark.
fn body(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    text.split_once('\n').map(|(_, rest)| rest.trim_start_matches('\n').to_string()).unwrap_or(text)
}

fn export(data: &Path) -> String {
    let out = data.join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    body(&out.join("План.md"))
}

/// A stage recorded before the record kept shapes has none. The plan is
/// the only file that holds it, so the plan's reading gives it one - and
/// a re-import puts every such stage back where the hub wrote it.
#[test]
fn a_stage_recorded_without_a_shape_takes_the_plans() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("План.md"), PLAN).unwrap();

    // The rows an older rigger left: a version and nothing about its heading.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    for name in ["v0.2.0", "v0.3.0"] {
        db.execute("INSERT INTO versions (project_id, name, status) VALUES (1, ?1, 'planned')", [name])
            .unwrap();
    }
    drop(db);

    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    assert_eq!(export(data.path()), PLAN);
}

/// A stage the owner moved by hand moves in the record too. The plan used
/// to defer to whatever shape was recorded - meant for a stage the
/// changelog also holds - and so a plan reordered after its first import
/// exported in the old order for ever.
#[test]
fn a_plan_reordered_by_hand_reimports_in_the_new_order() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("План.md"), PLAN).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    assert_eq!(export(data.path()), PLAN);

    let moved = "\
# План

## Блок «Первый»

### v0.3.0 · Третий

- [ ] сделать другое

## Блок «Второй»

### v0.2.0 · Второй

- [ ] сделать одно
";
    std::fs::write(hub.join("План.md"), moved).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    assert_eq!(export(data.path()), moved);
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

/// A version a tag closed after the hub was last read has no place in the
/// changelog yet. It is written where the next entry goes - at the top -
/// not below the oldest entry, under the prose that ends the file.
#[test]
fn a_version_shipped_since_the_last_import_leads_the_changelog() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let root = data.path().join("sample");
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("README.md"), "x").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feat: start"]);

    std::fs::write(hub.join("План.md"), "# План\n\n## v0.2.0 · Второй\n\n- [x] сделать одно\n").unwrap();
    let changes = "\
# Изменения

Закрытые этапы, новые сверху.

## v0.1.0 · Первый — выпущена 2026-09-01

Что было сделано.

## Основание — закрыто 2026-08-30

Проза в конце файла.
";
    std::fs::write(hub.join("Изменения.md"), changes).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();

    git(&root, &["tag", "v0.2.0"]);
    rigger(data.path()).args(["sync", "sample"]).assert().success();

    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    let written = body(&out.join("Изменения.md"));
    let second = written.find("## v0.2.0").expect("the shipped version is in the changelog");
    let first = written.find("## v0.1.0").unwrap();
    let prose = written.find("## Основание").unwrap();
    assert!(second < first && first < prose, "{written}");
    assert!(written.starts_with("# Изменения\n\nЗакрытые этапы, новые сверху.\n\n## v0.2.0"), "{written}");
}

/// A plan that has not been told the stage shipped is stale, not a
/// decision. Its empty boxes do not reopen the tasks the record closed,
/// and its depth and place do not follow the stage into the changelog.
#[test]
fn a_stale_plan_neither_reopens_a_closed_task_nor_shapes_a_shipped_stage() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let root = data.path().join("sample");
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("README.md"), "x").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feat: start"]);

    let plan = "\
# План

## Блок «Первый»

### v0.2.0 · Второй

- [ ] сделать одно
- [ ] сделать другое

**Результат:** что-то работает.
";
    std::fs::write(hub.join("План.md"), plan).unwrap();
    std::fs::write(hub.join("Изменения.md"), "# Изменения\n\n## v0.1.0 · Первый — выпущена 2026-09-01\n\nБыло.\n").unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();

    // The record closes one task - the way `close_task` does - and a tag closes the stage.
    let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
    db.execute(
        "UPDATE tasks SET status = 'done', closed_at = '2026-09-02T10:00:00Z' WHERE title = 'сделать одно'",
        [],
    )
    .unwrap();
    drop(db);
    git(&root, &["tag", "v0.2.0"]);
    rigger(data.path()).args(["sync", "sample"]).assert().success();

    // The plan is read again, unchanged and behind the times.
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();

    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    let changes = body(&out.join("Изменения.md"));
    let plan_out = body(&out.join("План.md"));
    assert!(
        changes.contains("## v0.2.0 · Второй — выпущен"),
        "the stage is a changelog entry, at the changelog's depth: {changes}"
    );
    assert!(
        changes.contains("- [x] сделать одно\n- [ ] сделать другое"),
        "the closed task stays closed: {changes}"
    );
    assert!(changes.contains("**Результат:** что-то работает."), "its prose travels with it: {changes}");
    assert!(
        changes.find("## v0.2.0").unwrap() < changes.find("## v0.1.0").unwrap(),
        "newest first: {changes}"
    );
    assert!(!plan_out.contains("v0.2.0"), "a shipped stage has left the plan: {plan_out}");
}

/// A version in both files keeps the changelog's shape: the plan's copy
/// of a shipped stage - in a major map, at another depth - must not move
/// it out of the changelog.
#[test]
fn a_stage_in_both_files_keeps_the_changelogs_shape() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let plan = "\
# План

## Карта

### v0.1.0 · Первый

Уже вышел.

### v0.2.0 · Второй

- [ ] сделать одно
";
    let changes = "\
# Изменения

## v0.1.0 · Первый — выпущена 2026-09-01

Что было сделано.
";
    std::fs::write(hub.join("План.md"), plan).unwrap();
    std::fs::write(hub.join("Изменения.md"), changes).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    // And again, the way a line's hubs are read: with the rows already there.
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();

    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    assert_eq!(body(&out.join("Изменения.md")), changes);
}

/// A hub rigger wrote is a description of the record, not an instruction to
/// it - and that holds for versions as much as for tasks.
///
/// A generated plan lists the open stages and nothing else, so a shipped
/// stage appears in neither file once the changelog has scrolled. Reading
/// such a hub back used to mean "neither file names it, so somebody struck
/// it": importing the line's sixteen generated hubs once struck 367
/// versions and 976 tasks. The guard existed for tasks and not for
/// versions, one line below.
#[test]
fn a_generated_hub_read_back_strikes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());

    let plan = "\
# План

## v0.1.0 · Первый

- [ ] сделать одно

## v0.2.0 · Второй

- [ ] сделать другое
";
    std::fs::write(hub.join("План.md"), plan).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();

    let count = |what: &str| -> i64 {
        let db = rusqlite::Connection::open(data.path().join("profiles").join("line").join("rigger.db")).unwrap();
        let sql = format!("SELECT COUNT(*) FROM {what} WHERE status = 'dropped'");
        db.query_row(&sql, [], |r| r.get(0)).unwrap()
    };
    assert_eq!(count("versions"), 0, "nothing is struck yet");

    // The hub is now written by rigger, the way every hub of the line is.
    let out = data.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data.path()).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    // The raw file, not `body`: `body` drops the first line, which is the
    // mark itself.
    assert!(
        std::fs::read_to_string(out.join("План.md")).unwrap().contains("generated by rigger"),
        "the export marks what it wrote"
    );

    // A generated plan that names only some of the stages - which is what a
    // real one does once a stage has shipped.
    let partial = "\
<!-- generated by rigger; edits here are overwritten -->

# План

## v0.2.0 · Второй

- [ ] сделать другое
";
    std::fs::write(out.join("План.md"), partial).unwrap();
    std::fs::write(out.join("Изменения.md"), "# Изменения\n").unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&out).assert().success();

    assert_eq!(count("versions"), 0, "a generated plan describes the record; it does not strike from it");
    assert_eq!(count("tasks"), 0, "and neither does it strike the tasks of a stage it no longer lists");
}
