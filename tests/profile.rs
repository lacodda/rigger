//! Profiles: one binary, several ways of working.
//!
//! What the release promises: each profile has a database of its own, the
//! one rigger kept before profiles is inherited by the default profile
//! without losing a row, and every command reads the current profile
//! without being told.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env_remove("RIGGER_PROFILE");
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn init_creates_the_config_and_the_default_profile() {
    let data = tempfile::tempdir().unwrap();
    let out = output(data.path(), &["init"]);
    assert!(
        out.contains("Created") && out.contains("config.toml") && out.contains("'line' profile"),
        "{out}"
    );
    assert!(data.path().join("profiles").join("line").join("rigger.db").is_file());

    let list = output(data.path(), &["profile", "list"]);
    assert!(list.starts_with("* line"), "the default profile is in use: {list}");

    let doctor = output(data.path(), &["doctor"]);
    assert!(doctor.contains("profile:   line"), "{doctor}");
}

/// The database rigger kept before profiles is the default profile's,
/// moved into place on first use, with the old file kept.
#[test]
fn the_database_from_before_profiles_is_inherited_by_the_default_profile() {
    let data = tempfile::tempdir().unwrap();
    // A record made the old way: one file at the root of the data
    // directory, no config. Built with this rigger and moved into the old
    // place, because that is a real record with a real schema.
    rigger(data.path()).arg("init").assert().success();
    let root = data.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();
    let moved = data.path().join("profiles").join("line").join("rigger.db");
    let legacy = data.path().join("rigger.db");
    std::fs::rename(&moved, &legacy).unwrap();
    std::fs::remove_dir_all(data.path().join("profiles")).unwrap();
    std::fs::remove_file(data.path().join("config.toml")).unwrap();

    // The first command that needs the database moves it, and says so.
    let out = rigger(data.path()).args(["project", "list"]).assert().success();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("Moved the database into the 'line' profile"), "{stderr}");
    assert!(String::from_utf8_lossy(&out.get_output().stdout).starts_with("repo"), "not a row was lost");
    assert!(moved.is_file(), "the database is now the profile's");
    assert!(!legacy.exists(), "and no longer at the root");
    assert!(data.path().join("rigger.db.before-profiles").is_file(), "the old file is kept");

    // Only once: the next command is quiet, and `init` finds it initialised.
    let again = rigger(data.path()).args(["project", "list"]).assert().success();
    assert!(String::from_utf8_lossy(&again.get_output().stderr).is_empty());
    rigger(data.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Already initialised"));
}

#[test]
fn a_profile_has_a_database_of_its_own_and_use_switches_every_command() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let root = data.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&root).assert().success();

    let out = output(
        data.path(),
        &[
            "profile",
            "add",
            "work",
            "--kind",
            "tickets",
            "--root",
            "C:\\work",
            "--id-pattern",
            "[A-Z]{2,8}-\\d+",
            "--inbox",
            "C:\\work\\inbox",
        ],
    );
    assert!(out.contains("Added the 'work' profile (tickets)"), "{out}");
    assert!(data.path().join("profiles").join("work").join("rigger.db").is_file());
    let config = std::fs::read_to_string(data.path().join("config.toml")).unwrap();
    assert!(config.contains("[profiles.work]") && config.contains("kind = \"tickets\""), "{config}");
    assert!(config.contains("current = \"line\""), "adding does not switch: {config}");

    output(data.path(), &["profile", "use", "work"]);
    let list = output(data.path(), &["project", "list"]);
    assert!(list.contains("No projects yet"), "the work profile has a record of its own: {list}");
    let show = output(data.path(), &["profile", "show"]);
    assert!(show.starts_with("work\n") && show.contains("ids:       [A-Z]{2,8}-\\d+"), "{show}");

    output(data.path(), &["profile", "use", "line"]);
    let list = output(data.path(), &["project", "list"]);
    assert!(list.starts_with("repo"), "the line's projects are where they were: {list}");
}

#[test]
fn the_environment_names_a_profile_over_the_config() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    output(data.path(), &["profile", "add", "work", "--kind", "tickets"]);

    let out = rigger(data.path()).env("RIGGER_PROFILE", "work").args(["doctor"]).assert().success();
    assert!(String::from_utf8_lossy(&out.get_output().stdout).contains("profile:   work"));

    rigger(data.path())
        .env("RIGGER_PROFILE", "nowhere")
        .args(["doctor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no profile named 'nowhere'"));
}

#[test]
fn adopt_walks_the_profiles_roots_when_told_nothing() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    let root = data.path().join("repos");
    let alpha = root.join("alpha");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();

    rigger(data.path())
        .args(["adopt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("names no roots"));

    let set = output(data.path(), &["profile", "set", "--root", root.to_str().unwrap()]);
    assert!(set.contains("Profile 'line' updated") && set.contains("roots:"), "{set}");

    let out = output(data.path(), &["adopt", "--check"]);
    assert!(out.contains("alpha  would record"), "{out}");

    rigger(data.path())
        .args(["profile", "set", "line"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nothing to set"));
}

#[test]
fn a_profile_name_is_one_word_and_not_taken() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .args(["profile", "add", "line"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
    rigger(data.path())
        .args(["profile", "add", "two words"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("one word"));
    rigger(data.path())
        .args(["profile", "use", "nowhere"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no profile named"));
}
