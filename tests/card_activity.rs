//! A card's activity, read from git.
//!
//! What the release promises: a branch or a commit that carries a card's
//! key ties the card to the repository without anybody running `task link`;
//! `task show` says which branches, the last commit and how long the card
//! has stood still; a branch deleted after its merge leaves the card, the
//! commits made for it stay; and an id that names no card makes none.

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as TestCommand;

fn rigger(data: &Path) -> TestCommand {
    let mut cmd = TestCommand::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// Runs git in `dir`, with an identity of its own so the fixture does not
/// depend on the machine's git configuration.
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

fn commit(dir: &Path, file: &str, message: &str) {
    std::fs::write(dir.join(file), message).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

/// A desk with a card, and a repository where the card is worked without
/// anybody having said so: a commit on main that names it, and a branch
/// named after it whose own commit does not.
fn desk(data: &Path) -> std::path::PathBuf {
    rigger(data).arg("init").assert().success();
    let repo = data.join("webapp");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--quiet", "--initial-branch", "main"]);
    commit(&repo, "a.txt", "chore: start");
    commit(&repo, "b.txt", "feat(wa-4130): read the rtf header");
    // Another ticket whose id begins with the card's.
    commit(&repo, "c.txt", "fix(WA-41300): somebody else's ticket");
    git(&repo, &["checkout", "--quiet", "-b", "fix/WA-4130-rtf"]);
    commit(&repo, "d.txt", "fix: keep the rtf body");
    git(&repo, &["checkout", "--quiet", "main"]);
    rigger(data).args(["project", "add"]).arg(&repo).assert().success();
    rigger(data)
        .args(["task", "new", "rtf files are not supported", "--id", "WA-4130"])
        .assert()
        .success();
    repo
}

fn show(data: &Path, card: &str) -> serde_json::Value {
    serde_json::from_str(&output(data, &["task", "show", card, "--json"])).unwrap()
}

#[test]
fn a_branch_and_a_commit_that_carry_the_key_tie_the_card_to_the_repository() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());

    let out = output(data.path(), &["sync", "webapp"]);
    assert!(out.contains("card       WA-4130: 1 new commit, 1 branch, linked now"), "{out}");

    let json = show(data.path(), "WA-4130");
    let links = json["links"].as_array().unwrap();
    assert_eq!(links.len(), 1, "{json:#}");
    assert_eq!(links[0]["project"], "webapp");
    assert_eq!(links[0]["branch"], "fix/WA-4130-rtf", "the branch found is the link's: {json:#}");

    let activity = &json["activity"][0];
    assert_eq!(activity["project"], "webapp");
    assert_eq!(activity["commits"], 1, "WA-41300 is another ticket: {json:#}");
    assert_eq!(activity["branches"][0]["name"], "fix/WA-4130-rtf");
    assert!(activity["branches"][0]["remote"].is_null());
    // The branch tip is the newest movement, though its commit does not
    // repeat the key.
    assert_eq!(activity["last_commit"]["subject"], "fix: keep the rtf body", "{json:#}");
    assert_eq!(json["idle_days"], 0);

    let text = output(data.path(), &["task", "show", "WA-4130"]);
    assert!(text.contains("in git:"), "{text}");
    assert!(text.contains("branch fix/WA-4130-rtf"), "{text}");
    assert!(text.contains("idle:     moved today"), "{text}");

    let packet = output(data.path(), &["task", "context", "WA-4130"]);
    assert!(packet.contains("## In git"), "{packet}");
    assert!(packet.contains("fix: keep the rtf body"), "{packet}");

    // Read again, git says nothing new about the card.
    let again = output(data.path(), &["sync", "webapp"]);
    assert!(!again.contains("card "), "{again}");
    assert!(again.contains("nothing changed"), "{again}");
}

#[test]
fn a_deleted_branch_leaves_the_card_and_its_commits_stay() {
    let data = tempfile::tempdir().unwrap();
    let repo = desk(data.path());
    rigger(data.path()).args(["sync", "webapp"]).assert().success();

    git(&repo, &["merge", "--quiet", "--no-edit", "fix/WA-4130-rtf"]);
    git(&repo, &["branch", "--quiet", "-d", "fix/WA-4130-rtf"]);
    rigger(data.path()).args(["sync", "webapp"]).assert().success();

    let json = show(data.path(), "WA-4130");
    let activity = &json["activity"][0];
    assert_eq!(activity["branches"].as_array().unwrap().len(), 0, "{json:#}");
    assert_eq!(activity["commits"], 1, "{json:#}");
    // The link is the record's: the branch it names stays as history of
    // where the card was worked.
    assert_eq!(json["links"][0]["branch"], "fix/WA-4130-rtf");
}

#[test]
fn a_branch_only_on_the_remote_says_which_remote() {
    let data = tempfile::tempdir().unwrap();
    let origin = desk(data.path());
    let clone = data.path().join("clone");
    git(data.path(), &["clone", "--quiet", origin.to_str().unwrap(), clone.to_str().unwrap()]);
    rigger(data.path())
        .args(["project", "add"])
        .arg(&clone)
        .args(["--name", "webapp-clone"])
        .assert()
        .success();
    rigger(data.path()).args(["sync", "webapp-clone"]).assert().success();

    let json = show(data.path(), "WA-4130");
    let activity = json["activity"].as_array().unwrap();
    let cloned = activity.iter().find(|a| a["project"] == "webapp-clone").expect("the clone is read");
    assert_eq!(cloned["branches"][0]["name"], "fix/WA-4130-rtf", "{json:#}");
    assert_eq!(cloned["branches"][0]["remote"], "origin", "{json:#}");
}

#[test]
fn a_branch_named_by_hand_is_not_overwritten_by_git() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path())
        .args(["task", "link", "WA-4130", "webapp", "--branch", "hotfix/rtf"])
        .assert()
        .success();
    rigger(data.path()).args(["sync", "webapp"]).assert().success();
    let json = show(data.path(), "WA-4130");
    assert_eq!(json["links"][0]["branch"], "hotfix/rtf", "{json:#}");
}

#[test]
fn an_old_key_kept_as_an_alias_still_finds_the_work() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    // The ticket moved to another tracker; the branch still carries the
    // old id.
    rigger(data.path()).args(["task", "rename", "WA-4130", "API-7"]).assert().success();
    let out = output(data.path(), &["sync", "webapp"]);
    assert!(out.contains("card       API-7:"), "{out}");
}

#[test]
fn an_id_that_names_no_card_makes_none() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path()).args(["sync", "webapp"]).assert().success();
    let list = output(data.path(), &["task", "list", "--status", "all"]);
    assert!(list.contains("WA-4130"), "{list}");
    assert!(!list.contains("WA-41300"), "a ticket nobody handed over is not a card: {list}");
}

#[test]
fn task_show_json_is_the_documented_contract() {
    let data = tempfile::tempdir().unwrap();
    desk(data.path());
    rigger(data.path()).args(["sync", "webapp"]).assert().success();
    let json = show(data.path(), "WA-4130");
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/content/docs/reference/task.md")).unwrap();
    let mut found = std::collections::BTreeSet::new();
    keys(&json, &mut found);
    let missing: Vec<&String> = found.iter().filter(|k| !text.contains(&format!("`{k}`"))).collect();
    assert!(missing.is_empty(), "task.md does not name these fields of the JSON: {missing:?}\n{json:#}");
}

fn keys(value: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                out.insert(key.clone());
                keys(inner, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| keys(item, out)),
        _ => {}
    }
}
