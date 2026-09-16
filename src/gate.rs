//! The command that says a project is fit to commit, and running it.
//!
//! Every project of this line has one: what CI runs, spelt for a shell.
//! Until now it lived in a skill file, one copy per project, and drifted
//! from the workflow it was supposed to mirror - a gate written down in
//! seventeen places is a gate nobody can change. The record holds it once,
//! the context packet prints it, and `rigger gate` runs it.
//!
//! Running it is the point rather than printing it. "The gate is green" was
//! a thing an assistant said at the end of a sitting; now it is a thing the
//! record witnessed, with the moment it happened and how long it took.

use std::process::Command;
use std::time::Instant;

/// What a gate run came to.
pub struct Run {
    /// What the shell exited with; `None` when it was killed by a signal.
    pub code: Option<i32>,
    pub seconds: u64,
}

impl Run {
    pub fn passed(&self) -> bool {
        self.code == Some(0)
    }

    /// The line the record keeps: the verdict, the time it took, and the
    /// command, so that a run can be read back without looking up what the
    /// gate was at the time. A gate changes; a record of a run should not
    /// become a puzzle when it does.
    pub fn event_body(&self, command: &str) -> String {
        let verdict = match self.code {
            Some(0) => "green".to_string(),
            Some(code) => format!("red (exit {code})"),
            None => "red (stopped before it finished)".to_string(),
        };
        format!("{verdict} in {}: {command}", duration(self.seconds))
    }
}

/// Whether an event body written by `event_body` says the gate passed.
///
/// Read back rather than stored apart: the body is what an owner sees, and
/// a second column saying the same thing is a second thing to keep in step.
pub fn body_is_green(body: &str) -> bool {
    body.starts_with("green")
}

pub fn duration(seconds: u64) -> String {
    match seconds {
        0 => "under a second".to_string(),
        1 => "1 second".to_string(),
        s if s < 60 => format!("{s} seconds"),
        s => {
            let (m, rest) = (s / 60, s % 60);
            match (m, rest) {
                (1, 0) => "1 minute".to_string(),
                (m, 0) => format!("{m} minutes"),
                (1, r) => format!("1 minute {r}s"),
                (m, r) => format!("{m} minutes {r}s"),
            }
        }
    }
}

/// The shell a gate line is handed to, and how it is told to read one.
///
/// A gate is a shell line - `cargo fmt --check && cargo clippy && cargo
/// test` - not a program with arguments, so it needs a shell to read the
/// `&&`. Splitting it here instead would mean writing a shell, badly, and
/// the first project whose gate has a quoted path would find out.
#[cfg(windows)]
fn shell() -> (std::ffi::OsString, &'static str) {
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| std::ffi::OsString::from("cmd.exe"));
    (shell, "/C")
}

#[cfg(not(windows))]
fn shell() -> (std::ffi::OsString, &'static str) {
    (std::ffi::OsString::from("sh"), "-c")
}

/// Runs the gate in the project's own directory, with its output going
/// straight to the terminal.
///
/// Inherited rather than captured: a gate is watched while it runs, and a
/// clippy failure held back until the end and then replayed is a worse
/// version of what the terminal already does well. What the record keeps is
/// the verdict, not the transcript.
pub fn run(command: &str, dir: &std::path::Path) -> std::io::Result<Run> {
    let (program, flag) = shell();
    let started = Instant::now();
    let status = Command::new(program).arg(flag).arg(command).current_dir(dir).status()?;
    Ok(Run {
        code: status.code(),
        seconds: started.elapsed().as_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_green_run_reads_back_as_green_and_a_red_one_does_not() {
        let green = Run { code: Some(0), seconds: 12 }.event_body("cargo test");
        assert!(body_is_green(&green), "{green}");
        assert!(green.contains("cargo test"), "the command is in the line: {green}");

        let red = Run { code: Some(101), seconds: 3 }.event_body("cargo test");
        assert!(!body_is_green(&red), "{red}");
        assert!(red.contains("101"), "what it exited with is in the line: {red}");
    }

    /// A gate stopped by a signal is not a gate that passed. The distinction
    /// matters because `code` is `None` there, and `!= Some(0)` is easy to
    /// write as `== Some(1)` by accident.
    #[test]
    fn a_run_that_never_finished_is_red() {
        let stopped = Run { code: None, seconds: 8 };
        assert!(!stopped.passed());
        assert!(!body_is_green(&stopped.event_body("cargo test")));
    }

    #[test]
    fn a_duration_is_said_the_way_a_person_would() {
        assert_eq!(duration(0), "under a second");
        assert_eq!(duration(1), "1 second");
        assert_eq!(duration(45), "45 seconds");
        assert_eq!(duration(60), "1 minute");
        assert_eq!(duration(90), "1 minute 30s");
        assert_eq!(duration(125), "2 minutes 5s");
        assert_eq!(duration(180), "3 minutes");
    }
}
