//! Library scanner — walks a list of root directories, identifies game
//! files via [`super::platforms`], and produces [`LibraryEntry`] records.
//!
//! Runs synchronously on a blocking thread; the Tauri command wrapper in
//! [`super::commands`] is responsible for offloading to
//! `tauri::async_runtime::spawn_blocking` so the UI stays responsive.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use super::platforms::{platform_for_extension, refine_ambiguous};
use super::types::{LibraryEntry, Platform, ScanProgressEvent, ScanReport};

const MAX_DEPTH: usize = 6;
/// Files smaller than this are ignored — most legitimate ROMs/binaries
/// exceed 16 KB even for the tiniest 8-bit consoles, and this cuts down
/// false positives from stub `.exe` shims, README.bat files, etc.
const MIN_FILE_BYTES: u64 = 16 * 1024;

/// Hook the scanner uses to report progress. Implemented by the Tauri
/// command wrapper to emit `abyss://library/scan-progress` events; tests
/// can substitute a no-op.
pub trait ProgressSink: Send {
    fn on_progress(&self, event: ScanProgressEvent);
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct NoopSink;
impl ProgressSink for NoopSink {
    fn on_progress(&self, _: ScanProgressEvent) {}
}

/// Scan a set of root paths. Existing entries (keyed by stable id) are
/// preserved verbatim — this lets re-scans run cheaply without losing
/// IGDB enrichment from Phase 2.3.
pub fn scan_collect(
    roots: &[PathBuf],
    existing: &[LibraryEntry],
    progress: &dyn ProgressSink,
) -> (ScanReport, Vec<LibraryEntry>) {
    let started = Instant::now();
    let mut by_id: std::collections::HashMap<String, LibraryEntry> = existing
        .iter()
        .cloned()
        .map(|e| (e.id.clone(), e))
        .collect();
    let mut seen_ids: std::collections::HashSet<String> = Default::default();

    let mut report = ScanReport {
        roots: roots.to_vec(),
        total_files_seen: 0,
        games_found: 0,
        games_new: 0,
        games_kept: 0,
        elapsed_ms: 0,
    };

    for root in roots {
        if !root.exists() {
            log::warn!("library: scan path missing: {}", root.display());
            continue;
        }
        let mut files_in_root: u64 = 0;
        let mut games_in_root: u64 = 0;

        for entry in WalkDir::new(root)
            .max_depth(MAX_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            files_in_root += 1;
            report.total_files_seen += 1;

            if files_in_root.is_multiple_of(50) {
                progress.on_progress(ScanProgressEvent {
                    root: root.clone(),
                    files_seen: files_in_root,
                    games_found: games_in_root,
                    current_file: entry
                        .path()
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned()),
                });
            }

            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let ext_lower = ext.to_lowercase();
            let Some(mut platform) = platform_for_extension(&ext_lower) else {
                continue;
            };
            platform = refine_ambiguous(platform, path);

            let Ok(metadata) = entry.metadata() else { continue };
            if metadata.len() < MIN_FILE_BYTES {
                continue;
            }

            if let Some(record) = build_entry(path, &metadata, &ext_lower, platform) {
                seen_ids.insert(record.id.clone());
                match by_id.get_mut(&record.id) {
                    Some(existing) => {
                        // Same game (by stable id) re-observed — refresh
                        // the filesystem-derived fields but PRESERVE
                        // anything Phase 2.3 enriched (igdb, cover, …).
                        existing.path             = record.path;
                        existing.file_name        = record.file_name;
                        existing.stem             = record.stem;
                        existing.extension        = record.extension;
                        existing.size_bytes       = record.size_bytes;
                        existing.modified         = record.modified;
                        existing.platform         = record.platform;
                        report.games_kept += 1;
                    }
                    None => {
                        by_id.insert(record.id.clone(), record);
                        report.games_new += 1;
                    }
                }
                games_in_root += 1;
            }
        }
    }

    let stale: Vec<String> = by_id
        .keys()
        .filter(|id| !seen_ids.contains(*id))
        .cloned()
        .collect();
    for id in stale {
        by_id.remove(&id);
    }

    let mut entries: Vec<LibraryEntry> = by_id.into_values().collect();
    entries.sort_by_key(|e| e.file_name.to_lowercase());

    report.games_found = entries.len();
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    (report, entries)
}

fn build_entry(
    path: &Path,
    metadata: &std::fs::Metadata,
    extension: &str,
    platform: Platform,
) -> Option<LibraryEntry> {
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let size_bytes = metadata.len();
    let modified: DateTime<Utc> = metadata
        .modified()
        .ok()
        .and_then(|t| DateTime::<Utc>::from(t).into())
        .unwrap_or_else(Utc::now);

    let mut hasher = Sha256::new();
    hasher.update(normalise_for_hash(&stem).as_bytes());
    hasher.update(b"|");
    hasher.update(size_bytes.to_le_bytes());
    let id = hex::encode(&hasher.finalize()[..12]);

    Some(LibraryEntry {
        id,
        path: path.to_path_buf(),
        file_name,
        stem,
        extension: extension.to_string(),
        size_bytes,
        modified,
        platform,
        igdb: None,
        cover_local_path: None,
        last_enriched: None,
    })
}

/// Lower-case and strip common ROM-tag clutter so the hash is stable
/// across e.g. " (USA)" / " [!]" annotation variations on the same dump.
pub(super) fn normalise_for_hash(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    for ch in stem.chars() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren = (depth_paren - 1).max(0),
            '[' => depth_brack += 1,
            ']' => depth_brack = (depth_brack - 1).max(0),
            _ if depth_paren == 0 && depth_brack == 0 => {
                out.extend(ch.to_lowercase());
            }
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
