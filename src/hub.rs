//! Reading a notes hub: the plan, the changelog and the decision log.
//!
//! The hub is markdown written by hand over months, so the parser is
//! deliberately forgiving. It recognises what the files of a hub actually
//! look like rather than a format someone promised to keep:
//!
//! - a stage heading is any heading whose first word is a version (`v0.4.0`),
//!   at whatever heading level, with the title after a separator;
//! - a closed stage in the changelog carries a date somewhere in its heading,
//!   in any of the shapes the hubs use;
//! - a checkbox line under a stage is one of its tasks;
//! - a decision is a dated heading in the decision log.
//!
//! What it cannot read it reports; nothing is skipped silently.

use std::path::Path;

use anyhow::{Context, Result};

/// A stage: one version, its tasks, and whether the hub says it shipped.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    pub shipped_on: Option<String>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub title: String,
    pub done: bool,
}

/// A dated entry of the decision log, kept whole: the body is prose the
/// owner wrote, and rigger stores it rather than re-interpreting it.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub date: String,
    pub title: String,
    pub body: String,
}

/// What a hub yielded, plus what it could not.
#[derive(Debug, Default)]
pub struct Hub {
    pub open_stages: Vec<Stage>,
    pub closed_stages: Vec<Stage>,
    pub decisions: Vec<Decision>,
    pub questions: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn read(dir: &Path) -> Result<Hub> {
    let mut hub = Hub::default();
    read_file(dir, "План.md", &mut hub, |text, hub| {
        hub.questions = parse_questions(text);
        hub.open_stages = parse_stages(text);
    })?;
    read_file(dir, "Изменения.md", &mut hub, |text, hub| {
        hub.closed_stages = parse_stages(text);
    })?;
    read_file(dir, "Решения.md", &mut hub, |text, hub| {
        hub.decisions = parse_decisions(text);
    })?;
    Ok(hub)
}

fn read_file(dir: &Path, name: &str, hub: &mut Hub, parse: impl FnOnce(&str, &mut Hub)) -> Result<()> {
    let path = dir.join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            parse(&text, hub);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            hub.warnings.push(format!("{name} is missing from {}", dir.display()));
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Splits a heading line into its level and text: `## v0.4.0 · Title`.
fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line[hashes..].strip_prefix(' ')?;
    Some((hashes, rest.trim()))
}

/// A version at the start of a heading: `v0.4.0`, `v1.0`, `v0.4.0-rc1`.
fn leading_version(text: &str) -> Option<&str> {
    let word = text.split_whitespace().next()?;
    let rest = word.strip_prefix('v')?;
    let mut parts = rest.split('.');
    let first = parts.next()?;
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // At least one dot, so a bare `v2` in prose is not mistaken for a version.
    parts.next()?;
    Some(word)
}

/// A date anywhere in a heading tail, in the shapes the hubs use:
/// `2026-09-03` and `03.09.2026`. Returned normalised to ISO.
fn trailing_date(text: &str) -> Option<String> {
    for word in text.split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.')) {
        let digits: Vec<&str> = word.split(['-', '.']).collect();
        if digits.len() != 3 || !digits.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
            continue;
        }
        if digits[0].len() == 4 {
            return Some(format!("{}-{}-{}", digits[0], digits[1], digits[2]));
        }
        if digits[2].len() == 4 {
            return Some(format!("{}-{}-{}", digits[2], digits[1], digits[0]));
        }
    }
    None
}

/// The title of a stage: what follows the version, minus the release note.
///
/// Hub headings read `v0.2.0 · Three forms — shipped 2026-09-03`; the title
/// is the middle, and the tail after the em dash is bookkeeping.
fn stage_title(text: &str, version: &str) -> Option<String> {
    let rest = text[version.len()..].trim();
    let rest = rest.trim_start_matches(['·', '-', '—', ':']).trim();
    let title = rest.split(['—', '–']).next().unwrap_or(rest).trim();
    (!title.is_empty()).then(|| title.to_string())
}

fn parse_stages(text: &str) -> Vec<Stage> {
    let mut stages: Vec<Stage> = Vec::new();
    let mut level = 0usize;
    for line in text.lines() {
        if let Some((depth, head)) = heading(line) {
            match leading_version(head) {
                Some(version) => {
                    stages.push(Stage {
                        version: version.to_string(),
                        title: stage_title(head, version),
                        shipped_on: trailing_date(&head[version.len()..]),
                        tasks: Vec::new(),
                    });
                    level = depth;
                }
                // A heading at the stage's level or above ends it: tasks under
                // "Backlog" or the next block do not belong to the last stage.
                None if depth <= level => level = 0,
                None => {}
            }
            continue;
        }
        if level == 0 {
            continue;
        }
        if let Some(task) = parse_task(line)
            && let Some(stage) = stages.last_mut()
        {
            stage.tasks.push(task);
        }
    }
    stages
}

fn parse_task(line: &str) -> Option<Task> {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("- ").or_else(|| rest.strip_prefix("* "))?;
    let (mark, title) = rest.split_at(rest.char_indices().nth(3).map(|(i, _)| i)?);
    let done = match mark {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let title = title.trim();
    (!title.is_empty()).then(|| Task {
        title: title.to_string(),
        done,
    })
}

/// Questions waiting for the owner: the list under that heading, until the
/// next heading. Numbered or bulleted; the placeholder line is not a question.
fn parse_questions(text: &str) -> Vec<String> {
    let mut questions = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some((_, head)) = heading(line) {
            inside = head.starts_with("Ждёт решения владельца");
            continue;
        }
        if !inside {
            continue;
        }
        let trimmed = line.trim();
        let item = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| {
            trimmed
                .split_once(". ")
                .filter(|(n, _)| n.bytes().all(|b| b.is_ascii_digit()))
                .map(|(_, rest)| rest)
        });
        if let Some(item) = item {
            let item = item.trim();
            if !item.is_empty() && item != "(пусто)" {
                questions.push(item.to_string());
            }
        }
    }
    questions
}

fn parse_decisions(text: &str) -> Vec<Decision> {
    let mut decisions: Vec<Decision> = Vec::new();
    let mut body = String::new();
    for line in text.lines() {
        if let Some((_, head)) = heading(line) {
            if let Some(last) = decisions.last_mut() {
                last.body = body.trim().to_string();
            }
            body.clear();
            if let Some(date) = trailing_date(head) {
                let title = head
                    .split_once(['·', '—'])
                    .map(|(_, t)| t.trim())
                    .filter(|t| !t.is_empty())
                    .unwrap_or(head)
                    .to_string();
                decisions.push(Decision {
                    date,
                    title,
                    body: String::new(),
                });
            }
            continue;
        }
        if !decisions.is_empty() && line.trim() != "---" {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(last) = decisions.last_mut() {
        last.body = body.trim().to_string();
    }
    decisions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_heading_starts_a_stage_at_any_level() {
        let stages = parse_stages("# Plan\n### v0.4.0 · Title\n- [ ] one\n## v0.5.0 · Next\n- [x] two\n");
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].version, "v0.4.0");
        assert_eq!(stages[0].title.as_deref(), Some("Title"));
        assert_eq!(
            stages[0].tasks,
            vec![Task {
                title: "one".into(),
                done: false
            }]
        );
        assert_eq!(
            stages[1].tasks,
            vec![Task {
                title: "two".into(),
                done: true
            }]
        );
    }

    #[test]
    fn a_heading_at_or_above_the_stage_level_ends_it() {
        // The backlog list must not become tasks of the last stage.
        let stages = parse_stages("## v0.1.0 · A\n- [ ] real\n## Бэклог без версии\n- [ ] someday\n");
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].tasks.len(), 1);
    }

    #[test]
    fn a_deeper_heading_does_not_end_a_stage() {
        let stages = parse_stages("## v0.1.0 · A\n- [ ] before\n### Подробности\n- [ ] after\n");
        assert_eq!(stages[0].tasks.len(), 2);
    }

    #[test]
    fn the_release_note_is_not_part_of_the_title() {
        let stages = parse_stages("## v0.2.0 · Три формы и лесенка — выпущена 2026-09-03\n");
        assert_eq!(stages[0].title.as_deref(), Some("Три формы и лесенка"));
        assert_eq!(stages[0].shipped_on.as_deref(), Some("2026-09-03"));
    }

    #[test]
    fn every_wording_of_a_closed_stage_yields_its_date() {
        for head in [
            "## v1.0.0 · A — закрыт 2026-08-12",
            "## v1.0.0 · A — выпущен 2026-08-12",
            "## v1.0.0 · A — выпущена 2026-08-12",
            "## v1.0.0 · A — 2026-08-12",
            "## v1.0.0 · A — 12.08.2026",
        ] {
            let stages = parse_stages(head);
            assert_eq!(stages[0].shipped_on.as_deref(), Some("2026-08-12"), "{head}");
        }
    }

    #[test]
    fn an_open_stage_has_no_date() {
        let stages = parse_stages("## v0.9.0 · Inbox and digest\n- [ ] a\n");
        assert_eq!(stages[0].shipped_on, None);
    }

    #[test]
    fn headings_without_a_version_are_not_stages() {
        // atlas numbers its sessions S118 and has no versions at all.
        let stages = parse_stages("## S118 · Транскрипт (2026-08-06)\n- [ ] a\n## Блок «Читаю»\n");
        assert!(stages.is_empty());
    }

    #[test]
    fn a_bare_major_in_prose_is_not_a_version() {
        assert_eq!(leading_version("v2 is the goal"), None);
        assert_eq!(leading_version("v0.4.0 · Title"), Some("v0.4.0"));
    }

    #[test]
    fn questions_are_read_until_the_next_heading() {
        let text = "# План\n\n## Ждёт решения владельца\n\n1. First thing.\n- Second thing.\n\n## Мажорная карта\n\n- not a question\n";
        assert_eq!(parse_questions(text), vec!["First thing.", "Second thing."]);
    }

    #[test]
    fn the_placeholder_is_not_a_question() {
        assert!(parse_questions("## Ждёт решения владельца\n\n- (пусто)\n").is_empty());
    }

    #[test]
    fn decisions_keep_their_prose() {
        let text = "# Журнал решений\n\n---\n\n## 2026-09-03 · Что на экране, то и оценивается\n\nBody line one.\n\nBody line two.\n\n## 2026-09-02 · Основание\n\nOnly line.\n";
        let decisions = parse_decisions(text);
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].date, "2026-09-03");
        assert_eq!(decisions[0].title, "Что на экране, то и оценивается");
        assert_eq!(decisions[0].body, "Body line one.\n\nBody line two.");
        assert_eq!(decisions[1].body, "Only line.");
    }

    #[test]
    fn an_undated_heading_is_not_a_decision() {
        assert!(parse_decisions("## Журнал\n\nprose\n").is_empty());
    }

    #[test]
    fn a_missing_file_is_a_warning_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("План.md"), "## v0.1.0 · A\n- [ ] task\n").unwrap();
        let hub = read(dir.path()).unwrap();
        assert_eq!(hub.open_stages.len(), 1);
        assert_eq!(hub.warnings.len(), 2, "Изменения.md and Решения.md are missing: {:?}", hub.warnings);
    }
}
