//! A card's tray: the material that arrives for a task, and where it goes.
//!
//! A ticket comes with more than its text: screenshots, an export, a log a
//! tester left in a shared folder, and what the person handing it over knows
//! about where it is worked. The desk used to keep that in a folder per task
//! fed by a script; the script is gone and the folder stays, because files
//! are what the material is. So the tray is a directory per card, and the
//! directory is the truth about it - nothing about a tray is copied into the
//! database, and every screen that shows one reads it afresh, the way the
//! branches of a card are read from git.
//!
//! A tray holds a form, `incoming.md`, which the person fills in, and the
//! files. Sorting it moves everything into `sorted/<day>/` beside it - the
//! card's archive - and sets the form back to blank, so the next round of
//! material arrives into an empty tray and the last one is still there.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::card::Card;

/// The form a tray is made with.
pub const FORM: &str = "incoming.md";
/// Where sorted material goes, a folder per day.
pub const SORTED: &str = "sorted";
/// Names the directories `intake` looks in, over the platform's own.
///
/// For a person whose screenshots land somewhere else, and the only way a
/// scratch record reaches any at all - which is how the tests see it.
pub const INTAKE_ENV: &str = "RIGGER_INTAKE_DIRS";
/// The largest file taken without being asked, in megabytes.
pub const MAX_MB: u64 = 25;

const FORM_TEMPLATE: &str = include_str!("tray.form.md");

/// Recordings are listed and not copied: an assistant cannot read one, and
/// a screen recording is the largest thing a tester leaves behind.
const VIDEO: [&str; 6] = ["mp4", "avi", "mkv", "mov", "webm", "wmv"];

/// Files a folder keeps about itself, never anybody's material.
const SYSTEM_FILES: [&str; 3] = ["desktop.ini", "thumbs.db", ".ds_store"];

/// What lands on a desktop or in the downloads without being material: a
/// shortcut an installer put there, and a download still under way. Seen
/// in the first live run, where a desktop gave up three program shortcuts
/// as "fresh material".
const NOT_MATERIAL: [&str; 7] = ["lnk", "url", "crdownload", "part", "partial", "download", "tmp"];

fn extension_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn is_system_file(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    SYSTEM_FILES.contains(&name.as_str())
}

/// Where the trays of the profile in use live: what the profile names, or
/// `trays/` in the profile's own directory.
pub fn root() -> Result<PathBuf> {
    let config = crate::profile::Config::load()?;
    let (name, profile) = config.current()?;
    match &profile.trays {
        Some(dir) => Ok(dir.clone()),
        None => Ok(crate::profile::dir(&name)?.join("trays")),
    }
}

/// A key as a directory name: what a path cannot hold becomes `_`.
fn folder_name(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect()
}

/// The directory of a card's tray.
///
/// Named by the card's key. A card renamed when its tracker names it keeps
/// its old key as an alias, and a tray made under that key is moved to the
/// new one the first time it is looked for - so the tray follows the card
/// and there is never a second one for the same task.
pub fn locate(root: &Path, card: &Card) -> Result<PathBuf> {
    let dir = root.join(folder_name(&card.key));
    if dir.is_dir() {
        return Ok(dir);
    }
    for alias in &card.aliases {
        let old = root.join(folder_name(alias));
        if old.is_dir() && old != dir {
            std::fs::rename(&old, &dir).with_context(|| format!("cannot move the tray {} to {}", old.display(), dir.display()))?;
            return Ok(dir);
        }
    }
    Ok(dir)
}

/// The form of a card, blank.
pub fn blank_form(card: &Card) -> String {
    FORM_TEMPLATE.replace("\r\n", "\n").replace("{key}", &card.key).replace("{title}", &card.title)
}

/// Makes the tray and its form, when they are not there. Says whether it
/// made anything.
pub fn ensure(dir: &Path, card: &Card) -> Result<bool> {
    let form = dir.join(FORM);
    if form.is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::write(&form, blank_form(card)).with_context(|| format!("cannot write {}", form.display()))?;
    Ok(true)
}

/// Whether a form says anything: a line that is neither a heading nor one
/// of the form's own `>` lines is an answer.
///
/// Judged by the lines rather than against the blank form, so a form
/// filled before the card was renamed still reads as filled.
pub fn is_filled(form: &str) -> bool {
    form.lines().map(str::trim).any(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('>'))
}

/// The folders the form names under "Links to files": what `fetch` takes
/// from when it is not told where.
pub fn linked_sources(form: &str) -> Vec<PathBuf> {
    let mut inside = false;
    let mut out = Vec::new();
    for line in form.lines() {
        let line = line.trim();
        if let Some(heading) = line.strip_prefix("## ") {
            inside = heading.to_lowercase().starts_with("links to files");
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('>') || line.starts_with('#') {
            continue;
        }
        let path = line.trim_start_matches(['-', '*']).trim();
        if !path.is_empty() {
            out.push(PathBuf::from(path));
        }
    }
    out
}

/// A tray as it stands.
#[derive(Debug, Clone, Serialize)]
pub struct Tray {
    pub path: String,
    pub form: Form,
    /// What waits to be sorted, oldest first.
    pub files: Vec<Item>,
    /// The rounds sorted before, oldest first.
    pub sorted: Vec<Sorted>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Form {
    pub path: String,
    pub filled: bool,
}

/// A file in a tray.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    /// Relative to the tray, with `/` between the parts.
    pub path: String,
    pub bytes: u64,
    /// When the file was last written, in UTC: when the screenshot was
    /// taken, not when it was copied.
    pub modified: Option<String>,
}

/// One round of sorted material.
#[derive(Debug, Clone, Serialize)]
pub struct Sorted {
    pub day: String,
    pub files: usize,
}

impl Tray {
    /// Whether anything waits: a file, or a form with an answer in it.
    pub fn holds_anything(&self) -> bool {
        self.form.filled || !self.files.is_empty()
    }
}

/// Reads a tray that exists; `None` when there is none.
pub fn read(dir: &Path) -> Result<Option<Tray>> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let form_path = dir.join(FORM);
    let filled = std::fs::read_to_string(&form_path).map(|t| is_filled(&t)).unwrap_or(false);
    let mut files = Vec::new();
    for entry in entries(dir)? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == FORM || name == SORTED {
            continue;
        }
        walk(&entry.path(), &name, &mut files)?;
    }
    files.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| a.path.cmp(&b.path)));
    let mut sorted = Vec::new();
    let archive = dir.join(SORTED);
    if archive.is_dir() {
        for entry in entries(&archive)? {
            if entry.path().is_dir() {
                let mut inside = Vec::new();
                walk(&entry.path(), "", &mut inside)?;
                sorted.push(Sorted {
                    day: entry.file_name().to_string_lossy().to_string(),
                    files: inside.len(),
                });
            }
        }
        sorted.sort_by(|a, b| a.day.cmp(&b.day));
    }
    Ok(Some(Tray {
        path: dir.display().to_string(),
        form: Form {
            path: form_path.display().to_string(),
            filled,
        },
        files,
        sorted,
    }))
}

fn entries(dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
    let read = std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    let mut out: Vec<std::fs::DirEntry> = read.collect::<std::io::Result<_>>().with_context(|| format!("cannot read {}", dir.display()))?;
    out.sort_by_key(|e| e.file_name());
    Ok(out)
}

/// Every file under `path`, named relative to the tray.
fn walk(path: &Path, rel: &str, out: &mut Vec<Item>) -> Result<()> {
    if path.is_dir() {
        for entry in entries(path)? {
            let name = entry.file_name().to_string_lossy().to_string();
            let inner = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            walk(&entry.path(), &inner, out)?;
        }
        return Ok(());
    }
    let meta = std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    out.push(Item {
        path: rel.to_string(),
        bytes: meta.len(),
        modified: meta.modified().ok().and_then(utc),
    });
    Ok(())
}

fn utc(time: SystemTime) -> Option<String> {
    jiff::Timestamp::try_from(time)
        .ok()
        .map(|t| t.round(jiff::Unit::Second).unwrap_or(t).to_string())
}

/// A moment in UTC as the local clock shows it, to the minute: a person
/// matches a screenshot against "reproduced at 14:22" in their own time.
pub fn local_minute(utc: &str) -> String {
    match utc.parse::<jiff::Timestamp>() {
        Ok(t) => t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string(),
        Err(_) => utc.to_string(),
    }
}

/// A size as a person reads it.
pub fn size(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

/// What taking material into a tray came to.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Taken {
    /// Where it was taken from.
    pub from: Vec<String>,
    pub taken: Vec<Item>,
    /// What was left where it was, and why - never silently.
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    /// Where the file is: a recording or a large file is worked with in place.
    pub path: String,
    pub bytes: u64,
    pub why: Why,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// A recording, not copied unless asked.
    Video,
    /// Larger than the limit.
    TooBig,
    /// The same file is in the tray already.
    Present,
    /// A different file of the same name is in the tray already.
    Differs,
}

impl Why {
    pub fn describe(self) -> &'static str {
        match self {
            Why::Video => "a recording; describe what it shows, or take it with --video",
            Why::TooBig => "larger than the limit; work with it where it is, or raise --max-mb",
            Why::Present => "already in the tray",
            Why::Differs => "a different file of that name is in the tray; left where it is",
        }
    }
}

/// How to take: what to leave behind.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    pub max_bytes: u64,
    pub video: bool,
}

/// Takes one file into the tray under `rel`, by the rules.
fn take(source: &Path, dir: &Path, rel: &str, rules: Rules, out: &mut Taken, progress: &mut dyn FnMut(&Item)) -> Result<()> {
    let meta = std::fs::metadata(source).with_context(|| format!("cannot read {}", source.display()))?;
    let bytes = meta.len();
    let skip = |why| Skipped {
        path: source.display().to_string(),
        bytes,
        why,
    };
    if VIDEO.contains(&extension_of(source).as_str()) && !rules.video {
        out.skipped.push(skip(Why::Video));
        return Ok(());
    }
    if bytes > rules.max_bytes {
        out.skipped.push(skip(Why::TooBig));
        return Ok(());
    }
    let target = dir.join(rel);
    if let Ok(there) = std::fs::metadata(&target) {
        out.skipped.push(skip(if there.len() == bytes { Why::Present } else { Why::Differs }));
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    }
    std::fs::copy(source, &target).with_context(|| format!("cannot copy {} to {}", source.display(), target.display()))?;
    // The copy keeps the moment the file was written: a screenshot's time is
    // what matches it to a line in a log.
    if let Ok(modified) = meta.modified()
        && let Ok(file) = std::fs::File::options().write(true).open(&target)
    {
        let _ = file.set_modified(modified);
    }
    let item = Item {
        path: rel.to_string(),
        bytes,
        modified: meta.modified().ok().and_then(utc),
    };
    progress(&item);
    out.taken.push(item);
    Ok(())
}

/// Copies what a folder holds - or one file - into the tray, keeping the
/// folder's own layout under the tray.
pub fn fetch(dir: &Path, sources: &[PathBuf], rules: Rules, progress: &mut dyn FnMut(&Item)) -> Result<Taken> {
    let mut out = Taken::default();
    for source in sources {
        if !source.exists() {
            bail!("cannot reach {}: no such file or folder, or not from this machine", source.display());
        }
        out.from.push(source.display().to_string());
        if source.is_file() {
            let name = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            take(source, dir, &name, rules, &mut out, progress)?;
            continue;
        }
        let mut files = Vec::new();
        collect(source, "", &mut files)?;
        for (path, rel) in files {
            take(&path, dir, &rel, rules, &mut out, progress)?;
        }
    }
    Ok(out)
}

fn collect(dir: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) -> Result<()> {
    for entry in entries(dir)? {
        let name = entry.file_name().to_string_lossy().to_string();
        let inner = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let path = entry.path();
        if path.is_dir() {
            collect(&path, &inner, out)?;
        } else if !is_system_file(&path) {
            out.push((path, inner));
        }
    }
    Ok(())
}

/// Where fresh material is looked for: the screenshots, the downloads and
/// the desktop - or what `RIGGER_INTAKE_DIRS` names.
///
/// A scratch record looks nowhere unless it is told: a test that read the
/// downloads of the machine running it would be a test reaching past its
/// sandbox.
pub fn intake_dirs() -> Vec<PathBuf> {
    if let Some(named) = std::env::var_os(INTAKE_ENV) {
        return std::env::split_paths(&named).filter(|p| !p.as_os_str().is_empty()).collect();
    }
    if crate::paths::scratch() {
        return Vec::new();
    }
    let Some(user) = directories::UserDirs::new() else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    if let Some(pictures) = user.picture_dir() {
        dirs.push(pictures.join("Screenshots"));
    }
    dirs.extend(user.download_dir().map(Path::to_path_buf));
    dirs.extend(user.desktop_dir().map(Path::to_path_buf));
    dirs
}

/// Takes what was written in the last `seconds` from the intake folders,
/// oldest first. The folders are not walked into: a download that is a
/// folder was put there on purpose, and is fetched by name.
pub fn intake(dir: &Path, seconds: i64, rules: Rules, progress: &mut dyn FnMut(&Item)) -> Result<Taken> {
    let cutoff = SystemTime::now() - std::time::Duration::from_secs(seconds.max(0) as u64);
    let mut fresh = Vec::new();
    let mut out = Taken::default();
    for folder in intake_dirs() {
        let Ok(read) = std::fs::read_dir(&folder) else { continue };
        out.from.push(folder.display().to_string());
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || is_system_file(&path) || NOT_MATERIAL.contains(&extension_of(&path).as_str()) {
                continue;
            }
            if let Ok(modified) = meta.modified()
                && modified >= cutoff
            {
                fresh.push((modified, path));
            }
        }
    }
    fresh.sort();
    for (_, path) in fresh {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        take(&path, dir, &name, rules, &mut out, progress)?;
    }
    Ok(out)
}

/// `4h`, `90m`, `1d`, `2w`, or a bare number of hours.
pub fn parse_window(text: &str) -> Result<i64> {
    let text = text.trim();
    let (digits, unit) = match text.char_indices().last() {
        Some((i, c)) if c.is_ascii_alphabetic() => (&text[..i], c.to_ascii_lowercase()),
        _ => (text, 'h'),
    };
    let per = match unit {
        'm' => 60,
        'h' => 3600,
        'd' => 86_400,
        'w' => 7 * 86_400,
        _ => 0,
    };
    match digits.trim().parse::<i64>() {
        Ok(n) if n > 0 && per > 0 => Ok(n * per),
        _ => bail!("{text:?} is not a window of time; write it as `4h`, `90m`, `1d` or `2w`"),
    }
}

/// What sorting a tray came to.
#[derive(Debug, Clone, Serialize)]
pub struct Done {
    /// Files and folders moved.
    pub moved: usize,
    pub archive: String,
    /// Whether the filled form went with them.
    pub form: bool,
}

/// Sorting is over: everything in the tray moves to `sorted/<day>/`, a
/// filled form goes with it, and the form is set back to blank.
pub fn done(dir: &Path, card: &Card, day: &str) -> Result<Done> {
    let archive = dir.join(SORTED).join(day);
    let mut moved = 0;
    for entry in entries(dir)? {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == FORM || name == SORTED {
            continue;
        }
        std::fs::create_dir_all(&archive).with_context(|| format!("cannot create {}", archive.display()))?;
        let target = free_name(&archive, &name);
        std::fs::rename(entry.path(), &target).with_context(|| format!("cannot move {} to {}", entry.path().display(), target.display()))?;
        moved += 1;
    }
    let form_path = dir.join(FORM);
    let text = std::fs::read_to_string(&form_path).unwrap_or_default();
    let form = is_filled(&text);
    if form {
        std::fs::create_dir_all(&archive).with_context(|| format!("cannot create {}", archive.display()))?;
        let kept = free_name(&archive, FORM);
        std::fs::write(&kept, &text).with_context(|| format!("cannot write {}", kept.display()))?;
    }
    if form || !form_path.is_file() {
        std::fs::write(&form_path, blank_form(card)).with_context(|| format!("cannot write {}", form_path.display()))?;
    }
    Ok(Done {
        moved,
        archive: archive.display().to_string(),
        form,
    })
}

/// `name` in `dir`, or `stem (2).ext` and onwards when it is taken: a second
/// round sorted on the same day must not overwrite the first.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused name")
}

/// Every tray under the root, by folder name.
pub fn all(root: &Path) -> Result<Vec<(String, Tray)>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in entries(root)? {
        if !entry.path().is_dir() {
            continue;
        }
        if let Some(tray) = read(&entry.path())? {
            out.push((entry.file_name().to_string_lossy().to_string(), tray));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str) -> Card {
        Card {
            id: 1,
            key: key.to_string(),
            title: "rtf files are not supported".to_string(),
            status: "active".to_string(),
            aliases: Vec::new(),
            summary: None,
            created_at: String::new(),
            updated_at: None,
        }
    }

    #[test]
    fn the_blank_form_is_not_filled_and_one_answer_fills_it() {
        let blank = blank_form(&card("ACME-1"));
        assert!(blank.starts_with("# ACME-1 · rtf files are not supported"));
        assert!(!is_filled(&blank), "the form's own lines are not answers");
        let answered = blank.replace("## Links to files (optional)\n", "## Links to files (optional)\n\n\\\\server\\share\\3\n");
        assert!(is_filled(&answered));
        assert_eq!(linked_sources(&answered), vec![PathBuf::from("\\\\server\\share\\3")]);
    }

    #[test]
    fn only_the_links_section_names_sources() {
        let form = "# K\n\n## Where the work is done (required)\n\nC:\\work\\app\n\n## Links to files (optional)\n\n> Example:\n> \\\\x\\y\n- D:\\drop\\1\nD:\\drop\\2\n\n## Done before (optional)\n\nabc123\n";
        assert_eq!(linked_sources(form), vec![PathBuf::from("D:\\drop\\1"), PathBuf::from("D:\\drop\\2")]);
    }

    #[test]
    fn a_window_is_read_in_its_unit_and_a_bare_number_is_hours() {
        assert_eq!(parse_window("4").unwrap(), 4 * 3600);
        assert_eq!(parse_window("4h").unwrap(), 4 * 3600);
        assert_eq!(parse_window("90m").unwrap(), 90 * 60);
        assert_eq!(parse_window("1d").unwrap(), 86_400);
        assert_eq!(parse_window("2W").unwrap(), 14 * 86_400);
        assert!(parse_window("0h").is_err());
        assert!(parse_window("soon").is_err());
        assert!(parse_window("3y").is_err());
    }

    #[test]
    fn a_key_that_a_path_cannot_hold_is_made_one() {
        assert_eq!(folder_name("ACME-7310"), "ACME-7310");
        assert_eq!(folder_name("A/B:1"), "A_B_1");
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), "1").unwrap();
        assert_eq!(free_name(dir.path(), "a.png"), dir.path().join("a (2).png"));
        assert_eq!(free_name(dir.path(), "b.png"), dir.path().join("b.png"));
    }
}
