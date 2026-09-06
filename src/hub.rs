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
    /// How many blank lines stood between the stage and what followed it.
    ///
    /// A hub separates its stages with a rule, and the blank lines after
    /// that rule are the hub's own: most write one, one writes two, and one
    /// puts the next heading straight under the rule. Writing one for
    /// everybody moved every line below the first stage that disagreed.
    pub gap_after: usize,
    /// Where the stage stood among the stages of its file: first is 0.
    ///
    /// `after_prose` says which block a stage belongs to, and a block holds
    /// several: every stage of one hub's changelog sits after the same
    /// single run, so that number alone cannot tell seven of them apart.
    /// Without a place inside the block the export fell back on the order
    /// the rows came out of the table, which is not an order at all - it
    /// wrote that changelog oldest-first.
    ///
    /// A rank and not the line it sat on: an export writes the generated
    /// marker above everything, which moves every line of the file down by
    /// two, and a record that kept line numbers would drift on every run.
    pub rank: usize,
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
    /// How many blank lines stood between the run and what followed it.
    ///
    /// A hub separates its blocks with a rule, and the blank lines around
    /// that rule are the hub's own: most write one, one writes two, and two
    /// put the next heading straight under the rule. Writing one for
    /// everybody moved every line below the first run that disagreed.
    pub gap_after: usize,
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
    /// How many blank lines stood after that rule, before the next entry.
    ///
    /// Not a constant, and not a habit of a hub: one diary writes one blank
    /// line after most of its rules and two after four of them, and two
    /// others write none at all. Writing one for everybody moved every line
    /// below the first entry that disagreed.
    pub gap_after: usize,
    /// Where the entry stood among the entries of its diary: newest is 0.
    ///
    /// A date is not an order. Several sittings share a day - a diary of
    /// this line writes three on one date - and ordering by date alone left
    /// them to the order the rows came out of the table, which moved one
    /// entry of a real diary past its neighbour.
    pub rank: usize,
}

/// One line of a README's "Состояние" block: a date and what happened.
///
/// The date is kept whole, mark and all - a hub writes `2026-09-05 (ночь)`
/// to tell three sittings of one day apart, and splitting it to put it back
/// together would lose the mark.
#[derive(Debug, Clone, PartialEq)]
pub struct StateLine {
    /// The date and its mark, or nothing at all: a hub may close the block
    /// with a line that carries no date - "founded, sign and repository,
    /// see the changelog" - and dropping it lost the line every run.
    pub stamp: Option<String>,
    pub body: String,
    /// Blank lines between this line and the next. Its own count, because a
    /// hub need not be consistent: one of this line spaces its three newest
    /// entries and packs the fifty-three below them.
    pub gap_after: usize,
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
    pub state: Vec<StateLine>,
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
    read_file(dir, "README.md", &mut hub, |text, hub| {
        hub.state = parse_state(text);
        hub.prose.extend(parse_prose(text, "README.md"));
    })?;
    warn_about_repeats(&mut hub);
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

/// Whether a bold opening is the date of a state line, or just bold text.
///
/// A hub opens most lines with `**2026-09-05 (ночь)**`, and two of them
/// open one with a bold sentence instead - `**Кода пока нет намеренно.**`.
/// Taking that for a stamp put an em dash into the middle of a sentence, so
/// a stamp has to start with digits and a separator, the way a date does.
fn looks_like_a_date(text: &str) -> bool {
    let text = text.trim();
    let mut digits = text.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    // `2026-09-05`, `05.09.2026`, `2026-W36`: digits, a separator, digits.
    let rest = &text[digits..];
    let Some(rest) = rest.strip_prefix(['-', '.', '/']) else { return false };
    digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0
}

/// The "Состояние" block of a README: one dated line per thing worth
/// telling, newest first.
///
/// Nineteen of the twenty hubs of this line write it as `- **date** — what
/// happened`; the twentieth writes prose under that heading, and prose is
/// not a line, so it yields none and the file stays handwritten.
fn parse_state(text: &str) -> Vec<StateLine> {
    let mut out: Vec<StateLine> = Vec::new();
    let mut inside = false;
    let mut blank = 0usize;
    for line in text.lines() {
        if let Some((_, head)) = heading(line) {
            inside = head.starts_with("Состояние");
            continue;
        }
        if !inside {
            continue;
        }
        // Blank lines belong to the item above them, and are only counted
        // once another item follows - the ones before the next heading
        // separate the block, not its entries.
        if line.trim().is_empty() {
            blank += 1;
            continue;
        }
        let Some(rest) = line.trim().strip_prefix("- ") else { continue };
        // `- **2026-09-05 (ночь)** — что случилось`, or a line with no date
        // at all, which one hub uses to close the block.
        let line = match rest.strip_prefix("**").and_then(|r| r.split_once("**")) {
            Some((stamp, body)) if looks_like_a_date(stamp) => StateLine {
                stamp: Some(stamp.trim().to_string()),
                body: body.trim_start().trim_start_matches(['—', '-', '–']).trim().to_string(),
                gap_after: 0,
            },
            _ => StateLine {
                stamp: None,
                body: rest.trim().to_string(),
                gap_after: 0,
            },
        };
        if line.body.is_empty() {
            continue;
        }
        if let Some(previous) = out.last_mut() {
            previous.gap_after = blank;
        }
        blank = 0;
        out.push(line);
    }
    out
}

/// Warns when one version is written up twice in the same file.
///
/// The record holds one row per version, so an export can only write it
/// back once, and the second write-up is lost. That is a hub to mend, not
/// a shape to reproduce - one changelog of this line has two different
/// entries under the same number, and the export could only ever drop one
/// of them. Saying so is the honest answer; inventing a second row for a
/// number that is one release is not.
fn warn_about_repeats(hub: &mut Hub) {
    for (file, stages) in [("План.md", &hub.open_stages), ("Изменения.md", &hub.closed_stages)] {
        let mut seen: Vec<&str> = Vec::new();
        let mut said: Vec<&str> = Vec::new();
        for stage in stages {
            let version = stage.version.as_str();
            if seen.contains(&version) && !said.contains(&version) {
                said.push(version);
            }
            seen.push(version);
        }
        let said: Vec<String> = said.into_iter().map(str::to_string).collect();
        for version in said {
            hub.warnings.push(format!(
                "{file} writes up {version} more than once; the record keeps one entry per version, so an export writes one"
            ));
        }
    }
}

/// Splits a heading line into its level and text: `## v0.4.0 · Title`.
/// Tracks whether a walk is inside a fenced code block.
///
/// A `#` inside a fence is a shell comment, not a heading, and reading it
/// as one ends the run of prose it sits in - which lost a line of a real
/// README and the fence that closed it.
#[derive(Default)]
struct Fence(bool);

impl Fence {
    /// Feeds a line and says whether it is inside a fence, the opening and
    /// closing fences counting as inside.
    fn inside(&mut self, line: &str) -> bool {
        let was = self.0;
        if line.trim_start().starts_with("```") {
            self.0 = !self.0;
        }
        was || self.0
    }
}

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
                        gap_after: 1,
                        rank: stages.len(),
                    });
                    level = depth;
                }
                // A heading at the stage's level or above ends it: tasks under
                // "Backlog" or the next block do not belong to the last stage.
                None if depth <= level => {
                    settle(&mut stages, &mut notes);
                    level = 0;
                }
                // A deeper heading inside a stage is part of what was
                // written about it. One changelog of this line closes a
                // stage with `### Патч v0.24.1 — в тот же день` and the
                // account of that patch beneath it; skipping the line kept
                // the account and lost the heading over it, which read as
                // the patch having been written into the release.
                None if level > 0 => {
                    notes.push_str(line);
                    notes.push('\n');
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
        // The blank lines at the end are the hub's, not padding, and they
        // have to be counted here on the buffer as it was read - trimming
        // is what destroys them.
        let at = notes.find(&settled).unwrap_or(0) + settled.len();
        stage.gap_after = notes[at..].matches('\n').count().saturating_sub(1);
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
                    settle_entry(last, &body);
                }
                body.clear();
                entries.push(DiaryEntry {
                    heading: Some(head.to_string()),
                    date,
                    body: String::new(),
                    followed_by_rule: false,
                    gap_after: 1,
                    rank: entries.len(),
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
        settle_entry(last, &body);
    }
    entries
}

/// Hands a diary entry the lines written under it, and the separator that
/// followed them.
///
/// A rule between two entries belongs to neither: it separates them, and it
/// is kept off the body as a flag so the export can put it back where it
/// stood. The blank lines after that rule are part of the separator too,
/// and they have to be counted here, on the body as it was read - both
/// hand-overs trim it, and trimming is what destroys them.
fn settle_entry(entry: &mut DiaryEntry, body: &str) {
    let trimmed = body.trim();
    match trimmed.strip_suffix("---") {
        Some(rest) => {
            entry.body = rest.trim_end().to_string();
            entry.followed_by_rule = true;
            // What stood between the rule and whatever came next: the tail
            // the outer trim would have thrown away.
            let after = &body[body.rfind("---").map(|at| at + 3).unwrap_or(body.len())..];
            entry.gap_after = after.matches('\n').count().saturating_sub(1);
        }
        None => {
            entry.body = trimmed.to_string();
            // A diary that separates its entries with blank lines and no
            // rule chooses how many just as one with a rule does: this
            // line has a diary that leaves two.
            let at = body.find(trimmed).unwrap_or(0) + trimmed.len();
            entry.gap_after = body[at..].matches('\n').count().saturating_sub(1);
        }
    }
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
    // How deep the heading that opened the stage sat, so a deeper one can
    // be told from the one that ends it.
    let mut entry_depth = 0usize;
    let mut fence = Fence::default();

    fn flush(heading: Option<String>, body: &mut String, runs: &mut Vec<Prose>, file: &str) {
        let text = body.trim();
        // The blank lines at the end are the hub's, not padding: they are
        // counted before the body is trimmed, because trimming is what
        // destroys them.
        let gap_after = body[text.len() + body.find(text).unwrap_or(0)..].matches('\n').count().saturating_sub(1);
        if !text.is_empty() || heading.is_some() {
            runs.push(Prose {
                gap_after,
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
        // Inside a fence a  is a shell comment, not a heading.
        let fenced = fence.inside(line);
        if let Some((depth, head)) = heading(line).filter(|_| !fenced) {
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
                entry_depth = depth;
                heading_now = None;
                body.clear();
                continue;
            }
            // A heading deeper than the one that opened the stage is part
            // of what was written about it, not a run of its own: one
            // changelog closes a stage with `### Патч v0.24.1 — в тот же
            // день` and the account beneath it. `parse_stages` keeps such
            // a heading, so a run kept here too would print it twice.
            if inside_entry && depth > entry_depth {
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

    /// Stages of one block are told apart by their rank. `after_prose`
    /// names the block, and a changelog is one block holding all of them,
    /// so without a rank an export could only fall back on the order the
    /// rows came out of the table - and wrote one hub's changelog
    /// oldest-first.
    #[test]
    fn stages_of_one_block_keep_the_order_they_were_written_in() {
        let text = "# Изменения\n\n## v2.2.0 · Third\n\n## v2.1.0 · Second\n\n## v2.0.0 · First\n";
        let stages = parse_stages(text);
        let order: Vec<&str> = stages.iter().map(|s| s.version.as_str()).collect();
        assert_eq!(order, ["v2.2.0", "v2.1.0", "v2.0.0"]);
        // All three follow the same single run, so the rank is what tells
        // them apart.
        assert!(stages.iter().all(|s| s.after_prose == stages[0].after_prose), "one block");
        assert_eq!(stages.iter().map(|s| s.rank).collect::<Vec<_>>(), [0, 1, 2]);
    }

    /// The blank lines after the rule between two entries are part of the
    /// separator, and a diary chooses how many. One hub writes one after
    /// most of its rules and two after four of them; another writes none.
    /// Writing one for everybody moved every line below the first entry
    /// that disagreed.
    #[test]
    fn the_gap_after_a_rule_is_the_one_the_diary_wrote() {
        let diary = |gap: &str| {
            let text = format!("# Дневник\n\n## 2026-09-05 · Later\n\nЧто делали.\n\n---\n{gap}## 2026-09-04 · Earlier\n\nРаньше.\n");
            parse_diary(&text)
        };
        let one = diary("\n");
        assert!(one[0].followed_by_rule, "the rule is the separator, not the body");
        assert_eq!(one[0].gap_after, 1);
        assert_eq!(diary("\n\n")[0].gap_after, 2);
        assert_eq!(diary("")[0].gap_after, 0);
        // And the rule never lands in what was written.
        assert!(!one[0].body.contains("---"), "{:?}", one[0].body);
    }

    /// The blank lines a hub leaves after a stage are the hub's own. One
    /// changelog of this line puts the next heading straight under the
    /// rule and another leaves two blank lines; writing one for everybody
    /// moved every line below the first stage that disagreed.
    #[test]
    fn a_stage_keeps_the_gap_its_hub_left_after_it() {
        let stage = |gap: &str| {
            let text = format!("# Изменения\n\n## v0.2.0 · Second\n\nПро второй.\n\n---\n{gap}## v0.1.0 · First\n\nПро первый.\n");
            parse_stages(&text)
        };
        assert_eq!(stage("\n")[0].gap_after, 1);
        assert_eq!(stage("\n\n")[0].gap_after, 2);
        assert_eq!(stage("")[0].gap_after, 0);
        // The rule belongs to the prose it was written under, not to the
        // stage that follows it.
        assert!(stage("\n")[0].notes.ends_with("---"), "{:?}", stage("\n")[0].notes);
    }

    /// A version written up twice in one file is said out loud. The record
    /// holds one row per version, so an export can only write one of the
    /// two entries back, and silently dropping the other would look like a
    /// defect of the export rather than a hub to mend.
    #[test]
    fn a_version_written_up_twice_is_warned_about() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("План.md"), "# План\n").unwrap();
        std::fs::write(
            dir.path().join("Изменения.md"),
            "# Изменения\n\n## v0.1.0 · First — выпущен 2026-08-14\n\nОдин рассказ.\n\n## v0.1.0 · First — выпущен 2026-08-14\n\nДругой рассказ.\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Дневник.md"), "# Дневник\n").unwrap();
        let hub = read(dir.path()).unwrap();
        assert!(
            hub.warnings.iter().any(|w| w.contains("v0.1.0") && w.contains("more than once")),
            "{:?}",
            hub.warnings
        );
    }

    /// And a hub that writes each version once is not nagged.
    #[test]
    fn versions_written_once_raise_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("План.md"), "# План\n").unwrap();
        std::fs::write(
            dir.path().join("Изменения.md"),
            "# Изменения\n\n## v0.2.0 · Second — выпущен 2026-08-15\n\nПро второй.\n\n## v0.1.0 · First — выпущен 2026-08-14\n\nПро первый.\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Дневник.md"), "# Дневник\n").unwrap();
        let hub = read(dir.path()).unwrap();
        assert!(!hub.warnings.iter().any(|w| w.contains("more than once")), "{:?}", hub.warnings);
    }

    /// A heading deeper than the stage's own belongs to the stage. One
    /// changelog of this line closes a stage with `### Патч v0.24.1` and
    /// the account of that patch beneath it: skipping the line lost the
    /// heading, and keeping it in both the stage and a run of prose made
    /// the export print the whole account twice.
    #[test]
    fn a_heading_inside_a_stage_belongs_to_the_stage() {
        let text = "# Изменения\n\n## v0.2.0 · Second\n\nПро второй.\n\n### Патч v0.2.1\n\nПро патч.\n\n## v0.1.0 · First\n\nПро первый.\n";
        let stages = parse_stages(text);
        assert_eq!(stages.len(), 2, "the deeper heading is not a stage of its own");
        assert!(stages[0].notes.contains("### Патч v0.2.1"), "the heading is kept: {:?}", stages[0].notes);
        assert!(stages[0].notes.contains("Про патч."), "and so is what it introduced");
        // And no run of prose holds it as well, or the export writes it
        // once for the stage and once for the run.
        let runs = parse_prose(text, "Изменения.md");
        assert!(
            !runs
                .iter()
                .any(|r| r.heading.as_deref().is_some_and(|h| h.contains("Патч")) || r.body.contains("Про патч.")),
            "{runs:?}"
        );
    }

    /// A date is not an order. Several sittings share a day - one diary of
    /// this line writes three on one date - so entries carry the place
    /// they had, and ordering by date alone moved one past its neighbour.
    #[test]
    fn diary_entries_keep_the_place_they_had() {
        let text =
            "# Дневник\n\n## 2026-08-31 (2) · Later that day\n\nПотом.\n\n## 2026-08-31 · Earlier\n\nСначала.\n\n## 2026-08-30 · The day before\n\nНакануне.\n";
        let entries = parse_diary(text);
        assert_eq!(entries.iter().map(|e| e.rank).collect::<Vec<_>>(), [0, 1, 2]);
        // The two that share a day are told apart by rank alone.
        assert_eq!(entries[0].date, entries[1].date);
        assert!(entries[0].heading.as_deref().is_some_and(|h| h.contains("(2)")), "{:?}", entries[0].heading);
    }

    /// The state block is a list of dated lines, and a bold opening is not
    /// a date. Two hubs of this line open a line with a bold sentence, and
    /// reading that as a stamp put an em dash into the middle of it.
    #[test]
    fn a_bold_sentence_is_not_the_date_of_a_state_line() {
        let text = "# Хаб\n\n## Состояние\n\n- **2026-09-05 (ночь)** — что-то случилось\n- **Кода пока нет намеренно.** Хаб заведён авансом.\n- Основание — см. Изменения.\n";
        let state = parse_state(text);
        assert_eq!(state.len(), 3);
        assert_eq!(state[0].stamp.as_deref(), Some("2026-09-05 (ночь)"));
        assert_eq!(state[0].body, "что-то случилось");
        // The bold sentence stays part of what was written.
        assert_eq!(state[1].stamp, None);
        assert_eq!(state[1].body, "**Кода пока нет намеренно.** Хаб заведён авансом.");
        // And a line with no date at all survives.
        assert_eq!(state[2].stamp, None);
    }

    /// Each state line keeps the blank lines that followed it. One hub of
    /// this line spaces its newest entries and packs the rest, so a single
    /// flag for the list cannot hold it.
    #[test]
    fn a_state_line_keeps_the_gap_that_followed_it() {
        let text = "## Состояние\n\n- **2026-09-05** — новее\n\n- **2026-09-04** — старее\n- **2026-09-03** — ещё старее\n";
        let state = parse_state(text);
        assert_eq!(state.iter().map(|l| l.gap_after).collect::<Vec<_>>(), [1, 0, 0]);
    }

    /// A `#` inside a fenced block is a shell comment. One README writes
    /// one inside a fence, and reading it as a heading ended the run of
    /// prose there - losing that line and the fence that closed it.
    #[test]
    fn a_hash_inside_a_fence_is_not_a_heading() {
        let text = "# Хаб\n\n## Запуск\n\n```\ncargo build\n# и потом\n```\n\nПосле блока.\n";
        let runs = parse_prose(text, "README.md");
        let all: String = runs.iter().map(|r| r.body.as_str()).collect::<Vec<_>>().join("\n");
        assert!(all.contains("# и потом"), "the comment is kept: {all:?}");
        assert!(all.contains("cargo build"), "{all:?}");
        // And the fence did not start a heading of its own.
        assert!(!runs.iter().any(|r| r.heading.as_deref() == Some("# и потом")), "{runs:?}");
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
