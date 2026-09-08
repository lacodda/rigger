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
    let db = rusqlite::Connection::open(data.path().join("rigger.db")).unwrap();
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
