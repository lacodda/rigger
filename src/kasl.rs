//! kasl, the time tracker of the line, as rigger's neighbour on a machine.
//!
//! Two things pass between them, both through kasl's command line.
//!
//! **A session, told to kasl.** kasl measures the working day; rigger knows
//! what the day was spent on. Kept apart, the owner answers "how long did
//! v0.23 take" from two tools and a guess. So a session that opens tells
//! kasl what it is about, and a session that closes tells kasl the interval
//! is over - and kasl's report carries the stage by name:
//!
//! ```text
//! kasl focus start "<project> <version> · <title>" --from rigger
//! kasl focus stop --from rigger
//! ```
//!
//! `--from` lets kasl close only what rigger opened, so a focus the owner
//! started by hand is not ended by an assistant's session. A kasl without
//! the command, or no kasl at all, is silence: the session opens and closes
//! exactly as before, and nothing is printed - a line saying "kasl did not
//! answer" at every session would be read once and then never again.
//!
//! **Tickets, read from kasl's inbox.** At a desk kasl already polls the
//! tracker for the issues assigned to the person, and keeps them in its
//! inbox: new, changed, taken, asleep, gone. Asking the tracker a second
//! time from rigger would mean a second set of credentials, a second
//! schedule and two answers to the same question. So rigger reads kasl's:
//!
//! ```text
//! kasl inbox list --all --snoozed --json
//! ```
//!
//! and the shape of that answer is a contract, documented on the `task`
//! page. `--all` keeps the issues that have gone from the tracker, which is
//! how a card learns its ticket was closed; `--snoozed` keeps the sleeping
//! ones, which are still the person's.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The environment variable naming the kasl to run.
///
/// For a kasl that is not on the PATH, and the only way a scratch record
/// reaches one at all (`paths::scratch`) - which is how the tests see it.
pub const KASL_ENV: &str = "RIGGER_KASL";

/// How long kasl is given to answer. A session must not wait on a time
/// tracker that is checking for its own update over a slow network.
const PATIENCE: Duration = Duration::from_secs(5);

/// What telling kasl came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state", content = "said")]
pub enum Told {
    /// kasl took it.
    Taken,
    /// No kasl for this record: scratch, or not installed.
    Absent,
    /// kasl ran and said no - most often a kasl that does not know `focus`
    /// yet. Kept, with the first line it said, for `--json`.
    Refused(String),
}

fn program() -> Option<std::ffi::OsString> {
    match std::env::var_os(KASL_ENV) {
        Some(named) if !named.is_empty() => Some(named),
        _ if crate::paths::scratch() => None,
        _ => Some("kasl".into()),
    }
}

/// Opens an interval in kasl for the work a session is about.
pub fn start(task: &str) -> Told {
    match run(&["focus", "start", task, "--from", "rigger"]) {
        Ran::Said(_) => Told::Taken,
        Ran::Absent => Told::Absent,
        Ran::Refused(why) => Told::Refused(why),
    }
}

/// Closes the interval rigger opened.
pub fn stop() -> Told {
    match run(&["focus", "stop", "--from", "rigger"]) {
        Ran::Said(_) => Told::Taken,
        Ran::Absent => Told::Absent,
        Ran::Refused(why) => Told::Refused(why),
    }
}

/// What running kasl came to.
enum Ran {
    /// It succeeded, and this is what it printed.
    Said(String),
    Absent,
    /// It failed, with the first line it said about it.
    Refused(String),
}

/// Runs kasl and waits for it, within its patience.
///
/// Both of its streams are read while it runs, each on a thread of its own:
/// an inbox of two hundred issues is more than a pipe holds, and a kasl
/// blocked on writing to a pipe nobody reads would look exactly like a kasl
/// that does not answer.
fn run(args: &[&str]) -> Ran {
    let Some(program) = program() else {
        return Ran::Absent;
    };
    let child = Command::new(&program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ran::Absent,
        Err(e) => return Ran::Refused(e.to_string()),
    };
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let began = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if began.elapsed() < PATIENCE => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ran::Refused(format!("kasl did not answer within {} seconds", PATIENCE.as_secs()));
            }
            Err(e) => return Ran::Refused(e.to_string()),
        }
    };
    let said = stdout.join().unwrap_or_default();
    if status.success() {
        return Ran::Said(said);
    }
    let complaint = stderr.join().unwrap_or_default();
    let first = complaint.lines().find(|l| !l.trim().is_empty()).unwrap_or("kasl said no").trim();
    // kasl's own `error:` would stand beside rigger's: "error: ... error: ...".
    let first = first.strip_prefix("error:").map(str::trim).unwrap_or(first);
    Ran::Refused(first.to_string())
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// What a session is about, in the words kasl will show in its report.
pub fn task_name(project: &str, stage: Option<(&str, Option<&str>)>) -> String {
    match stage {
        Some((version, Some(title))) => format!("{project} {version} · {title}"),
        Some((version, None)) => format!("{project} {version}"),
        None => project.to_string(),
    }
}

/// One issue in kasl's inbox, as `kasl inbox list --json` gives it.
///
/// Only `key` and `summary` are required; every other field may be absent
/// or null, and fields rigger does not know are ignored - so kasl can say
/// more without breaking the reader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    /// The ranking field kasl sorts by, when the tracker has one.
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub first_seen: Option<String>,
    /// When the person took it in kasl.
    #[serde(default)]
    pub taken_at: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// When a sleeping issue is due back.
    #[serde(default)]
    pub snoozed_until: Option<String>,
    /// When it stopped appearing in the tracker: closed, or given to
    /// somebody else.
    #[serde(default)]
    pub gone_at: Option<String>,
}

impl Issue {
    pub fn is_gone(&self) -> bool {
        self.gone_at.is_some()
    }

    /// Asleep now. A snooze kasl has not woken yet is read against the local
    /// clock, the one kasl writes it in.
    pub fn is_asleep(&self) -> bool {
        let Some(until) = &self.snoozed_until else { return false };
        let now = jiff::Zoned::now().datetime().to_string();
        until.replace(' ', "T").as_str() > now.as_str()
    }

    /// The facts of the ticket, in one line: status, priority, score, and
    /// where it has gone.
    pub fn facts(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status) = &self.status {
            parts.push(status.clone());
        }
        if let Some(priority) = &self.priority {
            parts.push(format!("priority {priority}"));
        }
        if let Some(score) = self.score {
            parts.push(format!("score {}", trim_score(score)));
        }
        if let Some(at) = &self.taken_at {
            parts.push(format!("taken {}", day(at)));
        }
        if let Some(at) = &self.gone_at {
            parts.push(format!("gone {} - closed or reassigned", day(at)));
        } else if self.is_asleep()
            && let Some(until) = &self.snoozed_until
        {
            parts.push(format!("asleep until {}", day(until)));
        }
        parts.join(" · ")
    }
}

fn trim_score(score: f64) -> String {
    if score.fract() == 0.0 {
        format!("{}", score as i64)
    } else {
        format!("{score}")
    }
}

/// The day of a moment kasl wrote, with a `T` or a space before the time.
pub fn day(moment: &str) -> &str {
    moment.split(['T', ' ']).next().unwrap_or(moment)
}

/// kasl's answer, as the contract has it.
#[derive(Debug, Deserialize)]
struct InboxAnswer {
    issues: Vec<Issue>,
}

/// What asking kasl for its inbox came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbox {
    Read(Vec<Issue>),
    /// No kasl for this record: scratch, or not installed.
    Absent,
    /// kasl ran and could not give it: most often a kasl that does not
    /// list its inbox as JSON yet. With the first line it said.
    Refused(String),
}

/// The issues in kasl's inbox, gone and sleeping ones too, in kasl's order.
pub fn inbox() -> Inbox {
    match run(&["inbox", "list", "--all", "--snoozed", "--json"]) {
        Ran::Said(text) => match serde_json::from_str::<InboxAnswer>(text.trim()) {
            Ok(answer) => Inbox::Read(answer.issues),
            Err(e) => Inbox::Refused(format!("kasl's answer is not the inbox rigger reads ({e})")),
        },
        Ran::Absent => Inbox::Absent,
        Ran::Refused(why) => Inbox::Refused(why),
    }
}

/// The ticket a card is, when kasl's inbox has it: by the card's key or any
/// of its aliases. Silent when there is no kasl or it cannot say - a card
/// is shown the same without its ticket.
pub fn ticket_of(card: &crate::card::Card, issues: &Inbox) -> Option<Issue> {
    let Inbox::Read(issues) = issues else { return None };
    let names: Vec<&str> = std::iter::once(card.key.as_str()).chain(card.aliases.iter().map(String::as_str)).collect();
    issues.iter().find(|i| names.iter().any(|n| n.eq_ignore_ascii_case(&i.key))).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_named_by_its_stage() {
        assert_eq!(task_name("widget", Some(("v0.2.0", Some("Second")))), "widget v0.2.0 · Second");
        assert_eq!(task_name("widget", Some(("v0.2.0", None))), "widget v0.2.0");
        assert_eq!(task_name("widget", None), "widget");
    }

    #[test]
    fn an_issue_needs_a_key_and_a_summary_and_nothing_else() {
        let answer: InboxAnswer = serde_json::from_str(r#"{"issues":[{"key":"ACME-1","summary":"s","new_field":3}]}"#).unwrap();
        assert_eq!(answer.issues[0].key, "ACME-1");
        assert_eq!(answer.issues[0].status, None);
        assert!(!answer.issues[0].pinned);
        assert!(serde_json::from_str::<InboxAnswer>(r#"{"issues":[{"key":"ACME-1"}]}"#).is_err());
    }

    #[test]
    fn the_facts_say_where_a_ticket_went() {
        let mut issue: Issue = serde_json::from_str(r#"{"key":"ACME-1","summary":"s","status":"Open","priority":"High","score":8.0}"#).unwrap();
        assert_eq!(issue.facts(), "Open · priority High · score 8");
        issue.gone_at = Some("2026-09-24T10:00:00".to_string());
        assert_eq!(issue.facts(), "Open · priority High · score 8 · gone 2026-09-24 - closed or reassigned");
    }

    #[test]
    fn a_snooze_in_the_past_is_not_sleep() {
        let mut issue: Issue = serde_json::from_str(r#"{"key":"ACME-1","summary":"s"}"#).unwrap();
        issue.snoozed_until = Some("2001-01-01 09:00:00".to_string());
        assert!(!issue.is_asleep());
        issue.snoozed_until = Some("2999-01-01T09:00:00".to_string());
        assert!(issue.is_asleep());
    }
}
