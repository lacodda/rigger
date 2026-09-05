//! The two screens the owner reads, rather than the assistant.
//!
//! Everything before this release was a door for the assistant: a packet, a
//! protocol, a search. This is the first pair of commands meant for the
//! person who has to decide things - and the thing they most need is not
//! more detail but less of it.
//!
//! `inbox` is the queue of questions waiting for an answer, gathered from
//! every project at once. It exists because the questions were never in one
//! place: twenty-two of them sat in nine separate hubs, and three projects
//! turned out to be asking the same thing without knowing it.
//!
//! `digest` is the other direction - what moved, in five lines.

use serde::Serialize;

use crate::db::{DigestFacts, Waiting};

/// A group of questions that appear to ask the same thing.
#[derive(Debug, Serialize)]
pub struct Shared {
    pub subject: String,
    pub projects: Vec<String>,
}

/// The heading of a question, which is what identifies it in a list.
///
/// These are written as `**Subject.** Then the detail`, so the bold opening
/// is the subject and the rest is argument. Falls back to the first line.
pub fn subject(body: &str) -> String {
    let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();

    // Hubs mark a waiting question with a checkbox or a status glyph before
    // the heading; neither is part of what the question is about.
    let first = first
        .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '*' && c != '`')
        .trim_start_matches("[ ]")
        .trim_start_matches("[x]")
        .trim();

    // `**Subject**` or `**Subject:**` - take what is between the markers.
    if let Some(rest) = first.strip_prefix("**")
        && let Some(end) = rest.find("**")
    {
        let heading = rest[..end].trim().trim_end_matches([':', '.', ',', '—', '-']).trim();
        if !heading.is_empty() {
            return heading.to_string();
        }
    }

    // Otherwise the first sentence. Counted in characters, because these are
    // written in Russian as often as English and a byte index lands inside a
    // dash - which is how this panicked the first time it met a real hub.
    //
    // A dot is only an end when a space follows: `crates.io` and `v0.9.0`
    // are not sentences ending mid-word.
    let chars: Vec<char> = first.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let ends = match c {
            '?' | '!' => true,
            ':' | '—' => true,
            '.' => chars.get(i + 1).is_none_or(|next| next.is_whitespace()),
            _ => false,
        };
        if ends {
            let keep = if matches!(c, '?' | '!') { i + 1 } else { i };
            let cut: String = chars[..keep].iter().collect();
            let cut = cut.trim().trim_end_matches([':', '—', '-']).trim();
            if !cut.is_empty() {
                return cut.to_string();
            }
        }
    }
    first.trim_end_matches(['.', ':', '—']).trim().to_string()
}

/// Questions that several projects are asking in the same words.
///
/// Grouped by the subject line rather than by meaning: rigger does not guess
/// what two questions have in common, it notices that they are spelt alike.
/// That is enough for the case this exists for - the same question copied
/// into several hubs, which is how three projects came to be waiting on one
/// answer about the release calendar.
pub fn shared_subjects(waiting: &[Waiting]) -> Vec<Shared> {
    // Matched on a normalised key, but shown as a person wrote it: hubs
    // differ in case and punctuation, and the reader wants the wording.
    let mut groups: Vec<(String, String, Vec<String>)> = Vec::new();
    for question in waiting {
        let shown = subject(&question.body);
        let key = normalise(&shown);
        if key.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, _, projects)) => {
                if !projects.contains(&question.project) {
                    projects.push(question.project.clone());
                }
            }
            None => groups.push((key, shown, vec![question.project.clone()])),
        }
    }

    let mut shared: Vec<Shared> = groups
        .into_iter()
        .filter(|(_, _, projects)| projects.len() > 1)
        .map(|(_, subject, projects)| Shared { subject, projects })
        .collect();
    // Most-shared first: one answer that settles four projects is worth
    // seeing before one that settles two.
    shared.sort_by(|a, b| b.projects.len().cmp(&a.projects.len()).then(a.subject.cmp(&b.subject)));
    shared
}

/// A subject reduced to what two spellings of one question share: case and
/// surrounding punctuation differ between hubs, the words do not.
fn normalise(subject: &str) -> String {
    subject
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name of a stage, without the asides a hub carries.
///
/// A plan heading often trails a parenthetical - `*(deferred from v0.13.0,
/// owner's decision 02.09)*` - which is worth keeping in the hub and is pure
/// noise in a five-line digest.
pub fn stage_name(stage: &str) -> String {
    let cut = stage.find(" *(").or_else(|| stage.find(" (")).unwrap_or(stage.len());
    let name = stage[..cut].trim().trim_end_matches(['*', '—', '-', ',']).trim();
    if name.is_empty() { stage.trim().to_string() } else { name.to_string() }
}

/// The five lines of a digest for one project.
///
/// Five is the whole point: the owner reads this across a line of projects,
/// and a paragraph each would be the hub all over again. What shipped, what
/// was decided, what was learnt, what waits, what is next.
pub fn digest_lines(facts: &DigestFacts, next_stage: Option<&str>, quiet_days: Option<i64>, signal: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();

    // The signal goes first because it is the one line here the owner has to
    // act on: everything else is a report of what happened, and this is a
    // promise being broken while nothing happens.
    if let Some(signal) = signal {
        lines.push(signal.to_string());
    }

    if !facts.shipped.is_empty() {
        lines.push(match facts.shipped.len() {
            1 => format!("shipped {}", facts.shipped[0]),
            _ => format!(
                "shipped {} — {} ({} releases)",
                facts.shipped.first().map(String::as_str).unwrap_or(""),
                facts.shipped.last().map(String::as_str).unwrap_or(""),
                facts.shipped.len()
            ),
        });
    }

    let mut recorded = Vec::new();
    if facts.decisions > 0 {
        recorded.push(plural(facts.decisions, "decision", "decisions"));
    }
    if facts.findings > 0 {
        recorded.push(plural(facts.findings, "finding", "findings"));
    }
    if facts.changes > 0 {
        recorded.push(plural(facts.changes, "change", "changes"));
    }
    if !recorded.is_empty() {
        lines.push(format!("recorded {}", recorded.join(", ")));
    }

    if facts.waiting > 0 {
        lines.push(format!("waiting on you: {}", plural(facts.waiting, "question", "questions")));
    }

    if let Some(stage) = next_stage {
        lines.push(format!("next: {}", stage_name(stage)));
    }

    // Silence is a fact too, and the one most worth seeing: a project nobody
    // has touched in a fortnight is not the same as one shipped yesterday.
    if lines.is_empty()
        && let Some(days) = quiet_days
    {
        // Reached only when nothing else had anything to say, signal
        // included - a project raising one is never merely quiet.
        lines.push(match days {
            0 => "nothing recorded, though something happened today".to_string(),
            1 => "nothing this week; last touched yesterday".to_string(),
            n => format!("nothing this week; last touched {n} days ago"),
        });
    }
    lines
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting(project: &str, body: &str) -> Waiting {
        Waiting {
            project: project.into(),
            id: 1,
            date: "2026-09-04".into(),
            body: body.into(),
        }
    }

    #[test]
    fn a_bold_heading_is_the_subject() {
        assert_eq!(subject("**Место в календаре:** ярус не назначен."), "Место в календаре");
        assert_eq!(subject("**Social preview** - upload it by hand."), "Social preview");
    }

    #[test]
    fn a_question_without_a_heading_still_has_a_subject() {
        assert_eq!(subject("Which tier is this project? It matters for the rhythm."), "Which tier is this project?");
        assert_eq!(subject("Pick the release day"), "Pick the release day");
    }

    #[test]
    fn a_leading_blank_line_does_not_swallow_the_subject() {
        assert_eq!(subject("\n\n**The subject.** Detail."), "The subject");
    }

    #[test]
    fn questions_spelt_alike_across_projects_are_grouped() {
        // The case this exists for: three hubs carrying one question about
        // the release calendar, and the owner answering it three times.
        let questions = vec![
            waiting("austeris", "**Место в производственном календаре:** ярус не назначен."),
            waiting("dowel", "**Место в производственном календаре** — ярус B."),
            waiting("kasl-server", "**Место в производственном календаре:** в какой ярус."),
            waiting("kilna", "**Публичный анонс** — привязан к v1.0.0."),
        ];
        let shared = shared_subjects(&questions);
        assert_eq!(shared.len(), 1, "{shared:?}");
        assert_eq!(shared[0].projects.len(), 3);
        assert!(shared[0].projects.contains(&"dowel".to_string()));
        // A question only one project asks is not a group.
        assert!(!shared.iter().any(|s| s.subject.contains("анонс")), "{shared:?}");
    }

    #[test]
    fn one_project_asking_twice_is_not_a_group() {
        // Grouping is about an answer that settles several projects at once;
        // two questions in one project are just two questions.
        let questions = vec![
            waiting("dowel", "**The same subject.** First."),
            waiting("dowel", "**The same subject.** Second."),
        ];
        assert!(shared_subjects(&questions).is_empty());
    }

    #[test]
    fn the_most_shared_question_comes_first() {
        let questions = vec![
            waiting("a", "**Two.** x"),
            waiting("b", "**Two.** x"),
            waiting("c", "**Three.** y"),
            waiting("d", "**Three.** y"),
            waiting("e", "**Three.** y"),
        ];
        let shared = shared_subjects(&questions);
        assert_eq!(shared[0].subject, "Three");
        assert_eq!(shared[0].projects.len(), 3);
    }

    fn facts(shipped: &[&str], decisions: u32, findings: u32, changes: u32, waiting: u32) -> DigestFacts {
        DigestFacts {
            shipped: shipped.iter().map(|s| s.to_string()).collect(),
            decisions,
            findings,
            changes,
            waiting,
        }
    }

    #[test]
    fn a_digest_is_five_lines_at_most() {
        // The threshold is the product: a paragraph each would be the hub
        // all over again, which is what the owner stopped reading.
        let busy = facts(&["v0.6.0", "v0.7.0", "v0.8.0"], 12, 9, 40, 3);
        let lines = digest_lines(&busy, Some("v0.9.0 · Inbox"), Some(0), None);
        assert!(lines.len() <= 5, "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("3 releases")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("waiting on you: 3 questions")), "{lines:#?}");
    }

    #[test]
    fn a_stage_name_drops_the_hub_aside() {
        // Real headings carry them, and five lines have no room for one.
        assert_eq!(
            stage_name("v0.13.1 · MCP и Agent Skills *(отложено из v0.13.0, решение владельца 02.09)*"),
            "v0.13.1 · MCP и Agent Skills"
        );
        assert_eq!(stage_name("v0.9.0 · Inbox and digest"), "v0.9.0 · Inbox and digest");
        // A heading that is only an aside keeps something rather than nothing.
        assert!(!stage_name("*(pending)*").is_empty());
    }

    #[test]
    fn a_single_release_is_named_rather_than_counted() {
        let lines = digest_lines(&facts(&["v1.0.0"], 0, 0, 0, 0), None, Some(0), None);
        assert_eq!(lines[0], "shipped v1.0.0");
    }

    #[test]
    fn silence_is_reported_as_a_fact() {
        // A project nobody has touched in a fortnight is not the same as one
        // shipped yesterday, and an empty digest would read the same for both.
        let lines = digest_lines(&facts(&[], 0, 0, 0, 0), None, Some(14), None);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("14 days ago"), "{lines:?}");
    }

    /// A signal is not another thing that happened - it is a promise being
    /// broken while nothing happens, so it goes above the report and it
    /// stops the project reading as merely quiet.
    #[test]
    fn a_signal_leads_the_digest_and_displaces_the_silence() {
        let signal = "tier A asks for more: 7 weeks without a release";
        let lines = digest_lines(&facts(&[], 0, 0, 0, 0), None, Some(30), Some(signal));
        assert_eq!(lines[0], signal);
        // The silence line only speaks when nothing else did, and a signal
        // is something: "nothing this week" beside a broken promise reads
        // as though the promise were part of the nothing.
        assert!(!lines.iter().any(|l| l.contains("nothing this week")), "{lines:?}");
    }

    #[test]
    fn a_digest_with_a_signal_is_still_five_lines_at_most() {
        // Every other line present at once, and a signal on top: the cap is
        // the product, and a sixth line is the hub creeping back in.
        let busy = facts(&["v0.6.0", "v0.7.0"], 12, 9, 40, 3);
        let lines = digest_lines(
            &busy,
            Some("v0.9.0 · Inbox"),
            Some(0),
            Some("tier B asks for more: no turn in the focus for 7 weeks"),
        );
        assert!(lines.len() <= 5, "{} lines: {lines:#?}", lines.len());
    }

    #[test]
    fn counts_agree_with_their_nouns() {
        let lines = digest_lines(&facts(&[], 1, 0, 1, 1), None, None, None);
        assert!(lines.iter().any(|l| l.contains("1 decision, 1 change")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("1 question")), "{lines:?}");
    }
}
