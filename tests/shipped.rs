//! What a release did past its publish: installed for the owner, deployed.
//!
//! The release engine proved both and could only say so in the text of an event,
//! so "released, not installed" took reading prose to see. What the release
//! promises here: the engine's word lands in the record, a word about the install
//! leaves the delivery alone, and a version owes only what its project's
//! `release.toml` says a release ends with - a library has no install to miss.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;
use serde_json::Value;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn json(data: &Path, args: &[&str]) -> Value {
    let out = rigger(data).args(args).arg("--json").assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).unwrap_or_else(|e| panic!("rigger {args:?} --json: {e}"))
}

fn text(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success().get_output().stdout.clone();
    String::from_utf8(out).unwrap()
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
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

/// A project with a commit behind it and two versions it can ship.
fn project(data: &Path, work: &Path) -> PathBuf {
    let root = work.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch", "main"]);
    std::fs::write(root.join("a.txt"), "a").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feat: the first thing"]);
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    root
}

fn promise(root: &Path, text: &str) {
    std::fs::write(root.join("release.toml"), text).unwrap();
}

#[test]
fn the_engine_says_it_installed_and_deployed_and_the_version_keeps_it() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());

    let said = text(
        data.path(),
        &[
            "note",
            "sample",
            "--kind",
            "shipped",
            "--tag",
            "v0.2.0",
            "--release",
            "--registry",
            "crates.io",
            "--installed",
            "--deployed",
        ],
    );
    assert!(said.contains("sample v0.2.0 shipped - released and published, installed, deployed"), "{said}");

    let card = json(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert!(card["installed_at"].is_string(), "{card:#}");
    assert!(card["deployed_at"].is_string(), "{card:#}");
    assert_eq!(card["delivery"], "published");

    let said = text(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert!(said.contains("installed  on "), "{said}");
    assert!(said.contains("deployed   on "), "{said}");

    let event = text(data.path(), &["context", "sample"]);
    assert!(
        event.contains("v0.2.0 went out: the release with its archives, crates.io; installed and deployed"),
        "{event}"
    );
}

/// The install is proven after the release. Read as a word about the release, its
/// silence would turn a published version back into a tag, and one about the
/// deploy would take the install's away.
#[test]
fn a_word_about_the_install_leaves_the_rest_alone() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "npm"])
        .assert()
        .success();
    let card = json(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert!(card["installed_at"].is_null(), "{card:#}");
    assert!(card["deployed_at"].is_null(), "{card:#}");

    let said = text(data.path(), &["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--installed"]);
    assert!(said.contains("had shipped already; now released and published, installed"), "{said}");
    let installed = json(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert_eq!(installed["delivery"], "published", "{installed:#}");
    assert_eq!(installed["registries"][0], "npm", "{installed:#}");
    let at = installed["installed_at"].as_str().unwrap().to_string();
    assert!(installed["deployed_at"].is_null(), "{installed:#}");

    // The first moment stays, and a word about the deploy keeps the install.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--installed", "--deployed"])
        .assert()
        .success();
    let both = json(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert_eq!(both["installed_at"], at.as_str(), "{both:#}");
    assert!(both["deployed_at"].is_string(), "{both:#}");

    // The word itself is in the history, and is not called a release.
    let history = text(data.path(), &["context", "sample"]);
    assert!(history.contains("v0.2.0 installed and deployed"), "{history}");
    assert!(!history.contains("v0.2.0 went out as a tag"), "{history}");
}

/// On a version nothing else was said about, GitHub may still answer for the
/// release: the engine's word on the install must not settle that as "a tag".
#[test]
fn a_word_about_the_install_alone_says_nothing_of_the_delivery() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());

    let said = text(data.path(), &["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--installed"]);
    assert!(said.contains("sample v0.2.0 shipped - installed"), "{said}");
    let card = json(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert_eq!(card["status"], "shipped", "{card:#}");
    assert!(card["delivery"].is_null(), "{card:#}");
    assert!(card["installed_at"].is_string(), "{card:#}");
}

#[test]
fn the_flags_belong_to_a_release() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    for flag in ["--installed", "--deployed"] {
        rigger(data.path())
            .args(["note", "sample", "a finding", flag])
            .assert()
            .failure()
            .stderr(predicate::str::contains("describe a release"));
    }
}

#[test]
fn a_version_owes_what_its_release_promises_and_nothing_else() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let root = project(data.path(), work.path());
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "npm"])
        .assert()
        .success();

    // No file, and a file that promises neither: nothing to miss.
    let owed = |data: &Path| -> Value { json(data, &["version", "show", "sample", "v0.2.0"])["owed"].clone() };
    assert_eq!(owed(data.path()), serde_json::json!([]));
    assert_eq!(json(data.path(), &["show", "sample"])["owed"], serde_json::json!([]));
    assert!(!text(data.path(), &["context", "sample"]).contains("not installed"));
    promise(&root, "[gate]\ncommands = [\"cd .\"]\n");
    assert_eq!(owed(data.path()), serde_json::json!([]));

    // A promise of each is a debt of each, on every screen that says delivery.
    promise(&root, "[post.install]\n\n[post.deploy]\n");
    assert_eq!(owed(data.path()), serde_json::json!(["install", "deploy"]));
    let screen = text(data.path(), &["show", "sample"]);
    assert!(screen.contains("released and published, not installed, not deployed"), "{screen}");
    let packet = text(data.path(), &["context", "sample"]);
    assert!(packet.contains("- not installed, not deployed"), "{packet}");
    assert_eq!(
        json(data.path(), &["context", "sample"])["state"]["owed"],
        serde_json::json!(["install", "deploy"])
    );
    let card = text(data.path(), &["version", "show", "sample", "v0.2.0"]);
    assert!(card.contains("installed  not yet"), "{card}");
    assert!(card.contains("deployed   not yet"), "{card}");

    // Paid one at a time.
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--installed"])
        .assert()
        .success();
    assert_eq!(owed(data.path()), serde_json::json!(["deploy"]));
    let packet = text(data.path(), &["context", "sample"]);
    assert!(packet.contains("- not deployed"), "{packet}");
    assert!(!packet.contains("not installed"), "{packet}");
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--deployed"])
        .assert()
        .success();
    assert_eq!(owed(data.path()), serde_json::json!([]));
    assert!(json(data.path(), &["context", "sample"])["state"].get("owed").is_none());

    // Only the newest version owes: the older one is history.
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.3.0", "--release"])
        .assert()
        .success();
    assert_eq!(
        json(data.path(), &["version", "show", "sample", "v0.3.0"])["owed"],
        serde_json::json!(["install", "deploy"])
    );
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.1.0", "--release"])
        .assert()
        .success();
    assert_eq!(json(data.path(), &["version", "show", "sample", "v0.1.0"])["owed"], serde_json::json!([]));
}

/// What a release owes is said, and does not hold up the line: it is not a turn
/// left unfinished, or every project would stop at the first install it missed.
#[test]
fn a_debt_is_said_and_does_not_hold_the_queue() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let root = project(data.path(), work.path());
    promise(&root, "[post.install]\n");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "chore: release file"]);
    git(&root, &["tag", "v0.2.0"]);
    rigger(data.path()).args(["sync", "sample"]).assert().success();
    rigger(data.path())
        .args(["note", "sample", "--kind", "shipped", "--tag", "v0.2.0", "--release", "--registry", "npm"])
        .assert()
        .success();
    assert_eq!(json(data.path(), &["show", "sample"])["owed"], serde_json::json!(["install"]));
    let queue = json(data.path(), &["queue"]);
    assert!(queue["order"][0]["unfinished"].is_null(), "{queue:#}");
}

/// A migration only an old database runs: what the engine already said in its
/// text comes into the columns, its own sentences and nothing that merely sounds
/// like them.
#[test]
fn what_the_engine_said_in_text_comes_over_on_the_migration() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    project(data.path(), work.path());
    for tag in ["v0.1.0", "v0.2.0", "v0.3.0", "v0.4.0"] {
        rigger(data.path())
            .args(["note", "sample", "--kind", "shipped", "--tag", tag, "--release"])
            .assert()
            .success();
    }
    let db = {
        let doctor = text(data.path(), &["doctor"]);
        let line = doctor.lines().find(|l| l.starts_with("database:")).unwrap().to_string();
        PathBuf::from(line.trim_start_matches("database:").trim())
    };
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "ALTER TABLE versions DROP COLUMN installed_at;
             ALTER TABLE versions DROP COLUMN deployed_at;
             PRAGMA user_version = 34;",
        )
        .unwrap();
        let said = [
            (
                "v0.2.0",
                "v0.2.0 went out: the release with its archives\n\nreleased by `furca release run` from abc1234; every shop window checked; sample 0.2.0 installed from the release and proven by its hash; deployed by turnout",
            ),
            (
                "v0.3.0",
                "v0.3.0 went out: the release with its archives\n\nInstalled for the owner by hand over the old one, and rolled out to the stand.",
            ),
        ];
        for (_, body) in said {
            conn.execute(
                "INSERT INTO events (project_id, kind, body, author, created_at) SELECT id, 'shipped', ?1, 'engine', '2026-10-05T10:00:00Z' FROM projects WHERE name = 'sample'",
                [body],
            )
            .unwrap();
        }
        // Another version's sentence is not this version's.
        conn.execute(
            "INSERT INTO events (project_id, kind, body, author, created_at) SELECT id, 'shipped', 'v0.40.0 went out: x; sample 0.40.0 installed from the release and proven by its hash', 'engine', '2026-10-06T10:00:00Z' FROM projects WHERE name = 'sample'",
            [],
        )
        .unwrap();
    }

    let at = |version: &str| json(data.path(), &["version", "show", "sample", version]);
    let engine = at("v0.2.0");
    assert_eq!(engine["installed_at"], "2026-10-05T10:00:00Z", "{engine:#}");
    assert_eq!(engine["deployed_at"], "2026-10-05T10:00:00Z", "{engine:#}");
    let person = at("v0.3.0");
    assert!(person["installed_at"].is_null(), "{person:#}");
    assert!(person["deployed_at"].is_null(), "{person:#}");
    let unsaid = at("v0.4.0");
    assert!(unsaid["installed_at"].is_null(), "{unsaid:#}");
    assert!(at("v0.1.0")["installed_at"].is_null());
}
