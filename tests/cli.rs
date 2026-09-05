//! The binary surface as a user meets it, against a throwaway data directory.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn repo(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"declared-elsewhere\"\nversion = \"0.1.0\"\n").unwrap();
}

#[test]
fn version_prints_the_crate_version() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_describes_the_product() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("One seat for all your projects and tasks"));
}

#[test]
fn init_creates_the_database_and_is_idempotent() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created").and(predicate::str::contains("schema version")));
    assert!(data.path().join("rigger.db").exists());

    rigger(data.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Already initialised"));
}

#[test]
fn commands_before_init_point_at_init() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .args(["project", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run `rigger init` first"));
}

#[test]
fn project_add_names_the_project_after_its_directory() {
    // The manifest declares another name on purpose: the crate or package
    // name is the publishing name, not the one the owner calls the project.
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("repos").join("folder-name");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 'folder-name'").and(predicate::str::contains("remote: none")));

    rigger(data.path())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("folder-name"));
}

#[test]
fn project_add_reads_the_origin_remote() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("remoted");
    repo(&root);
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(
        root.join(".git").join("config"),
        "[remote \"origin\"]\n\turl = https://example.com/remoted.git\n",
    )
    .unwrap();
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .success()
        .stdout(predicate::str::contains("remote: https://example.com/remoted.git"));

    rigger(data.path())
        .args(["project", "show", "remoted", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"remote\": \"https://example.com/remoted.git\""));
}

#[test]
fn name_can_be_overridden_and_must_be_unique() {
    let data = tempfile::tempdir().unwrap();
    let first = data.path().join("a").join("same");
    let second = data.path().join("b").join("same");
    repo(&first);
    repo(&second);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path()).args(["project", "add"]).arg(&first).assert().success();
    rigger(data.path())
        .args(["project", "add"])
        .arg(&second)
        .assert()
        .failure()
        .stderr(predicate::str::contains("a project named 'same' already exists"));
    rigger(data.path())
        .args(["project", "add", "--name", "other"])
        .arg(&second)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 'other'"));
}

#[test]
fn the_same_path_is_not_recorded_twice() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("once");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();

    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    rigger(data.path())
        .args(["project", "add"])
        .arg(&root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already recorded as project 'once'"));
}

#[test]
fn show_of_an_unknown_project_fails_with_a_pointer() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .args(["project", "show", "ghost"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

/// A hub in the shape the real ones have: a plan with open stages and
/// questions, a changelog of shipped ones, a dated decision log.
fn hub(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("План.md"),
        "# План разработки\n\n## Ждёт решения владельца\n\n1. Pick a colour.\n2. Sign the binary.\n\n## v0.3.0 · Third stage\n\n- [ ] first task\n- [ ] second task\n\n## Бэклог без версии\n\n- [ ] someday, not a task of a stage\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Изменения.md"),
        "# Изменения\n\n## v0.2.0 · Second stage — выпущена 2026-09-03\n\n- Something shipped.\n\n## v0.1.0 · First stage — выпущен 2026-09-01\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Решения.md"),
        "# Журнал решений\n\n---\n\n## 2026-09-03 · The record is the database\n\nBecause prose cannot be filtered.\n",
    )
    .unwrap();
}

/// A database with one project named `proj`, whose repository carries a hub.
fn imported_project(data: &Path) -> std::path::PathBuf {
    let root = data.join("proj");
    repo(&root);
    hub(&root.join("hub"));
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    root
}

#[test]
fn import_reads_stages_tasks_decisions_and_questions() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(root.join("hub"))
        .assert()
        .success()
        .stdout(
            predicate::str::contains("versions   3 added")
                .and(predicate::str::contains("tasks      2 added"))
                .and(predicate::str::contains("decisions  1 added"))
                .and(predicate::str::contains("questions  2 added")),
        );

    rigger(data.path()).arg("doctor").assert().success().stdout(
        predicate::str::contains("versions:  3")
            .and(predicate::str::contains("tasks:     2"))
            .and(predicate::str::contains("events:    3")),
    );
}

#[test]
fn importing_an_unchanged_hub_again_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");

    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();
    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));

    // And the record did not grow behind the report.
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("versions:  3").and(predicate::str::contains("events:    3")));
}

#[test]
fn a_stage_that_shipped_is_updated_rather_than_duplicated() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();

    // The stage moves from the plan to the changelog, as it does when a
    // version ships: same version, now with a date.
    std::fs::write(hub_dir.join("План.md"), "# План\n\n## Ждёт решения владельца\n\n- (пусто)\n").unwrap();
    std::fs::write(
        hub_dir.join("Изменения.md"),
        "# Изменения\n\n## v0.3.0 · Third stage — выпущена 2026-09-05\n\n## v0.2.0 · Second stage — выпущена 2026-09-03\n\n## v0.1.0 · First stage — выпущен 2026-09-01\n",
    )
    .unwrap();

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(&hub_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("versions   0 added, 1 updated"));

    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("versions:  3"));
}

/// A stage whose heading numbers it one way and its release another - kasl
/// writes `v1.9 · Title — closed, released **v1.9.0**` - is recorded under
/// the number git will carry. And a database that already held it under the
/// old number is corrected rather than left with two rows for one stage.
#[test]
fn a_stage_renumbered_by_its_release_replaces_the_old_row() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");

    std::fs::write(hub_dir.join("Изменения.md"), "# Изменения\n\n## v1.9 · Очередь — закрыт 2026-09-03\n").unwrap();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();

    // The heading now names the release the stage shipped as.
    std::fs::write(
        hub_dir.join("Изменения.md"),
        "# Изменения\n\n## v1.9 · Очередь — закрыт 2026-09-03, выпущен **v1.9.0**\n",
    )
    .unwrap();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();

    rigger(data.path())
        .args(["context", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Last shipped: v1.9.0"));

    // One stage, one row. Two versions in total: the plan's open stage and
    // this one - a twin left behind would make it three.
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("versions:  2"));
}

/// The packet names the newest release, and versions are numbered, not
/// lettered: v0.10.0 comes after v0.9.0. Hubs list their changelog
/// newest-first, so ordering by row put the oldest entry on top.
#[test]
fn the_newest_release_is_the_highest_number_not_the_last_row() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let hub_dir = root.join("hub");

    // Written the way a hub writes it: newest at the top, one date for all.
    std::fs::write(
        hub_dir.join("Изменения.md"),
        "# Изменения\n\n## v0.10.0 · Tenth — выпущена 2026-09-02\n\n## v0.9.0 · Ninth — выпущена 2026-09-02\n\n## v0.1.0 · First — выпущена 2026-09-02\n",
    )
    .unwrap();
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(&hub_dir).assert().success();

    rigger(data.path())
        .args(["context", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Last shipped: v0.10.0"));
}

#[test]
fn import_of_a_missing_hub_reports_the_files_it_wanted() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());

    rigger(data.path())
        .args(["import", "proj", "--hub"])
        .arg(root.join("no-such-hub"))
        .assert()
        .success()
        .stdout(predicate::str::contains("План.md is missing").and(predicate::str::contains("nothing changed")));
}

#[test]
fn import_of_an_unknown_project_fails() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path())
        .args(["import", "ghost", "--hub"])
        .arg(root.join("hub"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project named 'ghost'"));
}

#[test]
fn context_shows_where_the_project_stands_and_what_is_next() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    rigger(data.path()).args(["context", "proj"]).assert().success().stdout(
        // The state line, the stage in progress with its open tasks, the
        // owner's queue and the decision - in that order.
        predicate::str::contains("Last shipped: v0.2.0 on 2026-09-03")
            .and(predicate::str::contains("## Current stage: v0.3.0 · Third stage"))
            .and(predicate::str::contains("- first task"))
            .and(predicate::str::contains("## Waiting for the owner"))
            .and(predicate::str::contains("Pick a colour."))
            .and(predicate::str::contains("The record is the database")),
    );
}

#[test]
fn a_recorded_note_reaches_the_packet_and_a_next_step_stands_alone() {
    let data = tempfile::tempdir().unwrap();
    imported_project(data.path());

    rigger(data.path())
        .args(["note", "proj", "The parser must take hubs as they are.", "--kind", "finding"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded a finding for proj"));
    rigger(data.path())
        .args(["note", "proj", "Ship the importer next.", "--kind", "next"])
        .assert()
        .success();
    rigger(data.path()).args(["wish", "proj", "Show days without a commit."]).assert().success();

    rigger(data.path()).args(["context", "proj"]).assert().success().stdout(
        predicate::str::contains("finding · The parser must take hubs as they are.")
            .and(predicate::str::contains("## Wishes, not yet sorted"))
            .and(predicate::str::contains("Show days without a commit."))
            .and(predicate::str::contains("## Next step\nShip the importer next."))
            // A next step is not repeated among the recent events.
            .and(predicate::str::contains("next · Ship").not()),
    );
}

/// The budget is a gate, not a hope: a project with a long history must still
/// produce a packet that fits, and say what it left out.
/// The token cost of a packet, from its own `--explain` line.
fn packet_cost(data: &Path, args: &[&str]) -> (usize, String) {
    let mut cmd = rigger(data);
    cmd.args(["context", "proj", "--explain"]).args(args);
    let text = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
    let total = text
        .lines()
        .find(|l| l.starts_with("total"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no total line in:\n{text}"));
    (total, text)
}

#[test]
fn the_packet_holds_the_budget_and_reports_what_it_dropped() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    // Enough decisions, each as long as the real hubs carry, that no budget
    // could hold them all - the packet must choose and say so.
    let body = "**A decision that took a paragraph.**\n\n".to_string() + &"Reasoning that runs on and on. ".repeat(40);
    for i in 0..60 {
        rigger(data.path())
            .args(["note", "proj", &format!("{body} number {i} of sixty"), "--kind", "decision"])
            .assert()
            .success();
    }

    let (total, text) = packet_cost(data.path(), &[]);
    assert!(total <= 3000, "packet is {total} tokens, over the budget:\n{text}");

    // Under a budget that cannot hold them, the packet says how many it left
    // out - the silent truncation this command exists to avoid.
    let (total, text) = packet_cost(data.path(), &["--budget", "600"]);
    assert!(total <= 600, "packet is {total} tokens, over the 600 asked for:\n{text}");
    assert!(
        text.contains("older events left out by the budget"),
        "the packet did not say what it dropped:\n{text}"
    );
}

/// A budget too small even for the fixed sections still produces a usable
/// packet: state, the current stage and the owner's queue are never dropped.
#[test]
fn the_essentials_survive_an_impossible_budget() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    let (_, text) = packet_cost(data.path(), &["--budget", "1"]);
    assert!(text.contains("## Current stage: v0.3.0"), "{text}");
    assert!(text.contains("## Waiting for the owner"), "{text}");
    assert!(text.contains("left out by the budget"), "{text}");
}

#[test]
fn context_as_json_carries_the_same_facts() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    rigger(data.path()).args(["context", "proj", "--json"]).assert().success().stdout(
        predicate::str::contains("\"project\": \"proj\"")
            .and(predicate::str::contains("\"version\": \"v0.3.0\""))
            .and(predicate::str::contains("\"events_omitted\": 0")),
    );
}

#[test]
fn open_print_shows_the_message_the_assistant_would_receive() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();

    rigger(data.path()).args(["open", "proj", "--print"]).assert().success().stdout(
        // The instruction first, so the packet is read as state rather
        // than as something to summarise; then the packet itself.
        predicate::str::contains("This is where the project stands")
            .and(predicate::str::contains("rigger note"))
            .and(predicate::str::contains("## Current stage: v0.3.0 · Third stage")),
    );
}

/// `open` runs whatever assistant the owner uses. Here that is a script that
/// records its arguments, which proves three things at once: the packet
/// arrives as one argument, the working directory is the project, and the
/// assistant's exit status becomes ours.
#[test]
fn open_runs_the_assistant_in_the_project_with_the_packet() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    let log = data.path().join("assistant.log");

    let (stub, args) = stub_assistant(data.path(), &log, 0);
    let mut cmd = rigger(data.path());
    cmd.env(open_env(), format!("{stub} {args}"));
    cmd.args(["open", "proj"]).assert().success();

    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(
        recorded.contains("This is where the project stands"),
        "the packet did not reach the assistant: {recorded}"
    );
    // The tail of the path, not the whole of it: macOS reports a temporary
    // directory through its /private symlink, and Windows shells report the
    // drive letter in their own case.
    let cwd = recorded
        .lines()
        // PowerShell writes a byte-order mark at the head of the file, so the
        // first line does not start with the text it appears to start with.
        .find_map(|l| l.trim_start_matches('\u{feff}').strip_prefix("cwd="))
        .unwrap_or_else(|| panic!("the assistant did not report where it ran: {recorded}"));
    assert!(
        cwd.to_lowercase().ends_with("proj"),
        "the assistant did not run in the project: ran in {cwd}, expected {}",
        root.display()
    );
}

/// Every Windows install of an npm CLI is a `.cmd` shim, and the process API
/// refuses to hand one an argument. `open` runs it the way a shell does, or
/// it cannot start the assistant the owner actually has.
#[cfg(windows)]
#[test]
fn open_can_launch_a_batch_file_assistant() {
    let data = tempfile::tempdir().unwrap();
    imported_project(data.path());
    let log = data.path().join("batch.log");
    let script = data.path().join("assistant.cmd");
    // The packet arrives on stdin, not as an argument: a batch file cannot
    // receive a multi-line argument at all. `findstr` copies stdin through.
    std::fs::write(
        &script,
        format!(
            "@echo off\r\necho cwd=%CD%>\"{}\"\r\nfindstr /n \"^\">>\"{}\"\r\n",
            log.display(),
            log.display()
        ),
    )
    .unwrap();

    let mut cmd = rigger(data.path());
    cmd.env(open_env(), script.display().to_string());
    cmd.args(["open", "proj"]).assert().success();

    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(
        recorded.contains("This is where the project stands"),
        "a .cmd assistant did not receive the packet: {recorded}"
    );
}

/// The packet is many lines and quotes plans that contain `<`, `>`, `&` and
/// `|`. As an argument to a batch file it cannot survive: Rust refuses a
/// multi-line one outright, and through `cmd /c` the newline ends the command
/// line and the metacharacters redirect it. On stdin it arrives whole.
#[cfg(windows)]
#[test]
fn a_batch_assistant_receives_every_line_of_the_packet() {
    let data = tempfile::tempdir().unwrap();
    let root = imported_project(data.path());
    rigger(data.path()).args(["import", "proj", "--hub"]).arg(root.join("hub")).assert().success();
    rigger(data.path())
        .args(["note", "proj", "Runs `open <project>` & pipes a|b, then ^ escapes.", "--kind", "finding"])
        .assert()
        .success();

    let log = data.path().join("meta.log");
    let script = data.path().join("assistant.cmd");
    // `findstr /n` numbers the lines, which keeps the blank ones a plain
    // copy would drop - and makes it visible that the whole packet arrived,
    // not just its first paragraph.
    std::fs::write(&script, format!("@echo off\r\nfindstr /n \"^\">\"{}\"\r\n", log.display())).unwrap();

    let mut cmd = rigger(data.path());
    cmd.env(open_env(), script.display().to_string());
    cmd.args(["open", "proj"]).assert().success();

    let recorded = std::fs::read_to_string(&log).unwrap();
    assert!(
        recorded.contains("`open <project>` & pipes a|b, then ^ escapes."),
        "the packet did not arrive whole: {recorded}"
    );
    // The last section proves the whole packet came through, not its head.
    assert!(recorded.contains("## Current stage"), "the packet was cut short: {recorded}");
    // Nothing was redirected into a stray file named after the next word.
    let strays: Vec<_> = std::fs::read_dir(data.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("project") || n.contains("pipes"))
        .collect();
    assert!(strays.is_empty(), "cmd redirected into {strays:?}");
}

#[test]
fn open_passes_on_the_assistants_exit_status() {
    let data = tempfile::tempdir().unwrap();
    imported_project(data.path());
    let log = data.path().join("assistant.log");

    let (stub, args) = stub_assistant(data.path(), &log, 3);
    let mut cmd = rigger(data.path());
    cmd.env(open_env(), format!("{stub} {args}"));
    cmd.args(["open", "proj"]).assert().code(3);
}

#[test]
fn open_reports_a_missing_assistant_by_name() {
    let data = tempfile::tempdir().unwrap();
    imported_project(data.path());

    let mut cmd = rigger(data.path());
    cmd.env(open_env(), "no-such-assistant-binary");
    cmd.args(["open", "proj"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot run `no-such-assistant-binary`"));
}

fn open_env() -> &'static str {
    "RIGGER_ASSISTANT"
}

/// Writes a script that appends its working directory and arguments to `log`
/// and exits with `code`. Returns the interpreter and its leading arguments,
/// so the caller can build the command line rigger will run.
fn stub_assistant(dir: &Path, log: &Path, code: i32) -> (String, String) {
    let log = log.display().to_string().replace('\\', "/");
    if cfg!(windows) {
        let script = dir.join("stub.ps1");
        std::fs::write(
            &script,
            format!("\"cwd=$((Get-Location).Path)\" | Out-File -Append -Encoding utf8 '{log}'\n$args | Out-File -Append -Encoding utf8 '{log}'\nexit {code}\n"),
        )
        .unwrap();
        (
            "powershell.exe".to_string(),
            format!("-NoProfile -ExecutionPolicy Bypass -File {}", script.display()),
        )
    } else {
        let script = dir.join("stub.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\necho \"cwd=$(pwd)\" >> '{log}'\necho \"$@\" >> '{log}'\nexit {code}\n"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        ("sh".to_string(), script.display().to_string())
    }
}

#[test]
fn backup_copies_the_database_beside_itself() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .arg("backup")
        .assert()
        .success()
        .stdout(predicate::str::contains("Copied to"));

    let copies: Vec<_> = std::fs::read_dir(data.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bak"))
        .collect();
    assert_eq!(copies.len(), 1, "expected one backup, found {copies:?}");
    // Stamped with the schema it holds, so a restore says what it restores.
    let name = copies[0].file_name().to_string_lossy().to_string();
    assert!(
        name.starts_with("rigger.v") && name.contains('-'),
        "a backup must name the schema it holds: {name}"
    );
}

/// rigger never exits 2, whatever it is asked.
///
/// 2 is clap's own code for a usage error, and it is also the code an
/// assistant's Stop hook uses to refuse the stop and hold the turn open. A
/// hook is written once into a settings file and never looked at again, so
/// a typo in it - or an older rigger on the PATH without the subcommand -
/// would wedge every session it fired in. Found by installing the hook for
/// real: the rigger on PATH was one release behind, and the command exited
/// 2 rather than being ignored.
#[test]
fn a_usage_error_never_exits_with_the_code_that_blocks_a_hook() {
    let data = tempfile::tempdir().unwrap();
    for args in [vec!["no-such-command"], vec!["session", "end", "--no-such-flag"], vec!["project"]] {
        let out = rigger(data.path()).args(&args).assert().failure();
        let code = out.get_output().status.code();
        assert_eq!(code, Some(1), "{args:?} exited {code:?}, and 2 blocks a Stop hook");
    }

    // Help and version are answers, not failures.
    for args in [vec!["--help"], vec!["--version"]] {
        rigger(data.path()).args(&args).assert().success();
    }
}

#[test]
fn doctor_reports_the_database_before_and_after_init() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("missing - run `rigger init`"));

    let root = data.path().join("one");
    repo(&root);
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema:    version").and(predicate::str::contains("projects:  1")));
    rigger(data.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\":"));
}
