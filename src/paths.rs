//! Where rigger keeps its data.
//!
//! One directory per platform, resolved through `directories`; the
//! `RIGGER_DATA_DIR` environment variable overrides it for tests and scripts.
//! The database is one file inside it - a later release adds one per profile.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub const DATA_DIR_ENV: &str = "RIGGER_DATA_DIR";
const DB_FILE: &str = "rigger.db";

pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let dirs = ProjectDirs::from("", "lacodda", "rigger").context("cannot determine the data directory for this platform")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

pub fn db_path() -> Result<PathBuf> {
    Ok(data_dir()?.join(DB_FILE))
}
