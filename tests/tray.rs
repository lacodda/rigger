//! A card's tray: the material that arrives for a task.
//!
//! What the release promises: a tray is a folder per card with a form in
//! it; `fetch` copies a folder's files in, keeping its layout, and leaves
//! recordings and large files where they are - saying so; `intake` takes
//! what was written lately in the screenshot and download folders; `done`
//! moves everything into the card's archive and sets the form back to
//! blank; the tray follows the card when its key changes; and the card's
//! screen and packet say what waits in it. Nothing here reaches past the
//! scratch record: the folders `intake` reads are named by the test.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env_remove("RIGGER_INTAKE_DIRS");
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn json(data: &Path, args: &[&str]) -> serde_json::Value {
    serde_json::from_str(&output(data, args)).unwrap()
}

/// A desk with one card in hand.
fn desk(data: &Path) {
    rigger(data).arg("init").assert().success();
    rigger(data)
        .args(["task", "new", "rtf files are not supported", "--id", "ACME-7310"])
        .assert()
        .success();
}

fn tray_dir(data: &Path) -> PathBuf {
    data.join("profiles").join("line").join("trays").join("ACME-7310")
}

fn write(path: &Path, bytes: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; bytes]).unwrap();
}

fn age(path: &Path, hours: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(hours * 3600)).unwrap();
}

/// Fills in one answer, the way a person would under a heading.
fn fill(form: &Path, heading: &str, answer: &str) {
    let text = std::fs::read_to_string(form).unwrap();
    assert!(text.contains(heading), "the form has no {heading:?}:\n{text}");
    std::fs::write(form, text.replace(heading, &format!("{heading}\n\n{answer}"))).unwrap();
}

#[test]
fn the_tray_of_the_card_in_hand_is_made_with_a_blank_form_once() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());

    let out = output(data.path(), &["tray", "show"]);
    assert!(out.contains("ACME-7310 · rtf files are not supported"), "{out}");
    assert!(out.contains("made the tray"), "{out}");
    assert!(out.contains("(blank)"), "{out}");
    let form = tray_dir(data.path()).join("incoming.md");
    let text = std::fs::read_to_string(&form).unwrap();
    assert!(text.starts_with("# ACME-7310 · rtf files are not supported"), "{text}");
    assert!(text.contains("## Links to files"), "{text}");

    // A second look makes nothing and keeps what the person wrote.
    fill(&form, "## The task as written (required)", "The customer cannot open rtf files.");
    let out = output(data.path(), &["tray", "show"]);
    assert!(!out.contains("made the tray"), "{out}");
    assert!(out.contains("(filled)"), "{out}");
    assert!(std::fs::read_to_string(&form).unwrap().contains("cannot open rtf"));

    let show = json(data.path(), &["tray", "show", "ACME-7310", "--json"]);
    assert_eq!(show["card"]["key"], "ACME-7310");
    assert_eq!(show["made"], false);
    assert_eq!(show["form"]["filled"], true);
}

#[test]
fn with_no_card_in_hand_the_tray_asks_which() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path())
        .args(["tray", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rigger tray show <KEY>"));
}

#[test]
fn fetch_keeps_the_layout_and_leaves_recordings_and_large_files_where_they_are() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let share = data.path().join("share");
    write(&share.join("shot-1.png"), 2000);
    write(&share.join("logs").join("app.log"), 500);
    write(&share.join("screen.mp4"), 100);
    write(&share.join("dump.bin"), 2 * 1024 * 1024);
    write(&share.join("logs").join("Thumbs.db"), 10);

    let out = output(data.path(), &["tray", "fetch", "--from", share.to_str().unwrap(), "--max-mb", "1"]);
    let tray = tray_dir(data.path());
    assert!(tray.join("shot-1.png").is_file(), "{out}");
    assert!(tray.join("logs").join("app.log").is_file(), "the folder's layout is kept: {out}");
    assert!(!tray.join("screen.mp4").exists(), "a recording is not copied: {out}");
    assert!(!tray.join("dump.bin").exists(), "a file over the limit is not copied: {out}");
    assert!(out.contains("screen.mp4") && out.contains("a recording"), "{out}");
    assert!(out.contains("dump.bin") && out.contains("larger than the limit"), "{out}");
    assert!(out.contains("Took 2 files"), "{out}");
    assert!(
        !tray.join("logs").join("Thumbs.db").exists() && !out.contains("Thumbs.db"),
        "a folder's file about itself is not material: {out}"
    );

    // Fetching the same folder again takes nothing twice, and a file of the
    // same name that is not the same file is left where it is.
    write(&share.join("shot-1.png"), 3000);
    let again = json(data.path(), &["tray", "fetch", "--from", share.to_str().unwrap(), "--max-mb", "1", "--json"]);
    assert_eq!(again["taken"].as_array().unwrap().len(), 0, "{again:#}");
    let why: Vec<(&str, &str)> = again["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["path"].as_str().unwrap(), s["why"].as_str().unwrap()))
        .collect();
    assert!(why.iter().any(|(p, w)| p.ends_with("shot-1.png") && *w == "differs"), "{why:?}");
    assert!(why.iter().any(|(p, w)| p.ends_with("app.log") && *w == "present"), "{why:?}");
    assert!(why.iter().any(|(p, w)| p.ends_with("screen.mp4") && *w == "video"), "{why:?}");
    assert!(why.iter().any(|(p, w)| p.ends_with("dump.bin") && *w == "too-big"), "{why:?}");
    assert_eq!(
        std::fs::metadata(tray.join("shot-1.png")).unwrap().len(),
        2000,
        "the tray's file is not overwritten"
    );

    // Asked for, a recording is taken.
    let video = json(data.path(), &["tray", "fetch", "--from", share.to_str().unwrap(), "--video", "--json"]);
    assert!(video["taken"].as_array().unwrap().iter().any(|i| i["path"] == "screen.mp4"), "{video:#}");
}

#[test]
fn fetch_without_a_folder_takes_the_ones_the_form_lists() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path())
        .args(["tray", "fetch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Links to files"));

    let first = data.path().join("tester-a");
    let second = data.path().join("tester-b");
    write(&first.join("a.png"), 10);
    write(&second.join("b.png"), 10);
    let form = tray_dir(data.path()).join("incoming.md");
    fill(&form, "## Links to files (optional)", &format!("{}\n{}", first.display(), second.display()));
    let out = output(data.path(), &["tray", "fetch"]);
    assert!(tray_dir(data.path()).join("a.png").is_file(), "{out}");
    assert!(tray_dir(data.path()).join("b.png").is_file(), "{out}");

    rigger(data.path())
        .args(["tray", "fetch", "--from"])
        .arg(data.path().join("nowhere"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot reach"));
}

#[test]
fn intake_takes_what_was_written_lately_and_nothing_older() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let shots = data.path().join("Screenshots");
    let downloads = data.path().join("Downloads");
    write(&shots.join("fresh.png"), 10);
    write(&shots.join("old.png"), 10);
    age(&shots.join("old.png"), 30);
    write(&downloads.join("export.csv"), 10);
    write(&downloads.join("folder").join("inside.txt"), 10);
    let dirs = std::env::join_paths([&shots, &downloads]).unwrap();

    let out = rigger(data.path())
        .env("RIGGER_INTAKE_DIRS", &dirs)
        .args(["tray", "intake", "--since", "4h"])
        .assert()
        .success();
    let out = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let tray = tray_dir(data.path());
    assert!(tray.join("fresh.png").is_file(), "{out}");
    assert!(tray.join("export.csv").is_file(), "{out}");
    assert!(!tray.join("old.png").exists(), "older than the window: {out}");
    assert!(
        !tray.join("inside.txt").exists() && !tray.join("folder").exists(),
        "folders are not walked into: {out}"
    );

    // What an installer or a browser leaves is not material: a shortcut, a
    // download still under way, the folder's own file about itself.
    for noise in ["kilna.lnk", "tracker.url", "big.iso.crdownload", "desktop.ini"] {
        write(&shots.join(noise), 10);
    }
    let out = rigger(data.path())
        .env("RIGGER_INTAKE_DIRS", &dirs)
        .args(["tray", "intake", "--since", "4h", "--json"])
        .assert()
        .success();
    let noisy: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(noisy["taken"].as_array().unwrap().len(), 0, "{noisy:#}");
    assert_eq!(
        noisy["skipped"].as_array().unwrap().iter().filter(|s| s["why"] != "present").count(),
        0,
        "noise is not even listed: {noisy:#}"
    );
    for noise in ["kilna.lnk", "tracker.url", "big.iso.crdownload", "desktop.ini"] {
        assert!(!tray.join(noise).exists(), "{noise} is not material");
    }

    // A wider window reaches the older file.
    rigger(data.path())
        .env("RIGGER_INTAKE_DIRS", &dirs)
        .args(["tray", "intake", "--since", "2d"])
        .assert()
        .success();
    assert!(tray.join("old.png").is_file());
}

#[test]
fn a_scratch_record_takes_from_no_folder_it_was_not_given() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path())
        .args(["tray", "intake"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("RIGGER_INTAKE_DIRS"));
    rigger(data.path())
        .args(["tray", "intake", "--since", "soon"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a window of time"));
}

#[test]
fn done_moves_the_material_to_the_archive_and_blanks_the_form() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    output(data.path(), &["tray", "show"]);
    let tray = tray_dir(data.path());
    write(&tray.join("shot.png"), 10);
    write(&tray.join("logs").join("app.log"), 10);
    let form = tray.join("incoming.md");
    fill(&form, "## The task as written (required)", "The customer cannot open rtf files.");

    let out = output(data.path(), &["tray", "done"]);
    let day = jiff::Zoned::now().date().to_string();
    let archive = tray.join("sorted").join(&day);
    assert!(out.contains("2 entries moved"), "{out}");
    assert!(archive.join("shot.png").is_file(), "{out}");
    assert!(archive.join("logs").join("app.log").is_file(), "{out}");
    assert!(
        std::fs::read_to_string(archive.join("incoming.md")).unwrap().contains("cannot open rtf"),
        "the filled form goes with the material"
    );
    let blank = std::fs::read_to_string(&form).unwrap();
    assert!(!blank.contains("cannot open rtf"), "the tray's form is blank again:\n{blank}");
    assert!(!tray.join("shot.png").exists());

    let show = json(data.path(), &["tray", "show", "--json"]);
    assert_eq!(show["files"].as_array().unwrap().len(), 0, "{show:#}");
    assert_eq!(show["sorted"][0]["day"], day.as_str());
    assert_eq!(show["sorted"][0]["files"], 3, "two files and the form: {show:#}");

    // A second round the same day lands beside the first, overwriting nothing.
    write(&tray.join("shot.png"), 20);
    output(data.path(), &["tray", "done"]);
    assert!(archive.join("shot (2).png").is_file());
    assert_eq!(std::fs::metadata(archive.join("shot.png")).unwrap().len(), 10);

    let out = output(data.path(), &["tray", "done"]);
    assert!(out.contains("nothing to sort"), "{out}");
}

#[test]
fn the_tray_follows_the_card_when_it_is_renamed() {
    let data = tempfile::tempdir().unwrap();
    rigger(data.path()).arg("init").assert().success();
    rigger(data.path()).args(["task", "new", "export drops the footer"]).assert().success();
    let local = json(data.path(), &["task", "active", "--json"])["key"].as_str().unwrap().to_string();
    output(data.path(), &["tray", "show"]);
    let root = data.path().join("profiles").join("line").join("trays");
    write(&root.join(&local).join("shot.png"), 10);

    rigger(data.path()).args(["task", "rename", &local, "ACME-9"]).assert().success();
    let show = json(data.path(), &["tray", "show", "ACME-9", "--json"]);
    assert_eq!(show["made"], false, "the old tray is found, not a new one made: {show:#}");
    assert!(root.join("ACME-9").join("shot.png").is_file());
    assert!(!root.join(&local).exists(), "there is one tray per task");
}

#[test]
fn the_list_names_only_trays_with_something_in_them() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path()).args(["task", "new", "another", "--id", "ACME-5"]).assert().success();
    output(data.path(), &["tray", "show", "ACME-5"]);
    output(data.path(), &["tray", "show", "ACME-7310"]);
    let out = output(data.path(), &["tray", "list"]);
    assert!(out.contains("Nothing waits"), "{out}");

    write(&tray_dir(data.path()).join("shot.png"), 10);
    let out = output(data.path(), &["tray", "list"]);
    assert!(out.contains("ACME-7310") && out.contains("1 file"), "{out}");
    assert!(!out.contains("ACME-5"), "{out}");
    let list = json(data.path(), &["tray", "list", "--json"]);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "rtf files are not supported");
}

#[test]
fn the_card_says_what_waits_in_its_tray() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let out = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(!out.contains("tray:"), "no tray, no line: {out}");
    assert!(json(data.path(), &["task", "show", "ACME-7310", "--json"])["tray"].is_null());

    output(data.path(), &["tray", "show"]);
    let tray = tray_dir(data.path());
    write(&tray.join("shot.png"), 10);
    fill(&tray.join("incoming.md"), "## Where the work is done (required)", "C:\\work\\webapp");

    let out = output(data.path(), &["task", "show", "ACME-7310"]);
    assert!(out.contains("the form filled, 1 file waiting"), "{out}");
    let packet = output(data.path(), &["task", "context", "ACME-7310"]);
    assert!(packet.contains("## Tray"), "{packet}");
    assert!(packet.contains("the form is filled; read it first"), "{packet}");
    assert!(packet.contains("1 file waiting: shot.png"), "{packet}");

    output(data.path(), &["tray", "done"]);
    let packet = output(data.path(), &["task", "context", "ACME-7310"]);
    assert!(packet.contains("the form is blank"), "{packet}");
    assert!(packet.contains("sorted before:"), "{packet}");
}

#[test]
fn the_profile_says_where_the_trays_are() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    let elsewhere = data.path().join("elsewhere");
    rigger(data.path()).args(["profile", "set", "--trays"]).arg(&elsewhere).assert().success();
    output(data.path(), &["tray", "show"]);
    assert!(elsewhere.join("ACME-7310").join("incoming.md").is_file());
    let profile = output(data.path(), &["profile", "show"]);
    assert!(profile.contains("trays:") && profile.contains("elsewhere"), "{profile}");
}
