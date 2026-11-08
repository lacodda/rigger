//! What an assistant's hook is told when it fires.
//!
//! The end-of-turn hook closes a sitting, and a turn can end without the
//! work ending: an assistant that started a long run in the background hands
//! the turn back to wait for it, and is woken when it finishes. The hook used
//! to close the sitting there. On v0.45.0 it closed while the assistant
//! waited for a mutation run, and everything written after - the release
//! included - belonged to no sitting, while the queue saw the project free.
//!
//! The assistant says which it is. Claude Code hands its `Stop` hook a JSON
//! document on standard input, with the background work still in flight and
//! the wakeups the session has scheduled; a turn that ends with either is a
//! pause, and the sitting stays open.

use std::io::{IsTerminal, Read};
use std::time::Duration;

use serde_json::Value;

/// How long the hook's input is waited for. The assistant writes it at once
/// and closes the pipe; a runner that leaves the pipe open must not hold the
/// hook - and the turn - for longer than this.
const PATIENCE: Duration = Duration::from_secs(2);

/// The words a background task's status ends with: anything else, or no
/// status at all, is work still in flight.
const OVER: [&str; 8] = ["completed", "complete", "done", "failed", "killed", "stopped", "cancelled", "canceled"];

/// Whether the turn that just ended left work that will wake the assistant
/// again: a background task still in flight, or a wakeup it scheduled.
///
/// Input that is not the document a hook is given, or none at all, is not a
/// pause: the hook closes the sitting as it always has.
pub fn paused(input: &str) -> bool {
    let Ok(said) = serde_json::from_str::<Value>(input) else {
        return false;
    };
    let entries = |key: &str| said.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let in_flight = |task: &Value| {
        task.get("status")
            .and_then(Value::as_str)
            .is_none_or(|status| !OVER.iter().any(|over| status.eq_ignore_ascii_case(over)))
    };
    entries("background_tasks").iter().any(in_flight) || !entries("session_crons").is_empty()
}

/// What the hook was handed on standard input, when it was handed anything.
///
/// Nothing is read from a terminal: a person who typed the command is not a
/// hook, and would be left waiting for input they were never asked for.
pub fn input() -> Option<String> {
    if std::io::stdin().is_terminal() {
        return None;
    }
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::stdin().take(1 << 20).read_to_string(&mut text);
        let _ = send.send(text);
    });
    receive.recv_timeout(PATIENCE).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The document Claude Code hands its `Stop` hook, as its reference shows
    /// it, with the two lists that tell a pause from an end.
    fn stop(tasks: &str, crons: &str) -> String {
        format!(
            r#"{{"session_id":"abc123","hook_event_name":"Stop","stop_hook_active":false,
                "background_tasks":{tasks},"session_crons":{crons}}}"#
        )
    }

    #[test]
    fn a_turn_handed_back_to_wait_for_its_own_work_is_a_pause() {
        let running = r#"[{"id":"task-001","type":"shell","status":"running","description":"cargo mutants","command":"cargo mutants"}]"#;
        assert!(paused(&stop(running, "[]")));
        // A wakeup scheduled is the assistant coming back as surely.
        let wakeup = r#"[{"id":"cron-001","schedule":"0 9 * * 1-5","recurring":true,"prompt":"check the build"}]"#;
        assert!(paused(&stop("[]", wakeup)));
        // A task that does not say how it stands is taken as running: keeping
        // a sitting open a turn too long costs less than closing it under work.
        assert!(paused(&stop(r#"[{"id":"t","type":"shell"}]"#, "[]")));
    }

    #[test]
    fn a_turn_with_nothing_left_running_is_an_end() {
        assert!(!paused(&stop("[]", "[]")));
        let finished = r#"[{"id":"t","type":"shell","status":"completed"},{"id":"u","type":"agent","status":"Failed"}]"#;
        assert!(!paused(&stop(finished, "[]")));
        // An assistant that does not send the lists, or sends nothing a hook
        // can read, is closed as before.
        assert!(!paused(r#"{"session_id":"abc123","hook_event_name":"Stop"}"#));
        assert!(!paused(""));
        assert!(!paused("not json"));
    }
}
