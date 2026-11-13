//! The line as a graph, driven through the binary.
//!
//! What the stage promises, in the order it promises it: a tie between two
//! projects is a fact of the record rather than a sentence in two hubs; a
//! project's screen prints who it stands beside; a decision can name the
//! principle it stands on, and a principle can be read across the whole
//! line; and a wish can carry the neighbour who asked for it.
//!
//! The drift checks are the reason all of this exists, so they are tested
//! against the shape that went unnoticed twice: one half of a pair tagged
//! while the other stood still.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

fn output(data: &Path, args: &[&str]) -> String {
    let out = rigger(data).args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

/// Two projects, each with a plan of two stages.
///
/// Synthetic throughout: the record this is written against holds the
/// owner's own line, and no part of it belongs in a repository.
fn two_projects(data: &Path) {
    rigger(data).arg("init").assert().success();
    for (name, stages) in [("alpha", ["v0.1.0", "v0.2.0"]), ("beta", ["v0.1.0", "v0.2.0"])] {
        let root = data.join(name);
        std::fs::create_dir_all(&root).unwrap();
        rigger(data).args(["project", "add"]).arg(&root).assert().success();
        let hub = data.join(format!("{name}-hub"));
        std::fs::create_dir_all(&hub).unwrap();
        let plan = format!("# План\n\n## {} · Первый\n\n- [ ] one\n\n## {} · Второй\n\n- [ ] two\n", stages[0], stages[1]);
        std::fs::write(hub.join("План.md"), plan).unwrap();
        rigger(data).args(["import", name, "--hub"]).arg(&hub).assert().success();
    }
}

/// Marks a version as shipped, the way a tag would.
fn ship(data: &Path, project: &str, version: &str, at: &str) {
    let db_path = data.join("profiles").join("line").join("rigger.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    let changed = db
        .execute(
            "UPDATE versions SET status = 'shipped', shipped_at = ?1, shipped_ts = ?2, shipped_source = 'tag' \
             WHERE name = ?3 AND project_id = (SELECT id FROM projects WHERE name = ?4)",
            rusqlite::params![at, format!("{at}T09:00:00Z"), version, project],
        )
        .unwrap();
    assert_eq!(changed, 1, "no version {version} of {project} to ship");
}

#[test]
fn a_tie_is_recorded_once_and_reads_the_same_from_both_ends() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());

    let added = output(data.path(), &["link", "add", "alpha@v0.2.0", "beta@v0.2.0", "--note", "one capability"]);
    assert!(added.contains("alpha v0.2.0 <-> beta v0.2.0 (pair)"), "{added}");

    // Recorded once: stating it a second time changes nothing rather than
    // piling up a second row that would report the same drift twice.
    let again = output(data.path(), &["link", "add", "alpha@v0.2.0", "beta@v0.2.0"]);
    assert!(again.contains("already recorded"), "{again}");
    assert_eq!(output(data.path(), &["link", "list"]).lines().count(), 1);

    // And it reads from the far end without a second row being written.
    let from_beta = output(data.path(), &["link", "list", "beta"]);
    assert!(from_beta.contains("beta v0.2.0 <-> alpha v0.2.0"), "{from_beta}");
}

/// The double the list used to print: the same pair bare and with versions.
/// Both are kept - the anchors are part of a link on purpose - but recording
/// the second names the first, from whichever end and in whichever order.
#[test]
fn a_tie_recorded_bare_and_with_versions_is_named_when_the_second_comes() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());

    let first = output(data.path(), &["link", "add", "alpha", "beta"]);
    assert!(!first.contains("tied this way already"), "{first}");

    // Written from the other end, with versions: still the same tie.
    let second = output(data.path(), &["link", "add", "beta@v0.2.0", "alpha@v0.1.0"]);
    assert!(second.contains("[2] beta v0.2.0 <-> alpha v0.1.0 (pair)"), "{second}");
    assert!(second.contains("beta and alpha are tied this way already:"), "{second}");
    assert!(second.contains("[1] beta <-> alpha, without versions"), "{second}");
    assert_eq!(output(data.path(), &["link", "list"]).lines().count(), 2);

    // The next step of the pair is history, not a double of the anchored
    // link; only the bare tie, which contradicts neither, is named.
    let third = output(data.path(), &["link", "add", "alpha@v0.2.0", "beta@v0.2.0"]);
    assert!(third.contains("[1] alpha <-> beta, without versions"), "{third}");
    assert!(!third.contains("[2]"), "{third}");

    // A consumer between the same two says something else.
    let uses = output(data.path(), &["link", "add", "alpha", "beta", "--kind", "consumer"]);
    assert!(!uses.contains("tied this way already"), "{uses}");

    let json: serde_json::Value = serde_json::from_str(&output(data.path(), &["link", "add", "alpha", "beta", "--json"])).unwrap();
    assert_eq!(json["changed"], false);
    let ids: Vec<i64> = json["alike"].as_array().unwrap().iter().map(|l| l["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, vec![2, 3]);
}

/// What one end calls a consumer the other calls a donor: one row, two
/// readings. A second row saying the same thing from the other side would
/// be a second truth, and one of them would go stale.
#[test]
fn a_directed_tie_turns_about_when_read_from_the_other_end() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["link", "add", "alpha", "beta", "--kind", "consumer"])
        .assert()
        .success();

    assert!(output(data.path(), &["link", "list", "alpha"]).contains("consumer  alpha -> beta"));
    assert!(output(data.path(), &["link", "list", "beta"]).contains("donor     beta -> alpha"));
}

/// The double the live record would have taken: the tie written again from
/// its other end. A donor is the consumer it mirrors, and a pair is a pair in
/// either order - already recorded, not a second row.
#[test]
fn a_tie_written_from_its_other_end_is_the_tie_already_recorded() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    output(data.path(), &["link", "add", "alpha", "beta", "--kind", "consumer"]);
    let again = output(data.path(), &["link", "add", "beta", "alpha", "--kind", "donor", "--note", "beta's reader"]);
    assert!(again.contains("[1] already recorded: beta -> alpha (donor)"), "{again}");
    assert!(!again.contains("tied this way already"), "{again}");
    // The note said from the far end is kept on the one row.
    assert_eq!(output(data.path(), &["link", "list"]).trim(), "[1] consumer  alpha -> beta  beta's reader");

    output(data.path(), &["link", "add", "beta@v0.2.0", "alpha@v0.1.0"]);
    let pair = output(data.path(), &["link", "add", "alpha@v0.1.0", "beta@v0.2.0"]);
    assert!(pair.contains("[2] already recorded: alpha v0.1.0 <-> beta v0.2.0 (pair)"), "{pair}");
    assert_eq!(output(data.path(), &["link", "list"]).lines().count(), 2);
}

/// A record from before ties were kept one way round: its donors become the
/// consumers they mirror and its pairs turn to one order, under their own ids,
/// and two rows that were one tie become one, the older, with the note.
#[test]
fn a_tie_kept_twice_from_both_ends_is_one_after_the_migration() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    let db_path = data.path().join("profiles").join("line").join("rigger.db");
    {
        let db = rusqlite::Connection::open(&db_path).unwrap();
        db.execute_batch(
            "DROP TABLE links;
             CREATE TABLE links (
                 id INTEGER PRIMARY KEY,
                 kind TEXT NOT NULL CHECK (kind IN ('pair', 'consumer', 'donor')),
                 from_project INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 from_version INTEGER REFERENCES versions(id) ON DELETE SET NULL,
                 to_project INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 to_version INTEGER REFERENCES versions(id) ON DELETE SET NULL,
                 note TEXT,
                 created_at TEXT NOT NULL,
                 CHECK (from_project <> to_project)
             );
             CREATE UNIQUE INDEX links_once ON links (
                 kind, from_project, COALESCE(from_version, -1), to_project, COALESCE(to_version, -1)
             );
             PRAGMA user_version = 37;",
        )
        .unwrap();
        let alpha: i64 = db.query_row("SELECT id FROM projects WHERE name = 'alpha'", [], |r| r.get(0)).unwrap();
        let beta: i64 = db.query_row("SELECT id FROM projects WHERE name = 'beta'", [], |r| r.get(0)).unwrap();
        let (low, high) = (alpha.min(beta), alpha.max(beta));
        let rows = [
            // One tie twice: as a consumer, and from the far end as a donor
            // that alone carries the note.
            (3, "consumer", alpha, beta, None),
            (5, "donor", beta, alpha, Some("the reader")),
            // A pair kept in the higher-first order, alone: turned round.
            (7, "pair", high, low, Some("one capability")),
        ];
        for (id, kind, from, to, note) in rows {
            db.execute(
                "INSERT INTO links (id, kind, from_project, to_project, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, '2026-10-01T00:00:00Z')",
                rusqlite::params![id, kind, from, to, note],
            )
            .unwrap();
        }
    }

    let list = output(data.path(), &["link", "list", "alpha"]);
    assert_eq!(list.lines().count(), 2, "{list}");
    assert!(list.contains("[3] consumer  alpha -> beta  the reader"), "{list}");
    assert!(list.contains("[7] pair      alpha <-> beta  one capability"), "{list}");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    let kept: Vec<(i64, String, bool)> = db
        .prepare("SELECT id, kind, from_project < to_project FROM links ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(kept[0].0, 3);
    assert_eq!(kept[1], (7, "pair".to_string(), true));
    // And the table refuses a donor, or a pair the other way round.
    assert!(
        db.execute("INSERT INTO links (kind, from_project, to_project, created_at) SELECT 'donor', 1, 2, 'x'", [])
            .is_err()
    );
}

/// A version the project does not have is a typo. Anchored silently to
/// nothing, the link could never drift - and a warning that cannot fire is
/// worse than no warning, because it reads as one that is watching.
#[test]
fn a_version_the_project_does_not_have_is_refused() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["link", "add", "alpha@v9.9.9", "beta"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no version v9.9.9"));
    assert!(output(data.path(), &["link", "list"]).contains("no links"));
}

/// The hubs of this line do not agree on how to spell a version - one
/// writes `v1.13` where another writes `v0.23.0` - so an anchor is matched
/// by value, the way `why` already reads one. Found on the live record the
/// first time a real pair was anchored.
#[test]
fn a_version_is_anchored_by_value_however_the_hub_spells_it() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    let added = output(data.path(), &["link", "add", "alpha@v0.2", "beta@0.2.0"]);
    // Recorded against the version as the record spells it, not as it was typed.
    assert!(added.contains("alpha v0.2.0 <-> beta v0.2.0"), "{added}");

    // And it is the same anchor, so stating it the other way changes nothing.
    assert!(output(data.path(), &["link", "add", "alpha@v0.2.0", "beta@v0.2.0"]).contains("already recorded"));
    assert_eq!(output(data.path(), &["link", "list"]).lines().count(), 1);
}

#[test]
fn a_project_cannot_be_tied_to_itself() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["link", "add", "alpha", "alpha"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("a link joins two projects"));
}

/// The shape that went unnoticed twice: one half of a pair tagged while
/// the other stood still. Nothing in the record said so, because nothing
/// read two projects at once.
#[test]
fn a_half_that_shipped_without_its_other_half_is_reported() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["link", "add", "alpha@v0.2.0", "beta@v0.2.0", "--note", "the shared door"])
        .assert()
        .success();

    // In step while neither is out.
    assert!(output(data.path(), &["link", "drift"]).contains("both halves in step"));

    ship(data.path(), "alpha", "v0.2.0", "2026-09-11");
    let drift = output(data.path(), &["link", "drift"]);
    assert!(drift.contains("alpha v0.2.0 shipped without beta v0.2.0"), "{drift}");
    assert!(drift.contains("the shared door"), "the note is carried: {drift}");

    // And the queue says it without being asked about links.
    let queue = output(data.path(), &["queue"]);
    assert!(queue.contains("Pairs out of step"), "{queue}");
    assert!(queue.contains("alpha v0.2.0 shipped without beta"), "{queue}");

    // Both halves out is the pair working, and the warning goes.
    ship(data.path(), "beta", "v0.2.0", "2026-09-11");
    assert!(output(data.path(), &["link", "drift"]).contains("both halves in step"));
    assert!(!output(data.path(), &["queue"]).contains("Pairs out of step"));
}

/// A consumer that has moved on past the version of what it draws on is a
/// consumer working. Reported every week it would be a line nobody reads.
#[test]
fn a_consumer_running_ahead_raises_nothing() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["link", "add", "alpha@v0.2.0", "beta@v0.2.0", "--kind", "consumer"])
        .assert()
        .success();
    ship(data.path(), "alpha", "v0.2.0", "2026-09-11");
    assert!(output(data.path(), &["link", "drift"]).contains("both halves in step"));
}

/// The screen of a project prints who it stands beside, and tells the
/// three kinds apart: drawing on a product is not standing beside it.
#[test]
fn the_screen_prints_the_neighbours_under_three_headings() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    let gamma = data.path().join("gamma");
    std::fs::create_dir_all(&gamma).unwrap();
    rigger(data.path()).args(["project", "add"]).arg(&gamma).assert().success();

    rigger(data.path()).args(["link", "add", "alpha@v0.2.0", "beta@v0.2.0"]).assert().success();
    rigger(data.path())
        .args(["link", "add", "alpha", "gamma", "--kind", "consumer"])
        .assert()
        .success();

    let screen = output(data.path(), &["show", "alpha"]);
    assert!(screen.contains("Paired with:"), "{screen}");
    assert!(screen.contains("beta v0.2.0 (here: v0.2.0)"), "{screen}");
    assert!(screen.contains("Draws on:"), "{screen}");
    assert!(screen.contains("gamma"), "{screen}");
    // Gamma's own screen states the other side of the same one row.
    assert!(output(data.path(), &["show", "gamma"]).contains("Drawn on by:"));
    // A project tied to nothing prints no heading at all.
    assert!(!output(data.path(), &["show", "beta"]).contains("Draws on:"));
}

#[test]
fn a_tie_can_be_forgotten_by_the_id_the_list_prints() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path()).args(["link", "add", "alpha", "beta"]).assert().success();
    let listed = output(data.path(), &["link", "list"]);
    let id = listed.split(']').next().unwrap().trim_start_matches('[');
    rigger(data.path()).args(["link", "remove", id]).assert().success();
    assert!(output(data.path(), &["link", "list"]).contains("no links"));
    rigger(data.path())
        .args(["link", "remove", id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no link"));
}

/// A principle is read across the line rather than one project at a time:
/// six decisions in six products look like six opinions until they are
/// read as one thread.
#[test]
fn a_principle_gathers_its_decisions_from_every_project() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args([
            "note",
            "alpha",
            "broke it outright",
            "--kind",
            "decision",
            "--principle",
            "no users, no compatibility",
        ])
        .assert()
        .success();
    rigger(data.path())
        .args([
            "note",
            "beta",
            "dropped the old column",
            "--kind",
            "decision",
            "--principle",
            "no users, no compatibility",
        ])
        .assert()
        .success();
    rigger(data.path())
        .args(["note", "beta", "measured the screen", "--kind", "decision", "--principle", "measure the screen"])
        .assert()
        .success();

    let thread = output(data.path(), &["why", "--principle", "no users, no compatibility"]);
    assert!(thread.contains("2 decisions across 2 projects"), "{thread}");
    assert!(thread.contains("broke it outright") && thread.contains("dropped the old column"), "{thread}");
    assert!(!thread.contains("measured the screen"), "a second principle is a second thread: {thread}");

    // The vocabulary is what has been used, ordered by how often.
    let names = output(data.path(), &["why", "--principles"]);
    assert!(names.contains("no users, no compatibility  —  2 decisions"), "{names}");
    assert!(names.contains("measure the screen  —  1 decision"), "{names}");

    // One project's share of a principle, when that is what is wanted.
    let one = output(data.path(), &["why", "alpha", "--principle", "no users, no compatibility"]);
    assert!(one.contains("1 decision across 1 project"), "{one}");
}

/// A name nobody has used is far likelier a misspelling than a new
/// principle, so the answer lists the ones that exist.
#[test]
fn an_unknown_principle_shows_the_ones_the_record_knows() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["note", "alpha", "a decision", "--kind", "decision", "--principle", "measure the screen"])
        .assert()
        .success();
    let answer = output(data.path(), &["why", "--principle", "mesure the screen"]);
    assert!(answer.contains("Nothing stands on"), "{answer}");
    assert!(answer.contains("measure the screen (1)"), "{answer}");
}

/// A principle names what a decision stands on. A change or a finding is
/// something that happened, not something believed - and a principle
/// silently dropped is one the owner thinks was recorded.
#[test]
fn only_a_decision_can_stand_on_a_principle() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["note", "alpha", "a finding", "--kind", "finding", "--principle", "measure the screen"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("a principle belongs to a decision"));
}

/// An order from a neighbour is a wish like any other - same row, same
/// `resolve`, same competition for a stage. What changes is that somebody
/// is waiting for it, and both screens say who.
#[test]
fn a_wish_from_a_neighbour_names_who_is_waiting() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["wish", "beta", "a gate for the docs address", "--from", "alpha"])
        .assert()
        .success()
        .stdout(predicate::str::contains("from alpha"));

    // On the line in the packet, not in a queue of its own.
    let packet = output(data.path(), &["context", "beta"]);
    assert!(packet.contains("(alpha asks) a gate for the docs address"), "{packet}");

    // As a group of its own in the inbox, where the questions are.
    let inbox = output(data.path(), &["inbox"]);
    assert!(inbox.contains("Neighbours are asking for:"), "{inbox}");
    assert!(inbox.contains("alpha"), "{inbox}");

    // And on the screen of the project being asked.
    assert!(output(data.path(), &["show", "beta"]).contains("Neighbours are asking for:"));

    // Sorted the same way any wish is, and then it is gone from all three.
    let id = packet
        .lines()
        .find(|l| l.contains("alpha asks"))
        .and_then(|l| l.split(']').next())
        .map(|l| l.trim_start_matches("- ["))
        .unwrap()
        .to_string();
    rigger(data.path()).args(["resolve", "beta", &id]).assert().success();
    assert!(!output(data.path(), &["inbox"]).contains("Neighbours are asking"));
    assert!(!output(data.path(), &["show", "beta"]).contains("Neighbours are asking"));
}

#[test]
fn a_neighbour_that_does_not_exist_is_refused() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path())
        .args(["wish", "beta", "something", "--from", "nowhere"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nowhere"));
    // And the wish is not recorded under a neighbour that could not be
    // found: a refused call leaves nothing behind.
    assert!(!output(data.path(), &["context", "beta"]).contains("something"));
}

/// The packet is read at the start of every session and has a budget the
/// gate holds it to. Adding a name to a wish line must not be paid for by
/// dropping something else.
#[test]
fn the_neighbour_on_a_wish_line_costs_the_packet_almost_nothing() {
    let data = tempfile::tempdir().unwrap();
    two_projects(data.path());
    rigger(data.path()).args(["wish", "beta", "a plain wish"]).assert().success();
    let plain = output(data.path(), &["context", "beta"]).len();
    rigger(data.path())
        .args(["wish", "beta", "an asked wish", "--from", "alpha"])
        .assert()
        .success();
    let with_asker = output(data.path(), &["context", "beta"]);
    assert!(with_asker.contains("(alpha asks)"), "{with_asker}");
    // The wish itself is the cost; the attribution is a dozen characters.
    assert!(
        with_asker.len() < plain + "- [99] (alpha asks) an asked wish\n".len() + 8,
        "{}",
        with_asker.len()
    );
}
