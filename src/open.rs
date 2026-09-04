//! Starting a session with the packet already in it.
//!
//! The assistant runs in the project's directory and is handed the context
//! packet as its first message, so a session begins where the last one
//! stopped instead of with someone reading notes aloud.
//!
//! rigger does not bundle an assistant: it launches whichever command the
//! owner already uses, `claude` by default, and passes the packet through
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

/// Runs the assistant in `dir`, handing it `packet` as its first message.
///
/// The child inherits this process's terminal - it is an interactive session,
/// not a captured one - and its exit status becomes ours.
pub fn run(dir: &Path, packet: &str) -> Result<i32> {
    let (program, args) = assistant();
    let program = resolve(&program);
    let mut command = launcher(&program);
    command.args(&args).arg(packet).current_dir(dir);
    let status = command.status().with_context(|| {
        format!(
            "cannot run `{}` in {}; set {ASSISTANT_ENV} to the command you use",
            program.to_string_lossy(),
            dir.display()
        )
    })?;
    Ok(status.code().unwrap_or(1))
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
fn launcher(program: &std::ffi::OsStr) -> Command {
    let is_batch = Path::new(program)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !is_batch {
        return Command::new(program);
    }
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| std::ffi::OsString::from("cmd.exe"));
    let mut command = Command::new(shell);
    command.arg("/c").arg(program);
    command
}

#[cfg(not(windows))]
fn launcher(program: &std::ffi::OsStr) -> Command {
    Command::new(program)
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
fn resolve(program: &str) -> std::ffi::OsString {
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
fn resolve(program: &str) -> std::ffi::OsString {
    std::ffi::OsString::from(program)
}

/// The instruction the packet arrives with, so that the assistant treats it
/// as the state of the work rather than as a request to summarise it.
pub fn first_message(packet: &str) -> String {
    format!(
        "This is where the project stands, from rigger. Pick up from the next step; \
         record what you decide or find with `rigger note`, and anything for the owner with `rigger wish`.\n\n{packet}"
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
    fn the_first_message_tells_the_assistant_what_the_packet_is() {
        let message = first_message("# proj\n");
        assert!(message.contains("rigger note"), "{message}");
        assert!(message.ends_with("# proj\n"), "the packet must come last: {message}");
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
