//! Starting a session: an assistant in the project's directory, with a first
//! message that says what the sitting is for.
//!
//! rigger does not bundle an assistant: it launches whichever command the
//! owner already uses, `claude` by default, and passes the message through
//! its argument. `RIGGER_ASSISTANT` names another one.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const ASSISTANT_ENV: &str = "RIGGER_ASSISTANT";
const DEFAULT_ASSISTANT: &str = "claude";

/// The command a session is opened with, and its arguments.
///
/// The variable is split on spaces so that flags travel with it - a person
/// who runs `claude --model opus` writes exactly that. Quoting is not
/// supported on purpose: a path with spaces belongs in a wrapper script,
/// and half-parsed quoting is worse than none.
pub fn assistant() -> (String, Vec<String>) {
    let raw = std::env::var(ASSISTANT_ENV).unwrap_or_else(|_| DEFAULT_ASSISTANT.to_string());
    let mut parts = raw.split_whitespace().map(str::to_string);
    let program = parts.next().unwrap_or_else(|| DEFAULT_ASSISTANT.to_string());
    (program, parts.collect())
}

/// Runs the assistant in `dir`, handing it `message` as its first message.
///
/// The child inherits this process's terminal - it is an interactive session,
/// not a captured one - and its exit status becomes ours.
///
/// An argument is the one way there is. Claude Code refuses to start with its
/// input piped ("stdin is not a terminal"), which is how this used to hand
/// over a packet of many lines on Windows: `open` failed there on every run,
/// found by running it in a console of its own on the owner's machine. So the
/// message has to be something a batch file can carry - see `carried_by_batch`.
pub fn run(dir: &Path, message: &str) -> Result<i32> {
    let (program, args) = assistant();
    let program = resolve(&program);
    let context = || {
        format!(
            "cannot run `{}` in {}; set {ASSISTANT_ENV} to the command you use",
            program.to_string_lossy(),
            dir.display()
        )
    };

    let status = with_message(&program, &args, message)?.current_dir(dir).status().with_context(context)?;
    Ok(status.code().unwrap_or(1))
}

/// The command that starts `program` with `args` and `message`.
#[cfg(windows)]
fn with_message(program: &std::ffi::OsStr, args: &[String], message: &str) -> Result<Command> {
    use std::os::windows::process::CommandExt;

    if !is_batch(program) {
        let mut command = Command::new(program);
        command.args(args).arg(message);
        return Ok(command);
    }
    if !carried_by_batch(message) {
        bail!("this cannot be handed to a batch file: {message}");
    }
    // Written out rather than left to Rust's quoting, which follows the rules
    // of a program's arguments and not of `cmd`. `cmd /c` strips the first and
    // last quote of what follows it, so the whole line goes in one more pair,
    // and a path with a space in it survives alongside a quoted message.
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| std::ffi::OsString::from("cmd.exe"));
    let mut command = Command::new(shell);
    let flags = args.iter().map(|a| format!(" {a}")).collect::<String>();
    command.arg("/c").raw_arg(format!("\"\"{}\"{flags} \"{message}\"\"", program.to_string_lossy()));
    Ok(command)
}

#[cfg(not(windows))]
fn with_message(program: &std::ffi::OsStr, args: &[String], message: &str) -> Result<Command> {
    let mut command = Command::new(program);
    command.args(args).arg(message);
    Ok(command)
}

/// Whether `cmd`, inside double quotes, hands this over as it is. A line break
/// ends the command line, a quote ends the quoting, and a percent sign is
/// expanded even inside it.
#[cfg_attr(not(windows), allow(dead_code))]
fn carried_by_batch(message: &str) -> bool {
    !message.contains(['\n', '\r', '"', '%'])
}

/// The command that will actually be spawned.
///
/// A `.cmd` or `.bat` assistant is run through `cmd /c`, the way a shell runs
/// it. Rust refuses to pass arguments to a batch file directly - cmd.exe
/// re-parses them, and a crafted argument could inject a command
/// (CVE-2024-24576) - and answers "batch file arguments are invalid". Every
/// Windows install of an npm CLI is a `.cmd` shim, so without this `open`
/// cannot start the assistant at all; found on the owner's machine straight
/// after the resolution fix above, by running it.
#[cfg(windows)]
pub fn launcher(program: &std::ffi::OsStr) -> Command {
    if !is_batch(program) {
        return Command::new(program);
    }
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| std::ffi::OsString::from("cmd.exe"));
    let mut command = Command::new(shell);
    command.arg("/c").arg(program);
    command
}

#[cfg(not(windows))]
pub fn launcher(program: &std::ffi::OsStr) -> Command {
    Command::new(program)
}

#[cfg(windows)]
fn is_batch(program: &std::ffi::OsStr) -> bool {
    Path::new(program)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// The name to actually launch.
///
/// On Windows a command installed by npm is a `.cmd` shim, and `claude`
/// alone resolves to the shell script beside it, which the process API
/// cannot execute: found by running this on the owner's machine, where
/// `claude` failed with "program not found" while `claude.cmd` answered.
/// A shell applies PATHEXT for you; a spawned process does not, so the
/// extensions are tried here, in the order Windows itself would.
#[cfg(windows)]
pub fn resolve(program: &str) -> std::ffi::OsString {
    use std::ffi::OsString;

    // An explicit extension, or a path the caller spelled out, is taken as is.
    if Path::new(program).extension().is_some() || program.contains(['/', '\\']) {
        return OsString::from(program);
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for ext in pathext.split(';').filter(|e| !e.is_empty()) {
                let candidate = dir.join(format!("{program}{ext}"));
                if candidate.is_file() {
                    return candidate.into_os_string();
                }
            }
        }
    }
    OsString::from(program)
}

#[cfg(not(windows))]
pub fn resolve(program: &str) -> std::ffi::OsString {
    std::ffi::OsString::from(program)
}

/// The packet as the MCP server hands it over, with the instruction that makes
/// it read as the state of the work rather than as a request to summarise it.
pub fn first_message_over_mcp(packet: &str) -> String {
    format!(
        "This is where the project stands, from rigger. Pick up from the next step; record what you decide \
         or find with `record_decision` and `record_finding` as it happens, leave the next session a line \
         with `set_next_step`, and send anything only the owner can settle to `ask_owner`.\n\n{packet}"
    )
}

/// Checks the directory is there before launching anything, so a stale path
/// in the record is reported as such rather than as a missing assistant.
pub fn check_dir(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        bail!("{} is not a directory any more; the project moved or was removed", dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_assistant_is_claude() {
        // Not `assistant()`: the environment is process-wide, and a test that
        // set it would leak into every other test in this binary.
        assert_eq!(DEFAULT_ASSISTANT, "claude");
    }

    #[test]
    fn flags_travel_with_the_command() {
        // The parsing `assistant()` does, without touching the environment.
        let raw = "claude --model opus";
        let mut parts = raw.split_whitespace().map(str::to_string);
        let program = parts.next().unwrap();
        let args: Vec<String> = parts.collect();
        assert_eq!(program, "claude");
        assert_eq!(args, vec!["--model", "opus"]);
    }

    #[test]
    fn what_cmd_would_read_is_not_carried_by_a_batch_file() {
        assert!(carried_by_batch("Work on x: take v0.2.0 to a release (2 changed files), `rigger context x`."));
        for bad in ["two\nlines", "two\rlines", "a \"quote\"", "100% done"] {
            assert!(!carried_by_batch(bad), "{bad}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_command_is_resolved_through_pathext() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("thing.cmd"), "@echo off\n").unwrap();
        // Prepending keeps the test independent of what else is installed.
        let path = format!("{};{}", dir.path().display(), std::env::var("PATH").unwrap_or_default());
        unsafe { std::env::set_var("PATH", &path) };

        let resolved = resolve("thing");
        assert!(
            Path::new(&resolved).extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd")),
            "{resolved:?} - a bare name must find the .cmd shim npm installs"
        );
        // A name that resolves to nothing is left alone, so the error names
        // what the owner actually typed.
        assert_eq!(resolve("no-such-command-anywhere"), std::ffi::OsString::from("no-such-command-anywhere"));
    }

    #[test]
    fn an_explicit_path_is_launched_as_written() {
        let spelled = if cfg!(windows) {
            "C:\\tools\\my-assistant.exe"
        } else {
            "/usr/local/bin/my-assistant"
        };
        assert_eq!(resolve(spelled), std::ffi::OsString::from(spelled));
    }

    #[test]
    fn a_missing_directory_is_reported_as_such() {
        let err = check_dir(Path::new("no/such/place")).unwrap_err().to_string();
        assert!(err.contains("moved or was removed"), "{err}");
    }
}
