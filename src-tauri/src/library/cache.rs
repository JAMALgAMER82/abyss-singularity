//! Library cache — on-disk JSON storage for scanned entries.
//!
//! Stored under the Tauri app data dir as `library.json`. We write to a
//! temp file then `rename` to avoid leaving the file in a half-written
//! state if the process exits mid-save.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::types::LibraryEntry;

pub const LIBRARY_FILE: &str = "library.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub version: u32,
    pub entries: Vec<LibraryEntry>,
}

impl LibrarySnapshot {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new(entries: Vec<LibraryEntry>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            entries,
        }
    }
}

pub fn load(app_data_dir: &Path) -> Result<LibrarySnapshot> {
    let path = app_data_dir.join(LIBRARY_FILE);
    if !path.exists() {
        return Ok(LibrarySnapshot::default());
    }
    let bytes = fs::read(&path)
        .with_context(|| format!("reading library cache: {}", path.display()))?;
    let snapshot: LibrarySnapshot = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing library cache: {}", path.display()))?;
    Ok(snapshot)
}

pub fn save(app_data_dir: &Path, snapshot: &LibrarySnapshot) -> Result<PathBuf> {
    fs::create_dir_all(app_data_dir)
        .with_context(|| format!("creating app data dir: {}", app_data_dir.display()))?;

    let final_path = app_data_dir.join(LIBRARY_FILE);
    let tmp_path   = app_data_dir.join(format!("{LIBRARY_FILE}.tmp"));

    {
        let mut tmp = fs::File::create(&tmp_path)
            .with_context(|| format!("opening temp file: {}", tmp_path.display()))?;
        let bytes = serde_json::to_vec_pretty(snapshot)
            .context("serialising library snapshot")?;
        tmp.write_all(&bytes)
            .with_context(|| format!("writing temp file: {}", tmp_path.display()))?;
        tmp.sync_all().ok();
    }

    fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("renaming {} -> {}", tmp_path.display(), final_path.display()))?;
    Ok(final_path)
}
