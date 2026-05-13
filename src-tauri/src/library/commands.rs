//! Tauri commands exposed to the React frontend.
//!
//! Naming convention: `library_<verb>` — keeps the JS side namespaced
//! cleanly when `invoke('library_scan')` is called.

use std::path::PathBuf;

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::cache::{self, LibrarySnapshot};
use super::config;
use super::igdb::{self, IgdbClient};
use super::scanner::{self, ProgressSink};
use super::types::{LibraryConfig, LibraryEntry, ScanProgressEvent, ScanReport};

/// Where scan progress ticks land on the JS side.
pub const SCAN_PROGRESS_EVENT: &str = "abyss://library/scan-progress";
/// Where enrichment progress ticks land on the JS side.
pub const ENRICH_PROGRESS_EVENT: &str = "abyss://library/enrich-progress";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub report: ScanReport,
    pub entries: Vec<LibraryEntry>,
}

#[derive(Serialize, Clone)]
pub struct EnrichProgress {
    pub processed: usize,
    pub total: usize,
    pub matched: usize,
    pub current: Option<String>,
}

#[derive(Serialize)]
pub struct EnrichReport {
    pub processed: usize,
    pub matched: usize,
    pub skipped: usize,
    pub errors: usize,
    pub elapsed_ms: u64,
    pub entries: Vec<LibraryEntry>,
}

#[tauri::command]
pub fn library_get_config<R: Runtime>(app: AppHandle<R>) -> Result<LibraryConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn library_set_config<R: Runtime>(
    app: AppHandle<R>,
    config: LibraryConfig,
) -> Result<(), String> {
    config::save(&app, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn library_add_path<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
) -> Result<LibraryConfig, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    if !cfg.scan_paths.iter().any(|p| p == &path) {
        cfg.scan_paths.push(path);
    }
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
}

#[tauri::command]
pub fn library_remove_path<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
) -> Result<LibraryConfig, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    cfg.scan_paths.retain(|p| p != &path);
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
}

#[tauri::command]
pub fn library_load<R: Runtime>(app: AppHandle<R>) -> Result<Vec<LibraryEntry>, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?;
    let snapshot = cache::load(&dir).map_err(|e| format!("{e:#}"))?;
    Ok(snapshot.entries)
}

#[tauri::command]
pub fn library_set_igdb_credentials<R: Runtime>(
    app: AppHandle<R>,
    client_id: String,
    client_secret: String,
) -> Result<(), String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let id  = client_id.trim();
    let sec = client_secret.trim();
    cfg.igdb_client_id     = (!id.is_empty()).then(|| id.to_string());
    cfg.igdb_client_secret = (!sec.is_empty()).then(|| sec.to_string());
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn library_enrich_metadata<R: Runtime>(
    app: AppHandle<R>,
    force: Option<bool>,
) -> Result<EnrichReport, String> {
    let force = force.unwrap_or(false);
    let started = std::time::Instant::now();

    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let client_id     = cfg.igdb_client_id.clone()
        .ok_or_else(|| "IGDB client_id not set — add it under Settings".to_string())?;
    let client_secret = cfg.igdb_client_secret.clone()
        .ok_or_else(|| "IGDB client_secret not set — add it under Settings".to_string())?;
    let client = IgdbClient::new(client_id, client_secret)
        .map_err(|e| format!("{e:#}"))?;

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?;
    let mut entries = cache::load(&dir).map_err(|e| format!("{e:#}"))?.entries;

    let to_process: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| (force || e.igdb.is_none()).then_some(i))
        .collect();
    let total = to_process.len();

    let mut matched = 0usize;
    let mut errors  = 0usize;
    let mut processed = 0usize;

    for idx in to_process {
        let stem = entries[idx].stem.clone();
        let normalised = scanner::normalise_for_hash(&stem);
        let _ = app.emit(ENRICH_PROGRESS_EVENT, EnrichProgress {
            processed,
            total,
            matched,
            current: Some(stem.clone()),
        });

        match client.search_game(&normalised, 1).await {
            Ok(mut hits) if !hits.is_empty() => {
                let game = hits.remove(0);
                entries[idx].igdb = Some(igdb::to_metadata(game));
                entries[idx].last_enriched = Some(Utc::now());
                matched += 1;
            }
            Ok(_) => {
                // No match — still mark last_enriched so we don't retry on every press.
                entries[idx].last_enriched = Some(Utc::now());
            }
            Err(e) => {
                log::warn!("igdb: failed for {stem:?}: {e:#}");
                errors += 1;
            }
        }
        processed += 1;
    }

    let _ = app.emit(ENRICH_PROGRESS_EVENT, EnrichProgress {
        processed,
        total,
        matched,
        current: None,
    });

    let snapshot = LibrarySnapshot::new(entries.clone());
    cache::save(&dir, &snapshot).map_err(|e| format!("{e:#}"))?;

    Ok(EnrichReport {
        processed,
        matched,
        skipped: total.saturating_sub(processed),
        errors,
        elapsed_ms: started.elapsed().as_millis() as u64,
        entries,
    })
}

#[tauri::command]
pub async fn library_scan<R: Runtime>(app: AppHandle<R>) -> Result<ScanResult, String> {
    let app_for_blocking = app.clone();
    let app_for_event    = app.clone();

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<ScanResult, String> {
        let cfg = config::load(&app_for_blocking).map_err(|e| format!("{e:#}"))?;
        if cfg.scan_paths.is_empty() {
            return Ok(ScanResult {
                report: ScanReport {
                    roots: vec![],
                    total_files_seen: 0,
                    games_found: 0,
                    games_new: 0,
                    games_kept: 0,
                    elapsed_ms: 0,
                },
                entries: vec![],
            });
        }
        let dir = app_for_blocking
            .path()
            .app_data_dir()
            .map_err(|e| format!("resolving app data dir: {e}"))?;
        let existing = cache::load(&dir)
            .map_err(|e| format!("{e:#}"))?
            .entries;

        struct EventSink<R: Runtime>(AppHandle<R>);
        impl<R: Runtime> ProgressSink for EventSink<R> {
            fn on_progress(&self, event: ScanProgressEvent) {
                let _ = self.0.emit(SCAN_PROGRESS_EVENT, event);
            }
        }
        let sink = EventSink(app_for_event);

        let (report, entries) = scanner::scan_collect(&cfg.scan_paths, &existing, &sink);
        let snapshot = LibrarySnapshot::new(entries.clone());
        cache::save(&dir, &snapshot).map_err(|e| format!("{e:#}"))?;
        Ok(ScanResult { report, entries })
    })
    .await
    .map_err(|e| format!("scan task panicked: {e}"))??;

    // Auto-detect: after a scan finds games, try to assign every newly-
    // appeared platform to an already-installed emulator. Cheap (only
    // touches platforms with no existing assignment) and idempotent.
    if let Err(e) = crate::installer::commands::installer_auto_assign(app.clone()) {
        log::warn!("library: auto-assign after scan failed: {e}");
    }

    Ok(result)
}
