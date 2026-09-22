//! The calendar as a file a phone can subscribe to.
//!
//! The grid `calendar` prints is read at a desk. The Friday a release is due
//! is a date, and dates live in the calendar on the phone - where a week's
//! focus shows on Monday morning before anyone has opened a terminal. So the
//! same facts are written as iCalendar (RFC 5545): a whole-day event on each
//! release's Friday, and one spanning Monday to Friday for each week's focus.
//!
//! No network, no server: a file, written where the owner says. A calendar
//! app that imports it twice updates the events rather than doubling them,
//! because each one's `UID` is made of what it is about - the project and
//! version, or the week - and not of the moment it was written.

use std::fmt::Write as _;

use jiff::civil::Date;

use crate::calendar::Week;

/// A version aimed at a Friday.
#[derive(Debug, Clone)]
pub struct Release {
    pub project: String,
    pub version: String,
    pub title: Option<String>,
    pub week: Week,
}

/// What one week is for: the versions aimed at it, by project.
#[derive(Debug, Clone)]
pub struct Focus {
    pub week: Week,
    pub items: Vec<String>,
}

/// The whole file. `stamp` is the moment it is written, which RFC 5545
/// requires of every event; nothing else in the file depends on it.
pub fn render(releases: &[Release], focus: &[Focus], stamp: &str) -> String {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        format!("PRODID:-//rigger//rigger {}//EN", env!("CARGO_PKG_VERSION")),
        "CALSCALE:GREGORIAN".to_string(),
        "X-WR-CALNAME:rigger".to_string(),
    ];
    for week in focus {
        let mut summary = format!("Focus: {}", week.items.join(", "));
        if week.items.is_empty() {
            summary = "Focus".to_string();
        }
        lines.extend(event(
            &format!("focus-{}@rigger", week.week),
            stamp,
            week.week.monday(),
            // DTEND of a whole-day event is exclusive: Saturday closes a
            // span that includes Friday.
            week.week.friday().tomorrow().unwrap_or(week.week.friday()),
            &summary,
            &format!("The week {} - releases on {}", week.week, week.week.friday()),
        ));
    }
    for release in releases {
        let summary = match &release.title {
            Some(title) => format!("{} {} · {title}", release.project, release.version),
            None => format!("{} {}", release.project, release.version),
        };
        let friday = release.week.friday();
        lines.extend(event(
            &format!("release-{}-{}@rigger", uid_part(&release.project), uid_part(&release.version)),
            stamp,
            friday,
            friday.tomorrow().unwrap_or(friday),
            &summary,
            &format!("Aimed at {}", release.week),
        ));
    }
    lines.push("END:VCALENDAR".to_string());

    let mut out = String::new();
    for line in lines {
        // CRLF, as the format requires; a calendar that accepts LF is being
        // kind, and not every one is.
        let _ = write!(out, "{}\r\n", fold(&line));
    }
    out
}

fn event(uid: &str, stamp: &str, start: Date, end: Date, summary: &str, description: &str) -> Vec<String> {
    vec![
        "BEGIN:VEVENT".to_string(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{stamp}"),
        format!("DTSTART;VALUE=DATE:{}", basic(start)),
        format!("DTEND;VALUE=DATE:{}", basic(end)),
        format!("SUMMARY:{}", escape(summary)),
        format!("DESCRIPTION:{}", escape(description)),
        // A release day is a marker, not a meeting: it should not show the
        // owner as busy to anyone reading their availability.
        "TRANSP:TRANSPARENT".to_string(),
        "END:VEVENT".to_string(),
    ]
}

/// A date as the format writes one: `20260925`.
fn basic(date: Date) -> String {
    date.to_string().replace('-', "")
}

/// The moment in the form `DTSTAMP` takes: `20260922T101500Z`.
pub fn stamp(now: jiff::Timestamp) -> String {
    now.strftime("%Y%m%dT%H%M%SZ").to_string()
}

/// A piece of a `UID`: letters, digits, dots and dashes only, so that a
/// project name with a space or a slash cannot make two events one.
fn uid_part(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect()
}

/// Text as a property value: the four characters the format gives meaning
/// to are escaped, and a line break becomes the two characters `\n`.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out
}

/// Folds a line at 75 octets, as the format asks: the rest continues on the
/// next line after a single space. Folded between characters, never inside
/// one - a title in Cyrillic is two bytes a letter, and a fold through the
/// middle of one is a file no calendar will read.
fn fold(line: &str) -> String {
    const LIMIT: usize = 75;
    if line.len() <= LIMIT {
        return line.to_string();
    }
    let mut out = String::new();
    let mut width = 0;
    for c in line.chars() {
        // A continuation line starts with the space that marks it, which
        // counts towards its 75.
        if width + c.len_utf8() > LIMIT {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += c.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week(text: &str) -> Week {
        Week::parse(text).unwrap()
    }

    #[test]
    fn a_release_is_a_whole_day_on_its_friday() {
        let file = render(
            &[Release {
                project: "widget".into(),
                version: "v0.2.0".into(),
                title: Some("Second".into()),
                week: week("2026-W39"),
            }],
            &[],
            "20260922T100000Z",
        );
        assert!(file.starts_with("BEGIN:VCALENDAR\r\n"), "{file}");
        assert!(file.ends_with("END:VCALENDAR\r\n"), "{file}");
        assert!(file.contains("UID:release-widget-v0.2.0@rigger\r\n"), "{file}");
        assert!(file.contains("DTSTART;VALUE=DATE:20260925\r\n"), "{file}");
        assert!(file.contains("DTEND;VALUE=DATE:20260926\r\n"), "{file}");
        assert!(file.contains("SUMMARY:widget v0.2.0 · Second\r\n"), "{file}");
        // Every line ends in CRLF; a bare LF anywhere is a line the format
        // does not recognise.
        assert!(!file.replace("\r\n", "").contains('\n'), "{file}");
    }

    #[test]
    fn a_focus_spans_the_working_week() {
        let file = render(
            &[],
            &[Focus {
                week: week("2026-W39"),
                items: vec!["widget v0.2.0".into(), "sample v0.3.0".into()],
            }],
            "20260922T100000Z",
        );
        assert!(file.contains("UID:focus-2026-W39@rigger\r\n"), "{file}");
        assert!(file.contains("DTSTART;VALUE=DATE:20260921\r\n"), "{file}");
        assert!(file.contains("DTEND;VALUE=DATE:20260926\r\n"), "{file}");
        // The comma between items is the format's own separator, so it is
        // escaped rather than left to split the summary in two.
        assert!(file.contains("SUMMARY:Focus: widget v0.2.0\\, sample v0.3.0\r\n"), "{file}");
    }

    #[test]
    fn text_is_escaped_and_long_lines_fold_between_characters() {
        assert_eq!(escape("a;b,c\\d\ne"), "a\\;b\\,c\\\\d\\ne");
        let long = format!("SUMMARY:{}", "Ритм и факты ".repeat(10));
        let folded = fold(&long);
        for piece in folded.split("\r\n") {
            assert!(piece.len() <= 75, "{} octets: {piece}", piece.len());
        }
        // Unfolding gives the line back whole: a fold never lands inside a
        // two-byte letter.
        assert_eq!(folded.replace("\r\n ", ""), long);
    }

    #[test]
    fn a_uid_is_made_of_what_the_event_is_about() {
        assert_eq!(uid_part("my app/v1"), "my_app_v1");
    }
}
