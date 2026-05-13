//! Archive extraction. We accept .zip and .7z — anything else fails fast.
//!
//! Runs on a blocking thread (cargo's `zip` and `sevenz-rust2` are sync).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::ArchiveFormat;

/// Extract `archive_path` into `dest_dir`. Returns the destination path.
pub fn extract(
    archive_path: &Path,
    dest_dir:     &Path,
    format:       ArchiveFormat,
) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("create extract dir {}", dest_dir.display()))?;
    match format {
        ArchiveFormat::Zip    => extract_zip(archive_path, dest_dir),
        ArchiveFormat::SevenZ => extract_7z(archive_path, dest_dir),
    }?;
    Ok(dest_dir.to_path_buf())
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .with_context(|| format!("open zip {}", archive_path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("parsing zip header")?;
    zip.extract(dest_dir).context("extracting zip")?;
    Ok(())
}

fn extract_7z(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    sevenz_rust2::decompress_file(archive_path, dest_dir)
        .with_context(|| format!("extracting 7z {}", archive_path.display()))?;
    Ok(())
}
