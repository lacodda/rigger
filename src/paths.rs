//! Where rigger keeps its data.
//!
//! One directory per platform, resolved through `directories`; the
//! `RIGGER_DATA_DIR` environment variable overrides it for tests and scripts.
//! The config lives at its root; each profile keeps its database under
//! `profiles/<name>/`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub const DATA_DIR_ENV: &str = "RIGGER_DATA_DIR";

pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let dirs = ProjectDirs::from("", "lacodda", "rigger").context("cannot determine the data directory for this platform")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

/// Whether this record is a scratch one: kept where `RIGGER_DATA_DIR` says,
/// for a test or a script, rather than where the owner's lives.
///
/// Whatever rigger does outside its own files - a toast on the desktop, a
/// question to GitHub, an interval in kasl - is done for the owner's record
/// only. A scratch record's projects are made up, and a test that popped a
/// toast or started a kasl interval on the machine running it would be a
/// test writing past its sandbox. A scratch record reaches those only
/// through a program it names itself, which is also how the tests see them.
pub fn scratch() -> bool {
    std::env::var_os(DATA_DIR_ENV).is_some()
}

/// The database of the profile in use.
pub fn db_path() -> Result<PathBuf> {
    crate::profile::current_db_path()
}
