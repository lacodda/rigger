//! The wishes file stops being a place the record has to be told about.
//!
//! It used to be read by a person at the start of every session and emptied
//! by hand. Now `import` takes what is in it, a wish arrives through
//! `rigger wish` or the assistant's tool, and `doctor` says so if the file
//! fills up again.
//!
//! Most of these check what is *not* a wish. Every hub of this line keeps
//! the file, nearly all of them empty, and each shape of emptiness read as
//! a wish at some point while this was written.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn rigger(data: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rigger").unwrap();
    cmd.env("RIGGER_DATA_DIR", data);
    cmd
}

/// A project with a hub that has the one file `import` insists on.
fn project(data: &Path) -> std::path::PathBuf {
    let root = data.join("sample");
    std::fs::create_dir_all(&root).unwrap();
    rigger(data).arg("init").assert().success();
    rigger(data).args(["project", "add"]).arg(&root).assert().success();
    let hub = data.join("hub");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(hub.join("План.md"), "# План\n\n## v0.2.0 · Второй\n\n- [ ] сделать одно\n").unwrap();
    hub
}

fn wishes(data: &Path, hub: &Path, text: &str) -> Vec<String> {
    std::fs::write(hub.join("Хотелки.md"), text).unwrap();
    rigger(data).args(["import", "sample", "--hub"]).arg(hub).assert().success();
    let out = rigger(data).args(["context", "sample"]).output().unwrap();
    let packet = String::from_utf8(out.stdout).unwrap();
    let Some((_, rest)) = packet.split_once("## Wishes, not yet sorted\n") else {
        return Vec::new();
    };
    rest.split("\n## ")
        .next()
        .unwrap_or("")
        .lines()
        .filter(|l| l.starts_with("- ["))
        .map(str::to_string)
        .collect()
}

/// The template as every hub of this line keeps it: a title, one line
/// saying what the file is for, a rule, and a placeholder.
const EMPTY: &str = "\
# Хотелки

Inbox пожеланий владельца. Разбирается в начале каждой сессии: пункты уходят в План, файл очищается до этого шаблона.

---

- (пусто)
";

#[test]
fn the_empty_template_holds_no_wishes() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    assert!(wishes(data.path(), &hub, EMPTY).is_empty(), "the template is not a wish");
}

/// The same file with the placeholder taken out, which is how several of
/// these hubs are left.
#[test]
fn the_template_without_even_a_placeholder_holds_no_wishes() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let text = EMPTY.rsplit_once("---").map(|(head, _)| format!("{head}---\n")).unwrap();
    assert!(wishes(data.path(), &hub, &text).is_empty(), "{text}");
}

/// Emptying the file, these hubs leave a line saying where the wishes went.
/// Taking that in would put settled things back into a packet, where
/// somebody would sort them a second time.
#[test]
fn a_note_saying_the_wishes_were_already_sorted_is_not_a_wish() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let text = "\
# Хотелки

Inbox пожеланий владельца.

---

*Разобрано 11.09.2026: знак по уровням — дефектом в v0.11.0 «Открытые двери».*
";
    assert!(wishes(data.path(), &hub, text).is_empty(), "a note about the past is not a wish");
}

/// A real wish runs to several paragraphs - what it is, where, and how to
/// fix it - so a blank line is no boundary. Split on one, the only
/// non-empty wishes file of this line came back as six wishes.
#[test]
fn a_wish_of_several_paragraphs_stays_one_wish() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let text = "\
# Хотелки

Inbox пожеланий владельца.

---

**03.09.2026 · Знак в вебе собран не по правилу уровней** (найдено сверкой линейки)

Правило линейки: **S ≤27px, M 28–63px, L ≥64px**. Нашлось два места:

- `favicon.ico` — 32px и 48px сплошная заливка
- `icon-32.png` — 32px сплошная заливка

⚠️ **По `icon-32.png` нужно решение, а не автоматическая правка.**

**Как чинить:** образец — `export-assets.mjs`, гейт проверен отвержением.
";
    let found = wishes(data.path(), &hub, text);
    assert_eq!(found.len(), 1, "one wish, not one per paragraph: {found:#?}");
    assert!(found[0].contains("Знак в вебе"), "{found:#?}");
}

/// Two wishes are two, and what tells them apart is the dated bold opening
/// each one starts with - not bold text, which appears inside a wish too.
#[test]
fn two_dated_wishes_are_two() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let text = "\
# Хотелки

Inbox пожеланий владельца.

---

**03.09.2026 · Первое** что-то одно.

**Как чинить:** вот так.

**05.09.2026 · Второе** что-то другое.
";
    let found = wishes(data.path(), &hub, text);
    assert_eq!(found.len(), 2, "{found:#?}");
}

/// Reading the same file twice does not pile up copies: a wish is
/// identified by its text, the way a question is.
#[test]
fn reading_the_file_twice_does_not_double_the_wishes() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());
    let text = "# Хотелки\n\n---\n\n**03.09.2026 · Первое** что-то одно.\n";
    assert_eq!(wishes(data.path(), &hub, text).len(), 1);
    assert_eq!(wishes(data.path(), &hub, text).len(), 1, "read again, still one");
}

/// The file is no longer read at every session, so the one thing the record
/// owes it is to say when something is waiting in it.
#[test]
fn doctor_says_when_something_is_left_in_the_file() {
    let data = tempfile::tempdir().unwrap();
    let hub = project(data.path());

    std::fs::write(hub.join("Хотелки.md"), EMPTY).unwrap();
    rigger(data.path()).args(["import", "sample", "--hub"]).arg(&hub).assert().success();
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("wishes left in").not());

    std::fs::write(hub.join("Хотелки.md"), "# Хотелки\n\n---\n\n**03.09.2026 · Первое** что-то одно.\n").unwrap();
    rigger(data.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("wishes left in").and(predicate::str::contains("sample")));
}
