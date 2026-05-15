//! Tauri commands for emulator orchestration.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime, State};

use super::config;
use super::launcher::{spawn_and_track, ProcessRegistry, SpawnRequest};
use super::recipes::{builtin_recipes, expand_args};
use super::types::{EmulatorEntry, LaunchHandle, OrchestrationConfig, RunningProcess};
use crate::installer::controller_setup;
use crate::library::cache;

#[tauri::command]
pub fn orch_get_config<R: Runtime>(app: AppHandle<R>) -> Result<OrchestrationConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn orch_set_config<R: Runtime>(
    app: AppHandle<R>,
    config: OrchestrationConfig,
) -> Result<(), String> {
    config::save(&app, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn orch_builtin_recipes() -> Vec<EmulatorEntry> {
    builtin_recipes()
}

#[tauri::command]
pub async fn orch_launch<R: Runtime>(
    app: AppHandle<R>,
    registry: State<'_, Arc<ProcessRegistry>>,
    entry_id: String,
) -> Result<LaunchHandle, String> {
    // 1. Resolve the library entry by id.
    let app_for_data = app.clone();
    let dir = app_for_data
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?;
    let snapshot = cache::load(&dir).map_err(|e| format!("{e:#}"))?;
    let entry = snapshot
        .entries
        .into_iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| format!("library entry not found: {entry_id}"))?;

    // 2. Look up the platform's assigned emulator.
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let emulator_id = cfg
        .assignments
        .get(&entry.platform)
        .cloned()
        .ok_or_else(|| {
            format!(
                "no emulator assigned to platform {:?} — set one under Settings > Emulators",
                entry.platform
            )
        })?;
    let emulator = cfg
        .emulators
        .iter()
        .find(|e| e.id == emulator_id)
        .cloned()
        .ok_or_else(|| format!("assigned emulator {emulator_id} is missing from config"))?;

    if emulator.exe.as_os_str().is_empty() {
        return Err(format!(
            "emulator {} has no exe path set — choose one under Settings",
            emulator.name
        ));
    }

    // Re-seed per-emulator default controller bindings before every
    // launch. Idempotent (no-op when the user already has a config) and
    // covers the case where someone wipes their emulator config between
    // install and play, or where the install-time seed never ran (older
    // Abyss builds upgraded in place).
    let seeded = controller_setup::apply_all_defaults();
    if !seeded.is_empty() {
        log::info!("controller_setup: pre-launch seeded {:?}", seeded);
    }

    // 3. Expand args and launch.
    let mut args = expand_args(&emulator.args, &entry);
    // For PC platforms the emulator is "pc-direct" and the game itself is the exe.
    let exe = if emulator.id == "pc-direct" {
        entry.path.clone()
    } else {
        emulator.exe.clone()
    };

    // RetroArch needs `-L <core.dll>` to auto-launch a specific libretro
    // core — without it, the frontend opens its menu asking the user to
    // pick a core. Pick the right .dll based on the game's platform and
    // prepend it. Cores live in `<retroarch_dir>/RetroArch-Win64/cores/`.
    if emulator.id == "retroarch" {
        if let Some(core_name) = super::recipes::retroarch_core_for(entry.platform) {
            let core_path = exe
                .parent()
                .map(|p| p.join("cores").join(core_name));
            if let Some(p) = core_path {
                if p.exists() {
                    args.insert(0, p.to_string_lossy().into_owned());
                    args.insert(0, "-L".into());
                } else {
                    log::warn!("retroarch core missing for {:?}: {}", entry.platform, p.display());
                }
            }
        }

        // libretro core file-opener workaround: Genesis Plus GX and a
        // handful of other cores can't open paths containing `(` / `)`
        // (verified 2026-05-14 on Mega Drive ROMs like
        // "OutRun (Japan).md"). RetroArch loads fine, the core's fopen
        // call returns "Unable to open file" and the launch exits 1.
        // If the game path contains those characters, hard-link or copy
        // the ROM to a sanitised filename under %TEMP% and substitute it
        // into the args. The link lives until the next launch wipes it.
        if let Some(safe_path) = sanitised_path_for_libretro(&entry.path) {
            for arg in args.iter_mut() {
                if std::path::Path::new(arg.as_str()) == entry.path.as_path() {
                    *arg = safe_path.to_string_lossy().into_owned();
                }
            }
        }
    }

    let handle = spawn_and_track(
        app.clone(),
        registry.inner().clone(),
        SpawnRequest {
            emulator_id: emulator.id.clone(),
            entry_id:    entry.id.clone(),
            exe,
            args,
            working_dir: emulator.working_dir,
            env:         emulator.env,
        },
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    // "One-app" feel without SetParent: minimise the Abyss window so the
    // emulator owns the screen. The launcher's exit watcher restores it
    // when the game closes (see launcher.rs after the child wait).
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.minimize();
    }

    Ok(handle)
}

/// If `original` contains characters known to break some libretro core
/// file openers (`(`, `)`), copy/link the file to a sanitised name
/// under `%TEMP%` and return the new path. Returns `None` when no
/// sanitisation is needed. Best-effort: failures fall back to using
/// the original path (which is what would happen anyway without us).
///
/// **Crucially skips multi-file game formats** — a `.cue` file is just
/// a tiny text descriptor that references sibling `.bin` tracks by
/// relative path. Hard-linking the .cue into a sanitised-name temp dir
/// without also linking the .bin tracks makes the core fail to open
/// the actual data ("Pepsiman (Japan).cue" symptom — exits 1 in 139ms,
/// no stderr, looks like a generic crash). Modern RetroArch handles
/// parens fine in the multi-file path, so just pass through.
#[cfg(target_os = "windows")]
fn sanitised_path_for_libretro(original: &std::path::Path) -> Option<std::path::PathBuf> {
    let name = original.file_name()?.to_str()?;
    if !name.contains('(') && !name.contains(')') && !name.contains('[') && !name.contains(']') {
        return None;
    }
    let stem = original.file_stem()?.to_str()?;
    let ext  = original.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Multi-file disc-image / playlist descriptors — sibling files would
    // need linking too. Bail out of sanitisation rather than ship a half-
    // sanitised set that breaks the load. (Modern RetroArch + every core
    // we ship handles parens in this path.)
    const MULTI_FILE_DESCRIPTORS: &[&str] = &[
        "cue",  // CD-ROM track descriptor → .bin
        "gdi",  // Dreamcast track descriptor → .raw / .bin
        "ccd",  // CloneCD descriptor → .img / .sub
        "toc",  // CD-text descriptor → various
        "m3u",  // multi-disc playlist → .cue / .iso
        "mds",  // Alcohol descriptor → .mdf
    ];
    if MULTI_FILE_DESCRIPTORS.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
        log::info!(
            "libretro: skipping paren-sanitisation for multi-file descriptor {} \
             (would orphan sibling tracks)",
            name
        );
        return None;
    }

    // Strip everything that's not [A-Za-z0-9._-] from the stem.
    let safe_stem: String = stem.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe_stem = safe_stem.trim_matches('_').to_string();
    if safe_stem.is_empty() { return None; }

    let dir = std::env::temp_dir().join("abyss-libretro-roms");
    if std::fs::create_dir_all(&dir).is_err() { return None; }
    let dest = if ext.is_empty() { dir.join(&safe_stem) } else { dir.join(format!("{safe_stem}.{ext}")) };

    // Avoid re-copying when the existing copy already matches the source.
    let src_meta = std::fs::metadata(original).ok();
    let dst_meta = std::fs::metadata(&dest).ok();
    let same_size_modified = match (src_meta.as_ref(), dst_meta.as_ref()) {
        (Some(s), Some(d)) => s.len() == d.len() && s.modified().ok() == d.modified().ok(),
        _ => false,
    };
    if !same_size_modified {
        let _ = std::fs::remove_file(&dest);
        // Try a hard link first (instant, zero disk overhead) — only
        // works on the same volume. Fall back to a copy otherwise.
        if std::fs::hard_link(original, &dest).is_ok() {
            log::info!("libretro: hard-linked {} -> {} for paren-safe path", original.display(), dest.display());
        } else if std::fs::copy(original, &dest).is_ok() {
            log::info!("libretro: copied {} -> {} for paren-safe path", original.display(), dest.display());
        } else {
            log::warn!("libretro: couldn't sanitise path for {}", original.display());
            return None;
        }
    }
    Some(dest)
}

#[cfg(not(target_os = "windows"))]
fn sanitised_path_for_libretro(_original: &std::path::Path) -> Option<std::path::PathBuf> { None }

#[tauri::command]
pub fn orch_terminate(
    registry: State<'_, Arc<ProcessRegistry>>,
    run_id: String,
) -> Result<bool, String> {
    Ok(registry.kill(&run_id))
}

#[tauri::command]
pub fn orch_list_running(
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<Vec<RunningProcess>, String> {
    Ok(registry.list())
}
