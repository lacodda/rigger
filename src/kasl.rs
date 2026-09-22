//! A session, told to kasl: the time and the work in one truth.
//!
//! kasl measures the working day; rigger knows what the day was spent on.
//! Kept apart, the owner answers "how long did v0.23 take" from two tools
//! and a guess. So a session that opens tells kasl what it is about, and a
//! session that closes tells kasl the interval is over - and kasl's report
//! carries the stage by name.
//!
//! The door is kasl's command line, and the contract is two commands:
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

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

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
    run(&["focus", "start", task, "--from", "rigger"])
}

/// Closes the interval rigger opened.
pub fn stop() -> Told {
    run(&["focus", "stop", "--from", "rigger"])
}

fn run(args: &[&str]) -> Told {
    let Some(program) = program() else {
        return Told::Absent;
    };
    let child = Command::new(&program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Told::Absent,
        Err(e) => return Told::Refused(e.to_string()),
    };
    let began = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Told::Taken,
            Ok(Some(_)) => {
                let mut said = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut stderr, &mut said);
                }
                let first = said.lines().find(|l| !l.trim().is_empty()).unwrap_or("kasl said no").trim();
                return Told::Refused(first.to_string());
            }
            Ok(None) if began.elapsed() < PATIENCE => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Told::Refused(format!("kasl did not answer within {} seconds", PATIENCE.as_secs()));
            }
            Err(e) => return Told::Refused(e.to_string()),
        }
    }
}

/// What a session is about, in the words kasl will show in its report.
pub fn task_name(project: &str, stage: Option<(&str, Option<&str>)>) -> String {
    match stage {
        Some((version, Some(title))) => format!("{project} {version} · {title}"),
        Some((version, None)) => format!("{project} {version}"),
        None => project.to_string(),
    }
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
}
