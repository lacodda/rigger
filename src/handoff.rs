//! Handing a card over: the text for the team that owns the fix.
//!
//! When the cause of a task lies outside the code in hand - a server, a
//! desktop client, a neighbour's API - the work is to tell the people who
//! own it, and the text has a shape: what is asked, how it is now, what is
//! needed, why, and where it was seen. The desk that came before kept that
//! shape as advice in a reference file and two hand-written copies of every
//! text, one in markdown for a chat and one in Jira's markup; the copies
//! drifted the moment either was corrected.
//!
//! So the shape is a form in the card's tray, filled in once, and both
//! texts are made from it. The record writes nothing a person did not: it
//! fills in what it knows (the title, the key, what was found and decided,
//! quoted as material) and leaves the words to whoever sends them - in the
//! language the other team reads.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::card::Card;

/// The folder in a tray the handoffs are kept in.
pub const DIR: &str = "handoff";

const FORM_TEMPLATE: &str = include_str!("handoff.form.md");

/// A text past this many lines has to open with a TL;DR: a reader decides
/// from the first lines whether a task is theirs.
const LONG_LINES: usize = 15;
/// How wide a line is taken to be when a paragraph is counted in lines, the
/// way a ticket shows it.
const LINE_WIDTH: usize = 100;

/// The sections of the form, in the order the text reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Title,
    Tldr,
    Ask,
    Now,
    Needed,
    Why,
    Example,
    CheckedOn,
}

impl Section {
    const ALL: [Section; 8] = [
        Section::Title,
        Section::Tldr,
        Section::Ask,
        Section::Now,
        Section::Needed,
        Section::Why,
        Section::Example,
        Section::CheckedOn,
    ];
    const REQUIRED: [Section; 5] = [Section::Title, Section::Ask, Section::Now, Section::Needed, Section::Why];

    fn heading(self) -> &'static str {
        match self {
            Section::Title => "Title",
            Section::Tldr => "TL;DR",
            Section::Ask => "Ask",
            Section::Now => "Now",
            Section::Needed => "Needed",
            Section::Why => "Why",
            Section::Example => "Example",
            Section::CheckedOn => "Checked on",
        }
    }

    /// The section a `## ` heading opens, by its words before any
    /// parenthesis; `None` for a heading that is not one of the text's.
    fn of(heading: &str) -> Option<Section> {
        let name = heading.split('(').next().unwrap_or(heading).trim().to_lowercase();
        Section::ALL.into_iter().find(|s| s.heading().to_lowercase() == name)
    }
}

/// A handoff as the form says it: the answer of each section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parts {
    answers: Vec<(Section, String)>,
}

impl Parts {
    fn get(&self, section: Section) -> Option<&str> {
        self.answers
            .iter()
            .find(|(s, _)| *s == section)
            .map(|(_, t)| t.as_str())
            .filter(|t| !t.is_empty())
    }

    pub fn title(&self) -> Option<&str> {
        self.get(Section::Title)
    }

    /// The required sections left empty, by heading.
    pub fn missing(&self) -> Vec<&'static str> {
        Section::REQUIRED.into_iter().filter(|s| self.get(*s).is_none()).map(Section::heading).collect()
    }

    /// The body after the title and the TL;DR, as it will be sent.
    fn body(&self) -> Vec<&str> {
        Section::ALL[2..].iter().filter_map(|s| self.get(*s)).collect()
    }

    /// Whether the text runs long enough to need a TL;DR: more lines than a
    /// reader takes in at a glance, a long line counted as the lines it
    /// wraps into. Lines rather than paragraphs, because the form's own
    /// sections are five short paragraphs and are not a long text.
    pub fn is_long(&self) -> bool {
        let lines: usize = self
            .body()
            .iter()
            .flat_map(|t| t.lines())
            .map(|l| l.trim().chars().count().div_ceil(LINE_WIDTH))
            .sum();
        lines > LONG_LINES
    }

    pub fn has_tldr(&self) -> bool {
        self.get(Section::Tldr).is_some()
    }
}

/// Reads a form: under each heading of the text, the lines that are not
/// the form's own `>` lines. Inside a fenced block every line is an answer,
/// a `>` included - an example is pasted as it is.
pub fn parse(form: &str) -> Parts {
    let mut answers: Vec<(Section, Vec<String>)> = Vec::new();
    let mut current: Option<Section> = None;
    let mut fenced = false;
    for line in form.lines() {
        let trimmed = line.trim_end();
        if !fenced && let Some(heading) = trimmed.strip_prefix("## ") {
            current = Section::of(heading);
            if let Some(section) = current {
                answers.push((section, Vec::new()));
            }
            continue;
        }
        if !fenced && trimmed.starts_with("# ") {
            current = None;
            continue;
        }
        let Some(section) = current else { continue };
        if trimmed.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && trimmed.trim_start().starts_with('>') {
            continue;
        }
        if let Some((s, lines)) = answers.last_mut()
            && *s == section
        {
            lines.push(trimmed.to_string());
        }
    }
    Parts {
        answers: answers.into_iter().map(|(s, lines)| (s, tidy(&lines))).collect(),
    }
}

/// Lines as one text: no blank lines at either end, and never two blank
/// lines in a row - the form's own lines leave gaps where they were.
fn tidy(lines: &[String]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim().is_empty() && out.last().is_none_or(|l| l.trim().is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// The text in markdown - for a chat, a mail, a message.
pub fn markdown(parts: &Parts) -> String {
    let mut out = format!("### {}\n", parts.title().unwrap_or_default());
    if let Some(tldr) = parts.get(Section::Tldr) {
        out.push_str(&format!("\n**TL;DR:** {tldr}\n"));
    }
    for text in parts.body() {
        out.push_str(&format!("\n{text}\n"));
    }
    out
}

/// The same text in Jira's markup, made from the same form so the two can
/// never say different things.
pub fn jira(parts: &Parts) -> String {
    let mut out = format!("h3. {}\n", inline(parts.title().unwrap_or_default()));
    if let Some(tldr) = parts.get(Section::Tldr) {
        out.push_str(&format!("\n*TL;DR:* {}\n", jira_block(tldr)));
    }
    for text in parts.body() {
        out.push_str(&format!("\n{}\n", jira_block(text)));
    }
    out
}

/// A block of markdown in Jira's markup: fences, lists and headings by the
/// line, emphasis, code and links inside it.
fn jira_block(text: &str) -> String {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let lead = line.trim_start();
        if let Some(lang) = lead.strip_prefix("```") {
            out.push(match (fenced, lang.trim()) {
                (false, "") => "{code}".to_string(),
                (false, lang) => format!("{{code:{lang}}}"),
                (true, _) => "{code}".to_string(),
            });
            fenced = !fenced;
            continue;
        }
        if fenced {
            out.push(line.to_string());
            continue;
        }
        let indent = (line.len() - lead.len()) / 2;
        if let Some(item) = lead.strip_prefix("- ").or_else(|| lead.strip_prefix("* ")) {
            out.push(format!("{} {}", "*".repeat(indent + 1), inline(item)));
        } else if let Some(item) = numbered(lead) {
            out.push(format!("{} {}", "#".repeat(indent + 1), inline(item)));
        } else if let Some((level, heading)) = heading(lead) {
            out.push(format!("h{level}. {}", inline(heading)));
        } else {
            out.push(inline(line));
        }
    }
    out.join("\n")
}

/// The item of a numbered list line: `1. text` is `text`.
fn numbered(line: &str) -> Option<&str> {
    let digits = line.find(|c: char| !c.is_ascii_digit())?;
    (digits > 0).then_some(())?;
    line[digits..].strip_prefix(". ")
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    ((1..=6).contains(&level))
        .then(|| line[level..].strip_prefix(' '))
        .flatten()
        .map(|h| (level, h))
}

/// Emphasis, code and links inside a line: `**b**` is `*b*`, `*i*` and
/// `_i_` are `_i_`, `` `c` `` is `{{c}}`, `[t](u)` is `[t|u]`.
fn inline(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let closing = |from: usize, pat: &[char]| -> Option<usize> { (from..chars.len().saturating_sub(pat.len() - 1)).find(|&j| chars[j..j + pat.len()] == *pat) };
    while i < chars.len() {
        let c = chars[i];
        if c == '`'
            && let Some(end) = closing(i + 1, &['`'])
        {
            out.push_str("{{");
            out.extend(&chars[i + 1..end]);
            out.push_str("}}");
            i = end + 1;
        } else if c == '*'
            && chars.get(i + 1) == Some(&'*')
            && let Some(end) = closing(i + 2, &['*', '*'])
        {
            out.push('*');
            out.push_str(&inline(&chars[i + 2..end].iter().collect::<String>()));
            out.push('*');
            i = end + 2;
        } else if (c == '*' || c == '_')
            && chars.get(i + 1).is_some_and(|n| !n.is_whitespace())
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && let Some(end) = closing(i + 1, &[c])
            && end > i + 1
        {
            out.push('_');
            out.extend(&chars[i + 1..end]);
            out.push('_');
            i = end + 1;
        } else if c == '['
            && let Some(mid) = closing(i + 1, &[']', '('])
            && let Some(end) = closing(mid + 2, &[')'])
        {
            out.push('[');
            out.extend(&chars[i + 1..mid]);
            out.push('|');
            out.extend(&chars[mid + 2..end]);
            out.push(']');
            i = end + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// A name for a file: what a path cannot hold becomes `-`, lower case.
pub fn slug(to: &str) -> String {
    let out: String = to
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    out.trim_matches(['-', '.']).to_string()
}

/// The blank form for a card and an addressee, with what the record knows:
/// `material` is quoted under "From the record", one entry a line.
pub fn blank_form(card: &Card, to: &str, material: &[String]) -> String {
    let material = match material.is_empty() {
        true => "> (nothing yet)".to_string(),
        false => material
            .iter()
            .map(|m| format!("> - {}", m.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    FORM_TEMPLATE
        .replace("\r\n", "\n")
        .replace("{to}", to)
        .replace("{key}", &card.key)
        .replace("{title}", &card.title)
        .replace("{material}", &material)
}

/// The three files of one handoff: the form, and the two texts made from it.
#[derive(Debug, Clone, Serialize)]
pub struct Files {
    pub form: PathBuf,
    pub markdown: PathBuf,
    pub jira: PathBuf,
}

impl Files {
    fn at(dir: &Path, stem: &str) -> Files {
        Files {
            form: dir.join(format!("{stem}.form.md")),
            markdown: dir.join(format!("{stem}.md")),
            jira: dir.join(format!("{stem}.jira.txt")),
        }
    }
}

/// The handoff to `to` being written: the newest form for that addressee
/// not yet sent, or a new one dated `today`. A form begun yesterday and
/// filled today is the same handoff, not a reason for a blank one.
pub fn files_for(tray: &Path, to: &str, today: &str) -> Result<Files> {
    let dir = tray.join(DIR);
    let mut unsent: Vec<Files> = list(tray)?
        .into_iter()
        .filter(|h| h.to == to && !h.sent)
        .map(|h| Files::at(&dir, &format!("{}-{}", h.to, h.day)))
        .collect();
    Ok(unsent.pop().unwrap_or_else(|| Files::at(&dir, &format!("{to}-{today}"))))
}

/// One handoff in a tray.
#[derive(Debug, Clone, Serialize)]
pub struct Handed {
    pub to: String,
    pub day: String,
    /// Whether the texts were made from the form.
    pub sent: bool,
    /// The title the text went out under, when it did.
    pub title: Option<String>,
    pub path: String,
}

/// The handoffs of a tray, oldest first.
pub fn list(tray: &Path) -> Result<Vec<Handed>> {
    let dir = tray.join(DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".form.md") else { continue };
        // `<to>-<yyyy-mm-dd>`: the day is the last ten characters.
        let Some(split) = stem.len().checked_sub(11).filter(|at| stem.as_bytes().get(*at) == Some(&b'-')) else {
            continue;
        };
        let (to, day) = (&stem[..split], &stem[split + 1..]);
        let files = Files::at(&dir, stem);
        let sent = files.markdown.is_file() && files.jira.is_file();
        let title = sent
            .then(|| std::fs::read_to_string(&files.markdown).ok())
            .flatten()
            .and_then(|t| t.lines().next().map(|l| l.trim_start_matches('#').trim().to_string()));
        out.push(Handed {
            to: to.to_string(),
            day: day.to_string(),
            sent,
            title,
            path: files.form.display().to_string(),
        });
    }
    out.sort_by(|a, b| a.day.cmp(&b.day).then_with(|| a.to.cmp(&b.to)));
    Ok(out)
}

/// Why a filled form cannot be sent yet, or nothing.
pub fn refusal(parts: &Parts) -> Option<String> {
    let missing = parts.missing();
    if !missing.is_empty() {
        return Some(format!("the form is not filled: {} empty", missing.join(", ")));
    }
    if parts.is_long() && !parts.has_tldr() {
        return Some(format!(
            "the text runs past {LONG_LINES} lines and has no TL;DR; a reader decides from the first lines whether it is theirs"
        ));
    }
    None
}

/// Writes the form, when it is not there. Says whether it wrote it.
pub fn ensure_form(files: &Files, form: &str) -> Result<bool> {
    if files.form.is_file() {
        return Ok(false);
    }
    if let Some(parent) = files.form.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    }
    std::fs::write(&files.form, form).with_context(|| format!("cannot write {}", files.form.display()))?;
    Ok(true)
}

/// Makes both texts from a form that is filled; refuses one that is not.
pub fn send(files: &Files) -> Result<(Parts, String)> {
    let form = std::fs::read_to_string(&files.form).with_context(|| format!("cannot read {}", files.form.display()))?;
    let parts = parse(&form);
    if let Some(why) = refusal(&parts) {
        bail!("{why}\n  fill it in: {}", files.form.display());
    }
    let text = markdown(&parts);
    std::fs::write(&files.markdown, &text).with_context(|| format!("cannot write {}", files.markdown.display()))?;
    std::fs::write(&files.jira, jira(&parts)).with_context(|| format!("cannot write {}", files.jira.display()))?;
    Ok((parts, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            id: 1,
            key: "ACME-7310".to_string(),
            title: "Archived alerts look live".to_string(),
            status: "active".to_string(),
            aliases: Vec::new(),
            summary: None,
            created_at: String::new(),
            updated_at: None,
            snoozed_until: None,
        }
    }

    fn filled() -> String {
        blank_form(&card(), "alerts-api", &["2026-09-20 · finding: the list has no archive flag".to_string()])
            .replace(
                "## Ask (required)\n",
                "## Ask (required)\n\nPlease return whether an alert is archived in the alert list.\n",
            )
            .replace(
                "## Now (required)\n",
                "## Now (required)\n\nThe client does not get the flag and **guesses**.\n",
            )
            .replace(
                "## Needed (required)\n",
                "## Needed (required)\n\nAn explicit field - we suggest `isArchived`, the name is open.\n",
            )
            .replace(
                "## Why (required)\n",
                "## Why (required)\n\nWhy: without it the archive cannot be shown or filtered.\n",
            )
    }

    #[test]
    fn a_blank_form_is_refused_and_names_what_is_empty() {
        let parts = parse(&blank_form(&card(), "alerts-api", &[]));
        assert_eq!(parts.title(), Some("Archived alerts look live"), "the record fills in the title");
        assert_eq!(parts.missing(), vec!["Ask", "Now", "Needed", "Why"]);
        assert!(refusal(&parts).unwrap().contains("Ask, Now, Needed, Why"));
    }

    #[test]
    fn the_form_and_the_material_stay_out_of_the_text() {
        let parts = parse(&filled());
        assert_eq!(refusal(&parts), None);
        let text = markdown(&parts);
        assert!(text.starts_with("### Archived alerts look live\n"), "{text}");
        assert!(text.contains("Please return whether") && text.contains("Related task: ACME-7310."), "{text}");
        assert!(!text.contains('>'), "no line of the form: {text}");
        assert!(!text.contains("finding"), "the material is not sent: {text}");
    }

    #[test]
    fn both_texts_say_the_same_in_their_own_markup() {
        let parts = parse(&filled());
        let jira = jira(&parts);
        assert!(jira.starts_with("h3. Archived alerts look live\n"), "{jira}");
        assert!(jira.contains("*guesses*") && jira.contains("{{isArchived}}"), "{jira}");
        assert!(!jira.contains("**") && !jira.contains('`'), "{jira}");
    }

    #[test]
    fn a_long_text_needs_a_tldr() {
        let long = filled().replace(
            "## Example (optional)\n",
            &format!("## Example (optional)\n\n{}\n", "A line of an example.\n".repeat(12)),
        );
        // Five short sections are not long; twelve lines more are.
        assert_eq!(refusal(&parse(&filled())), None);
        let parts = parse(&long);
        assert!(refusal(&parts).unwrap().contains("TL;DR"));
        let with = long.replace(
            "## TL;DR (required when the text runs long)\n",
            "## TL;DR (required when the text runs long)\n\nThe list needs an archive flag.\n",
        );
        let parts = parse(&with);
        assert_eq!(refusal(&parts), None);
        assert!(markdown(&parts).contains("\n**TL;DR:** The list needs an archive flag.\n"));
        assert!(jira(&parts).contains("\n*TL;DR:* The list needs an archive flag.\n"));
    }

    #[test]
    fn an_example_is_pasted_as_it_is() {
        let form = filled().replace("## Example (optional)\n", "## Example (optional)\n\n```json\n> { \"isArchived\": true }\n```\n");
        let parts = parse(&form);
        assert!(markdown(&parts).contains("```json\n> { \"isArchived\": true }\n```"));
        assert!(jira(&parts).contains("{code:json}\n> { \"isArchived\": true }\n{code}"), "{}", jira(&parts));
    }

    #[test]
    fn markdown_becomes_jira_markup() {
        assert_eq!(inline("a **b** and *c* and _d_ and `e`"), "a *b* and _c_ and _d_ and {{e}}");
        assert_eq!(inline("see [the spec](https://example.test/a)"), "see [the spec|https://example.test/a]");
        assert_eq!(inline("snake_case_name stays"), "snake_case_name stays");
        assert_eq!(jira_block("- one\n  - two\n1. first\n#### Sub"), "* one\n** two\n# first\nh4. Sub");
    }

    #[test]
    fn a_name_is_made_fit_for_a_file() {
        assert_eq!(slug("Alerts API"), "alerts-api");
        assert_eq!(slug("desktop/"), "desktop");
    }
}
