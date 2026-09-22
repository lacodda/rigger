//! A desktop notification: the week, said once, where the owner will see it.
//!
//! The Monday brief is a command, and a command is read by whoever runs it -
//! which on this line is mostly the assistant, not the owner. So the first
//! time rigger runs in a new week, whatever runs it, the brief's head goes
//! to the desktop as well: the focus, the Friday, what is late.
//!
//! No dependency does this: a notification is one call on each platform,
//! made through the tool the platform already has - PowerShell on Windows,
//! `osascript` on macOS, `notify-send` elsewhere. A crate for it would be a
//! dependency for twenty lines, and the one kasl uses passes its text into
//! XML and a PowerShell string unescaped.
//!
//! Nothing here may fail a command. A toast that cannot be shown is a toast
//! not shown; the brief is still one `rigger week` away.

use std::io::Write;
use std::process::{Command, Stdio};

/// The environment variable naming a program to deliver the toast instead.
///
/// It receives the title as its one argument and the body on standard
/// input - not as a second argument, because a body has line breaks and a
/// batch file cannot receive one in an argument. It is also how a scratch
/// record toasts at all (`paths::scratch`), which is how the tests see one.
pub const NOTIFY_ENV: &str = "RIGGER_NOTIFY";

/// Who will show the toast.
pub enum Notifier {
    /// A program named by `RIGGER_NOTIFY`.
    Named(std::ffi::OsString),
    /// The platform's own.
    Platform,
}

/// The notifier for this record, or `None` when it must not toast.
pub fn notifier() -> Option<Notifier> {
    match std::env::var_os(NOTIFY_ENV) {
        Some(named) if !named.is_empty() => Some(Notifier::Named(named)),
        _ if crate::paths::scratch() => None,
        _ => Some(Notifier::Platform),
    }
}

/// Shows a toast. Gives back whether it was shown; never an error.
pub fn show(notifier: &Notifier, title: &str, body: &str) -> bool {
    match notifier {
        Notifier::Named(program) => feed(Command::new(program).arg(title), body),
        Notifier::Platform => platform(title, body),
    }
}

/// Runs a command with `input` on its standard input, quietly.
fn feed(command: &mut Command, input: &str) -> bool {
    let child = command.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    child.wait().is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn platform(title: &str, body: &str) -> bool {
    // `-EncodedCommand`, not the script on standard input or in `-Command`:
    // PowerShell reads its input in the console's code page, which turns a
    // Cyrillic title into question marks, while an encoded command is
    // UTF-16 by definition. It is also not quoted by anyone on the way, and
    // like `-Command` it is not subject to the execution policy - which is
    // what forbids a `.ps1` on some machines.
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encode(&windows_script(title, body))])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A script as `-EncodedCommand` takes it: UTF-16LE, then base64.
#[cfg(any(windows, test))]
fn encode(script: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            match i <= chunk.len() {
                true => out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char),
                false => out.push('='),
            }
        }
    }
    out
}

/// The script that shows a toast through the Windows runtime.
///
/// Sent as PowerShell's own app id: a toast needs one registered with the
/// shell, and rigger has no installer to register its own.
#[cfg(any(windows, test))]
fn windows_script(title: &str, body: &str) -> String {
    let mut texts = format!("<text>{}</text>", xml(title));
    for line in body.lines().filter(|l| !l.trim().is_empty()).take(2) {
        texts.push_str(&format!("<text>{}</text>", xml(line)));
    }
    let toast = format!("<toast><visual><binding template=\"ToastGeneric\">{texts}</binding></visual></toast>");
    [
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null",
        "[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null",
        "$xml = New-Object Windows.Data.Xml.Dom.XmlDocument",
        &format!("$xml.LoadXml('{}')", toast.replace('\'', "''")),
        "$toast = New-Object Windows.UI.Notifications.ToastNotification $xml",
        "[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe').Show($toast)",
        "",
    ]
    .join("\n")
}

/// Text as XML character data.
#[cfg(any(windows, test))]
fn xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(target_os = "macos")]
fn platform(title: &str, body: &str) -> bool {
    // AppleScript reads the text from its arguments rather than from the
    // script, so no quoting of the owner's words is needed at all.
    Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 2 of argv) with title (item 1 of argv)",
            "-e",
            "end run",
        ])
        .arg(title)
        .arg(body)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn platform(title: &str, body: &str) -> bool {
    Command::new("notify-send")
        .args(["--app-name", "rigger", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owners_words_cannot_break_the_script() {
        let script = windows_script("rigger · 2026-W39", "Focus: a <b> & c's \"d\"\nsecond\nthird is dropped");
        assert!(script.contains("<text>Focus: a &lt;b&gt; &amp; c''s &quot;d&quot;</text>"), "{script}");
        assert!(script.contains("<text>second</text>"), "{script}");
        // A toast has room for a title and two lines; a third would be cut
        // by the shell anyway, and saying so here keeps it from being sent.
        assert!(!script.contains("third"), "{script}");
    }

    /// Checked against what PowerShell itself produces for these strings:
    /// `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('...'))`.
    #[test]
    fn an_encoded_command_is_utf16_in_base64() {
        assert_eq!(encode("a"), "YQA=");
        assert_eq!(encode("ab"), "YQBiAA==");
        assert_eq!(encode("abc"), "YQBiAGMA");
        assert_eq!(encode("я"), "TwQ=");
    }
}
