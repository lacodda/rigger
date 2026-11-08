//! Whether anyone is at a sitting.
//!
//! The record knows a sitting is open; it cannot know whether someone is at
//! it. An assistant that is still working and one whose terminal was closed
//! an hour ago leave the same row behind, and the record had to treat both
//! alike: `go` refused every open sitting, and the end-of-turn hook closed
//! every one - including the sitting an assistant had only paused while it
//! waited for its own background work.
//!
//! So the process that runs the assistant holds its sitting: `go` takes a
//! lock on a file named after the sitting, beside the database, and keeps it
//! until the assistant exits. The lock is the operating system's, and the
//! operating system lets go of it when the process ends however it ends -
//! closed, crashed or killed. That is the whole of the trick: a lock that
//! cannot be taken is someone at work, one that can be is someone gone. No
//! process id is kept, so none can be reused, and nothing has to be cleaned
//! up for the answer to stay true.
//!
//! A sitting opened by `session start` has no such file. Nothing runs for as
//! long as it is open, so the record says it cannot tell, as before.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Who is at an open sitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The process that opened it is running: the assistant is at work.
    Live,
    /// It was held, and whatever held it has gone.
    Gone,
    /// Nothing ever held it, so the record cannot say.
    Unknown,
}

impl Presence {
    /// The presence as a contract field: `true` for someone at work, `false`
    /// for someone gone, and nothing where the record cannot say.
    pub fn live(self) -> Option<bool> {
        match self {
            Presence::Live => Some(true),
            Presence::Gone => Some(false),
            Presence::Unknown => None,
        }
    }
}

/// A sitting held for as long as this lives.
#[derive(Debug)]
pub struct Hold {
    file: File,
    path: PathBuf,
}

impl Drop for Hold {
    fn drop(&mut self) {
        // Removed before it is unlocked, so nothing can find the file free
        // and call the sitting's holder gone in the moment between: a file
        // that is not there says only "nothing holds it", which is true.
        let _ = std::fs::remove_file(&self.path);
        let _ = self.file.unlock();
    }
}

/// The file a sitting is held by: beside the database it is recorded in, so
/// that two profiles - two records - never share one.
fn path(db: &Path, session: i64) -> PathBuf {
    db.parent().unwrap_or(Path::new(".")).join("sittings").join(format!("{session}.lock"))
}

/// Holds a sitting until the returned value is dropped.
pub fn take(db: &Path, session: i64) -> Result<Hold> {
    let path = path(db, session);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    // A look at a free file takes its lock for an instant (see `presence`),
    // so a busy lock is tried again for a moment before it counts as held.
    for _ in 0..20 {
        match file.try_lock() {
            Ok(()) => return Ok(Hold { file, path }),
            Err(TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(TryLockError::Error(e)) => return Err(e).with_context(|| format!("cannot lock {}", path.display())),
        }
    }
    bail!("the sitting {session} is already held by another process")
}

/// Who is at a sitting, asked of the operating system.
///
/// Asking takes the lock for an instant when it is free. Two askers at once
/// may both see the other's instant and call a gone holder live - the safe
/// mistake, which the next look puts right.
pub fn presence(db: &Path, session: i64) -> Presence {
    let path = path(db, session);
    let Ok(file) = File::open(&path) else {
        return Presence::Unknown;
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            Presence::Gone
        }
        Err(TryLockError::WouldBlock) => Presence::Live,
        Err(TryLockError::Error(_)) => Presence::Unknown,
    }
}

/// Forgets the file of a sitting that has closed, unless something still
/// holds it: that holder removes its own file when it lets go.
pub fn clear(db: &Path, session: i64) {
    if presence(db, session) == Presence::Gone {
        let _ = std::fs::remove_file(path(db, session));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three answers, and the one property everything else stands on:
    /// letting go - here by dropping, in life by the process ending - is
    /// enough for the next look to see nobody there.
    #[test]
    fn a_held_sitting_is_live_until_its_holder_lets_go() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("rigger.db");
        assert_eq!(presence(&db, 7), Presence::Unknown, "a sitting nothing ever held");

        let hold = take(&db, 7).unwrap();
        assert_eq!(presence(&db, 7), Presence::Live);
        assert!(take(&db, 7).is_err(), "a held sitting was taken a second time");
        // Another sitting is not this one.
        assert_eq!(presence(&db, 8), Presence::Unknown);

        drop(hold);
        assert_eq!(presence(&db, 7), Presence::Unknown, "letting go cleans up after itself");
    }

    /// A holder that ended without letting go - the case a lock exists for -
    /// leaves its file behind, and the file is free.
    #[test]
    fn a_holder_that_died_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("rigger.db");
        // What a killed process leaves: the file, and no lock on it, because
        // the operating system let go of the lock when the process ended.
        std::fs::create_dir_all(path(&db, 3).parent().unwrap()).unwrap();
        std::fs::write(path(&db, 3), "").unwrap();
        assert_eq!(presence(&db, 3), Presence::Gone);

        // Taken over, it is live again; closed, its file is forgotten.
        let again = take(&db, 3).unwrap();
        assert_eq!(presence(&db, 3), Presence::Live);
        clear(&db, 3);
        assert_eq!(presence(&db, 3), Presence::Live, "a live holder's file was cleared from under it");
        drop(again);
        assert_eq!(presence(&db, 3), Presence::Unknown);
        assert_eq!(Presence::Live.live(), Some(true));
        assert_eq!(Presence::Gone.live(), Some(false));
        assert_eq!(Presence::Unknown.live(), None);
    }
}
