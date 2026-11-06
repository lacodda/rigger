//! A plan whose numbers changed.
//!
//! A stage's number is not the stage: a stage is inserted, and every stage
//! behind it moves up one. Reading such a plan used to walk the numbers and
//! the positions, so the tasks of whoever stood on a number took the words of
//! whoever stands there now, closed tasks became closed strangers, and what
//! did not fit stayed behind as duplicates. Each test here is one way that
//! happened on the line, or one way the cure could overreach.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn project(data: &Path) -> PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    hub
}

fn import(data: &Path, hub: &Path) -> Value {
    let out = rigger(data).args(["import", "sample", "--json", "--hub"]).arg(hub).assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

/// Reads a plan as a person wrote it.
fn write_plan(data: &Path, hub: &Path, plan: &str) -> Value {
    std::fs::write(hub.join("План.md"), plan).unwrap();
    import(data, hub)
}

/// Writes the plan out as the record's own, so the file carries the mark of a
/// generated one, and returns its text.
fn export(data: &Path) -> String {
    let out = data.join("out");
    std::fs::create_dir_all(&out).unwrap();
    rigger(data).args(["export", "sample", "--hub"]).arg(&out).assert().success();
    std::fs::read_to_string(out.join("План.md")).unwrap()
}

/// A renumbering by hand of a generated file: the record's own hub, written out
/// and edited in place below its mark. Its other files are still what the record
/// wrote, so they do not speak in the reading and only the plan does.
fn edit_generated(data: &Path, plan: &str) -> Value {
    let generated = export(data);
    let mark = generated.lines().next().unwrap();
    assert!(mark.contains("rigger"), "not a generated plan: {generated}");
    write_plan(data, &data.join("out"), &format!("{mark}\n\n{plan}"))
}

/// A version's tasks as `(id, title, status)`, in plan order.
fn tasks(data: &Path, version: &str) -> Vec<(i64, String, String)> {
    let out = rigger(data).args(["version", "show", "sample", version, "--json"]).assert().success();
    let shown: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    shown["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["id"].as_i64().unwrap(),
                t["title"].as_str().unwrap().to_string(),
                t["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn titles(data: &Path, version: &str) -> Vec<String> {
    tasks(data, version).into_iter().map(|(_, title, _)| title).collect()
}

fn id_of(data: &Path, version: &str, title: &str) -> i64 {
    tasks(data, version)
        .into_iter()
        .find(|(_, t, _)| t == title)
        .unwrap_or_else(|| panic!("{title} is not in {version}"))
        .0
}

fn plan(data: &Path) -> Vec<(String, String)> {
    let out = rigger(data).args(["version", "list", "sample", "--json"]).assert().success();
    let list: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    list["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["version"].as_str().unwrap().to_string(), v["title"].as_str().unwrap_or("").to_string()))
        .collect()
}

const PLAN: &str =
    "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · Second\n\n- [ ] alpha\n- [ ] beta\n- [ ] gamma\n\n## v0.4.0 · Third\n\n- [ ] delta\n";
const SHIFTED: &str = "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · New\n\n- [ ] fresh\n\n## v0.4.0 · Second\n\n- [ ] alpha\n- [ ] beta\n- [ ] gamma\n\n## v0.5.0 · Third\n\n- [ ] delta\n";

/// kilna, 27.09: a generated plan edited by hand, its tail shifted to make room
/// for a stage. Nothing was struck, because a generated plan strikes nothing,
/// and eleven tasks stood in two or three neighbouring stages at once.
#[test]
fn a_stage_inserted_into_a_generated_plan_leaves_no_duplicates() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    let alpha = id_of(data.path(), "v0.3.0", "alpha");
    let delta = id_of(data.path(), "v0.4.0", "delta");

    let report = edit_generated(data.path(), SHIFTED);

    assert_eq!(report["tasks_moved"], 4, "{report}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["fresh"]);
    assert_eq!(titles(data.path(), "v0.4.0"), ["alpha", "beta", "gamma"]);
    assert_eq!(titles(data.path(), "v0.5.0"), ["delta"]);
    assert_eq!(id_of(data.path(), "v0.4.0", "alpha"), alpha, "the task itself moved, not a copy of it");
    assert_eq!(id_of(data.path(), "v0.5.0", "delta"), delta);
}

/// dowel, 24.09: a new stage took over the number of a stage with finished
/// work. Its tasks took the new words and kept `done`, and the real ones turned
/// up a stage later as new cards closed at the moment of the import.
#[test]
fn a_new_stage_on_an_old_number_inherits_nothing_from_it() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.3.0 · Family\n\n- [x] built\n- [x] measured\n- [ ] shipped\n\n## v0.4.0 · Later\n\n- [ ] after\n",
    );
    let built = id_of(data.path(), "v0.3.0", "built");

    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.3.0 · Stranger\n\n- [ ] strange\n- [ ] odd\n\n## v0.4.0 · Family\n\n- [x] built\n- [x] measured\n- [ ] shipped\n\n## v0.5.0 · Later\n\n- [ ] after\n",
    );

    let newcomers = tasks(data.path(), "v0.3.0");
    assert_eq!(newcomers.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(), ["strange", "odd"]);
    assert!(newcomers.iter().all(|t| t.2 == "new"), "a stranger inherited a status: {newcomers:?}");
    assert_eq!(id_of(data.path(), "v0.4.0", "built"), built, "finished work keeps its own card");
    let moved = tasks(data.path(), "v0.4.0");
    assert_eq!(moved.iter().map(|t| t.2.as_str()).collect::<Vec<_>>(), ["done", "done", "new"]);
    assert_eq!(titles(data.path(), "v0.5.0"), ["after"]);
}

/// What the old stage held and no stage lists any more is struck even from a
/// generated plan, which otherwise strikes nothing, and the report says so.
#[test]
fn what_a_stage_left_behind_on_its_number_is_struck() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    let report = edit_generated(
        data.path(),
        "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · New\n\n- [ ] fresh\n\n## v0.4.0 · Second\n\n- [ ] alpha\n\n## v0.5.0 · Third\n\n- [ ] delta\n",
    );

    assert_eq!(report["tasks_dropped"], 2, "{report}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["fresh"]);
    let warnings = report["warnings"].to_string();
    assert!(
        warnings.contains("v0.3.0") && warnings.contains("New") && warnings.contains("Second"),
        "{warnings}"
    );
}

/// A plan the record wrote does not strike a line for not listing it, while the
/// stage is the one it was: a task added after the export is not listed either.
#[test]
fn a_generated_plan_strikes_nothing_from_a_stage_that_kept_its_title() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    let report = edit_generated(data.path(), "# План\n\n## v0.3.0 · Second\n\n- [ ] alpha\n");

    assert_eq!(report["tasks_dropped"], 0, "{report}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["alpha", "beta", "gamma"]);
}

/// A line standing in two stages is two tasks. nitid and midda both repeat a
/// line in neighbouring stages on purpose: a stage adding the same words must
/// not take them from a stage that still lists them.
#[test]
fn the_same_words_in_two_stages_are_two_tasks() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] update\n\n## v0.3.0 · Second\n\n- [ ] other\n",
    );
    let first = id_of(data.path(), "v0.2.0", "update");

    // In either order: the stage naming its own line is the one that keeps it,
    // wherever in the plan the stage asking for the words stands.
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.1.5 · Before\n\n- [ ] update\n\n## v0.2.0 · First\n\n- [ ] update\n\n## v0.3.0 · Second\n\n- [ ] other\n- [ ] update\n",
    );

    assert_eq!(id_of(data.path(), "v0.2.0", "update"), first);
    assert_ne!(id_of(data.path(), "v0.1.5", "update"), first);
    assert_ne!(id_of(data.path(), "v0.3.0", "update"), first);
    assert_eq!(titles(data.path(), "v0.3.0"), ["other", "update"]);
}

/// A line moved by hand to another stage, with no number changing, is the same
/// task, and a closed one stays closed on the way.
#[test]
fn a_line_moved_to_another_stage_keeps_its_history() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [x] closed\n- [ ] stays\n\n## v0.3.0 · Second\n\n- [ ] other\n",
    );
    let closed = id_of(data.path(), "v0.2.0", "closed");

    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] stays\n\n## v0.3.0 · Second\n\n- [ ] other\n- [x] closed\n",
    );

    assert_eq!(titles(data.path(), "v0.2.0"), ["stays"]);
    assert_eq!(id_of(data.path(), "v0.3.0", "closed"), closed);
    assert_eq!(tasks(data.path(), "v0.3.0")[1].2, "done");
}

/// A shipped stage's task is history. The same words in the plan are new work,
/// and must not carry the closed task out of what went out, even when nothing in
/// the reading names it: a changelog still as the record wrote it does not speak.
#[test]
fn a_task_of_a_shipped_stage_stays_where_it_shipped() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(
        hub.join("Изменения.md"),
        "# Изменения\n\n## v0.1.0 · Past — выпущен 2026-01-01\n\n- [x] update\n",
    )
    .unwrap();
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] other\n");
    let past = id_of(data.path(), "v0.1.0", "update");

    let report = edit_generated(data.path(), "# План\n\n## v0.2.0 · First\n\n- [ ] other\n- [ ] update\n");

    assert_eq!(report["tasks_moved"], 0, "{report}");
    assert_eq!(id_of(data.path(), "v0.1.0", "update"), past);
    assert_ne!(id_of(data.path(), "v0.2.0", "update"), past);
}

/// A task struck earlier is not carried to a new stage by its words: its history
/// says it was taken out, and the line listed now is new work.
#[test]
fn a_struck_task_in_another_stage_is_not_carried_over() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] old\n- [ ] stays\n\n## v0.3.0 · Second\n\n- [ ] other\n",
    );
    let old = id_of(data.path(), "v0.2.0", "old");
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] stays\n\n## v0.3.0 · Second\n\n- [ ] other\n",
    );

    let report = write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] stays\n\n## v0.3.0 · Second\n\n- [ ] other\n- [ ] old\n",
    );

    assert_eq!(report["tasks_moved"], 0, "{report}");
    assert_ne!(id_of(data.path(), "v0.3.0", "old"), old);
}

/// A task inserted above another used to take the position of the one below it
/// and reword it, whose line then took the next position, and so down the list.
#[test]
fn a_line_inserted_above_another_does_not_reword_it() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [x] closed\n- [ ] open\n");
    let closed = id_of(data.path(), "v0.2.0", "closed");
    let open = id_of(data.path(), "v0.2.0", "open");

    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] inserted\n- [x] closed\n");

    let now = tasks(data.path(), "v0.2.0");
    assert_eq!(now[0].1, "inserted");
    assert_eq!(now[0].2, "new", "it took over a closed task: {now:?}");
    assert_eq!(now[1].0, closed);
    assert!(now.iter().all(|t| t.0 != open), "what the plan no longer lists is struck: {now:?}");
}

/// A line reworded in place is still that line, while the stage is that stage.
#[test]
fn a_line_reworded_in_a_stage_that_kept_its_title_is_the_same_task() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] first words\n- [ ] other\n");
    let id = id_of(data.path(), "v0.2.0", "first words");

    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] better words\n- [ ] other\n");

    assert_eq!(id_of(data.path(), "v0.2.0", "better words"), id);
    assert_eq!(titles(data.path(), "v0.2.0"), ["better words", "other"]);
}

/// A stage taken out of a generated plan, the ones behind it moving down: what
/// the last number held went to the number before it, and nothing is left under
/// it to show as a stage with no work in it.
#[test]
fn a_number_a_generated_plan_emptied_does_not_stay_as_a_ghost() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    let report = edit_generated(data.path(), "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · Third\n\n- [ ] delta\n");

    assert_eq!(report["versions_dropped"], 1, "{report}");
    let numbers: Vec<String> = plan(data.path()).into_iter().map(|(number, _)| number).collect();
    assert_eq!(numbers, ["v0.2.0", "v0.3.0"]);
    assert_eq!(titles(data.path(), "v0.3.0"), ["delta"]);
}

/// A stage added after the export is one a generated plan does not name, and it
/// keeps its work: only a stage emptied by the plan's own lines goes.
#[test]
fn a_stage_a_generated_plan_never_heard_of_stays() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    let generated = export(data.path());
    rigger(data.path())
        .args(["version", "add", "sample", "v0.9.0", "--title", "Added later", "--task", "kept"])
        .assert()
        .success();

    write_plan(data.path(), &hub, &generated.replace("delta", "delta, reworded"));

    assert_eq!(titles(data.path(), "v0.9.0"), ["kept"]);
    assert!(plan(data.path()).iter().any(|(number, _)| number == "v0.9.0"));
}

/// A plan read under `--check` carries nothing anywhere.
#[test]
fn a_checked_reading_moves_nothing() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    std::fs::write(hub.join("План.md"), SHIFTED).unwrap();

    let out = rigger(data.path())
        .args(["import", "sample", "--check", "--json", "--hub"])
        .arg(&hub)
        .assert()
        .success();
    let report: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();

    assert_eq!(report["tasks_moved"], 4, "{report}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["alpha", "beta", "gamma"]);
}

/// A line added where a struck one once stood is a new task, not that one back.
#[test]
fn a_new_line_does_not_revive_a_struck_one_by_its_position() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] keep\n- [ ] struck\n");
    let struck = id_of(data.path(), "v0.2.0", "struck");
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] keep\n");

    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · First\n\n- [ ] keep\n- [ ] added\n");

    assert_ne!(id_of(data.path(), "v0.2.0", "added"), struck);
}

/// A new stage may be written with no lines yet. It stands on its number, though
/// every task the number held has gone to the stage that lists it now.
#[test]
fn a_stage_with_no_lines_yet_stays_on_the_number_it_took() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    edit_generated(
        data.path(),
        "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · New\n\n## v0.4.0 · Second\n\n- [ ] alpha\n- [ ] beta\n- [ ] gamma\n\n## v0.5.0 · Third\n\n- [ ] delta\n",
    );

    let now = plan(data.path());
    assert!(now.contains(&("v0.3.0".to_string(), "New".to_string())), "{now:?}");
    assert!(titles(data.path(), "v0.3.0").is_empty());
}

/// A stage a generated plan does not name, having given one of its tasks to a
/// stage that lists it, keeps the rest: only an empty one goes.
#[test]
fn a_stage_that_gave_up_one_task_keeps_the_others() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · Second\n\n- [ ] goes\n- [ ] stays\n",
    );

    edit_generated(data.path(), "# План\n\n## v0.2.0 · First\n\n- [ ] one\n- [ ] goes\n");

    assert_eq!(titles(data.path(), "v0.2.0"), ["one", "goes"]);
    assert_eq!(titles(data.path(), "v0.3.0"), ["stays"]);
    assert!(plan(data.path()).iter().any(|(number, _)| number == "v0.3.0"));
}

/// A plan keeps its shipped stages in the major map, often under words of its
/// own. That copy is not another stage on the number: it strikes nothing, though
/// it lists less and nothing else in the reading speaks for the stage.
#[test]
fn a_shipped_stage_copied_into_the_plan_under_another_title_loses_nothing() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(
        hub.join("Изменения.md"),
        "# Изменения\n\n## v0.1.0 · Past — выпущен 2026-01-01\n\n- [x] one\n- [ ] two\n",
    )
    .unwrap();
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · Next\n\n- [ ] next\n");

    edit_generated(data.path(), "# План\n\n## v0.1.0 · In the map\n\n- [x] one\n\n## v0.2.0 · Next\n\n- [ ] next\n");

    assert_eq!(titles(data.path(), "v0.1.0"), ["one", "two"]);
}

/// A changelog says what went out under a number. A line of its own takes
/// nothing from the plan, even a line the plan no longer lists.
#[test]
fn a_changelog_takes_nothing_from_the_plan() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · Next\n\n- [ ] same words\n- [ ] other\n");
    let planned = id_of(data.path(), "v0.2.0", "same words");

    std::fs::write(
        hub.join("Изменения.md"),
        "# Изменения\n\n## v0.1.0 · Past — выпущен 2026-01-01\n\n- [x] same words\n",
    )
    .unwrap();
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · Next\n\n- [ ] other\n");

    assert_ne!(id_of(data.path(), "v0.1.0", "same words"), planned);
}

/// Nor does a stage that shipped take a task from the plan, read from the plan's
/// own copy of it.
#[test]
fn the_plan_copy_of_a_shipped_stage_takes_nothing_from_the_plan() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("Изменения.md"), "# Изменения\n\n## v0.1.0 · Past — выпущен 2026-01-01\n\n- [x] old\n").unwrap();
    write_plan(data.path(), &hub, "# План\n\n## v0.2.0 · Next\n\n- [ ] same words\n- [ ] other\n");
    let planned = id_of(data.path(), "v0.2.0", "same words");

    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.1.0 · Past\n\n- [x] old\n- [x] same words\n\n## v0.2.0 · Next\n\n- [ ] other\n",
    );

    assert_ne!(id_of(data.path(), "v0.1.0", "same words"), planned);
}

// ---- `version move` --------------------------------------------------------

fn moved(data: &Path, args: &[&str]) -> Value {
    let out = rigger(data).args(["version", "move", "sample"]).args(args).arg("--json").assert().success();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn refused(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(["version", "move", "sample"]).args(args).assert().failure();
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

/// A stage moved to a free number is the same row: its tasks need no matching,
/// and what the record exports reads under the new number.
#[test]
fn a_stage_moves_to_a_free_number_with_all_it_holds() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    let alpha = id_of(data.path(), "v0.3.0", "alpha");

    let answer = moved(data.path(), &["v0.3.0", "v0.7.0"]);

    assert_eq!(answer["moved"][0]["from"], "v0.3.0");
    assert_eq!(answer["moved"][0]["to"], "v0.7.0");
    assert_eq!(id_of(data.path(), "v0.7.0", "alpha"), alpha);
    let written = export(data.path());
    assert!(written.contains("## v0.7.0 · Second"), "{written}");
    assert!(!written.contains("v0.3.0"), "the old number stayed in the heading: {written}");
    // In the file's order as well: the stage went behind the one it now follows.
    assert!(written.find("v0.4.0").unwrap() < written.find("v0.7.0").unwrap(), "{written}");
}

/// A number somebody holds is refused, and refused without touching anything.
#[test]
fn a_number_in_use_is_refused_without_shift() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    let said = refused(data.path(), &["v0.2.0", "v0.3.0"]);

    assert!(said.contains("v0.3.0") && said.contains("Second") && said.contains("--shift"), "{said}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["alpha", "beta", "gamma"]);
    assert_eq!(titles(data.path(), "v0.2.0"), ["one"]);
}

/// `--shift` opens a place: what stands on the number moves up one, and the one
/// behind it if it is in the way, as far as the first free number and no further.
#[test]
fn shift_moves_up_only_what_stands_in_the_way() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · A\n\n- [ ] a\n\n## v0.3.0 · B\n\n- [ ] b\n\n## v0.4.0 · C\n\n- [ ] c\n\n## v0.6.0 · D\n\n- [ ] d\n\n## v0.7.0 · E\n\n- [ ] e\n",
    );
    let e = id_of(data.path(), "v0.7.0", "e");

    let answer = moved(data.path(), &["v0.7.0", "v0.3.0", "--shift"]);

    assert_eq!(answer["moved"].as_array().unwrap().len(), 3, "{answer}");
    let now = plan(data.path());
    let expected = [("v0.2.0", "A"), ("v0.3.0", "E"), ("v0.4.0", "B"), ("v0.5.0", "C"), ("v0.6.0", "D")];
    let expected: Vec<(String, String)> = expected.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    assert_eq!(now, expected);
    assert_eq!(id_of(data.path(), "v0.3.0", "e"), e);
    let written = export(data.path());
    let order: Vec<usize> = ["v0.2.0 · A", "v0.3.0 · E", "v0.4.0 · B", "v0.5.0 · C", "v0.6.0 · D"]
        .iter()
        .map(|heading| written.find(heading).unwrap_or_else(|| panic!("{heading} is not in {written}")))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "out of order in the file: {written}");
}

/// The number the moving stage gives up is a free number like any other: moving
/// a stage down the plan shifts up to it and stops.
#[test]
fn shift_stops_at_the_number_the_stage_gave_up() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · A\n\n- [ ] a\n\n## v0.3.0 · B\n\n- [ ] b\n\n## v0.4.0 · C\n\n- [ ] c\n\n## v0.5.0 · D\n\n- [ ] d\n",
    );

    moved(data.path(), &["v0.4.0", "v0.2.0", "--shift"]);

    let expected = [("v0.2.0", "C"), ("v0.3.0", "A"), ("v0.4.0", "B"), ("v0.5.0", "D")];
    let expected: Vec<(String, String)> = expected.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    assert_eq!(plan(data.path()), expected);
}

/// A plan renumbered in the record is read back as nothing to do: the trip
/// through a hub changes nothing, by digest or, with the mark taken off, by text.
#[test]
fn a_plan_renumbered_in_the_record_reads_back_as_itself() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    let before: Vec<i64> = tasks(data.path(), "v0.3.0").into_iter().map(|t| t.0).collect();
    moved(data.path(), &["v0.4.0", "v0.3.0", "--shift"]);

    let written = export(data.path());
    let without_mark = written.split_once('\n').unwrap().1.trim_start_matches('\n');
    let report = write_plan(data.path(), &hub, without_mark);

    for count in ["tasks_added", "tasks_moved", "tasks_dropped", "versions_added", "versions_dropped"] {
        assert_eq!(report[count], 0, "{count}: {report}");
    }
    let after: Vec<i64> = tasks(data.path(), "v0.4.0").into_iter().map(|t| t.0).collect();
    assert_eq!(before, after);
}

/// Prose between the blocks of a plan stays where it stands, and a stage crossing
/// it changes block with its number.
#[test]
fn a_stage_crosses_the_blocks_of_a_plan_with_its_number() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## Near\n\n### v0.2.0 · A\n\n- [ ] a\n\n### v0.3.0 · B\n\n- [ ] b\n\n## Far\n\n### v0.4.0 · C\n\n- [ ] c\n",
    );

    moved(data.path(), &["v0.4.0", "v0.3.0", "--shift"]);

    let written = export(data.path());
    let near = written.find("## Near").unwrap();
    let far = written.find("## Far").unwrap();
    let c = written.find("### v0.3.0 · C").expect(&written);
    let b = written.find("### v0.4.0 · B").expect(&written);
    assert!(near < c && c < far && far < b, "{written}");
}

/// A link anchored on a stage is anchored on the stage, not on its number.
#[test]
fn a_link_anchored_on_a_stage_follows_it() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    let other = data.path().join("other");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&other).assert().success();
    rigger(data.path())
        .args(["version", "add", "other", "v0.1.0", "--title", "Theirs"])
        .assert()
        .success();
    rigger(data.path()).args(["link", "add", "sample@v0.3.0", "other@v0.1.0"]).assert().success();

    moved(data.path(), &["v0.3.0", "v0.7.0"]);

    let out = rigger(data.path()).args(["link", "list", "sample", "--json"]).assert().success();
    let links = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(links.contains("v0.7.0") && !links.contains("v0.3.0"), "{links}");
}

/// A number that went out belongs to its tag: a shipped stage does not move, and
/// no stage still to come goes at or behind the last release.
#[test]
fn what_shipped_holds_its_number() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    std::fs::write(hub.join("Изменения.md"), "# Изменения\n\n## v0.2.0 · Past — выпущен 2026-01-01\n\n- [x] done\n").unwrap();
    write_plan(data.path(), &hub, "# План\n\n## v0.3.0 · Next\n\n- [ ] next\n");

    assert!(refused(data.path(), &["v0.2.0", "v0.9.0"]).contains("has shipped"));
    assert!(refused(data.path(), &["v0.3.0", "v0.2.0"]).contains("last release"));
    assert!(refused(data.path(), &["v0.3.0", "v0.1.0"]).contains("last release"));
    assert!(refused(data.path(), &["v0.8.0", "v0.9.0"]).contains("not a stage"));
    assert_eq!(titles(data.path(), "v0.3.0"), ["next"]);
}

/// A major has no next number to shift to: it is a place on a map.
#[test]
fn shift_does_not_push_a_major_out_of_the_way() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.3.0 · Next\n\n- [ ] next\n\n## v1.0.0 · Major\n\n- [ ] criteria\n",
    );

    let said = refused(data.path(), &["v0.3.0", "v1.0.0", "--shift"]);

    assert!(said.contains("opens a major"), "{said}");
    assert_eq!(titles(data.path(), "v1.0.0"), ["criteria"]);
}

/// A number the plan once struck is free. The struck row keeps its history out
/// of the way, and the number reads as the stage that holds it now.
#[test]
fn a_struck_number_is_free_to_move_to() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · Second\n\n- [ ] alpha\n",
    );

    moved(data.path(), &["v0.3.0", "v0.4.0"]);

    assert_eq!(titles(data.path(), "v0.4.0"), ["alpha"]);
    assert_eq!(
        titles(data.path(), "v0.4"),
        ["alpha"],
        "found by value it is the live stage, not the struck one"
    );
    let numbers: Vec<String> = plan(data.path()).into_iter().map(|(number, _)| number).collect();
    assert_eq!(numbers, ["v0.2.0", "v0.4.0"]);
}

/// `version add` on a number the plan struck plans a stage. It used to find the
/// struck row, say the stage was there already, and put the tasks under a stage
/// nothing shows.
#[test]
fn a_struck_number_can_be_planned_again() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v0.2.0 · First\n\n- [ ] one\n\n## v0.3.0 · Second\n\n- [ ] alpha\n",
    );

    rigger(data.path())
        .args(["version", "add", "sample", "v0.4.0", "--title", "Again", "--task", "fresh"])
        .assert()
        .success();

    assert!(plan(data.path()).contains(&("v0.4.0".to_string(), "Again".to_string())));
    assert_eq!(titles(data.path(), "v0.4.0"), ["fresh"]);
}

/// Other spellings of a number keep their shape when shifted.
#[test]
fn a_shift_keeps_how_a_plan_spells_its_numbers() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(
        data.path(),
        &hub,
        "# План\n\n## v1.9 · A\n\n- [ ] a\n\n## v1.10 · B\n\n- [ ] b\n\n## v1.10.1 · P\n\n- [ ] p\n\n## v1.10.2 · Q\n\n- [ ] q\n",
    );

    moved(data.path(), &["v1.9", "v1.10", "--shift"]);
    let numbers: Vec<String> = plan(data.path()).into_iter().map(|(number, _)| number).collect();
    assert_eq!(numbers, ["v1.10", "v1.10.1", "v1.10.2", "v1.11"]);

    moved(data.path(), &["v1.11", "v1.10.1", "--shift"]);
    let spelled: Vec<(String, String)> = plan(data.path());
    let expected = [("v1.10", "A"), ("v1.10.1", "B"), ("v1.10.2", "P"), ("v1.10.3", "Q")];
    let expected: Vec<(String, String)> = expected.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    assert_eq!(spelled, expected);
}

/// Moving a stage to the number it has does nothing, and says so.
#[test]
fn a_stage_moved_to_its_own_number_stays() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    let answer = moved(data.path(), &["v0.3", "v0.3.0"]);

    assert_eq!(answer["moved"].as_array().unwrap().len(), 0, "{answer}");
    assert_eq!(titles(data.path(), "v0.3.0"), ["alpha", "beta", "gamma"]);
}

/// What is not a version is refused the way `version add` refuses it.
#[test]
fn a_number_has_to_be_written_as_a_tag_would_be() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    write_plan(data.path(), &hub, PLAN);

    assert!(refused(data.path(), &["v0.3.0", "0.7"]).contains("not a version"));
}
