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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stage {
    pub version: String,
    pub title: Option<String>,
    pub shipped_on: Option<String>,
    pub tasks: Vec<Task>,
    /// Everything written under the heading that is not a task.
    ///
    /// Kept because the record has to hold it before a hub can be generated
    /// from the record: measured on this project's own changelog, 88% of it
    /// is this prose, and an export that did not know it would delete it.
    pub notes: String,
    /// How deep its heading sat (`##` is 2). A hub nests stages under block
    /// headings, and writing them all back at one level would flatten the
    /// document into a list.
    pub depth: usize,
    /// The prose written after the tasks, when there was any.
    ///
    /// A stage is explained before its list and concluded after it, and
    /// 123 of the stages in this line's hubs do both - more than do only
    /// the first. One field and a flag for which side it fell on could
    /// hold either half but not both, so the closing line of every such
    /// stage was moved above its own list on the way back out.
    pub notes_after: String,
    /// The heading exactly as it was written, after the `#` marks.
    ///
    /// An export cannot compose this. One hub writes `— выпущен 2026-09-03`
    /// and the next `— выпущена`, because the word agrees with whatever
    /// noun the owner had in mind; recomposing it rewrote three headings of
    /// a real hub on the first live run. What the record can do is keep the
    /// line and put it back.
    pub heading: String,
    /// How many runs of prose stood before this stage in its file.
    ///
    /// A plan groups its stages under block headings, and the blocks are
    /// prose. Without this the export had nowhere to put a stage but after
    /// all the prose, which piled every stage of a real plan below every
    /// block heading and lost the grouping entirely.
    pub after_prose: usize,
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

/// A run of prose in a hub file that belongs to no stage: a preamble, a
/// map, a heading that groups stages into blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct Prose {
    pub file: String,
    /// Where it sat among the file's other runs, so it can be put back.
    pub position: usize,
    /// The heading it came under, kept whole (`## Блок «Владелец»`).
    pub heading: Option<String>,
    pub body: String,
}

/// A diary entry: one sitting as the hub records it.
#[derive(Debug, Clone, PartialEq)]
pub struct DiaryEntry {
    pub date: String,
    /// The heading exactly as it was written, after the `##`.
    ///
    /// Not "the part after the date": a hub writes
    /// `2026-09-03 (ночь, позже) · v0.2.2`, where the parenthetical follows
    /// the date with a space and the title with a separator. Splitting it
    /// and putting it back together inserted a separator that was never
    /// there, on every entry of a busy day.
    pub heading: Option<String>,
    pub body: String,
    /// Whether a horizontal rule stood between this entry and the next. A
    /// hub that writes one writes it before every entry but the first.
    pub followed_by_rule: bool,
}

/// What a hub yielded, plus what it could not.
#[derive(Debug, Default)]
pub struct Hub {
    pub open_stages: Vec<Stage>,
    pub closed_stages: Vec<Stage>,
    pub decisions: Vec<Decision>,
    pub questions: Vec<String>,
    pub diary: Vec<DiaryEntry>,
    pub prose: Vec<Prose>,
    pub warnings: Vec<String>,
}

pub fn read(dir: &Path) -> Result<Hub> {
    let mut hub = Hub::default();
    read_file(dir, "План.md", &mut hub, |text, hub| {
        hub.questions = parse_questions(text);
        hub.open_stages = parse_stages(text);
        hub.prose.extend(parse_prose(text, "План.md"));
    })?;
    read_file(dir, "Изменения.md", &mut hub, |text, hub| {
        hub.closed_stages = parse_stages(text);
        hub.prose.extend(parse_prose(text, "Изменения.md"));
    })?;
    read_file(dir, "Решения.md", &mut hub, |text, hub| {
        hub.decisions = parse_decisions(text);
    })?;
    read_file(dir, "Дневник.md", &mut hub, |text, hub| {
        hub.diary = parse_diary(text);
        hub.prose.extend(parse_prose(text, "Дневник.md"));
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

/// The version a stage actually shipped as, when its heading names one after
/// the word "выпущен": `v1.9 · Title — закрыт 2026-09-03, выпущен **v1.9.0**`.
fn released_version(tail: &str) -> Option<&str> {
    let at = tail.find("выпущен")?;
    let rest = &tail[at..];
    rest.split(|c: char| c.is_whitespace() || c == '*')
        .filter_map(|word| leading_version(word).filter(|v| v.len() == word.len()))
        .next()
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
    let mut notes = String::new();
    // How many runs of prose have gone by, counted exactly as `parse_prose`
    // counts them so the two views of one file agree about where a stage
    // sits among the blocks.
    let runs = prose_positions(text);
    for (at_line, line) in text.lines().enumerate() {
        if let Some((depth, head)) = heading(line) {
            match leading_version(head) {
                Some(version) => {
                    settle(&mut stages, &mut notes);
                    let tail = &head[version.len()..];
                    stages.push(Stage {
                        // A stage may be numbered differently from the release
                        // it became: kasl writes `v1.9 · Title - closed, shipped
                        // **v1.9.0**`. The tag is what git will confirm, so the
                        // released number wins when the heading names one.
                        version: released_version(tail).unwrap_or(version).to_string(),
                        title: stage_title(head, version),
                        shipped_on: trailing_date(tail),
                        tasks: Vec::new(),
                        notes: String::new(),
                        depth,
                        notes_after: String::new(),
                        heading: head.to_string(),
                        after_prose: runs[at_line],
                    });
                    level = depth;
                }
                // A heading at the stage's level or above ends it: tasks under
                // "Backlog" or the next block do not belong to the last stage.
                None if depth <= level => {
                    settle(&mut stages, &mut notes);
                    level = 0;
                }
                None => {}
            }
            continue;
        }
        if level == 0 {
            continue;
        }
        if let Some(task) = parse_task(line) {
            if let Some(stage) = stages.last_mut() {
                // The first task closes the opening prose. Whatever was
                // collected up to here explains the stage; whatever comes
                // after its list concludes it, and both halves are kept,
                // because most stages of this line have both.
                if stage.tasks.is_empty() && !notes.trim().is_empty() {
                    stage.notes = notes.trim().to_string();
                    notes.clear();
                }
                stage.tasks.push(task);
            }
            continue;
        }
        // Everything under a stage that is not a task is the prose written
        // about it, and the record has to hold it for an export to put it
        // back. Kept verbatim, blank lines and all, so the shape survives.
        notes.push_str(line);
        notes.push('\n');
    }
    settle(&mut stages, &mut notes);
    stages
}

/// Hands the collected prose to the stage it was written under.
///
/// Which half it becomes follows from the list: a stage that has already
/// taken its opening prose, or that has tasks, is being handed its closing
/// prose; one that has neither is being handed its opening.
fn settle(stages: &mut [Stage], notes: &mut String) {
    // Only when there is something to hand over. A stage is settled twice -
    // once by the block heading that closes it, once by the next stage's
    // heading - and an unconditional write let the second, empty call wipe
    // what the first had just stored. That lost the closing line of the
    // last stage of every block in a real plan.
    if !notes.trim().is_empty()
        && let Some(stage) = stages.last_mut()
    {
        let settled = notes.trim().to_string();
        match stage.tasks.is_empty() && stage.notes.is_empty() {
            true => stage.notes = settled,
            false => stage.notes_after = settled,
        }
    }
    notes.clear();
}

/// The diary, newest entry first: `## 2026-09-05 · Title` and its prose.
fn parse_diary(text: &str) -> Vec<DiaryEntry> {
    let mut entries: Vec<DiaryEntry> = Vec::new();
    let mut body = String::new();
    for line in text.lines() {
        if let Some((depth, head)) = heading(line) {
            // Only the entry headings themselves start an entry; a deeper
            // heading inside one is part of what was written.
            if depth <= 2
                && let Some(date) = trailing_date(head)
            {
                if let Some(last) = entries.last_mut() {
                    last.body = body.trim().to_string();
                }
                body.clear();
                entries.push(DiaryEntry {
                    heading: Some(head.to_string()),
                    date,
                    body: String::new(),
                    followed_by_rule: false,
                });
                continue;
            }
        }
        if !entries.is_empty() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(last) = entries.last_mut() {
        last.body = body.trim().to_string();
    }
    // A rule between entries belongs to neither: it separates them. Kept off
    // the body as a flag so the export can put it back where it stood.
    for entry in entries.iter_mut() {
        if let Some(rest) = entry.body.strip_suffix("---") {
            entry.body = rest.trim_end().to_string();
            entry.followed_by_rule = true;
        }
    }
    entries
}

/// Prose of a file that belongs to no stage and no entry: the preamble, and
/// the headings that group stages into blocks.
///
/// Kept by position so the export can put each run back where it was rather
/// than piling it all at the top.
fn parse_prose(text: &str, file: &str) -> Vec<Prose> {
    prose_and_positions(text, file).0
}

/// How many runs of prose stood before each line of a file.
///
/// The same walk as `parse_prose`, so the two views of one file cannot
/// disagree about where a stage sits among the blocks. Written as one
/// function with two outputs rather than two functions with one rule.
fn prose_positions(text: &str) -> Vec<usize> {
    prose_and_positions(text, "").1
}

fn prose_and_positions(text: &str, file: &str) -> (Vec<Prose>, Vec<usize>) {
    let mut runs: Vec<Prose> = Vec::new();
    let mut positions: Vec<usize> = Vec::new();
    let mut body = String::new();
    let mut heading_now: Option<String> = None;
    let mut inside_entry = false;

    fn flush(heading: Option<String>, body: &mut String, runs: &mut Vec<Prose>, file: &str) {
        let text = body.trim();
        if !text.is_empty() || heading.is_some() {
            runs.push(Prose {
                file: file.to_string(),
                position: runs.len(),
                heading,
                body: text.to_string(),
            });
        }
        body.clear();
    }

    for line in text.lines() {
        // How many runs stand before this line once the one being collected
        // is closed. A stage heading closes the run it stands under, so that
        // pending run counts: the stage belongs after it, not before it.
        positions.push(runs.len() + usize::from(heading_now.is_some() || !body.trim().is_empty()));
        // The mark an export leaves is bookkeeping, not content. Captured as
        // prose it would be written back with a second mark above it, and
        // the record would gain a run on every round trip.
        if line.trim() == crate::export::MARK {
            continue;
        }
        if let Some((depth, head)) = heading(line) {
            // A heading starts an entry only if something else will capture
            // it: a version heading becomes a stage, and a dated one in the
            // diary becomes an entry. A dated heading that is *not* a
            // version - `## Этап 0 · Зафиксировать продукт — закрыт
            // 2026-09-02` in a changelog - is captured by nobody, so
            // treating it as an entry dropped the whole section.
            let is_diary = file.contains("Дневник");
            let starts_entry = leading_version(head).is_some() || (is_diary && depth <= 2 && trailing_date(head).is_some());
            if starts_entry {
                if !inside_entry {
                    flush(heading_now.take(), &mut body, &mut runs, file);
                }
                inside_entry = true;
                heading_now = None;
                body.clear();
                continue;
            }
            // A heading that starts nothing is itself part of the prose, and
            // it closes whatever run came before it.
            if !inside_entry {
                flush(heading_now.take(), &mut body, &mut runs, file);
            }
            inside_entry = false;
            heading_now = Some(line.to_string());
            body.clear();
            continue;
        }
        if !inside_entry {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !inside_entry {
        flush(heading_now.take(), &mut body, &mut runs, file);
    }
    (runs, positions)
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
        // The checkbox forms first: `- [ ] ` is a bullet with a box, and
        // stripping only the bullet left the box standing in front of the
        // question, so an export wrote it twice.
        let item = ["- [ ] ", "- [x] ", "- [X] ", "- ", "* "]
            .iter()
            .find_map(|lead| trimmed.strip_prefix(lead))
            .or_else(|| {
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
    fn a_stage_takes_the_number_it_shipped_as() {
        // kasl numbers stages and releases apart; git will carry the release.
        let stages = parse_stages("## v1.9 · Очередь и backfill — закрыт 2026-09-03, выпущен **v1.9.0**\n");
        assert_eq!(stages[0].version, "v1.9.0");
        assert_eq!(stages[0].title.as_deref(), Some("Очередь и backfill"));
        assert_eq!(stages[0].shipped_on.as_deref(), Some("2026-09-03"));
    }

    #[test]
    fn a_stage_without_a_separate_release_keeps_its_own_number() {
        let stages = parse_stages("## v0.2.0 · Три формы — выпущена 2026-09-03\n");
        assert_eq!(stages[0].version, "v0.2.0");
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

    /// A stage is settled twice - by the block heading that closes it and
    /// by the next stage's heading - so an unconditional hand-over let the
    /// second, empty one wipe what the first had stored. That lost the
    /// closing line of the last stage of every block in a real plan.
    #[test]
    fn a_stage_keeps_the_prose_that_followed_its_tasks() {
        let text = "# План

# Блок A

## v0.1.0 · One

- [ ] task one

**Результат:** первое.

                    # Блок B

## v0.2.0 · Two

- [ ] task two

**Результат:** второе.
";
        let stages = parse_stages(text);
        assert_eq!(stages.len(), 2);
        // Both, not just the last: the first is the one the defect ate.
        // It followed the tasks, so it is the closing half - an export
        // reading the opening half would move it above its own list.
        assert!(stages[0].notes_after.contains("первое"), "{:?}", stages[0]);
        assert!(stages[1].notes_after.contains("второе"), "{:?}", stages[1]);
        assert!(stages[0].notes.is_empty() && stages[1].notes.is_empty(), "nothing opened either stage");
    }

    #[test]
    fn a_missing_file_is_a_warning_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("План.md"), "## v0.1.0 · A\n- [ ] task\n").unwrap();
        let hub = read(dir.path()).unwrap();
        assert_eq!(hub.open_stages.len(), 1);
        // Named rather than counted: a count says nothing about which file
        // the reader failed to find, and this test exists to prove the
        // reader reports rather than fails.
        let missing: Vec<&str> = ["Изменения.md", "Решения.md", "Дневник.md"]
            .into_iter()
            .filter(|name| !hub.warnings.iter().any(|w| w.contains(name)))
            .collect();
        assert!(missing.is_empty(), "not warned about: {missing:?} in {:?}", hub.warnings);
        assert!(!hub.warnings.iter().any(|w| w.contains("План.md")), "{:?}", hub.warnings);
    }
}
