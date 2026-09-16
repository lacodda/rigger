//! A questionnaire the owner answered, read into the record.
//!
//! Before a plan is reworked the owner is handed a page: a handful of forks
//! with the options laid out, and a list of ideas to tick. They answer it,
//! the answers steer the next few months of a project - and then the page
//! is the only place any of it is written down. The plan that comes out
//! says what was decided; it does not say what it was decided *against*,
//! and that is the half worth keeping. A year later "why is `find` going
//! through nooma rather than FTS5" is answerable only by finding the
//! artifact again.
//!
//! So the answers come in as events. A fork becomes a decision naming the
//! option taken and the ones it beat. An idea taken becomes a wish, to be
//! sorted into the plan like any other. An idea left untaken becomes a
//! decision too - "not this, and here is when it was considered" - because
//! an idea nobody records is an idea that comes back every quarter.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// A questionnaire, as the page that produced it writes one out.
#[derive(Debug, Deserialize)]
pub struct Answers {
    /// What the page called itself, for the decision to cite.
    #[serde(default)]
    pub questionnaire: Option<String>,
    /// The day it was answered, `2026-09-11`. The events are dated by it
    /// rather than by the moment of the import: the decision happened when
    /// the owner made it, and a record that says otherwise puts the
    /// reasoning after the work it explains.
    #[serde(default)]
    pub answered: Option<String>,
    #[serde(default)]
    pub forks: Vec<Fork>,
    #[serde(default)]
    pub ideas: Vec<Idea>,
}

/// A choice between options, with one of them taken.
#[derive(Debug, Deserialize)]
pub struct Fork {
    /// What the fork was about.
    pub question: String,
    /// The option the owner took.
    pub chosen: String,
    /// The options it was taken over. The point of the whole exercise.
    #[serde(default)]
    pub against: Vec<String>,
    /// Why, in the owner's words or the page's.
    #[serde(default)]
    pub why: Option<String>,
}

/// An idea offered, taken or not.
#[derive(Debug, Deserialize)]
pub struct Idea {
    pub idea: String,
    /// Whether the owner ticked it.
    #[serde(default)]
    pub taken: bool,
    #[serde(default)]
    pub why: Option<String>,
}

/// An event the answers turn into.
pub struct Entry {
    pub kind: &'static str,
    pub body: String,
}

pub fn read(path: &std::path::Path) -> Result<Answers> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let answers: Answers = serde_json::from_str(&text).with_context(|| format!("{} is not a questionnaire rigger understands", path.display()))?;
    if answers.forks.is_empty() && answers.ideas.is_empty() {
        bail!("{} answers nothing: it has neither forks nor ideas", path.display());
    }
    Ok(answers)
}

impl Answers {
    /// What the questionnaire is called in the text of an event.
    fn cite(&self) -> String {
        match (&self.questionnaire, &self.answered) {
            (Some(name), Some(day)) => format!("{name}, {day}"),
            (Some(name), None) => name.clone(),
            (None, Some(day)) => format!("questionnaire of {day}"),
            (None, None) => "a questionnaire".to_string(),
        }
    }

    /// The events these answers become, in the order they are recorded.
    pub fn entries(&self) -> Vec<Entry> {
        let cite = self.cite();
        let mut out = Vec::new();

        for fork in &self.forks {
            let mut body = format!("{}: {}", fork.question.trim(), fork.chosen.trim());
            if !fork.against.is_empty() {
                // The rejected options, spelt out. A decision that records
                // only what was chosen is a decision nobody can revisit:
                // the question it answered is gone with the page.
                let against: Vec<&str> = fork.against.iter().map(|o| o.trim()).filter(|o| !o.is_empty()).collect();
                if !against.is_empty() {
                    body.push_str(&format!("\n\nRather than: {}", against.join("; ")));
                }
            }
            if let Some(why) = fork.why.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
                body.push_str(&format!("\n\n{why}"));
            }
            body.push_str(&format!("\n\nAnswered on {cite}."));
            out.push(Entry { kind: "decision", body });
        }

        for idea in &self.ideas {
            let text = idea.idea.trim();
            if text.is_empty() {
                continue;
            }
            let why = idea.why.as_deref().map(str::trim).filter(|w| !w.is_empty());
            let body = if idea.taken {
                // A wish, to be sorted into the plan like any other. Not a
                // decision: an idea the owner took is work not yet placed.
                match why {
                    Some(why) => format!("{text}\n\n{why}\n\nTaken on {cite}."),
                    None => format!("{text}\n\nTaken on {cite}."),
                }
            } else {
                match why {
                    Some(why) => format!("Not taking: {text}\n\n{why}\n\nConsidered on {cite}."),
                    None => format!("Not taking: {text}\n\nConsidered on {cite}."),
                }
            };
            out.push(Entry {
                kind: if idea.taken { "wish" } else { "decision" },
                body,
            });
        }
        out
    }

    /// The day the events are dated by: the day it was answered, when the
    /// page says so.
    pub fn at(&self) -> Option<String> {
        let day = self.answered.as_deref()?.trim();
        // A date rigger writes elsewhere, so that these sit among the other
        // events of that day rather than at the moment of the import.
        //
        // Checked by shape, not by counting: `11.09.2026` is also ten
        // characters with eight digits in it, and counting alone let it
        // through as `11.09.2026T00:00:00Z` - a timestamp nothing can read
        // and every query would sort to the wrong end of the record.
        let mut parts = day.split('-');
        let ok = matches!(
            (parts.next(), parts.next(), parts.next(), parts.next()),
            (Some(y), Some(m), Some(d), None)
                if y.len() == 4 && m.len() == 2 && d.len() == 2
                    && [y, m, d].iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        );
        ok.then(|| format!("{day}T00:00:00Z"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Answers {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_fork_records_what_it_was_decided_against() {
        let answers = parse(
            r#"{
              "questionnaire": "plan review 03ab557f",
              "answered": "2026-09-11",
              "forks": [{
                "question": "Where does knowledge of the code come from",
                "chosen": "nooma as a library",
                "against": ["a tree-sitter index of its own", "asking an assistant each time"],
                "why": "One index for the line, not one per product."
              }]
            }"#,
        );
        let entries = answers.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "decision");
        let body = &entries[0].body;
        assert!(body.contains("nooma as a library"), "{body}");
        assert!(
            body.contains("Rather than: a tree-sitter index of its own; asking an assistant each time"),
            "{body}"
        );
        assert!(body.contains("One index for the line"), "{body}");
        assert!(body.contains("plan review 03ab557f, 2026-09-11"), "{body}");
    }

    /// An idea ticked is work not yet placed, so it is a wish; one left
    /// unticked is a decision, or it comes back every quarter.
    #[test]
    fn a_taken_idea_is_a_wish_and_an_untaken_one_is_a_decision() {
        let answers = parse(
            r#"{
              "answered": "2026-09-11",
              "ideas": [
                {"idea": "A registry of the line as public JSON", "taken": true},
                {"idea": "A second binary for the alias", "taken": false, "why": "An alias is a link."}
              ]
            }"#,
        );
        let entries = answers.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, "wish");
        assert!(entries[0].body.starts_with("A registry of the line"), "{}", entries[0].body);
        assert_eq!(entries[1].kind, "decision");
        assert!(entries[1].body.starts_with("Not taking: A second binary"), "{}", entries[1].body);
        assert!(entries[1].body.contains("An alias is a link."), "{}", entries[1].body);
    }

    /// The events belong to the day the owner answered, not to the day
    /// somebody got round to importing the file.
    #[test]
    fn the_events_are_dated_by_the_day_it_was_answered() {
        assert_eq!(
            parse(r#"{"answered":"2026-09-11","ideas":[{"idea":"x"}]}"#).at().as_deref(),
            Some("2026-09-11T00:00:00Z")
        );
        // Anything that is not a plain day is no date at all, and the
        // import falls back to now rather than writing a broken timestamp.
        assert_eq!(parse(r#"{"answered":"11.09.2026","ideas":[{"idea":"x"}]}"#).at(), None);
        assert_eq!(parse(r#"{"ideas":[{"idea":"x"}]}"#).at(), None);
    }

    /// `taken` defaults to false, which is the safe way round: an idea
    /// whose tick the page forgot to write becomes a decision not to take
    /// it, which is visible, rather than a wish nobody asked for.
    #[test]
    fn an_idea_without_a_tick_is_not_taken() {
        let answers = parse(r#"{"ideas":[{"idea":"something"}]}"#);
        assert_eq!(answers.entries()[0].kind, "decision");
    }

    #[test]
    fn a_questionnaire_with_no_name_still_cites_itself() {
        let answers = parse(r#"{"answered":"2026-09-11","ideas":[{"idea":"x","taken":true}]}"#);
        assert!(
            answers.entries()[0].body.contains("questionnaire of 2026-09-11"),
            "{}",
            answers.entries()[0].body
        );
    }
}
