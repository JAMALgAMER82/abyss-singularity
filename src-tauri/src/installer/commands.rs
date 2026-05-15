//! Tauri commands for the emulator auto-installer.

use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::download::{fetch_to_file, resolve_url};
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
            let dir = root.join(&m.id);
            let exe = locate_exe(&dir, &m.exe_relpath);
            EmulatorInstallState {
                manifest:  m,
                installed: exe.is_some(),
                exe,
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

    // 1. Download — resolve `gh-latest://…` pseudo-URLs first so manifests
    //    are self-healing against upstream release renames.
    let resolved_url = resolve_url(&manifest.url).await.map_err(|e| {
        let msg = format!("resolving install URL: {e:#}");
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Error {
            id: id.clone(), message: msg.clone(),
        });
        msg
    })?;
    let id_for_dl = id.clone();
    let app_for_dl = app.clone();
    fetch_to_file(&resolved_url, &tmp, move |bytes_done, bytes_total| {
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

    let exe = locate_exe(&dest, &manifest.exe_relpath).ok_or_else(|| {
        let msg = format!(
            "extracted archive but expected exe missing: {} (and no matching .exe found anywhere under {})",
            dest.join(&manifest.exe_relpath).display(),
            dest.display(),
        );
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Error {
            id: id.clone(), message: msg.clone(),
        });
        msg
    })?;

    // 3. Splice into OrchestrationConfig — upsert an EmulatorEntry with
    //    the recipe args from the existing orchestration::recipes for
    //    this id, and the freshly-installed exe path.
    splice_into_orch_config(&app, &manifest, &exe)?;

    // 3a. RetroArch ships with zero libretro cores out of the box — the
    //     base archive is just the frontend. NES, Genesis, GBC etc. games
    //     would fail to launch with "core not loaded". Pull the matching
    //     `RetroArch_cores.7z` bundle and extract it next to retroarch.exe
    //     so every system the user can play actually runs first-time.
    if manifest.id == "retroarch" {
        if let Err(e) = download_retroarch_cores(&app, &dest, &manifest.url).await {
            // Non-fatal: RetroArch still launches, user can grab cores via
            // its Online Updater. Log so the toast shows what happened.
            log::warn!("retroarch cores bundle failed: {e:#}");
        }
    }

    // 4. Seed any per-emulator controller defaults that won't auto-detect.
    //    Currently just PCSX2 (whose first-run ini binds Pad 1 to keyboard
    //    only); idempotent and never overwrites a pre-existing config.
    let _ = super::controller_setup::apply_all_defaults();

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
        // Existing entry: only patch the exe (and the display name —
        // both are install-state) so a startup repair sweep can't undo
        // user-customised args or platform assignments. Args/platforms
        // only refresh from the recipe when the entry is empty — i.e.
        // when we'd otherwise have nothing valid to launch with.
        entry.exe = exe_path.clone();
        entry.name = manifest.name.clone();
        if let Some(r) = &recipe {
            if entry.args.is_empty() {
                entry.args = r.args.clone();
            }
            if entry.platforms.is_empty() {
                entry.platforms = r.platforms.clone();
            }
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

/// Fetch Sony's official PS3 firmware (`PS3UPDAT.PUP`) from the PSN
/// CDN and ask the installed RPCS3 to install it. Sony publishes this
/// blob on their own infrastructure for any PS3 owner to download, so
/// redistributing the link (we don't host the file) is fine. Without
/// the firmware RPCS3 refuses to launch any PS3 disc; this lifts the
/// last hard blocker for PS3 emulation on a fresh Abyss install.
#[tauri::command]
pub async fn installer_install_rpcs3_firmware<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let root = emulators_root(&app)?;
    let rpcs3_dir = root.join("rpcs3");
    let rpcs3_exe = locate_exe(&rpcs3_dir, "rpcs3.exe")
        .ok_or_else(|| "RPCS3 isn't installed yet — install it from Settings → Emulators first.".to_string())?;

    // Sony's currently-published stable PS3 firmware build. They rotate
    // the timestamp hash inside the path on each release; if this 404s
    // on a future version bump, the user can fall back to manually
    // grabbing the PUP from https://www.playstation.com/en-us/support/hardware/ps3/system-software/
    // and using File → Install Firmware in RPCS3.
    // Sony rotates the timestamp directory in the firmware URL on each
    // release — the previous hard-coded build's path 404s as of May
    // 2026. Pin to the current firmware 4.93 release (March 2026).
    const PS3_FIRMWARE_URL: &str =
        "http://dus01.ps3.update.playstation.net/update/ps3/image/us/2026_0318_a2b60b6ac1d2e49e230144345616927c/PS3UPDAT.PUP";

    let cache_dir = app.path()
        .app_cache_dir()
        .map_err(|e| format!("resolving cache dir: {e}"))?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir cache: {e}"))?;
    let pup = cache_dir.join("PS3UPDAT.PUP");

    log::info!("rpcs3 firmware: downloading from Sony CDN");
    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Download {
        id: "rpcs3-firmware".into(), bytes_done: 0, bytes_total: None,
    });
    let app_for_dl = app.clone();
    fetch_to_file(PS3_FIRMWARE_URL, &pup, move |bytes_done, bytes_total| {
        let _ = app_for_dl.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Download {
            id: "rpcs3-firmware".into(), bytes_done, bytes_total,
        });
    })
    .await
    .map_err(|e| format!("downloading firmware: {e:#}"))?;

    // Hand it to RPCS3 — `--installfw <pup>` installs and exits.
    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Extract {
        id: "rpcs3-firmware".into(),
    });
    log::info!("rpcs3 firmware: invoking --installfw");
    let output = crate::util::silent_cmd_tokio(&rpcs3_exe)
        .args(["--installfw", pup.to_string_lossy().as_ref()])
        .output()
        .await
        .map_err(|e| format!("spawning rpcs3 --installfw: {e}"))?;
    let _ = std::fs::remove_file(&pup);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "RPCS3 firmware install exited {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Finalize {
        id: "rpcs3-firmware".into(), exe: rpcs3_exe,
    });
    log::info!("rpcs3 firmware: installed successfully");
    Ok(())
}

/// Download + extract `RetroArch_cores.7z` matching the RetroArch
/// version we just installed. The base RetroArch archive is the
/// frontend only — without the cores bundle, every libretro launch
/// fails with "core not loaded". Bundle URL is derived from the
/// matching frontend URL so we always pull the cores for the version
/// we actually installed.
async fn download_retroarch_cores<R: Runtime>(
    app:           &AppHandle<R>,
    install_dir:   &std::path::Path,
    frontend_url:  &str,
) -> anyhow::Result<()> {
    // The cores asset sits next to the frontend in the libretro
    // buildbot tree — same path, swap the filename.
    let cores_url = frontend_url.rsplit_once('/')
        .map(|(prefix, _)| format!("{prefix}/RetroArch_cores.7z"))
        .unwrap_or_else(|| frontend_url.to_string());
    log::info!("retroarch cores: fetching {}", cores_url);

    let tmp = install_dir.with_extension("cores.download");
    let _ = std::fs::remove_file(&tmp);

    // Stream the download, surfacing the same install-progress events the
    // base download used so the wizard's percentage UI shows real numbers.
    let app_for_dl = app.clone();
    fetch_to_file(&cores_url, &tmp, move |bytes_done, bytes_total| {
        let _ = app_for_dl.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Download {
            id: "retroarch-cores".into(), bytes_done, bytes_total,
        });
    })
    .await?;

    let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress::Extract {
        id: "retroarch-cores".into(),
    });

    // RetroArch extracts at <install_dir>/RetroArch-Win64/. The cores
    // archive lays out as RetroArch-Win64/cores/*.dll — same root — so
    // extracting it into <install_dir> merges cleanly.
    let dest = install_dir.to_path_buf();
    let tmp_for_extract = tmp.clone();
    tauri::async_runtime::spawn_blocking(move || {
        extract(&tmp_for_extract, &dest, super::types::ArchiveFormat::SevenZ)
    })
    .await
    .map_err(|e| anyhow::anyhow!("cores extract task panicked: {e}"))??;
    let _ = std::fs::remove_file(&tmp);
    log::info!("retroarch cores: extracted into {}", install_dir.display());
    Ok(())
}

/// Locate the launchable .exe for a freshly-extracted emulator. First
/// tries the manifest's `exe_relpath` verbatim (cheap, always wins when
/// the archive shape matches). If that misses — which happens when the
/// upstream archive has been re-rooted into a versioned subdir, e.g.
/// Cemu shipping under `Cemu_2.0-90/` — walks `dest` and returns the
/// first .exe whose basename matches the relpath's basename. Bounded
/// to 6 levels deep so a pathological archive can't hang the install.
fn locate_exe(dest: &std::path::Path, exe_relpath: &str) -> Option<std::path::PathBuf> {
    let direct = dest.join(exe_relpath);
    if direct.exists() {
        return Some(direct);
    }
    let wanted = std::path::Path::new(exe_relpath)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_ascii_lowercase())?;

    walkdir::WalkDir::new(dest)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
        .find(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_str()
                    .map(|s| s.to_ascii_lowercase() == wanted)
                    .unwrap_or(false)
        })
        .map(|e| e.into_path())
}

/// Install every emulator from the manifest list that isn't already on
/// disk. Best-effort: failures don't abort the batch — each error is
/// emitted as an `InstallProgress::Error` event and the report ID is
/// added to the `failed` vec, but installation continues. Returns the
/// count of successfully-installed emulators and the IDs that failed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallAllReport {
    pub installed:    Vec<String>,
    pub already_present: Vec<String>,
    pub failed:       Vec<(String, String)>,
}

#[tauri::command]
pub async fn installer_install_all<R: Runtime>(
    app: AppHandle<R>,
) -> Result<InstallAllReport, String> {
    // Mark "install_all attempted" the moment we start so any concurrent
    // caller (e.g. the first-launch background auto-installer racing with
    // a user click on the wizard's Install All button) sees the timestamp
    // and bails — no two flows try to download the same emulator.zip
    // simultaneously and clobber each other's `.download` temp file.
    if let Ok(mut cfg) = crate::library::config::load(&app) {
        if cfg.emulators_install_attempted_at.is_none() {
            cfg.emulators_install_attempted_at = Some(chrono::Utc::now());
            let _ = crate::library::config::save(&app, &cfg);
        }
    }

    let root = emulators_root(&app)?;
    let mut installed = Vec::new();
    let mut already_present = Vec::new();
    let mut failed = Vec::new();

    for m in manifests::all() {
        let dir = root.join(&m.id);
        if locate_exe(&dir, &m.exe_relpath).is_some() {
            already_present.push(m.id.clone());
            continue;
        }
        match installer_install(app.clone(), m.id.clone()).await {
            Ok(_)  => installed.push(m.id.clone()),
            Err(e) => {
                log::warn!("install_all: {} failed: {e}", m.id);
                failed.push((m.id.clone(), e));
            }
        }
    }
    Ok(InstallAllReport { installed, already_present, failed })
}

/// Seed default controller configs for emulators that don't auto-detect
/// XInput on first launch (currently just PCSX2). Idempotent — never
/// overwrites a user's existing config. Returns the list of emulator ids
/// we touched.
#[tauri::command]
pub fn installer_configure_controllers() -> Vec<String> {
    super::controller_setup::apply_all_defaults()
}

/// Re-scan installed emulator folders and patch the orchestration config
/// where the recorded `exe` path is empty or stale. This recovers the
/// "install succeeded on disk but extraction nested into a versioned
/// subdir" case (Cemu shipping under `Cemu_2.0-90/Cemu.exe`, etc.) so
/// the user doesn't have to re-download. Idempotent.
#[tauri::command]
pub fn installer_repair<R: Runtime>(app: AppHandle<R>) -> Result<usize, String> {
    let root = emulators_root(&app)?;
    let mut repaired = 0usize;
    for m in manifests::all() {
        let dir = root.join(&m.id);
        if !dir.exists() { continue; }
        if let Some(found) = locate_exe(&dir, &m.exe_relpath) {
            splice_into_orch_config(&app, &m, &found)?;
            repaired += 1;
        }
    }
    // Prune orphan entries — emulator ids that used to ship in
    // built-in recipes (Ryujinx, Citra, Mednafen, the duplicate
    // standalone DuckStation) but were removed because the upstream
    // project went away or was redundant. We only drop entries whose
    // `exe` is empty so a power user who manually pointed a slot at
    // their own install isn't silently disturbed.
    let recipe_ids: std::collections::HashSet<String> =
        crate::orchestration::recipes::builtin_recipes()
            .into_iter()
            .map(|r| r.id)
            .collect();
    let mut cfg = orch_config::load(&app).map_err(|e| format!("{e:#}"))?;
    let before = cfg.emulators.len();
    cfg.emulators.retain(|e| {
        recipe_ids.contains(&e.id) || !e.exe.as_os_str().is_empty()
    });

    // One-time arg migration: older Abyss builds shipped DuckStation
    // (under the `pcsx-redux` id) without `-fastboot`, which forces
    // users to provide a PS1 BIOS dump. New default skips BIOS via HLE
    // for ~95% of commercial titles. Upgrade existing configs that
    // still match the old default exactly; anything customised stays.
    if let Some(entry) = cfg.emulators.iter_mut().find(|e| e.id == "pcsx-redux") {
        let old_default = vec!["-fullscreen".to_string(), "{game_path}".to_string()];
        if entry.args == old_default {
            entry.args = vec![
                "-fastboot".to_string(),
                "-fullscreen".to_string(),
                "{game_path}".to_string(),
            ];
            log::info!("installer_repair: upgraded pcsx-redux args to skip PS1 BIOS via -fastboot");
        }
    }
    if cfg.emulators.len() != before {
        // Drop any assignments that pointed at the removed ids.
        let live_ids: std::collections::HashSet<String> =
            cfg.emulators.iter().map(|e| e.id.clone()).collect();
        cfg.assignments.retain(|_, v| live_ids.contains(v));
        orch_config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
        log::info!(
            "installer_repair: pruned {} orphan emulator entries",
            before - cfg.emulators.len(),
        );
    }
    // Take the chance to seed any per-emulator controller defaults that
    // haven't been written yet (existing user upgrading from an Abyss
    // build that predated controller_setup, fresh install with PCSX2
    // already on disk, etc.). Cheap, idempotent.
    let _ = super::controller_setup::apply_all_defaults();
    // Best-effort BIOS auto-discovery — scans common emulator folders +
    // Downloads / Documents for any existing BIOS dump and copies it
    // into the right emulator's `bios/` folder. Pure local file walk,
    // no network, no copyrighted-file distribution. Skipped silently
    // when no match exists.
    match super::bios_finder::auto_install_all() {
        Ok(found) if !found.is_empty() => {
            for (slot, paths) in &found {
                log::info!("bios_finder: installed {slot} BIOS at {} location(s)", paths.len());
            }
        }
        Ok(_)  => {}
        Err(e) => log::warn!("bios_finder failed: {e}"),
    }
    Ok(repaired)
}

/// Download + run Sunshine and Moonlight's official Windows installers.
/// Sunshine triggers a UAC prompt (Local System service). Moonlight is
/// silent per-user. After install, writes the discovered exe paths into
/// the streaming config so Abyss's Stream tab works immediately.
#[tauri::command]
pub async fn installer_install_streaming_apps<R: Runtime>(
    app: AppHandle<R>,
) -> Result<super::streaming_apps::StreamingInstallReport, String> {
    let report = super::streaming_apps::install_both()
        .await
        .map_err(|e| format!("{e:#}"))?;

    // Reflect the discovered paths into the streaming config so the Stream
    // tab + Sunshine service-detection both see them on next read.
    let mut cfg = crate::streaming::config::load(&app).map_err(|e| format!("{e:#}"))?;
    if let Some(p) = &report.sunshine_path  { cfg.sunshine_exe  = Some(p.clone()); }
    if let Some(p) = &report.moonlight_path { cfg.moonlight_exe = Some(p.clone()); }
    if cfg.sunshine_admin_url.is_none() {
        cfg.sunshine_admin_url = Some("https://localhost:47990".into());
    }
    // If autoset_sunshine_creds ran successfully, persist the credentials
    // so the in-app `stream_pair_client` flow (which authenticates against
    // Sunshine's REST API) works without ever asking the user for them.
    if let Some(u) = &report.auto_creds_user { cfg.sunshine_admin_user = Some(u.clone()); }
    if let Some(p) = &report.auto_creds_pass { cfg.sunshine_admin_pass = Some(p.clone()); }
    crate::streaming::config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(report)
}

/// Tauri-exposed wrapper around the BIOS finder so the wizard can
/// trigger an explicit search after the user finishes "Install all
/// emulators". Returns the slot labels we filled in.
#[tauri::command]
pub fn installer_autodetect_bios() -> Result<Vec<String>, String> {
    super::bios_finder::auto_install_all()
        .map(|m| m.into_keys().collect())
        .map_err(|e| format!("{e:#}"))
}

/// User picked a BIOS file via the OS dialog; copy it into the
/// matching emulator's BIOS folder (by filename + size signature).
/// Returns the slot label, or an error if the file isn't a recognised
/// BIOS.
#[tauri::command]
pub fn installer_install_bios_file(path: String) -> Result<String, String> {
    let p = std::path::PathBuf::from(&path);
    super::bios_finder::install_picked_file(&p)
        .map_err(|e| format!("{e:#}"))?
        .ok_or_else(|| format!(
            "{} doesn't look like a recognised console BIOS (filename or size mismatch). \
             Supported: scph1001.bin / scph5500.bin / … for PS1, scph10000.bin / 39001 / 77001 \
             for PS2, dc_boot.bin / dc_flash.bin for Dreamcast.",
             p.file_name().and_then(|n| n.to_str()).unwrap_or("the file")
        ))
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
