//! Tauri commands for the emulator auto-installer.

use std::path::PathBuf;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::download::fetch_to_file;
use super::extract::extract;
use super::manifests;
use super::types::{
    EmulatorInstallState, EmulatorManifest, InstallProgress, InstallReport,
    INSTALL_PROGRESS_EVENT,
};
use crate::orchestration::{config as orch_config, types::EmulatorEntry};
use crate::library::types::Platform;

/// Sub-directory under the app data dir where extracted emulators live.
fn emulators_root<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?
        .join("emulators");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create emulators dir: {e}"))?;
    Ok(dir)
}

#[tauri::command]
pub fn installer_available() -> Vec<EmulatorManifest> {
    manifests::all()
}

#[tauri::command]
pub fn installer_status<R: Runtime>(app: AppHandle<R>) -> Result<Vec<EmulatorInstallState>, String> {
    let root = emulators_root(&app)?;
    let out = manifests::all()
        .into_iter()
        .map(|m| {
            let exe = root.join(&m.id).join(&m.exe_relpath);
            let installed = exe.exists();
            EmulatorInstallState {
                manifest:  m,
                installed,
                exe:       installed.then_some(exe),
            }
        })
        .collect();
    Ok(out)
}

#[tauri::command]
pub async fn installer_install<R: Runtime>(
    app: AppHandle<R>,
    id:  String,
) -> Result<InstallReport, String> {
    let started = Instant::now();
    let manifest = manifests::find_by_id(&id)
        .ok_or_else(|| format!("unknown emulator id: {id}"))?;
    let root  = emulators_root(&app)?;
    let dest  = root.join(&manifest.id);
    let tmp   = dest.with_extension("download");

    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Start { id: id.clone() });

    // Wipe any previous attempt so a re-install is reproducible.
    if dest.exists()     { let _ = std::fs::remove_dir_all(&dest); }
    if tmp.exists()      { let _ = std::fs::remove_file(&tmp); }
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;

    // 1. Download
    let id_for_dl = id.clone();
    let app_for_dl = app.clone();
    fetch_to_file(&manifest.url, &tmp, move |bytes_done, bytes_total| {
        let _ = app_for_dl.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Download {
            id: id_for_dl.clone(), bytes_done, bytes_total,
        });
    })
    .await
    .map_err(|e| {
        let msg = format!("{e:#}");
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Error {
            id: id.clone(), message: msg.clone(),
        });
        msg
    })?;

    // 2. Extract — blocking, runs on the thread pool.
    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Extract { id: id.clone() });
    let dest_for_extract = dest.clone();
    let tmp_for_extract  = tmp.clone();
    let format           = manifest.archive_format;
    tauri::async_runtime::spawn_blocking(move || {
        extract(&tmp_for_extract, &dest_for_extract, format)
    })
    .await
    .map_err(|e| format!("extract task panicked: {e}"))?
    .map_err(|e| {
        let msg = format!("{e:#}");
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Error {
            id: id.clone(), message: msg.clone(),
        });
        msg
    })?;
    let _ = std::fs::remove_file(&tmp);

    let exe = dest.join(&manifest.exe_relpath);
    if !exe.exists() {
        let msg = format!("extracted archive but expected exe missing: {}", exe.display());
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Error {
            id: id.clone(), message: msg.clone(),
        });
        return Err(msg);
    }

    // 3. Splice into OrchestrationConfig — upsert an EmulatorEntry with
    //    the recipe args from the existing orchestration::recipes for
    //    this id, and the freshly-installed exe path.
    splice_into_orch_config(&app, &manifest, &exe)?;

    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Finalize {
        id: id.clone(), exe: exe.clone(),
    });

    Ok(InstallReport {
        id,
        exe,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
pub fn installer_uninstall<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let root = emulators_root(&app)?;
    let dir  = root.join(&id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("uninstall {}: {e}", dir.display()))?;
    }
    // Also drop the entry + assignments from OrchestrationConfig.
    let mut cfg = orch_config::load(&app).map_err(|e| format!("{e:#}"))?;
    cfg.emulators.retain(|e| e.id != id);
    cfg.assignments.retain(|_, v| v != &id);
    orch_config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// After a successful install, make sure OrchestrationConfig has an
/// emulator entry for this id with the freshly-installed exe and the
/// recipe's args. If the id already exists, just update its exe.
fn splice_into_orch_config<R: Runtime>(
    app:      &AppHandle<R>,
    manifest: &EmulatorManifest,
    exe:      &std::path::Path,
) -> Result<(), String> {
    let recipes = crate::orchestration::recipes::builtin_recipes();
    let recipe  = recipes.into_iter().find(|r| r.id == manifest.id);

    let mut cfg = orch_config::load(app).map_err(|e| format!("{e:#}"))?;
    let exe_path = exe.to_path_buf();

    if let Some(entry) = cfg.emulators.iter_mut().find(|e| e.id == manifest.id) {
        entry.exe = exe_path.clone();
        entry.name = manifest.name.clone();
        if let Some(r) = &recipe {
            entry.args = r.args.clone();
            entry.platforms = r.platforms.clone();
        }
    } else {
        let (args, platforms) = match recipe {
            Some(r) => (r.args, r.platforms),
            None    => (vec![], manifest.platforms.clone()),
        };
        cfg.emulators.push(EmulatorEntry {
            id:          manifest.id.clone(),
            name:        manifest.name.clone(),
            exe:         exe_path,
            args,
            working_dir: None,
            env:         Default::default(),
            platforms,
        });
    }

    // Auto-assign: for every platform this emulator covers, if no
    // assignment exists yet, point it at us. This is the "ROMs without
    // exe should know which emulator runs them" behaviour the user asked
    // for — installing RetroArch immediately makes every retro platform
    // playable without further configuration.
    let entry = cfg.emulators.iter().find(|e| e.id == manifest.id).cloned();
    if let Some(e) = entry {
        for platform in &e.platforms {
            cfg.assignments.entry(*platform).or_insert_with(|| e.id.clone());
        }
    }

    orch_config::save(app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// Auto-assign helper: for every platform with at least one configured
/// emulator whose exe exists, set the platform → first-suitable-emulator
/// mapping if it's unset. RetroArch wins ties for platforms it covers.
#[tauri::command]
pub fn installer_auto_assign<R: Runtime>(app: AppHandle<R>) -> Result<Vec<(Platform, String)>, String> {
    let mut cfg = orch_config::load(&app).map_err(|e| format!("{e:#}"))?;

    let candidates: Vec<EmulatorEntry> = cfg
        .emulators
        .iter()
        .filter(|e| !e.exe.as_os_str().is_empty())
        .cloned()
        .collect();

    let mut applied: Vec<(Platform, String)> = vec![];

    // Collect every platform any installed emulator covers.
    let mut all_platforms: std::collections::BTreeSet<Platform> = Default::default();
    for e in &candidates {
        for p in &e.platforms { all_platforms.insert(*p); }
    }

    for platform in all_platforms {
        if cfg.assignments.contains_key(&platform) { continue }
        // Prefer RetroArch (highest coverage); fall back to first match.
        let pick = candidates
            .iter()
            .find(|e| e.id == "retroarch" && e.platforms.contains(&platform))
            .or_else(|| candidates.iter().find(|e| e.platforms.contains(&platform)));
        if let Some(emu) = pick {
            cfg.assignments.insert(platform, emu.id.clone());
            applied.push((platform, emu.id.clone()));
        }
    }
    orch_config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(applied)
}
