//! A project worked on in a folder with no repository.
//!
//! What the release claims: the kind follows the disk - a directory without
//! git is recorded as a folder, and the same path recorded again once git
//! arrives becomes a repository; git is never asked about a folder, yet the
//! line's skill offers it like any product; its skill is written from a
//! template of its own; and the line about it lives in the record only
//! where no manifest says one.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd.env("RIGGER_SKILLS_DIR", data.join("skills"));
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn refusal(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().failure();
    String::from_utf8(out.get_output().stderr.clone()).unwrap()
}

fn json(data: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    serde_json::from_str(&output(data, &all)).unwrap()
}

fn git_init(root: &Path) {
    let status = std::process::Command::new("git")
        .args(["init", "--quiet", "--initial-branch", "main"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success());
}

/// A line of two: `sample`, a repository whose manifest describes it, and
/// `texts`, a folder of work with nothing that describes it.
fn line(data: &Path) -> (PathBuf, PathBuf) {
    rigger(data).arg("init").assert().success();
    let repo = data.join("sample");
    std::fs::create_dir_all(&repo).unwrap();
    git_init(&repo);
    std::fs::write(repo.join("Cargo.toml"), "[package]\nname = \"sample\"\ndescription = \"A sample product\"\n").unwrap();
    rigger(data).args(["project", "add"]).arg(&repo).assert().success();

    let folder = data.join("texts");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("first.md"), "# The first text\n").unwrap();
    let said = output(data, &["project", "add", folder.to_str().unwrap()]);
    assert!(said.contains("no repository"), "the add says what it recorded: {said}");
    assert!(said.contains("--about"), "and how to say what it is: {said}");
    (repo, folder)
}

#[test]
fn a_directory_without_git_is_recorded_as_a_folder_and_one_with_git_as_a_repository() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    assert_eq!(json(data.path(), &["project", "show", "texts"])["kind"], "folder");
    assert_eq!(json(data.path(), &["project", "show", "sample"])["kind"], "repo");

    let list = output(data.path(), &["project", "list"]);
    assert!(list.contains("(a folder, no repository)"), "{list}");
    let show = output(data.path(), &["show", "texts"]);
    assert!(show.contains("- a folder, no repository"), "{show}");
}

#[test]
fn git_is_never_asked_about_a_folder() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());

    // Named, it says why there is nothing to read rather than failing.
    let said = output(data.path(), &["sync", "texts"]);
    assert!(said.contains("folder with no repository"), "{said}");
    // Across the line, it is passed over without a word.
    let all = rigger(data.path()).arg("sync").assert().success();
    let all = String::from_utf8(all.get_output().stdout.clone()).unwrap() + &String::from_utf8(all.get_output().stderr.clone()).unwrap();
    assert!(!all.contains("texts"), "{all}");
    // And the doctor does not list it as waiting for a sync it cannot have.
    let doctor = output(data.path(), &["doctor"]);
    assert!(!doctor.contains("texts"), "{doctor}");

    // The packet says what kind of place it is, so no session goes
    // looking for commits.
    let packet = output(data.path(), &["context", "texts"]);
    assert!(packet.contains("A folder with no repository"), "{packet}");
    assert_eq!(json(data.path(), &["context", "texts"])["state"]["kind"], "folder");
    assert_eq!(json(data.path(), &["context", "sample"])["state"]["kind"], "repo");
}

/// A folder has a hub like any product, and a generated file of it edited
/// by hand is a drift the doctor names - the next export would overwrite
/// the edit without a word.
#[test]
fn the_hub_of_a_folder_is_vouched_for_like_any_other() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    let hub = data.path().join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), "# План\n\n## Ждёт решения владельца\n\n- (пусто)\n").unwrap();
    rigger(data.path()).args(["import", "texts", "--hub"]).arg(&hub).assert().success();
    rigger(data.path()).args(["export", "texts", "--adopt", "--hub"]).arg(&hub).assert().success();
    let clean = output(data.path(), &["doctor", "--hubs"]);
    assert!(!clean.contains("texts"), "{clean}");

    let plan = std::fs::read_to_string(hub.join("План.md")).unwrap();
    std::fs::write(hub.join("План.md"), plan + "\nA line written by hand.\n").unwrap();
    let drift = output(data.path(), &["doctor", "--hubs"]);
    assert!(drift.contains("texts"), "{drift}");
}

#[test]
fn the_line_offers_a_folder_but_the_public_registry_does_not() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    rigger(data.path())
        .args(["project", "set", "texts", "--about", "A library of short texts"])
        .assert()
        .success();

    let skill = output(data.path(), &["skill", "--line"]);
    assert!(skill.contains("| `texts` | A library of short texts |"), "{skill}");
    assert!(skill.contains("| `sample` | A sample product |"), "{skill}");

    let registry: Value = serde_json::from_str(&output(data.path(), &["export", "--line"])).unwrap();
    let names: Vec<&str> = registry["products"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["sample"], "a folder ships nothing public");
}

#[test]
fn a_folder_has_a_skill_of_its_own_kind() {
    let data = tempfile::tempdir().unwrap();
    let (_, folder) = line(data.path());
    rigger(data.path())
        .args(["project", "set", "texts", "--about", "A library of short texts"])
        .assert()
        .success();

    let skill = output(data.path(), &["skill", "texts"]);
    assert!(skill.starts_with("---\nname: texts\ndescription: texts - A library of short texts."), "{skill}");
    let folder = dunce::canonicalize(&folder).unwrap();
    assert!(
        skill.contains(&format!("The work lives in {}, a folder with no repository", folder.display())),
        "{skill}"
    );
    assert!(skill.contains("rigger rules texts"), "{skill}");
    assert!(skill.contains("names a ritual"), "a word after the name is a ritual: {skill}");
    assert!(!skill.contains("before a commit"), "a folder has no commits: {skill}");

    // A line's own template for repositories speaks of code and commits;
    // a folder does not borrow it.
    std::fs::write(data.path().join("skill.md"), "---\nname: {{name}}\ndescription: Code in {{path}}\n---\n").unwrap();
    let skill = output(data.path(), &["skill", "texts"]);
    assert!(!skill.contains("Code in"), "{skill}");
    assert!(output(data.path(), &["skill", "sample"]).contains("Code in"));

    // Its own is followed.
    std::fs::write(
        data.path().join("skill.folder.md"),
        "---\nname: {{name}}\ndescription: Texts in {{path}}\n---\n",
    )
    .unwrap();
    assert!(output(data.path(), &["skill", "texts"]).contains("Texts in"));

    // The built-in one can be printed by naming the project.
    let template = output(data.path(), &["skill", "--print-template", "texts"]);
    assert!(template.contains("a folder with no repository"), "{template}");
    let template = output(data.path(), &["skill", "--print-template"]);
    assert!(!template.contains("a folder with no repository"), "{template}");
}

#[test]
fn the_line_about_a_project_is_the_manifests_where_there_is_one() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());

    // A folder says nothing until told.
    assert!(output(data.path(), &["skill", "texts"]).contains("a project recorded in rigger"));
    let set = json(data.path(), &["project", "set", "texts", "--about", "A library of short texts"]);
    assert_eq!(set["about"], "A library of short texts");
    assert!(output(data.path(), &["show", "texts"]).contains("A library of short texts"));

    // A repository whose manifest describes it is refused a second line.
    let refused = refusal(data.path(), &["project", "set", "sample", "--about", "Something else"]);
    assert!(refused.contains("described by its manifest"), "{refused}");
    assert!(refused.contains("A sample product"), "{refused}");
    assert_eq!(json(data.path(), &["project", "show", "sample"])["about"], Value::Null);

    // What would break the catalogue of skills is refused before it is kept.
    let refused = refusal(data.path(), &["project", "set", "texts", "--about", "texts for <reader>"]);
    assert!(refused.contains("catalogue"), "{refused}");

    // And forgotten on asking.
    rigger(data.path()).args(["project", "set", "texts", "--no-about"]).assert().success();
    assert_eq!(json(data.path(), &["project", "show", "texts"])["about"], Value::Null);
}

#[test]
fn a_description_over_the_limit_is_refused_rather_than_written() {
    let data = tempfile::tempdir().unwrap();
    line(data.path());
    let long = "word ".repeat(220);
    rigger(data.path()).args(["project", "set", "texts", "--about", &long]).assert().success();
    let refused = refusal(data.path(), &["skill", "texts", "--install"]);
    assert!(refused.contains("1024"), "{refused}");
    assert!(!data.path().join("skills").join("texts").exists());
}

#[test]
fn a_folder_that_git_arrives_in_becomes_a_repository_on_the_same_path() {
    let data = tempfile::tempdir().unwrap();
    let (_, folder) = line(data.path());
    git_init(&folder);
    let said = output(data.path(), &["project", "add", folder.to_str().unwrap()]);
    assert!(said.contains("has a repository now"), "{said}");
    assert_eq!(json(data.path(), &["project", "show", "texts"])["kind"], "repo");
    // The same project, not a second one.
    let list = json(data.path(), &["project", "list"]);
    assert_eq!(list.as_array().unwrap().iter().filter(|p| p["name"] == "texts").count(), 1);

    // A repository recorded again is still refused as already recorded.
    let refused = refusal(data.path(), &["project", "add", folder.to_str().unwrap()]);
    assert!(refused.contains("already recorded"), "{refused}");
}
